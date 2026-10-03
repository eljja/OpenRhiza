// nic_drivers/nvme.rs
use core::ptr::{read_volatile, write_volatile};

pub struct NvmeController {
    mmio_base: u64,
}

impl NvmeController {
    pub fn init(mmio_base: u64) -> Option<Self> {
        let mut timeout = 100_000_000;
        unsafe {
            let mut cc = read_volatile((mmio_base + 0x14) as *const u32);
            cc &= !0x01;
            write_volatile((mmio_base + 0x14) as *mut u32, cc);
            
            while (read_volatile((mmio_base + 0x1C) as *const u32) & 0x01) != 0 && timeout > 0 {
                // spin loop wait
                timeout -= 1;
            }

            write_volatile((mmio_base + 0x24) as *mut u32, 0x000F_000F);
            write_volatile((mmio_base + 0x28) as *mut u64, 0x300000); 
            write_volatile((mmio_base + 0x30) as *mut u64, 0x310000); 

            cc |= 0x01; 
            cc |= (6 << 16) | (4 << 20);
            write_volatile((mmio_base + 0x14) as *mut u32, cc);
        }

        Some(NvmeController { mmio_base })
    }
}
