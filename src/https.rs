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

pub enum PlainHttpState {
    Disconnected,
    SendingRequest,
    ReceivingData(Vec<u8>),
    Complete(ApiResponse),
    Error(&'static str),
}

pub struct ApiResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl ApiResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
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
    headers: Vec<(String, String)>,
    tls: TlsClient,
}

pub struct PlainHttpClient {
    handle: SocketHandle,
    local_port: u16,
    state: PlainHttpState,
    target_ip: IpAddress,
    port: u16,
    request: Vec<u8>,
    request_offset: usize,
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
                    flush_tls_outbound(socket, &mut self.tls);
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
                    flush_tls_outbound(socket, &mut self.tls);
                    if !self.tls.has_data_to_send() {
                        self.state = HttpsState::ReceivingData(Vec::new());
                    }
                }
                HttpsState::ReceivingData(buf) => {
                    flush_tls_outbound(socket, &mut self.tls);
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
        Self::new_with_headers(handle, target_ip, port, host, method, path, body, Vec::new())
    }

    pub fn new_with_headers(
        handle: SocketHandle,
        target_ip: IpAddress,
        port: u16,
        host: &'static str,
        method: ApiMethod,
        path: &str,
        body: Vec<u8>,
        headers: Vec<(String, String)>,
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
            headers,
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
                    crate::serial_println!(
                        "[HTTPS API] RequestSent: pending_tls_bytes={} socket_may_recv={} socket_state={:?}",
                        self.tls.send_buf().len(),
                        socket.can_recv(),
                        socket.state()
                    );
                    if !self.tls.has_data_to_send() {
                        crate::serial_println!("[HTTPS API] All TLS request bytes flushed; waiting for response");
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

                        crate::serial_println!(
                            "[HTTPS API] Socket closing with {} decrypted bytes buffered",
                            buf.len()
                        );

                        let response_bytes = core::mem::take(buf);
                        let response_len = response_bytes.len();
                        if let Some(response) = parse_http_response(response_bytes) {
                            crate::serial_println!(
                                "[HTTPS API] Response complete: status={} body={} bytes",
                                response.status_code,
                                response.body.len()
                            );
                            self.state = ApiState::Complete(response);
                        } else {
                            crate::println!(
                                "[HTTPS API] HTTP parse failure after {} decrypted bytes",
                                response_len
                            );
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
        let mut request = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\n",
            self.method_name(),
            self.path,
            self.host
        );

        for (name, value) in &self.headers {
            request.push_str(name);
            request.push_str(": ");
            request.push_str(value);
            request.push_str("\r\n");
        }

        match self.method {
            ApiMethod::Get => {
                request.push_str("Connection: close\r\n\r\n");
                request
            }
            ApiMethod::Post => {
                request.push_str("Content-Type: application/json\r\n");
                request.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
                request.push_str("Connection: close\r\n\r\n");
                request.push_str(core::str::from_utf8(&self.body).unwrap_or(""));
                request
            }
        }
    }
}

impl PlainHttpClient {
    pub fn new(
        handle: SocketHandle,
        target_ip: IpAddress,
        port: u16,
        request: Vec<u8>,
    ) -> Self {
        Self {
            handle,
            local_port: crate::net::allocate_tcp_local_port(),
            state: PlainHttpState::Disconnected,
            target_ip,
            port,
            request,
            request_offset: 0,
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
                PlainHttpState::Disconnected => {
                    crate::serial_println!("[HTTP] Connecting to {}:{}...", self.target_ip, self.port);
                    let cx = stack.iface.context();
                    if socket.connect(cx, (self.target_ip, self.port), self.local_port).is_ok() {
                        self.state = PlainHttpState::SendingRequest;
                    } else {
                        self.state = PlainHttpState::Error("Connection setup failed");
                    }
                }
                PlainHttpState::SendingRequest => {
                    if socket.state() == State::Closed {
                        self.state = PlainHttpState::Error("Connection failed");
                        return;
                    }
                    if socket.can_send() {
                        let remaining = &self.request[self.request_offset..];
                        if !remaining.is_empty() {
                            let sent = socket.send_slice(remaining).unwrap_or(0);
                            self.request_offset += sent;
                        }
                        if self.request_offset >= self.request.len() {
                            crate::serial_println!("[HTTP] Request bytes flushed; waiting for response");
                            self.state = PlainHttpState::ReceivingData(Vec::new());
                        }
                    }
                }
                PlainHttpState::ReceivingData(buf) => {
                    while socket.can_recv() {
                        let mut temp_buf = [0u8; 1024];
                        if let Ok(size) = socket.recv_slice(&mut temp_buf) {
                            if size > 0 {
                                buf.extend_from_slice(&temp_buf[..size]);
                                continue;
                            }
                        }
                        break;
                    }

                    if socket.state() == State::CloseWait || socket.state() == State::Closed {
                        let response_bytes = core::mem::take(buf);
                        if let Some(response) = parse_http_response(response_bytes) {
                            crate::serial_println!(
                                "[HTTP] Response complete: status={} body={} bytes",
                                response.status_code,
                                response.body.len()
                            );
                            self.state = PlainHttpState::Complete(response);
                        } else {
                            self.state = PlainHttpState::Error("HTTP parse failure");
                        }
                    }
                }
                PlainHttpState::Complete(_) => {}
                PlainHttpState::Error(_) => {}
            }
        }
    }

    pub fn take_response(&mut self) -> Option<ApiResponse> {
        if let PlainHttpState::Complete(_) = self.state {
            if let PlainHttpState::Complete(response) =
                core::mem::replace(&mut self.state, PlainHttpState::Disconnected)
            {
                return Some(response);
            }
        }
        None
    }

    pub fn error_message(&self) -> Option<&'static str> {
        match self.state {
            PlainHttpState::Error(message) => Some(message),
            _ => None,
        }
    }
}

fn flush_tls_outbound(socket: &mut Socket, tls: &mut TlsClient) {
    if tls.has_data_to_send() && socket.can_send() {
        let sent = socket.send_slice(tls.send_buf()).unwrap_or(0);
        if sent > 0 {
            tls.consume_send_buf(sent);
        }
    }
}

fn feed_tls_inbound(socket: &mut Socket, tls: &mut TlsClient) {
    while socket.can_recv() {
        let mut temp_buf = [0; 1024];
        if let Ok(size) = socket.recv_slice(&mut temp_buf) {
            if size > 0 {
                tls.feed_data(&temp_buf[..size]);
                continue;
            }
        }
        break;
    }
}

fn parse_http_response(data: Vec<u8>) -> Option<ApiResponse> {
    let body_start = find_header_terminator(&data)?;
    let header_bytes = &data[..body_start];
    let status_code = parse_status_code(header_bytes)?;
    let headers = parse_headers(header_bytes);

    Some(ApiResponse {
        status_code,
        headers,
        body: data[body_start..].to_vec(),
    })
}

fn find_header_terminator(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(3) {
        if &data[i..i + 4] == b"\r\n\r\n" {
            return Some(i + 4);
        }
    }

    for i in 0..data.len().saturating_sub(1) {
        if &data[i..i + 2] == b"\n\n" {
            return Some(i + 2);
        }
    }

    crate::serial_println!(
        "[HTTPS API] Unable to find HTTP header terminator in {} bytes",
        data.len()
    );
    if !data.is_empty() {
        let preview_len = core::cmp::min(data.len(), 96);
        crate::serial_println!(
            "[HTTPS API] Raw preview: {:?}",
            &data[..preview_len]
        );
    }
    None
}

fn parse_status_code(header_bytes: &[u8]) -> Option<u16> {
    let first_line_end = header_bytes
        .iter()
        .position(|&byte| byte == b'\n')
        .unwrap_or(header_bytes.len());
    let mut first_line = &header_bytes[..first_line_end];
    if first_line.last() == Some(&b'\r') {
        first_line = &first_line[..first_line.len().saturating_sub(1)];
    }

    let first_space = first_line.iter().position(|&byte| byte == b' ')?;
    let rest = &first_line[first_space + 1..];
    if rest.len() < 3 {
        return None;
    }

    let digits = &rest[..3];
    if !digits.iter().all(|byte| byte.is_ascii_digit()) {
        crate::serial_println!(
            "[HTTPS API] Invalid HTTP status line bytes: {:?}",
            first_line
        );
        return None;
    }

    Some(
        ((digits[0] - b'0') as u16) * 100
            + ((digits[1] - b'0') as u16) * 10
            + ((digits[2] - b'0') as u16),
    )
}

fn parse_headers(header_bytes: &[u8]) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    let text = match core::str::from_utf8(header_bytes) {
        Ok(text) => text,
        Err(_) => return headers,
    };

    for (index, line) in text.lines().enumerate() {
        if index == 0 {
            continue;
        }
        let trimmed = line.trim_end_matches('\r');
        if trimmed.is_empty() {
            continue;
        }
        let Some((name, value)) = trimmed.split_once(':') else {
            continue;
        };
        headers.push((String::from(name.trim()), String::from(value.trim())));
    }

    headers
}
