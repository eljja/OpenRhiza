use core::arch::x86_64::{__cpuid, __cpuid_count};
use alloc::vec::Vec;
use x86_64::instructions::port::Port;
use bootloader::bootinfo::{BootInfo, MemoryRegionType};

#[derive(Debug)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub bar0: u32,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
}

pub struct SystemIdentity {
    pub cpu_cores: u32,
    pub total_memory: usize,
    pub storage_detected: bool,
    pub pci_devices: Vec<PciDevice>,
}

pub static mut DMA_BASE: u32 = 0;
pub static mut DMA_OFFSET: u32 = 0;
pub static mut PHYS_MEM_OFFSET: u64 = 0;

impl SystemIdentity {
    pub fn scan(boot_info: &'static BootInfo) -> Self {
        // 1. Scan CPUID to determine the logical core count.
        let cores = Self::get_cpu_count();
        
        // 2. Scan usable memory regions and enumerate the PCI bus.
        let mut mem: usize = 0;
        for region in boot_info.memory_map.iter() {
            if region.region_type == MemoryRegionType::Usable {
                let start = region.range.start_addr();
                let end = region.range.end_addr();
                mem += (end - start) as usize;
                
                unsafe {
                    // Find a contiguous block of at least 4 MiB above the 4 MiB physical mark.
                    if DMA_BASE == 0 && (end - start) >= 0x400000 && start >= 0x400000 {
                        DMA_BASE = start as u32;
                    }
                }
            }
        }
        
        let storage = Self::check_storage_interface();
        let pci_devices = Self::enumerate_pci();
        
        SystemIdentity {
            cpu_cores: cores,
            total_memory: mem,
            storage_detected: storage,
            pci_devices,
        }
    }

    fn get_cpu_count() -> u32 {
        let max_basic = __cpuid(0).eax;

        if max_basic >= 0x0B {
            for subleaf in 0..=4 {
                let leaf = __cpuid_count(0x0B, subleaf);
                let level_type = (leaf.ecx >> 8) & 0xFF;
                let logical_count = leaf.ebx & 0xFFFF;

                if level_type == 1 && logical_count != 0 {
                    return logical_count;
                }
            }
        }

        let max_extended = __cpuid(0x8000_0000).eax;
        if max_extended >= 0x8000_0008 {
            let extended = __cpuid(0x8000_0008);
            let cores_minus_one = extended.ecx & 0xFF;
            if cores_minus_one != 0 {
                return cores_minus_one + 1;
            }
        }

        let leaf1 = __cpuid(1);
        let logical_count = (leaf1.ebx >> 16) & 0xFF;
        if logical_count != 0 {
            logical_count
        } else {
            1
        }
    }

    fn check_storage_interface() -> bool {
        unsafe {
            let mut status_port = Port::<u8>::new(0x177);
            let status = status_port.read();
            status != 0xFF
        }
    }

    // --- PCI bus enumeration ---
    fn enumerate_pci() -> Vec<PciDevice> {
        let mut devices = Vec::new();
        // Walk buses 0..255, devices/slots 0..31, and all 8 functions.
        for bus in 0..=255 {
            for device in 0..=31 {
                for func in 0..=7 {
                    let vendor_id = Self::pci_read_word(bus, device, func, 0);
                    if vendor_id == 0xFFFF {
                        if func == 0 {
                            break;
                        }
                        continue;
                    }

                    let device_id = Self::pci_read_word(bus, device, func, 2);
                    let bar0 = Self::pci_read_dword(bus, device, func, 0x10);
                    let class_info = Self::pci_read_dword(bus, device, func, 0x08);
                    let prog_if = ((class_info >> 8) & 0xFF) as u8;
                    let subclass = ((class_info >> 16) & 0xFF) as u8;
                    let class_code = ((class_info >> 24) & 0xFF) as u8;

                    if class_code == 0x0C && subclass == 0x03 {
                        crate::serial_println!(
                            "[PCI] USB controller bus {} dev {} func {} prog_if={:#04X} BAR0={:#010X}",
                            bus, device, func, prog_if, bar0
                        );
                    }

                    devices.push(PciDevice {
                        bus,
                        device,
                        func,
                        vendor_id,
                        device_id,
                        bar0,
                        class_code,
                        subclass,
                        prog_if,
                    });
                }
            }
        }
        devices
    }

    // Raw helper to read a 16-bit word from PCI configuration space (0xCF8, 0xCFC).
    fn pci_read_word(bus: u8, device: u8, func: u8, offset: u8) -> u16 {
        let address: u32 = 0x80000000 | ((bus as u32) << 16) | ((device as u32) << 11) | ((func as u32) << 8) | ((offset as u32) & 0xFC);
        
        unsafe {
            let mut addr_port = Port::<u32>::new(0xCF8);
            let mut data_port = Port::<u32>::new(0xCFC);
            addr_port.write(address);
            let data = data_port.read();
            ((data >> ((offset & 2) * 8)) & 0xFFFF) as u16
        }
    }

    // Raw helper to read a 32-bit dword from PCI configuration space (used for BARs).
    fn pci_read_dword(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
        let address: u32 = 0x80000000 | ((bus as u32) << 16) | ((device as u32) << 11) | ((func as u32) << 8) | ((offset as u32) & 0xFC);
        
        unsafe {
            let mut addr_port = Port::<u32>::new(0xCF8);
            let mut data_port = Port::<u32>::new(0xCFC);
            addr_port.write(address);
            data_port.read()
        }
    }
}
