use alloc::vec::Vec;
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

pub struct NexusClient {
    handle: SocketHandle,
    state: HttpsState,
    target_ip: IpAddress,
    port: u16,
    hardware_id: &'static str,
    tls: TlsClient,
}

impl NexusClient {
    pub fn new(handle: SocketHandle, target_ip: IpAddress, port: u16, hardware_id: &'static str) -> Self {
        NexusClient {
            handle,
            state: HttpsState::Disconnected,
            target_ip,
            port,
            hardware_id,
            tls: TlsClient::new("openrhiza.com"),
        }
    }

    pub fn poll(&mut self) {
        let mut net_stack = NET_STACK.lock();
        if let Some(stack) = net_stack.as_mut() {
            let socket = stack.sockets.get_mut::<Socket>(self.handle);

            match &mut self.state {
                HttpsState::Disconnected => {
                    crate::serial_println!("[HTTPS] Connecting to Nexus API...");
                    let cx = stack.iface.context();
                    if socket.connect(cx, (self.target_ip, self.port), 49152).is_ok() {
                        self.state = HttpsState::Connecting;
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
