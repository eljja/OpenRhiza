// src/arch/x86_64/discovery.rs
use core::arch::x86_64::__cpuid;
use alloc::vec::Vec;
use x86_64::instructions::port::Port;

#[derive(Debug)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub bar0: u32,
}

pub struct SystemIdentity {
    pub cpu_cores: u32,
    pub total_memory: usize,
    pub storage_detected: bool,
    pub pci_devices: Vec<PciDevice>,
}

impl SystemIdentity {
    pub fn scan() -> Self {
        // 1. CPUID 스캔 (실제 코어 수 확인)
        let cores = Self::get_cpu_count();
        
        // 2. 초기 메모리와 PCI 버스 스캔
        let mem = 0; 
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
                    devices.push(PciDevice { bus, device, vendor_id, device_id, bar0 });
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