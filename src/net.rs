// src/net.rs
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};
use lazy_static::lazy_static;
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address};
use spin::Mutex;

lazy_static! {
    pub static ref RX_QUEUE: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    pub static ref TX_QUEUE: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    pub static ref ACTIVE_E1000: Mutex<Option<crate::e1000::E1000>> = Mutex::new(None);
}

const NET_STAGE_NATIVE_FALLBACK: u8 = 0;
const NET_STAGE_SANDBOX_CANDIDATE: u8 = 1;
const NET_STAGE_SANDBOX_ACTIVE: u8 = 2;

static NET_DRIVER_STAGE: AtomicU8 = AtomicU8::new(NET_STAGE_NATIVE_FALLBACK);
static NATIVE_RX_PACKETS: AtomicU32 = AtomicU32::new(0);
static NATIVE_TX_PACKETS: AtomicU32 = AtomicU32::new(0);
static SANDBOX_RX_PACKETS: AtomicU32 = AtomicU32::new(0);
static SANDBOX_TX_PACKETS: AtomicU32 = AtomicU32::new(0);
static SANDBOX_HEALTH_FLAGS: AtomicU32 = AtomicU32::new(0);
static SANDBOX_PROMOTION_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
static SANDBOX_PROMOTION_DENIED: AtomicU32 = AtomicU32::new(0);
static SANDBOX_PROMOTION_SUCCEEDED: AtomicU32 = AtomicU32::new(0);
static SANDBOX_LEASE_HEARTBEAT_TICK: AtomicU64 = AtomicU64::new(0);
static SANDBOX_LEASE_EXPIRED: AtomicU32 = AtomicU32::new(0);

const NET_SANDBOX_LEASE_TIMEOUT_TICKS: u64 = 3_000;

const NET_HEALTH_CLAIMED: u32 = 0x01;
const NET_HEALTH_PCI_CONFIG: u32 = 0x02;
const NET_HEALTH_MMIO: u32 = 0x04;
const NET_HEALTH_DMA: u32 = 0x08;
const NET_HEALTH_IRQ: u32 = 0x10;
const NET_HEALTH_TX_QUEUE: u32 = 0x20;
const NET_HEALTH_SUSTAINED_RX: u32 = 0x40;
const NET_HEALTH_SUSTAINED_TX: u32 = 0x80;
const NET_HEALTH_REQUIRED_FOR_ACTIVE: u32 = NET_HEALTH_CLAIMED
    | NET_HEALTH_PCI_CONFIG
    | NET_HEALTH_MMIO
    | NET_HEALTH_DMA
    | NET_HEALTH_IRQ
    | NET_HEALTH_TX_QUEUE
    | NET_HEALTH_SUSTAINED_RX
    | NET_HEALTH_SUSTAINED_TX;

lazy_static! {
    static ref SANDBOX_HEALTH_DRIVER: Mutex<Option<String>> = Mutex::new(None);
    static ref ACTIVE_SANDBOX_DRIVER: Mutex<Option<String>> = Mutex::new(None);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetDriverStage {
    NativeFallback,
    SandboxCandidate,
    SandboxActive,
}

impl NetDriverStage {
    pub fn as_str(self) -> &'static str {
        match self {
            NetDriverStage::NativeFallback => "native-fallback",
            NetDriverStage::SandboxCandidate => "sandbox-candidate",
            NetDriverStage::SandboxActive => "sandbox-active",
        }
    }
}

impl From<u32> for NetDriverStage {
    fn from(value: u32) -> Self {
        match value {
            1 => NetDriverStage::SandboxCandidate,
            2 => NetDriverStage::SandboxActive,
            _ => NetDriverStage::NativeFallback,
        }
    }
}

impl From<NetDriverStage> for u8 {
    fn from(value: NetDriverStage) -> Self {
        match value {
            NetDriverStage::NativeFallback => NET_STAGE_NATIVE_FALLBACK,
            NetDriverStage::SandboxCandidate => NET_STAGE_SANDBOX_CANDIDATE,
            NetDriverStage::SandboxActive => NET_STAGE_SANDBOX_ACTIVE,
        }
    }
}

pub fn attach_native_e1000(nic: crate::e1000::E1000) {
    *ACTIVE_E1000.lock() = Some(nic);
}

pub fn set_driver_stage(stage: NetDriverStage) {
    NET_DRIVER_STAGE.store(u8::from(stage), Ordering::Relaxed);
}

pub fn driver_stage() -> NetDriverStage {
    match NET_DRIVER_STAGE.load(Ordering::Relaxed) {
        NET_STAGE_SANDBOX_CANDIDATE => NetDriverStage::SandboxCandidate,
        NET_STAGE_SANDBOX_ACTIVE => NetDriverStage::SandboxActive,
        _ => NetDriverStage::NativeFallback,
    }
}

pub fn sandbox_ethernet_active() -> bool {
    driver_stage() == NetDriverStage::SandboxActive
}

pub fn begin_sandbox_ethernet_cutover_trial(driver_id: &str) -> u32 {
    let driver_id = driver_id.trim();
    if driver_id.is_empty() {
        return 0;
    }

    // This is a core-owned quiesce gate, not a sandbox self-promotion path.
    // It temporarily stops native RX/TX so a candidate can program live rings.
    set_driver_stage(NetDriverStage::SandboxActive);
    *ACTIVE_SANDBOX_DRIVER.lock() = Some(String::from(driver_id));
    SANDBOX_LEASE_HEARTBEAT_TICK.store(
        crate::task::timer::TICKS.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
    1
}

pub fn rollback_sandbox_ethernet_cutover(reason: &str) {
    set_driver_stage(NetDriverStage::NativeFallback);
    *ACTIVE_SANDBOX_DRIVER.lock() = None;
    SANDBOX_LEASE_EXPIRED.fetch_add(1, Ordering::Relaxed);
    crate::serial_println!(
        "[Net Driver Runtime] sandbox ethernet cutover rollback: {}",
        reason
    );
}

pub fn enqueue_sandbox_ethernet_probe_tx() {
    let mut frame = Vec::with_capacity(60);
    frame.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    frame.extend_from_slice(&[0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    frame.extend_from_slice(&[0x88, 0xb5]);
    frame.extend_from_slice(b"OpenRhiza sandbox e1000 cutover probe");
    while frame.len() < 60 {
        frame.push(0);
    }
    TX_QUEUE.lock().push(frame);
}

pub fn sandbox_ethernet_health_flags() -> u32 {
    SANDBOX_HEALTH_FLAGS.load(Ordering::Relaxed)
}

pub fn report_sandbox_ethernet_health(driver_id: &str, flags: u32) -> u32 {
    if driver_id.trim().is_empty() || driver_stage() == NetDriverStage::NativeFallback {
        return 0;
    }

    *SANDBOX_HEALTH_DRIVER.lock() = Some(String::from(driver_id.trim()));
    let sanitized = flags & NET_HEALTH_REQUIRED_FOR_ACTIVE;
    SANDBOX_HEALTH_FLAGS.fetch_or(sanitized, Ordering::Relaxed);
    1
}

pub fn sandbox_ethernet_heartbeat(driver_id: &str, flags: u32) -> u32 {
    let driver_id = driver_id.trim();
    if driver_id.is_empty() {
        return 0;
    }
    if SANDBOX_HEALTH_DRIVER.lock().as_deref() != Some(driver_id) {
        return 0;
    }

    SANDBOX_HEALTH_FLAGS.fetch_or(flags & NET_HEALTH_REQUIRED_FOR_ACTIVE, Ordering::Relaxed);
    SANDBOX_LEASE_HEARTBEAT_TICK.store(
        crate::task::timer::TICKS.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
    1
}

pub fn request_sandbox_ethernet_promotion(driver_id: &str) -> u32 {
    SANDBOX_PROMOTION_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    let requested_driver = driver_id.trim();
    let Some(health_driver) = SANDBOX_HEALTH_DRIVER.lock().clone() else {
        SANDBOX_PROMOTION_DENIED.fetch_add(1, Ordering::Relaxed);
        return 0;
    };
    if health_driver != requested_driver {
        SANDBOX_PROMOTION_DENIED.fetch_add(1, Ordering::Relaxed);
        return 0;
    }

    let flags = SANDBOX_HEALTH_FLAGS.load(Ordering::Relaxed);
    if (flags & NET_HEALTH_REQUIRED_FOR_ACTIVE) != NET_HEALTH_REQUIRED_FOR_ACTIVE {
        SANDBOX_PROMOTION_DENIED.fetch_add(1, Ordering::Relaxed);
        return 0;
    }

    set_driver_stage(NetDriverStage::SandboxActive);
    *ACTIVE_SANDBOX_DRIVER.lock() = Some(String::from(requested_driver));
    SANDBOX_LEASE_HEARTBEAT_TICK.store(
        crate::task::timer::TICKS.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
    SANDBOX_PROMOTION_SUCCEEDED.fetch_add(1, Ordering::Relaxed);
    1
}

pub fn watchdog() {
    if driver_stage() != NetDriverStage::SandboxActive {
        return;
    }

    let now = crate::task::timer::TICKS.load(Ordering::Relaxed);
    let last = SANDBOX_LEASE_HEARTBEAT_TICK.load(Ordering::Relaxed);
    if last == 0 || now.saturating_sub(last) <= NET_SANDBOX_LEASE_TIMEOUT_TICKS {
        return;
    }

    set_driver_stage(NetDriverStage::NativeFallback);
    *ACTIVE_SANDBOX_DRIVER.lock() = None;
    SANDBOX_LEASE_EXPIRED.fetch_add(1, Ordering::Relaxed);
    crate::serial_println!(
        "[Net Driver Runtime] sandbox ethernet lease expired; restored native e1000 fallback."
    );
}

pub fn queue_rx_packet(ptr: u32, len: u32) {
    unsafe {
        let slice = core::slice::from_raw_parts(ptr as *const u8, len as usize);
        let mut packet = Vec::with_capacity(len as usize);
        packet.extend_from_slice(slice);
        crate::serial_println!(
            "[OS Net] RX: Ingress Packet received from NIC (Wasm Sandbox), {} bytes",
            len
        );
        SANDBOX_RX_PACKETS.fetch_add(1, Ordering::Relaxed);
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
        if NET_PACKET_TRACE {
            crate::serial_println!(
                "[OS Net] TX: Transmission dispatched via SMOLTCP, {} bytes",
                len
            );
        }
        if sandbox_ethernet_active() {
            SANDBOX_TX_PACKETS.fetch_add(1, Ordering::Relaxed);
            TX_QUEUE.lock().push(buffer);
        } else if let Some(nic) = ACTIVE_E1000.lock().as_ref() {
            if !nic.send_packet(&buffer) {
                crate::serial_println!("[OS Net] Native e1000 TX ring busy; packet dropped");
            } else {
                NATIVE_TX_PACKETS.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            TX_QUEUE.lock().push(buffer);
        }
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

static NEXT_TCP_PORT: AtomicU16 = AtomicU16::new(49152);
const NET_PACKET_TRACE: bool = false;

pub fn init_network() {
    let mut device = WasmEthernetDevice;
    let hardware_addr = if let Some(mac) = ACTIVE_E1000.lock().as_ref().map(|nic| nic.mac) {
        HardwareAddress::Ethernet(EthernetAddress(mac))
    } else {
        HardwareAddress::Ethernet(EthernetAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]))
    };

    let mut config = Config::new(hardware_addr);
    config.random_seed = 0x123456789ABCDEF0;

    let mut iface = Interface::new(config, &mut device, Instant::from_millis(0));
    iface.update_ip_addrs(|ip_addrs| {
        ip_addrs
            .push(IpCidr::new(
                IpAddress::Ipv4(Ipv4Address::new(10, 0, 2, 15)),
                24,
            ))
            .unwrap();
    });

    iface
        .routes_mut()
        .add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2))
        .unwrap();

    let mut sockets = SocketSet::new(vec![]);

    // ICMP Echo / Ping Socket
    let icmp_rx_buffer = smoltcp::socket::icmp::PacketBuffer::new(
        vec![smoltcp::socket::icmp::PacketMetadata::EMPTY; 10],
        vec![0; 1024],
    );
    let icmp_tx_buffer = smoltcp::socket::icmp::PacketBuffer::new(
        vec![smoltcp::socket::icmp::PacketMetadata::EMPTY; 10],
        vec![0; 1024],
    );
    let icmp_socket = smoltcp::socket::icmp::Socket::new(icmp_rx_buffer, icmp_tx_buffer);
    sockets.add(icmp_socket);

    *NET_STACK.lock() = Some(NetStack { iface, sockets });
    crate::serial_println!(
        "[OS Net] smoltcp ICMP/TCP/IP Layer initialized over Dummy MAC on 10.0.2.15"
    );
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

pub fn create_udp_socket(rx_capacity: usize, tx_capacity: usize) -> SocketHandle {
    let udp_rx_buffer = smoltcp::socket::udp::PacketBuffer::new(
        vec![smoltcp::socket::udp::PacketMetadata::EMPTY; 4],
        vec![0; rx_capacity],
    );
    let udp_tx_buffer = smoltcp::socket::udp::PacketBuffer::new(
        vec![smoltcp::socket::udp::PacketMetadata::EMPTY; 4],
        vec![0; tx_capacity],
    );
    let udp_socket = smoltcp::socket::udp::Socket::new(udp_rx_buffer, udp_tx_buffer);

    let mut stack_lock = NET_STACK.lock();
    if let Some(stack) = stack_lock.as_mut() {
        stack.sockets.add(udp_socket)
    } else {
        panic!("Network stack not initialized!");
    }
}

pub fn destroy_socket(handle: SocketHandle) {
    let mut stack_lock = NET_STACK.lock();
    if let Some(stack) = stack_lock.as_mut() {
        stack.sockets.remove(handle);
    }
}

pub fn allocate_tcp_local_port() -> u16 {
    let port = NEXT_TCP_PORT.fetch_add(1, Ordering::Relaxed);
    if port >= 65000 {
        NEXT_TCP_PORT.store(49152, Ordering::Relaxed);
        49152
    } else {
        port
    }
}

pub fn poll(timestamp_ms: i64) {
    if !sandbox_ethernet_active() {
        if let Some(nic) = ACTIVE_E1000.lock().as_mut() {
            nic.poll_rx(|packet| {
                if NET_PACKET_TRACE {
                    crate::serial_println!(
                        "[OS Net] RX: Native e1000 ingress packet, {} bytes",
                        packet.len()
                    );
                }
                NATIVE_RX_PACKETS.fetch_add(1, Ordering::Relaxed);
                RX_QUEUE.lock().push(packet.to_vec());
            });
        }
    }

    if let Some(stack) = &mut *NET_STACK.lock() {
        let mut device = WasmEthernetDevice;
        stack.iface.poll(
            Instant::from_millis(timestamp_ms),
            &mut device,
            &mut stack.sockets,
        );
    }
}

pub fn status_block() -> alloc::string::String {
    let native_attached = ACTIVE_E1000.lock().is_some();
    let active_driver = ACTIVE_SANDBOX_DRIVER
        .lock()
        .clone()
        .unwrap_or_else(|| String::from("none"));
    alloc::format!(
        "[Net Driver Runtime] stage={} native_e1000={} active_driver={} rx_native={} tx_native={} rx_sandbox={} tx_sandbox={} tx_queue={} rx_queue={} health_flags={:#04x} last_heartbeat={} lease_expired={} promote_attempts={} promote_denied={} promote_ok={}",
        driver_stage().as_str(),
        native_attached as u8,
        active_driver,
        NATIVE_RX_PACKETS.load(Ordering::Relaxed),
        NATIVE_TX_PACKETS.load(Ordering::Relaxed),
        SANDBOX_RX_PACKETS.load(Ordering::Relaxed),
        SANDBOX_TX_PACKETS.load(Ordering::Relaxed),
        TX_QUEUE.lock().len(),
        RX_QUEUE.lock().len(),
        SANDBOX_HEALTH_FLAGS.load(Ordering::Relaxed),
        SANDBOX_LEASE_HEARTBEAT_TICK.load(Ordering::Relaxed),
        SANDBOX_LEASE_EXPIRED.load(Ordering::Relaxed),
        SANDBOX_PROMOTION_ATTEMPTS.load(Ordering::Relaxed),
        SANDBOX_PROMOTION_DENIED.load(Ordering::Relaxed),
        SANDBOX_PROMOTION_SUCCEEDED.load(Ordering::Relaxed)
    )
}
