// src/net.rs
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;
use lazy_static::lazy_static;
use smoltcp::phy::{Device, DeviceCapabilities, RxToken, TxToken, Medium};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address};
use smoltcp::iface::{Config, Interface, SocketSet, SocketHandle};

lazy_static! {
    pub static ref RX_QUEUE: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    pub static ref TX_QUEUE: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
}

pub fn queue_rx_packet(ptr: u32, len: u32) {
    unsafe {
        let slice = core::slice::from_raw_parts(ptr as *const u8, len as usize);
        let mut packet = Vec::with_capacity(len as usize);
        packet.extend_from_slice(slice);
        crate::serial_println!("[OS Net] RX: Ingress Packet received from NIC (Wasm Sandbox), {} bytes", len);
        RX_QUEUE.lock().push(packet);
    }
}

pub struct WasmEthernetDevice;

impl Device for WasmEthernetDevice {
    type RxToken<'a> = WasmRxToken;
    type TxToken<'a> = WasmTxToken;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut rx = RX_QUEUE.lock();
        if !rx.is_empty() {
            let packet = rx.remove(0);
            Some((WasmRxToken(packet), WasmTxToken))
        } else {
            None
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(WasmTxToken)
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1500;
        caps.max_burst_size = Some(1);
        caps.medium = Medium::Ethernet;
        caps
    }
}

pub struct WasmRxToken(Vec<u8>);

impl RxToken for WasmRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = self.0.clone();
        f(&mut buffer)
    }
}

pub struct WasmTxToken;

impl TxToken for WasmTxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = vec![0; len];
        let result = f(&mut buffer);
        crate::serial_println!("[OS Net] TX: Transmission dispatched via SMOLTCP, {} bytes", len);
        TX_QUEUE.lock().push(buffer);
        result
    }
}

pub struct NetStack {
    pub iface: Interface,
    pub sockets: SocketSet<'static>,
}

lazy_static! {
    pub static ref NET_STACK: Mutex<Option<NetStack>> = Mutex::new(None);
}

pub fn init_network() {
    let mut device = WasmEthernetDevice;
    let hardware_addr = HardwareAddress::Ethernet(EthernetAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]));
    
    let mut config = Config::new(hardware_addr);
    config.random_seed = 0x123456789ABCDEF0;
    
    let mut iface = Interface::new(config, &mut device, Instant::from_millis(0));
    iface.update_ip_addrs(|ip_addrs| {
        ip_addrs.push(IpCidr::new(IpAddress::Ipv4(Ipv4Address::new(10, 0, 2, 15)), 24)).unwrap();
    });

    iface.routes_mut().add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2)).unwrap();

    let mut sockets = SocketSet::new(vec![]);
    
    // ICMP Echo / Ping Socket
    let icmp_rx_buffer = smoltcp::socket::icmp::PacketBuffer::new(vec![smoltcp::socket::icmp::PacketMetadata::EMPTY; 10], vec![0; 1024]);
    let icmp_tx_buffer = smoltcp::socket::icmp::PacketBuffer::new(vec![smoltcp::socket::icmp::PacketMetadata::EMPTY; 10], vec![0; 1024]);
    let icmp_socket = smoltcp::socket::icmp::Socket::new(icmp_rx_buffer, icmp_tx_buffer);
    sockets.add(icmp_socket);

    *NET_STACK.lock() = Some(NetStack { iface, sockets });
    crate::serial_println!("[OS Net] smoltcp ICMP/TCP/IP Layer initialized over Dummy MAC on 10.0.2.15");
}

pub fn create_tcp_socket() -> SocketHandle {
    let tcp_rx_buffer = smoltcp::socket::tcp::SocketBuffer::new(vec![0; 4096]);
    let tcp_tx_buffer = smoltcp::socket::tcp::SocketBuffer::new(vec![0; 4096]);
    let tcp_socket = smoltcp::socket::tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);
    
    let mut stack_lock = NET_STACK.lock();
    if let Some(stack) = stack_lock.as_mut() {
        stack.sockets.add(tcp_socket)
    } else {
        panic!("Network stack not initialized!");
    }
}

pub fn poll(timestamp_ms: i64) {
    if let Some(stack) = &mut *NET_STACK.lock() {
        let mut device = WasmEthernetDevice;
        stack.iface.poll(Instant::from_millis(timestamp_ms), &mut device, &mut stack.sockets);
    }
}
