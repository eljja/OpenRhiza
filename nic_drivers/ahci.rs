// nic_drivers/ahci.rs
use core::ptr::{read_volatile, write_volatile};

pub struct AhciController {
    mmio_base: u64,
}

impl AhciController {
    pub fn init(mmio_base: u64) -> Option<Self> {
        unsafe {
            let mut ghc = read_volatile((mmio_base + 0x04) as *const u32);
            ghc |= 1 << 31;
            write_volatile((mmio_base + 0x04) as *mut u32, ghc);

            let pi = read_volatile((mmio_base + 0x0C) as *const u32);
            for i in 0..32 {
                if (pi & (1 << i)) != 0 {
                    let port_base = mmio_base + 0x100 + (i as u64 * 0x80);
                    let _sig = read_volatile((port_base + 0x24) as *const u32);
                }
            }
        }
        Some(AhciController { mmio_base })
    }
}
