// nic_drivers/intel_hda.rs
use core::ptr::{read_volatile, write_volatile};

pub struct IntelHda {
    mmio_base: u64,
}

impl IntelHda {
    pub fn init(mmio_base: u64) -> Option<Self> {
        let mut timeout = 100_000;
        unsafe {
            let mut gctl = read_volatile((mmio_base + 0x08) as *const u32);
            gctl &= !0x00000001; // CRST
            write_volatile((mmio_base + 0x08) as *mut u32, gctl);

            while (read_volatile((mmio_base + 0x08) as *const u32) & 0x01) != 0 && timeout > 0 { timeout -= 1; }

            gctl |= 0x00000001;
            write_volatile((mmio_base + 0x08) as *mut u32, gctl);
        }
        Some(IntelHda { mmio_base })
    }
}
