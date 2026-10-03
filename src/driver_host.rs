use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lazy_static::lazy_static;
use spin::Mutex;

const MAX_GRANTS: usize = 32;
const MAX_DMA_GRANTS: usize = 64;
const MAX_VIRTIO_LIVE_GATES: usize = 16;
const DEFAULT_MMIO_WINDOW_BYTES: u32 = 0x20_000;
const DEFAULT_PIO_WINDOW_BYTES: u16 = 0x20;
const DRIVER_HANDLE_BASE: u32 = 0x4452_0000; // "DR"
const DMA_HANDLE_BASE: u32 = 0x444D_0000; // "DM"
const CAP_PCI_CONFIG: u32 = 0x01;
const CAP_MMIO: u32 = 0x02;
const CAP_PIO: u32 = 0x04;
const CAP_DMA: u32 = 0x08;
const CAP_IRQ: u32 = 0x10;
const QEMU_VIRTIO_MMIO_BASE: u64 = 0x0a00_0000;
const QEMU_VIRTIO_MMIO_STRIDE: u64 = 0x200;
const QEMU_VIRTIO_MMIO_WINDOW_BYTES: u32 = 0x200;
const QEMU_VIRTIO_MMIO_COUNT: u32 = 32;
const VIRTIO_MAX_SAFE_GATE_QUEUE_INDEX: u32 = 7;
const VIRTIO_MAGIC: u32 = 0x7472_6976;
const VIRTIO_MMIO_MAGIC_OFFSET: u32 = 0x000;
const VIRTIO_MMIO_VERSION_OFFSET: u32 = 0x004;
const VIRTIO_MMIO_DEVICE_ID_OFFSET: u32 = 0x008;
const VIRTIO_MMIO_QUEUE_SEL_OFFSET: u32 = 0x030;
const VIRTIO_MMIO_QUEUE_NUM_MAX_OFFSET: u32 = 0x034;
const VIRTIO_GATE_HANDLE_BASE: u32 = 0x5647_0000; // "VG"
const VIRTIO_GATE_FEATURES_OK: u32 = 0x01;
const VIRTIO_GATE_QUEUE_OK: u32 = 0x02;
const VIRTIO_GATE_NOTIFY_OK: u32 = 0x04;
const VIRTIO_GATE_IRQ_OK: u32 = 0x08;
const VIRTIO_GATE_REQUIRED: u32 =
    VIRTIO_GATE_FEATURES_OK | VIRTIO_GATE_QUEUE_OK | VIRTIO_GATE_NOTIFY_OK | VIRTIO_GATE_IRQ_OK;

#[derive(Clone, Debug)]
struct DeviceDescriptor {
    match_key: String,
    class_key: String,
    bus: u8,
    device: u8,
    func: u8,
    class_code: u8,
    subclass: u8,
    bar0: u32,
    pci_config_available: bool,
    fixed_mmio_base: u64,
    fixed_mmio_len: u32,
    irq_line: u32,
}

#[derive(Clone, Debug)]
struct DriverGrant {
    handle: u32,
    match_key: String,
    driver_id: String,
    bus: u8,
    device: u8,
    func: u8,
    pci_config_available: bool,
    caps: u32,
    mmio_base: u64,
    mmio_len: u32,
    pio_base: u16,
    pio_len: u16,
    irq_line: u32,
    dma_bytes: u32,
    irq_poll_count: u32,
    last_heartbeat_tick: u64,
}

#[derive(Clone, Debug)]
struct DmaGrant {
    handle: u32,
    owner: u32,
    phys: u32,
    len: u32,
}

#[derive(Clone, Debug)]
// Compatibility proof ledger for existing virtio sandbox driver seeds.
//
// This must stay a bounded evidence record only: it may validate feature,
// queue, notify, and IRQ/used-ring evidence, but it must not become the place
// where virtio routing policy, payload construction, or boot-default ownership
// is implemented. New transports should move toward a generic promotion
// evidence ABI and keep protocol behavior in sandbox capabilities.
struct VirtioLiveGate {
    gate: u32,
    owner_handle: u32,
    driver_id: String,
    match_key: String,
    device_id: u32,
    queue_index: u32,
    queue_size: u32,
    dma_handle: u32,
    dma_phys: u32,
    irq_line: u32,
    prepared_tick: u64,
    validation_flags: u32,
    negotiated_features: u32,
    notify_count: u32,
    irq_complete_count: u32,
    committed: bool,
}

#[derive(Clone, Debug, Default)]
struct DriverHostCounters {
    claim_attempts: u32,
    claim_ok: u32,
    claim_denied: u32,
    release_ok: u32,
    release_denied: u32,
    lease_expired: u32,
    virtio_gate_prepare_ok: u32,
    virtio_gate_prepare_denied: u32,
    virtio_gate_validation_ok: u32,
    virtio_gate_validation_denied: u32,
    virtio_gate_commit_ok: u32,
    virtio_gate_commit_denied: u32,
    virtio_gate_rollback_ok: u32,
}

lazy_static! {
    static ref DEVICES: Mutex<Vec<DeviceDescriptor>> = Mutex::new(Vec::new());
    static ref GRANTS: Mutex<Vec<DriverGrant>> = Mutex::new(Vec::new());
    static ref DMA_GRANTS: Mutex<Vec<DmaGrant>> = Mutex::new(Vec::new());
    static ref VIRTIO_LIVE_GATES: Mutex<Vec<VirtioLiveGate>> = Mutex::new(Vec::new());
    static ref COUNTERS: Mutex<DriverHostCounters> = Mutex::new(DriverHostCounters::default());
}

fn pci_config_address(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

pub fn install_identity(identity: &crate::arch::x86_64::discovery::SystemIdentity) {
    let mut devices = DEVICES.lock();
    devices.clear();

    for device in &identity.pci_devices {
        devices.push(DeviceDescriptor {
            match_key: format!("pci:{:04x}:{:04x}", device.vendor_id, device.device_id),
            class_key: format!("pci:class:{:02x}{:02x}", device.class_code, device.subclass),
            bus: device.bus,
            device: device.device,
            func: device.func,
            class_code: device.class_code,
            subclass: device.subclass,
            bar0: device.bar0,
            pci_config_available: true,
            fixed_mmio_base: 0,
            fixed_mmio_len: 0,
            irq_line: 0,
        });
    }
}

pub fn install_aarch64_qemu_virtio_mmio_stubs() {
    install_platform_mmio_range(
        "aarch64-qemu-virt",
        "virtio:mmio",
        QEMU_VIRTIO_MMIO_BASE,
        QEMU_VIRTIO_MMIO_STRIDE,
        QEMU_VIRTIO_MMIO_WINDOW_BYTES,
        QEMU_VIRTIO_MMIO_COUNT,
        16,
    );
}

pub fn install_platform_mmio_range(
    platform_id: &str,
    class_key: &str,
    base: u64,
    stride: u64,
    window_len: u32,
    count: u32,
    first_irq: u32,
) {
    let mut devices = DEVICES.lock();
    for index in 0..count.min(32) {
        let mmio_base = base.saturating_add(stride.saturating_mul(index as u64));
        let match_key = format!("virtio-mmio:{:#010x}", mmio_base);
        if devices.iter().any(|device| device.match_key == match_key) {
            continue;
        }
        devices.push(DeviceDescriptor {
            match_key,
            class_key: String::from(class_key.trim()),
            bus: 0,
            device: index as u8,
            func: 0,
            class_code: 0,
            subclass: 0,
            bar0: 0,
            pci_config_available: false,
            fixed_mmio_base: mmio_base,
            fixed_mmio_len: window_len,
            irq_line: first_irq.saturating_add(index),
        });
    }
    crate::serial_println!(
        "[Driver Host ABI] installed {} {} MMIO capability stubs for {}.",
        count.min(32),
        class_key,
        platform_id
    );
}

fn find_device(match_key: &str) -> Option<DeviceDescriptor> {
    let key = match_key.trim();
    DEVICES
        .lock()
        .iter()
        .find(|device| device.match_key == key || device.class_key == key)
        .cloned()
}

pub fn claim_device(match_key: &str, driver_id: &str, caps: u32) -> u32 {
    {
        let mut counters = COUNTERS.lock();
        counters.claim_attempts = counters.claim_attempts.saturating_add(1);
    }
    let Some(device) = find_device(match_key) else {
        let mut counters = COUNTERS.lock();
        counters.claim_denied = counters.claim_denied.saturating_add(1);
        return 0;
    };

    let mut grants = GRANTS.lock();
    if let Some(existing) = grants
        .iter_mut()
        .find(|grant| grant.match_key == match_key && grant.driver_id == driver_id)
    {
        existing.caps |= sanitize_caps(caps);
        let mut counters = COUNTERS.lock();
        counters.claim_ok = counters.claim_ok.saturating_add(1);
        return existing.handle;
    }
    if grants.len() >= MAX_GRANTS {
        let mut counters = COUNTERS.lock();
        counters.claim_denied = counters.claim_denied.saturating_add(1);
        return 0;
    }

    let handle = DRIVER_HANDLE_BASE | ((grants.len() as u32 + 1) & 0xFFFF);
    let bar0_is_io = (device.bar0 & 0x1) != 0;
    let legacy_ide_pio = device.class_code == 0x01 && device.subclass == 0x01;
    let fixed_mmio = device.fixed_mmio_base != 0 && device.fixed_mmio_len != 0;
    let grant = DriverGrant {
        handle,
        match_key: String::from(match_key.trim()),
        driver_id: String::from(driver_id.trim()),
        bus: device.bus,
        device: device.device,
        func: device.func,
        pci_config_available: device.pci_config_available,
        caps: sanitize_caps(caps),
        mmio_base: if fixed_mmio {
            device.fixed_mmio_base
        } else if !bar0_is_io && device.bar0 != 0 {
            (device.bar0 & 0xFFFF_FFF0) as u64
        } else {
            0
        },
        mmio_len: if fixed_mmio {
            device.fixed_mmio_len
        } else if !bar0_is_io && device.bar0 != 0 {
            DEFAULT_MMIO_WINDOW_BYTES
        } else {
            0
        },
        pio_base: if legacy_ide_pio {
            0x0170
        } else if bar0_is_io {
            (device.bar0 & 0xFFFC) as u16
        } else {
            0
        },
        pio_len: if legacy_ide_pio {
            0x0090
        } else if bar0_is_io {
            DEFAULT_PIO_WINDOW_BYTES
        } else {
            0
        },
        irq_line: device.irq_line,
        dma_bytes: 0,
        irq_poll_count: 0,
        last_heartbeat_tick: crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed),
    };
    grants.push(grant);
    let mut counters = COUNTERS.lock();
    counters.claim_ok = counters.claim_ok.saturating_add(1);
    handle
}

pub fn claim_mmio_device(match_key: &str, driver_id: &str, caps: u32) -> u32 {
    claim_device(match_key, driver_id, caps & (CAP_MMIO | CAP_DMA | CAP_IRQ))
}

pub fn release_device(handle: u32) -> u32 {
    let mut grants = GRANTS.lock();
    let Some(index) = grants.iter().position(|grant| grant.handle == handle) else {
        let mut counters = COUNTERS.lock();
        counters.release_denied = counters.release_denied.saturating_add(1);
        return 0;
    };

    grants.remove(index);
    drop(grants);

    let mut dmas = DMA_GRANTS.lock();
    dmas.retain(|dma| dma.owner != handle);
    VIRTIO_LIVE_GATES
        .lock()
        .retain(|gate| gate.owner_handle != handle);
    let mut counters = COUNTERS.lock();
    counters.release_ok = counters.release_ok.saturating_add(1);
    1
}

pub fn handle_heartbeat(handle: u32) -> u32 {
    let mut grants = GRANTS.lock();
    let Some(grant) = grants.iter_mut().find(|grant| grant.handle == handle) else {
        return 0;
    };
    grant.last_heartbeat_tick =
        crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
    1
}

pub fn watchdog() {
    const DRIVER_HANDLE_LEASE_TIMEOUT_TICKS: u64 = 5_000;

    let now = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
    let mut expired_handles: Vec<u32> = Vec::new();
    {
        let grants = GRANTS.lock();
        for grant in grants.iter() {
            if now.saturating_sub(grant.last_heartbeat_tick) > DRIVER_HANDLE_LEASE_TIMEOUT_TICKS {
                expired_handles.push(grant.handle);
            }
        }
    }

    if expired_handles.is_empty() {
        return;
    }

    for handle in expired_handles {
        if release_device(handle) != 0 {
            let mut counters = COUNTERS.lock();
            counters.lease_expired = counters.lease_expired.saturating_add(1);
            crate::serial_println!(
                "[Driver Host ABI] released stale sandbox device handle {:#010x}.",
                handle
            );
        }
    }
}

fn sanitize_caps(caps: u32) -> u32 {
    caps & (CAP_PCI_CONFIG | CAP_MMIO | CAP_PIO | CAP_DMA | CAP_IRQ)
}

fn has_cap(grant: &DriverGrant, cap: u32) -> bool {
    (grant.caps & cap) != 0
}

fn grant_for(handle: u32) -> Option<DriverGrant> {
    GRANTS
        .lock()
        .iter()
        .find(|grant| grant.handle == handle)
        .cloned()
}

fn mmio_addr(grant: &DriverGrant, offset: u32, width: u32) -> Option<u64> {
    let end = offset.checked_add(width)?;
    if grant.mmio_base == 0 || end > grant.mmio_len {
        return None;
    }
    let phys_mem_offset = unsafe { crate::arch::x86_64::discovery::PHYS_MEM_OFFSET };
    Some(phys_mem_offset + grant.mmio_base + offset as u64)
}

fn pio_port(grant: &DriverGrant, offset: u16, width: u16) -> Option<u16> {
    let end = offset.checked_add(width)?;
    if grant.pio_base == 0 || end > grant.pio_len {
        return None;
    }
    grant.pio_base.checked_add(offset)
}

pub fn mmio_read32(handle: u32, offset: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_MMIO) {
        return 0;
    }
    let Some(addr) = mmio_addr(&grant, offset, 4) else {
        return 0;
    };
    unsafe { core::ptr::read_volatile(addr as usize as *const u32) }
}

pub fn mmio_write32(handle: u32, offset: u32, value: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_MMIO) {
        return 0;
    }
    let Some(addr) = mmio_addr(&grant, offset, 4) else {
        return 0;
    };
    unsafe { core::ptr::write_volatile(addr as usize as *mut u32, value) };
    1
}

pub fn pio_read8(handle: u32, offset: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_PIO) {
        return 0;
    }
    let Some(port) = pio_port(&grant, offset as u16, 1) else {
        return 0;
    };
    crate::arch::x86_64::port::read_port_u8(port) as u32
}

pub fn pio_read16(handle: u32, offset: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_PIO) {
        return 0;
    }
    let Some(port) = pio_port(&grant, offset as u16, 2) else {
        return 0;
    };
    crate::arch::x86_64::port::read_port_u16(port) as u32
}

pub fn pio_read32(handle: u32, offset: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_PIO) {
        return 0;
    }
    let Some(port) = pio_port(&grant, offset as u16, 4) else {
        return 0;
    };
    crate::arch::x86_64::port::read_port_u32(port)
}

pub fn pio_write8(handle: u32, offset: u32, value: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_PIO) {
        return 0;
    }
    let Some(port) = pio_port(&grant, offset as u16, 1) else {
        return 0;
    };
    crate::arch::x86_64::port::write_port_u8(port, value as u8);
    1
}

pub fn pio_write16(handle: u32, offset: u32, value: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_PIO) {
        return 0;
    }
    let Some(port) = pio_port(&grant, offset as u16, 2) else {
        return 0;
    };
    crate::arch::x86_64::port::write_port_u16(port, value as u16);
    1
}

pub fn pio_write32(handle: u32, offset: u32, value: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_PIO) {
        return 0;
    }
    let Some(port) = pio_port(&grant, offset as u16, 4) else {
        return 0;
    };
    crate::arch::x86_64::port::write_port_u32(port, value);
    1
}

pub fn pci_config_read32(handle: u32, offset: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_PCI_CONFIG) {
        return 0;
    }
    if !grant.pci_config_available {
        return 0;
    }
    if offset > 0xFC {
        return 0;
    }
    let address = pci_config_address(grant.bus, grant.device, grant.func, offset as u8);
    crate::arch::x86_64::port::write_port_u32(0xCF8, address);
    crate::arch::x86_64::port::read_port_u32(0xCFC)
}

pub fn pci_config_write32(handle: u32, offset: u32, value: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_PCI_CONFIG) {
        return 0;
    }
    if !grant.pci_config_available {
        return 0;
    }
    if offset > 0xFC {
        return 0;
    }
    let address = pci_config_address(grant.bus, grant.device, grant.func, offset as u8);
    crate::arch::x86_64::port::write_port_u32(0xCF8, address);
    crate::arch::x86_64::port::write_port_u32(0xCFC, value);
    1
}

pub fn dma_alloc(handle: u32, byte_len: u32, align: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_DMA) {
        return 0;
    }
    if byte_len == 0 || byte_len > 0x10000 || DMA_GRANTS.lock().len() >= MAX_DMA_GRANTS {
        return 0;
    }

    let alignment = align.max(4096).next_power_of_two();
    unsafe {
        let base = crate::arch::x86_64::discovery::DMA_BASE;
        if base == 0 {
            return 0;
        }
        let current = base.saturating_add(crate::arch::x86_64::discovery::DMA_OFFSET);
        let aligned = (current + alignment - 1) & !(alignment - 1);
        let next_offset = aligned.saturating_sub(base).saturating_add(byte_len);
        crate::arch::x86_64::discovery::DMA_OFFSET = (next_offset + 4095) & !4095;

        let virt = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + aligned as u64;
        core::ptr::write_bytes(virt as usize as *mut u8, 0, byte_len as usize);

        let mut dmas = DMA_GRANTS.lock();
        let dma_handle = DMA_HANDLE_BASE | ((dmas.len() as u32 + 1) & 0xFFFF);
        dmas.push(DmaGrant {
            handle: dma_handle,
            owner: handle,
            phys: aligned,
            len: byte_len,
        });

        if let Some(grant) = GRANTS
            .lock()
            .iter_mut()
            .find(|grant| grant.handle == handle)
        {
            grant.dma_bytes = grant.dma_bytes.saturating_add(byte_len);
        }

        dma_handle
    }
}

fn dma_for(owner: u32, dma_handle: u32) -> Option<DmaGrant> {
    DMA_GRANTS
        .lock()
        .iter()
        .find(|dma| dma.owner == owner && dma.handle == dma_handle)
        .cloned()
}

pub fn dma_phys(handle: u32, dma_handle: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_DMA) {
        return 0;
    }
    dma_for(handle, dma_handle).map(|dma| dma.phys).unwrap_or(0)
}

pub fn dma_len(handle: u32, dma_handle: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_DMA) {
        return 0;
    }
    dma_for(handle, dma_handle).map(|dma| dma.len).unwrap_or(0)
}

pub fn dma_write(handle: u32, dma_handle: u32, offset: u32, bytes: &[u8]) -> u32 {
    let Some(dma) = dma_for(handle, dma_handle) else {
        return 0;
    };
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_DMA) {
        return 0;
    }
    let Some(end) = offset.checked_add(bytes.len() as u32) else {
        return 0;
    };
    if end > dma.len {
        return 0;
    }
    unsafe {
        let virt =
            crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + dma.phys as u64 + offset as u64;
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), virt as usize as *mut u8, bytes.len());
    }
    bytes.len() as u32
}

pub fn dma_read(handle: u32, dma_handle: u32, offset: u32, out: &mut [u8]) -> u32 {
    let Some(dma) = dma_for(handle, dma_handle) else {
        return 0;
    };
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if !has_cap(&grant, CAP_DMA) {
        return 0;
    }
    let Some(end) = offset.checked_add(out.len() as u32) else {
        return 0;
    };
    if end > dma.len {
        return 0;
    }
    unsafe {
        let virt =
            crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + dma.phys as u64 + offset as u64;
        core::ptr::copy_nonoverlapping(virt as usize as *const u8, out.as_mut_ptr(), out.len());
    }
    out.len() as u32
}

pub fn irq_poll(handle: u32) -> u32 {
    let mut grants = GRANTS.lock();
    let Some(grant) = grants.iter_mut().find(|grant| grant.handle == handle) else {
        return 0;
    };
    if !has_cap(grant, CAP_IRQ) {
        return 0;
    }
    grant.irq_poll_count = grant.irq_poll_count.saturating_add(1);
    grant.irq_line
}

pub fn irq_ack(handle: u32, _mask: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    if has_cap(&grant, CAP_IRQ) {
        1
    } else {
        0
    }
}

pub fn virtio_prepare_live_gate(
    handle: u32,
    expected_device_id: u32,
    queue_index: u32,
    dma_handle: u32,
    queue_size: u32,
) -> u32 {
    let Some(grant) = grant_for(handle) else {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_prepare_denied = counters.virtio_gate_prepare_denied.saturating_add(1);
        return 0;
    };
    if !grant.match_key.starts_with("virtio-mmio:")
        || !has_cap(&grant, CAP_MMIO)
        || !has_cap(&grant, CAP_DMA)
        || !has_cap(&grant, CAP_IRQ)
    {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_prepare_denied = counters.virtio_gate_prepare_denied.saturating_add(1);
        return 0;
    }

    let magic = mmio_read32(handle, VIRTIO_MMIO_MAGIC_OFFSET);
    let version = mmio_read32(handle, VIRTIO_MMIO_VERSION_OFFSET);
    let device_id = mmio_read32(handle, VIRTIO_MMIO_DEVICE_ID_OFFSET);
    if magic != VIRTIO_MAGIC
        || version < 1
        || device_id == 0
        || device_id != expected_device_id
        || queue_size == 0
    {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_prepare_denied = counters.virtio_gate_prepare_denied.saturating_add(1);
        return 0;
    }

    let Some(dma) = dma_for(handle, dma_handle) else {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_prepare_denied = counters.virtio_gate_prepare_denied.saturating_add(1);
        return 0;
    };
    let required_dma_bytes = (queue_index.saturating_add(1)).saturating_mul(4096);
    if dma.phys == 0 || dma.len < required_dma_bytes {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_prepare_denied = counters.virtio_gate_prepare_denied.saturating_add(1);
        return 0;
    }

    if queue_index > VIRTIO_MAX_SAFE_GATE_QUEUE_INDEX {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_prepare_denied = counters.virtio_gate_prepare_denied.saturating_add(1);
        return 0;
    }
    let _ = mmio_write32(handle, VIRTIO_MMIO_QUEUE_SEL_OFFSET, queue_index);
    let queue_num_max = mmio_read32(handle, VIRTIO_MMIO_QUEUE_NUM_MAX_OFFSET);
    let _ = mmio_write32(handle, VIRTIO_MMIO_QUEUE_SEL_OFFSET, 0);
    if queue_num_max == 0 || queue_size > queue_num_max {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_prepare_denied = counters.virtio_gate_prepare_denied.saturating_add(1);
        return 0;
    }

    let mut gates = VIRTIO_LIVE_GATES.lock();
    if gates.len() >= MAX_VIRTIO_LIVE_GATES {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_prepare_denied = counters.virtio_gate_prepare_denied.saturating_add(1);
        return 0;
    }
    gates.retain(|gate| !(gate.owner_handle == handle && gate.queue_index == queue_index));
    let gate = VIRTIO_GATE_HANDLE_BASE | ((gates.len() as u32 + 1) & 0xFFFF);
    gates.push(VirtioLiveGate {
        gate,
        owner_handle: handle,
        driver_id: grant.driver_id.clone(),
        match_key: grant.match_key.clone(),
        device_id,
        queue_index,
        queue_size,
        dma_handle,
        dma_phys: dma.phys,
        irq_line: grant.irq_line,
        prepared_tick: crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed),
        validation_flags: 0,
        negotiated_features: 0,
        notify_count: 0,
        irq_complete_count: 0,
        committed: false,
    });
    let mut counters = COUNTERS.lock();
    counters.virtio_gate_prepare_ok = counters.virtio_gate_prepare_ok.saturating_add(1);
    gate
}

fn validate_virtio_gate<F>(gate: u32, mut update: F) -> u32
where
    F: FnMut(&mut VirtioLiveGate) -> bool,
{
    let mut gates = VIRTIO_LIVE_GATES.lock();
    let Some(entry) = gates.iter_mut().find(|entry| entry.gate == gate) else {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_validation_denied =
            counters.virtio_gate_validation_denied.saturating_add(1);
        return 0;
    };
    if entry.committed || !update(entry) {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_validation_denied =
            counters.virtio_gate_validation_denied.saturating_add(1);
        return 0;
    }
    let mut counters = COUNTERS.lock();
    counters.virtio_gate_validation_ok = counters.virtio_gate_validation_ok.saturating_add(1);
    1
}

pub fn virtio_record_feature_negotiation(
    gate: u32,
    device_features: u32,
    driver_features: u32,
    status: u32,
) -> u32 {
    validate_virtio_gate(gate, |entry| {
        if driver_features & !device_features != 0 || status & 0x08 == 0 || status & 0x80 != 0 {
            return false;
        }
        entry.negotiated_features = driver_features;
        entry.validation_flags |= VIRTIO_GATE_FEATURES_OK;
        true
    })
}

pub fn virtio_record_queue_programming(
    gate: u32,
    queue_index: u32,
    queue_size: u32,
    desc_phys: u32,
    driver_phys: u32,
    device_phys: u32,
) -> u32 {
    validate_virtio_gate(gate, |entry| {
        if queue_index != entry.queue_index
            || queue_size == 0
            || queue_size > entry.queue_size
            || desc_phys < entry.dma_phys
            || driver_phys < entry.dma_phys
            || device_phys < entry.dma_phys
        {
            return false;
        }
        entry.validation_flags |= VIRTIO_GATE_QUEUE_OK;
        true
    })
}

pub fn virtio_record_queue_notify(gate: u32, queue_index: u32) -> u32 {
    validate_virtio_gate(gate, |entry| {
        if queue_index != entry.queue_index {
            return false;
        }
        entry.notify_count = entry.notify_count.saturating_add(1);
        entry.validation_flags |= VIRTIO_GATE_NOTIFY_OK;
        true
    })
}

pub fn virtio_record_irq_completion(gate: u32, irq_status: u32, ack_mask: u32) -> u32 {
    validate_virtio_gate(gate, |entry| {
        if irq_status == 0 || ack_mask == 0 {
            return false;
        }
        entry.irq_complete_count = entry.irq_complete_count.saturating_add(1);
        entry.validation_flags |= VIRTIO_GATE_IRQ_OK;
        true
    })
}

pub fn virtio_commit_live_gate(gate: u32) -> u32 {
    let mut gates = VIRTIO_LIVE_GATES.lock();
    let Some(entry) = gates.iter_mut().find(|entry| entry.gate == gate) else {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_commit_denied = counters.virtio_gate_commit_denied.saturating_add(1);
        return 0;
    };
    if entry.validation_flags & VIRTIO_GATE_REQUIRED != VIRTIO_GATE_REQUIRED {
        let mut counters = COUNTERS.lock();
        counters.virtio_gate_commit_denied = counters.virtio_gate_commit_denied.saturating_add(1);
        return 0;
    }
    entry.committed = true;
    let mut counters = COUNTERS.lock();
    counters.virtio_gate_commit_ok = counters.virtio_gate_commit_ok.saturating_add(1);
    1
}

pub fn virtio_rollback_live_gate(gate: u32) -> u32 {
    let mut gates = VIRTIO_LIVE_GATES.lock();
    let Some(index) = gates.iter().position(|entry| entry.gate == gate) else {
        return 0;
    };
    gates.remove(index);
    let mut counters = COUNTERS.lock();
    counters.virtio_gate_rollback_ok = counters.virtio_gate_rollback_ok.saturating_add(1);
    1
}

pub fn status_block() -> String {
    let grants = GRANTS.lock();
    let dmas = DMA_GRANTS.lock();
    let gates = VIRTIO_LIVE_GATES.lock();
    let counters = COUNTERS.lock().clone();
    let mut out = format!(
        "[Driver Host ABI] devices={} grants={} dma_regions={} virtio_gates={} claim_attempts={} claim_ok={} claim_denied={} release_ok={} release_denied={} lease_expired={} virtio_gate_prepare_ok={} virtio_gate_prepare_denied={} virtio_gate_validation_ok={} virtio_gate_validation_denied={} virtio_gate_commit_ok={} virtio_gate_commit_denied={} virtio_gate_rollback_ok={}\n",
        DEVICES.lock().len(),
        grants.len(),
        dmas.len(),
        gates.len(),
        counters.claim_attempts,
        counters.claim_ok,
        counters.claim_denied,
        counters.release_ok,
        counters.release_denied,
        counters.lease_expired,
        counters.virtio_gate_prepare_ok,
        counters.virtio_gate_prepare_denied,
        counters.virtio_gate_validation_ok,
        counters.virtio_gate_validation_denied,
        counters.virtio_gate_commit_ok,
        counters.virtio_gate_commit_denied,
        counters.virtio_gate_rollback_ok
    );
    for grant in grants.iter().take(20) {
        out.push_str(&format!(
            "- handle={:#010x} caps={:#04x} key={} driver={} pci_config={} mmio={:#x}/{} pio={:#x}/{} irq={} dma={} irq_polls={} last_heartbeat={}\n",
            grant.handle,
            grant.caps,
            grant.match_key,
            grant.driver_id,
            grant.pci_config_available,
            grant.mmio_base,
            grant.mmio_len,
            grant.pio_base,
            grant.pio_len,
            grant.irq_line,
            grant.dma_bytes,
            grant.irq_poll_count,
            grant.last_heartbeat_tick
        ));
    }
    for gate in gates.iter().take(8) {
        out.push_str(&format!(
            "- virtio_gate={:#010x} owner={:#010x} key={} driver={} device_id={} queue={} queue_size={} dma={:#010x}/{:#010x} irq={} flags={:#04x} features={:#010x} notify_count={} irq_complete_count={} committed={} prepared_tick={}\n",
            gate.gate,
            gate.owner_handle,
            gate.match_key,
            gate.driver_id,
            gate.device_id,
            gate.queue_index,
            gate.queue_size,
            gate.dma_handle,
            gate.dma_phys,
            gate.irq_line,
            gate.validation_flags,
            gate.negotiated_features,
            gate.notify_count,
            gate.irq_complete_count,
            gate.committed,
            gate.prepared_tick
        ));
    }
    out
}
