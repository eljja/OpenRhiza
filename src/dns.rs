use alloc::vec::Vec;

use smoltcp::iface::SocketHandle;
use smoltcp::socket::udp::Socket;
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

use crate::net::NET_STACK;

const DNS_PORT: u16 = 53;
pub const DEFAULT_DNS_SERVER: Ipv4Address = Ipv4Address::new(10, 0, 2, 3);

pub enum DnsState {
    Disconnected,
    QuerySent,
    Complete(Ipv4Address),
    Error(&'static str),
}

pub struct DnsClient {
    handle: SocketHandle,
    local_port: u16,
    server: Ipv4Address,
    hostname: &'static str,
    txid: u16,
    bound: bool,
    query_sent: bool,
    wait_ticks: u32,
    state: DnsState,
}

impl DnsClient {
    pub fn new(handle: SocketHandle, server: Ipv4Address, hostname: &'static str) -> Self {
        let random = crate::crypto::random::random_bytes_32();
        let txid = u16::from_be_bytes([random[0], random[1]]);

        Self {
            handle,
            local_port: crate::net::allocate_tcp_local_port(),
            server,
            hostname,
            txid,
            bound: false,
            query_sent: false,
            wait_ticks: 0,
            state: DnsState::Disconnected,
        }
    }

    pub fn handle(&self) -> SocketHandle {
        self.handle
    }

    pub fn poll(&mut self) {
        let mut net_stack = NET_STACK.lock();
        if let Some(stack) = net_stack.as_mut() {
            let socket = stack.sockets.get_mut::<Socket>(self.handle);

            match self.state {
                DnsState::Disconnected => {
                    if !self.bound {
                        if socket.bind(self.local_port).is_err() {
                            self.state = DnsState::Error("DNS bind failed");
                            return;
                        }
                        self.bound = true;
                    }

                    if socket.can_send() {
                        let query = build_a_query(self.txid, self.hostname);
                        let endpoint = IpEndpoint::new(IpAddress::Ipv4(self.server), DNS_PORT);
                        if socket.send_slice(&query, endpoint).is_ok() {
                            crate::println!(
                                "[DNS] Query sent for {} via {}",
                                self.hostname,
                                self.server
                            );
                            self.query_sent = true;
                            self.wait_ticks = 0;
                            self.state = DnsState::QuerySent;
                        } else {
                            self.state = DnsState::Error("DNS send failed");
                        }
                    }
                }
                DnsState::QuerySent => {
                    self.wait_ticks += 1;

                    if socket.can_recv() {
                        let mut recv_buf = [0u8; 512];
                        match socket.recv_slice(&mut recv_buf) {
                            Ok((size, _meta)) => match parse_a_response(&recv_buf[..size], self.txid)
                            {
                                Some(ip) => {
                                    crate::println!("[DNS] {} resolved to {}", self.hostname, ip);
                                    self.state = DnsState::Complete(ip);
                                }
                                None => self.state = DnsState::Error("DNS parse failed"),
                            },
                            Err(_) => self.state = DnsState::Error("DNS recv failed"),
                        }
                    } else if self.wait_ticks > 3000 {
                        self.state = DnsState::Error("DNS timeout");
                    }
                }
                DnsState::Complete(_) => {}
                DnsState::Error(_) => {}
            }
        }
    }

    pub fn take_resolved_ip(&mut self) -> Option<Ipv4Address> {
        if let DnsState::Complete(_) = self.state {
            if let DnsState::Complete(ip) =
                core::mem::replace(&mut self.state, DnsState::Disconnected)
            {
                return Some(ip);
            }
        }
        None
    }

    pub fn error_message(&self) -> Option<&'static str> {
        match self.state {
            DnsState::Error(message) => Some(message),
            _ => None,
        }
    }
}

fn build_a_query(txid: u16, hostname: &str) -> Vec<u8> {
    let mut query = Vec::new();
    query.extend_from_slice(&txid.to_be_bytes());
    query.extend_from_slice(&0x0100u16.to_be_bytes());
    query.extend_from_slice(&1u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());

    for label in hostname.split('.') {
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }
    query.push(0);
    query.extend_from_slice(&1u16.to_be_bytes());
    query.extend_from_slice(&1u16.to_be_bytes());
    query
}

fn parse_a_response(packet: &[u8], expected_txid: u16) -> Option<Ipv4Address> {
    if packet.len() < 12 {
        return None;
    }

    let txid = u16::from_be_bytes([packet[0], packet[1]]);
    if txid != expected_txid {
        return None;
    }

    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    if (flags & 0x8000) == 0 || (flags & 0x000F) != 0 {
        return None;
    }

    let question_count = u16::from_be_bytes([packet[4], packet[5]]) as usize;
    let answer_count = u16::from_be_bytes([packet[6], packet[7]]) as usize;
    let authority_count = u16::from_be_bytes([packet[8], packet[9]]) as usize;
    let additional_count = u16::from_be_bytes([packet[10], packet[11]]) as usize;

    let mut offset = 12;
    for _ in 0..question_count {
        skip_name(packet, &mut offset)?;
        offset = offset.checked_add(4)?;
        if offset > packet.len() {
            return None;
        }
    }

    let total_records = answer_count + authority_count + additional_count;
    for _ in 0..total_records {
        skip_name(packet, &mut offset)?;
        if offset + 10 > packet.len() {
            return None;
        }

        let record_type = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
        let class = u16::from_be_bytes([packet[offset + 2], packet[offset + 3]]);
        let rdlength = u16::from_be_bytes([packet[offset + 8], packet[offset + 9]]) as usize;
        offset += 10;

        if offset + rdlength > packet.len() {
            return None;
        }

        if record_type == 1 && class == 1 && rdlength == 4 {
            return Some(Ipv4Address::new(
                packet[offset],
                packet[offset + 1],
                packet[offset + 2],
                packet[offset + 3],
            ));
        }

        offset += rdlength;
    }

    None
}

fn skip_name(packet: &[u8], offset: &mut usize) -> Option<()> {
    loop {
        if *offset >= packet.len() {
            return None;
        }

        let len = packet[*offset];
        if len & 0xC0 == 0xC0 {
            *offset = (*offset).checked_add(2)?;
            return Some(());
        }

        if len == 0 {
            *offset = (*offset).checked_add(1)?;
            return Some(());
        }

        *offset = (*offset).checked_add(1 + len as usize)?;
    }
}
