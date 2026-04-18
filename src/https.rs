use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use smoltcp::socket::tcp::{Socket, State};
use smoltcp::wire::IpAddress;
use smoltcp::iface::SocketHandle;
use crate::net::NET_STACK;
use crate::tls::{TlsClient, TlsState};

pub enum HttpsState {
    Disconnected,
    Connecting,
    TlsHandshake,
    RequestSent,
    ReceivingData(Vec<u8>),
    Complete(Vec<u8>, [u8; 64]),
    Error(&'static str),
}

pub enum ApiMethod {
    Get,
    Post,
}

pub enum ApiState {
    Disconnected,
    Connecting,
    TlsHandshake,
    RequestSent,
    ReceivingData(Vec<u8>),
    Complete(ApiResponse),
    Error(&'static str),
}

pub struct ApiResponse {
    pub status_code: u16,
    pub body: Vec<u8>,
}

pub struct NexusClient {
    handle: SocketHandle,
    local_port: u16,
    state: HttpsState,
    target_ip: IpAddress,
    port: u16,
    hardware_id: &'static str,
    tls: TlsClient,
}

pub struct ApiClient {
    handle: SocketHandle,
    local_port: u16,
    state: ApiState,
    target_ip: IpAddress,
    port: u16,
    host: &'static str,
    method: ApiMethod,
    path: String,
    body: Vec<u8>,
    tls: TlsClient,
}

impl NexusClient {
    pub fn new(handle: SocketHandle, target_ip: IpAddress, port: u16, hardware_id: &'static str) -> Self {
        NexusClient {
            handle,
            local_port: crate::net::allocate_tcp_local_port(),
            state: HttpsState::Disconnected,
            target_ip,
            port,
            hardware_id,
            tls: TlsClient::new("openrhiza.com"),
        }
    }

    pub fn handle(&self) -> SocketHandle {
        self.handle
    }

    pub fn poll(&mut self) {
        let mut net_stack = NET_STACK.lock();
        if let Some(stack) = net_stack.as_mut() {
            let socket = stack.sockets.get_mut::<Socket>(self.handle);

            match &mut self.state {
                HttpsState::Disconnected => {
                    crate::serial_println!("[HTTPS] Connecting to Nexus API...");
                    let cx = stack.iface.context();
                    if socket.connect(cx, (self.target_ip, self.port), self.local_port).is_ok() {
                        self.state = HttpsState::Connecting;
                    } else {
                        self.state = HttpsState::Error("Connection setup failed");
                    }
                }
                HttpsState::Connecting => {
                    if socket.state() == State::Established {
                        crate::serial_println!("[HTTPS] TCP Connected! Starting TLS Handshake...");
                        self.tls.start_handshake();
                        self.state = HttpsState::TlsHandshake;
                    } else if socket.state() == State::Closed {
                        self.state = HttpsState::Error("Connection failed");
                    }
                }
                HttpsState::TlsHandshake => {
                    if self.tls.has_data_to_send() && socket.can_send() {
                        let outbound = self.tls.take_send_buf();
                        let _ = socket.send_slice(&outbound);
                    }
                    if socket.can_recv() {
                        let mut temp_buf = [0; 1024];
                        if let Ok(size) = socket.recv_slice(&mut temp_buf) {
                            if size > 0 {
                                self.tls.feed_data(&temp_buf[..size]);
                            }
                        }
                    }
                    if self.tls.state == TlsState::Ready {
                        crate::serial_println!("[HTTPS] TLS Handshake Complete! Sending encrypted GET request...");
                        let path = format!("/api/nexus/{}.wasm", self.hardware_id.replace(':', "_"));
                        let req = format!("GET {} HTTP/1.1\r\nHost: openrhiza.com\r\nConnection: close\r\n\r\n", path);
                        self.tls.send_app_data(req.as_bytes());
                        self.state = HttpsState::RequestSent;
                    } else if self.tls.state == TlsState::Error {
                        self.state = HttpsState::Error("TLS Handshake failed");
                    }
                }
                HttpsState::RequestSent => {
                    if self.tls.has_data_to_send() && socket.can_send() {
                        let outbound = self.tls.take_send_buf();
                        let _ = socket.send_slice(&outbound);
                    }
                    if !self.tls.has_data_to_send() {
                        self.state = HttpsState::ReceivingData(Vec::new());
                    }
                }
                HttpsState::ReceivingData(buf) => {
                    if self.tls.has_data_to_send() && socket.can_send() {
                        let outbound = self.tls.take_send_buf();
                        let _ = socket.send_slice(&outbound);
                    }
                    if socket.can_recv() {
                        let mut temp_buf = [0; 1024];
                        if let Ok(size) = socket.recv_slice(&mut temp_buf) {
                            if size > 0 {
                                self.tls.feed_data(&temp_buf[..size]);
                            }
                        }
                    }
                    
                    if !self.tls.app_data_in.is_empty() {
                        buf.extend_from_slice(&self.tls.app_data_in);
                        self.tls.app_data_in.clear();
                    }

                    if socket.state() == State::CloseWait || socket.state() == State::Closed {
                        if !self.tls.app_data_in.is_empty() {
                            buf.extend_from_slice(&self.tls.app_data_in);
                            self.tls.app_data_in.clear();
                        }
                        
                        let data = core::mem::take(buf);
                        crate::serial_println!("[HTTPS] TLS connection closed. Total decrypted bytes: {}", data.len());
                        
                        let mut body_start = 0;
                        for i in 0..data.len().saturating_sub(4) {
                            if &data[i..i+4] == b"\r\n\r\n" {
                                body_start = i + 4;
                                break;
                            }
                        }
                        
                        let mut signature = [0u8; 64];
                        let mut found_sig = false;
                        if let Ok(header_text) = core::str::from_utf8(&data[..body_start]) {
                            for line in header_text.lines() {
                                if line.starts_with("X-Nexus-Signature:") {
                                    let hex_str = line["X-Nexus-Signature:".len()..].trim();
                                    if hex_str.len() == 128 {
                                        for i in 0..64 {
                                            if let Ok(byte) = u8::from_str_radix(&hex_str[i * 2..i * 2 + 2], 16) {
                                                signature[i] = byte;
                                            }
                                        }
                                        found_sig = true;
                                    }
                                }
                            }
                        }

                        if !found_sig {
                            crate::serial_println!("[SECURITY] Missing or invalid Ed25519 signature! Dropping Payload.");
                            self.state = HttpsState::Error("Signature verify failure");
                        } else {
                            let payload = data[body_start..].to_vec();
                            crate::serial_println!("[HTTPS] Payload extracted: {} bytes", payload.len());
                            self.state = HttpsState::Complete(payload, signature);
                        }
                    }
                }
                HttpsState::Complete(_, _) => {}
                HttpsState::Error(_) => {}
            }
        }
    }

    pub fn take_payload(&mut self) -> Option<(Vec<u8>, [u8; 64])> {
        if let HttpsState::Complete(_, _) = self.state {
            if let HttpsState::Complete(data, sig) = core::mem::replace(&mut self.state, HttpsState::Disconnected) {
                return Some((data, sig));
            }
        }
        None
    }
}

impl ApiClient {
    pub fn new(
        handle: SocketHandle,
        target_ip: IpAddress,
        port: u16,
        host: &'static str,
        method: ApiMethod,
        path: &str,
        body: Vec<u8>,
    ) -> Self {
        ApiClient {
            handle,
            local_port: crate::net::allocate_tcp_local_port(),
            state: ApiState::Disconnected,
            target_ip,
            port,
            host,
            method,
            path: String::from(path),
            body,
            tls: TlsClient::new(host),
        }
    }

    pub fn handle(&self) -> SocketHandle {
        self.handle
    }

    pub fn poll(&mut self) {
        let mut net_stack = NET_STACK.lock();
        if let Some(stack) = net_stack.as_mut() {
            let socket = stack.sockets.get_mut::<Socket>(self.handle);

            match &mut self.state {
                ApiState::Disconnected => {
                    crate::serial_println!("[HTTPS API] Connecting to {}...", self.host);
                    let cx = stack.iface.context();
                    if socket.connect(cx, (self.target_ip, self.port), self.local_port).is_ok() {
                        self.state = ApiState::Connecting;
                    } else {
                        self.state = ApiState::Error("Connection setup failed");
                    }
                }
                ApiState::Connecting => {
                    if socket.state() == State::Established {
                        crate::serial_println!("[HTTPS API] TCP Connected! Starting TLS Handshake...");
                        self.tls.start_handshake();
                        self.state = ApiState::TlsHandshake;
                    } else if socket.state() == State::Closed {
                        self.state = ApiState::Error("Connection failed");
                    }
                }
                ApiState::TlsHandshake => {
                    flush_tls_outbound(socket, &mut self.tls);
                    feed_tls_inbound(socket, &mut self.tls);

                    if self.tls.state == TlsState::Ready {
                        let request = self.build_request();
                        crate::serial_println!(
                            "[HTTPS API] TLS ready. Sending {} {} ({} body bytes)...",
                            self.method_name(),
                            self.path,
                            self.body.len()
                        );
                        self.tls.send_app_data(request.as_bytes());
                        self.state = ApiState::RequestSent;
                    } else if self.tls.state == TlsState::Error {
                        self.state = ApiState::Error("TLS Handshake failed");
                    }
                }
                ApiState::RequestSent => {
                    flush_tls_outbound(socket, &mut self.tls);
                    if !self.tls.has_data_to_send() {
                        self.state = ApiState::ReceivingData(Vec::new());
                    }
                }
                ApiState::ReceivingData(buf) => {
                    flush_tls_outbound(socket, &mut self.tls);
                    feed_tls_inbound(socket, &mut self.tls);

                    if !self.tls.app_data_in.is_empty() {
                        buf.extend_from_slice(&self.tls.app_data_in);
                        self.tls.app_data_in.clear();
                    }

                    if socket.state() == State::CloseWait || socket.state() == State::Closed {
                        if !self.tls.app_data_in.is_empty() {
                            buf.extend_from_slice(&self.tls.app_data_in);
                            self.tls.app_data_in.clear();
                        }

                        let response_bytes = core::mem::take(buf);
                        if let Some(response) = parse_http_response(response_bytes) {
                            crate::serial_println!(
                                "[HTTPS API] Response complete: status={} body={} bytes",
                                response.status_code,
                                response.body.len()
                            );
                            self.state = ApiState::Complete(response);
                        } else {
                            self.state = ApiState::Error("HTTP parse failure");
                        }
                    }
                }
                ApiState::Complete(_) => {}
                ApiState::Error(_) => {}
            }
        }
    }

    pub fn take_response(&mut self) -> Option<ApiResponse> {
        if let ApiState::Complete(_) = self.state {
            if let ApiState::Complete(response) =
                core::mem::replace(&mut self.state, ApiState::Disconnected)
            {
                return Some(response);
            }
        }
        None
    }

    pub fn error_message(&self) -> Option<&'static str> {
        match self.state {
            ApiState::Error(message) => Some(message),
            _ => None,
        }
    }

    fn method_name(&self) -> &'static str {
        match self.method {
            ApiMethod::Get => "GET",
            ApiMethod::Post => "POST",
        }
    }

    fn build_request(&self) -> String {
        match self.method {
            ApiMethod::Get => format!(
                "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                self.path,
                self.host
            ),
            ApiMethod::Post => format!(
                "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                self.path,
                self.host,
                self.body.len(),
                core::str::from_utf8(&self.body).unwrap_or("")
            ),
        }
    }
}

fn flush_tls_outbound(socket: &mut Socket, tls: &mut TlsClient) {
    if tls.has_data_to_send() && socket.can_send() {
        let outbound = tls.take_send_buf();
        let _ = socket.send_slice(&outbound);
    }
}

fn feed_tls_inbound(socket: &mut Socket, tls: &mut TlsClient) {
    if socket.can_recv() {
        let mut temp_buf = [0; 1024];
        if let Ok(size) = socket.recv_slice(&mut temp_buf) {
            if size > 0 {
                tls.feed_data(&temp_buf[..size]);
            }
        }
    }
}

fn parse_http_response(data: Vec<u8>) -> Option<ApiResponse> {
    let mut body_start = None;
    for i in 0..data.len().saturating_sub(3) {
        if &data[i..i + 4] == b"\r\n\r\n" {
            body_start = Some(i + 4);
            break;
        }
    }

    let body_start = body_start?;
    let header_bytes = &data[..body_start];
    let header_text = core::str::from_utf8(header_bytes).ok()?;
    let mut lines = header_text.lines();
    let status_line = lines.next()?;
    let mut parts = status_line.split_whitespace();
    let _http_version = parts.next()?;
    let status_code = parts.next()?.parse::<u16>().ok()?;

    Some(ApiResponse {
        status_code,
        body: data[body_start..].to_vec(),
    })
}
