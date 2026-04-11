use core::arch::x86_64::__cpuid;
use alloc::vec::Vec;
use x86_64::instructions::port::Port;
use bootloader::bootinfo::{BootInfo, MemoryRegionType};

#[derive(Debug)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
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
        // 1. CPUID 스캔 (실제 코어 수 확인)
        let cores = Self::get_cpu_count();
        
        // 2. 초기 메모리와 PCI 버스 스캔
        let mut mem: usize = 0;
        for region in boot_info.memory_map.iter() {
            if region.region_type == MemoryRegionType::Usable {
                let start = region.range.start_addr();
                let end = region.range.end_addr();
                mem += (end - start) as usize;
                
                unsafe {
                    // Find a contiguous block of at least 4MB above the 4MB physical mark
                    if DMA_BASE == 0 && (end - start) >= 0x400000 && start >= 0x400000 {
                        DMA_BASE = start as u32;
                    }
                }
            }
        }
        
        let storage = false;
        let pci_devices = Self::enumerate_pci();
        
        SystemIdentity {
            cpu_cores: cores,
            total_memory: mem,
            storage_detected: storage,
            pci_devices,
        }
    }

    fn get_cpu_count() -> u32 {
        // CPUID EAX=1 기능 호출
        let result = __cpuid(1);
        // EBX 레지스터의 [23:16] 비트에 논리 프로세서(Logical Processor) 개수가 담겨 있음
        ((result.ebx >> 16) & 0xFF) as u32
    }

    // --- PCI 버스 스캔(Enumeration) 로직 ---
    fn enumerate_pci() -> Vec<PciDevice> {
        let mut devices = Vec::new();
        // 버스(Bus) 0~255, 디바이스(Device/Slot) 0~31 순회
        for bus in 0..=255 {
            for device in 0..=31 {
                let vendor_id = Self::pci_read_word(bus, device, 0, 0);
                // Vendor ID가 0xFFFF면 장치가 없는 빈 슬롯입니다.
                if vendor_id != 0xFFFF {
                    let device_id = Self::pci_read_word(bus, device, 0, 2);
                    let bar0 = Self::pci_read_dword(bus, device, 0, 0x10); // BAR0는 오프셋 0x10에 위치함
                    
                    let class_info = Self::pci_read_dword(bus, device, 0, 0x08);
                    let prog_if = ((class_info >> 8) & 0xFF) as u8;
                    let subclass = ((class_info >> 16) & 0xFF) as u8;
                    let class_code = ((class_info >> 24) & 0xFF) as u8;
                    
                    if class_code == 0x0C && subclass == 0x03 && prog_if == 0x30 {
                        crate::serial_println!("xHCI BAR: {:#010X}", bar0);
                    }
                    
                    devices.push(PciDevice { bus, device, vendor_id, device_id, bar0, class_code, subclass, prog_if });
                }
            }
        }
        devices
    }

    // PCI Configuration Space(0xCF8, 0xCFC)에서 16비트(Word) 데이터를 읽어오는 원시 함수
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

    // PCI Configuration Space에서 32비트(Double Word) 데이터를 읽어오는 원시 함수 (BAR 등 메모리 주소 읽기용)
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