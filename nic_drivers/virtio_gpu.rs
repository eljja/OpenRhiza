// nic_drivers/virtio_gpu.rs
use core::ptr::{read_volatile, write_volatile};

pub struct VirtioGpu {
    mmio_base: u64,
}

impl VirtioGpu {
    pub fn init(mmio_base: u64) -> Option<Self> {
        let io_base = mmio_base as *mut u8;
        unsafe {
            write_volatile(io_base.add(0x12), 0);
            write_volatile(io_base.add(0x12), 1 | 2 | 8 | 4);
        }
        Some(VirtioGpu { mmio_base })
    }
}
