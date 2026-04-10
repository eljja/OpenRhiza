use rand_core::{RngCore, CryptoRng, impls};

pub struct HardwareRng;

impl HardwareRng {
    pub const fn new() -> Self {
        HardwareRng
    }
}

impl CryptoRng for HardwareRng {}

impl RngCore for HardwareRng {
    fn next_u32(&mut self) -> u32 {
        let mut val: u32 = 0;
        unsafe {
            // Loop until rdrand succeeds
            while core::arch::x86_64::_rdrand32_step(&mut val) == 0 {
                core::arch::x86_64::_mm_pause();
            }
        }
        val
    }

    fn next_u64(&mut self) -> u64 {
        let mut val: u64 = 0;
        unsafe {
            // Loop until rdrand succeeds
            while core::arch::x86_64::_rdrand64_step(&mut val) == 0 {
                core::arch::x86_64::_mm_pause();
            }
        }
        val
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        impls::fill_bytes_via_next(self, dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
