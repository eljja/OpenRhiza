use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lazy_static::lazy_static;
use spin::Mutex;

const MAX_GRANTS: usize = 32;
const MAX_DMA_GRANTS: usize = 64;
const DEFAULT_MMIO_WINDOW_BYTES: u32 = 0x20_000;
const DEFAULT_PIO_WINDOW_BYTES: u16 = 0x20;
const DRIVER_HANDLE_BASE: u32 = 0x4452_0000; // "DR"
const DMA_HANDLE_BASE: u32 = 0x444D_0000; // "DM"

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
}

#[derive(Clone, Debug)]
struct DriverGrant {
    handle: u32,
    match_key: String,
    driver_id: String,
    bus: u8,
    device: u8,
    func: u8,
    mmio_base: u64,
    mmio_len: u32,
    pio_base: u16,
    pio_len: u16,
    dma_bytes: u32,
    irq_poll_count: u32,
}

#[derive(Clone, Debug)]
struct DmaGrant {
    handle: u32,
    owner: u32,
    phys: u32,
    len: u32,
}

lazy_static! {
    static ref DEVICES: Mutex<Vec<DeviceDescriptor>> = Mutex::new(Vec::new());
    static ref GRANTS: Mutex<Vec<DriverGrant>> = Mutex::new(Vec::new());
    static ref DMA_GRANTS: Mutex<Vec<DmaGrant>> = Mutex::new(Vec::new());
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
        });
    }
}

fn find_device(match_key: &str) -> Option<DeviceDescriptor> {
    let key = match_key.trim();
    DEVICES
        .lock()
        .iter()
        .find(|device| device.match_key == key || device.class_key == key)
        .cloned()
}

pub fn claim_device(match_key: &str, driver_id: &str) -> u32 {
    let Some(device) = find_device(match_key) else {
        return 0;
    };

    let mut grants = GRANTS.lock();
    if let Some(existing) = grants
        .iter_mut()
        .find(|grant| grant.match_key == match_key && grant.driver_id == driver_id)
    {
        return existing.handle;
    }
    if grants.len() >= MAX_GRANTS {
        return 0;
    }

    let handle = DRIVER_HANDLE_BASE | ((grants.len() as u32 + 1) & 0xFFFF);
    let bar0_is_io = (device.bar0 & 0x1) != 0;
    let legacy_ide_pio = device.class_code == 0x01 && device.subclass == 0x01;
    let grant = DriverGrant {
        handle,
        match_key: String::from(match_key.trim()),
        driver_id: String::from(driver_id.trim()),
        bus: device.bus,
        device: device.device,
        func: device.func,
        mmio_base: if !bar0_is_io && device.bar0 != 0 {
            (device.bar0 & 0xFFFF_FFF0) as u64
        } else {
            0
        },
        mmio_len: if !bar0_is_io && device.bar0 != 0 {
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
        dma_bytes: 0,
        irq_poll_count: 0,
    };
    grants.push(grant);
    handle
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
    let Some(addr) = mmio_addr(&grant, offset, 4) else {
        return 0;
    };
    unsafe { core::ptr::read_volatile(addr as usize as *const u32) }
}

pub fn mmio_write32(handle: u32, offset: u32, value: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
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
    let Some(port) = pio_port(&grant, offset as u16, 1) else {
        return 0;
    };
    crate::arch::x86_64::port::read_port_u8(port) as u32
}

pub fn pio_read16(handle: u32, offset: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    let Some(port) = pio_port(&grant, offset as u16, 2) else {
        return 0;
    };
    crate::arch::x86_64::port::read_port_u16(port) as u32
}

pub fn pio_read32(handle: u32, offset: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
    let Some(port) = pio_port(&grant, offset as u16, 4) else {
        return 0;
    };
    crate::arch::x86_64::port::read_port_u32(port)
}

pub fn pio_write8(handle: u32, offset: u32, value: u32) -> u32 {
    let Some(grant) = grant_for(handle) else {
        return 0;
    };
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
    if offset > 0xFC {
        return 0;
    }
    let address = pci_config_address(grant.bus, grant.device, grant.func, offset as u8);
    crate::arch::x86_64::port::write_port_u32(0xCF8, address);
    crate::arch::x86_64::port::write_port_u32(0xCFC, value);
    1
}

pub fn dma_alloc(handle: u32, byte_len: u32, align: u32) -> u32 {
    if grant_for(handle).is_none() {
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

        if let Some(grant) = GRANTS.lock().iter_mut().find(|grant| grant.handle == handle) {
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
    dma_for(handle, dma_handle).map(|dma| dma.phys).unwrap_or(0)
}

pub fn dma_len(handle: u32, dma_handle: u32) -> u32 {
    dma_for(handle, dma_handle).map(|dma| dma.len).unwrap_or(0)
}

pub fn dma_write(handle: u32, dma_handle: u32, offset: u32, bytes: &[u8]) -> u32 {
    let Some(dma) = dma_for(handle, dma_handle) else {
        return 0;
    };
    let Some(end) = offset.checked_add(bytes.len() as u32) else {
        return 0;
    };
    if end > dma.len {
        return 0;
    }
    unsafe {
        let virt = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + dma.phys as u64 + offset as u64;
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), virt as usize as *mut u8, bytes.len());
    }
    bytes.len() as u32
}

pub fn dma_read(handle: u32, dma_handle: u32, offset: u32, out: &mut [u8]) -> u32 {
    let Some(dma) = dma_for(handle, dma_handle) else {
        return 0;
    };
    let Some(end) = offset.checked_add(out.len() as u32) else {
        return 0;
    };
    if end > dma.len {
        return 0;
    }
    unsafe {
        let virt = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + dma.phys as u64 + offset as u64;
        core::ptr::copy_nonoverlapping(virt as usize as *const u8, out.as_mut_ptr(), out.len());
    }
    out.len() as u32
}

pub fn irq_poll(handle: u32) -> u32 {
    let mut grants = GRANTS.lock();
    let Some(grant) = grants.iter_mut().find(|grant| grant.handle == handle) else {
        return 0;
    };
    grant.irq_poll_count = grant.irq_poll_count.saturating_add(1);
    0
}

pub fn irq_ack(handle: u32, _mask: u32) -> u32 {
    if grant_for(handle).is_some() { 1 } else { 0 }
}

pub fn status_block() -> String {
    let grants = GRANTS.lock();
    let dmas = DMA_GRANTS.lock();
    let mut out = format!(
        "[Driver Host ABI] devices={} grants={} dma_regions={}\n",
        DEVICES.lock().len(),
        grants.len(),
        dmas.len()
    );
    for grant in grants.iter().take(20) {
        out.push_str(&format!(
            "- handle={:#010x} key={} driver={} mmio={:#x}/{} pio={:#x}/{} dma={} irq_polls={}\n",
            grant.handle,
            grant.match_key,
            grant.driver_id,
            grant.mmio_base,
            grant.mmio_len,
            grant.pio_base,
            grant.pio_len,
            grant.dma_bytes,
            grant.irq_poll_count
        ));
    }
    out
}
