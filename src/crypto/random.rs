// src/crypto/random.rs
// Random generator using RDRAND with a TSC fallback.

/// Generate a 64-bit random value with the RDRAND instruction if supported.
fn rdrand64() -> Option<u64> {
    let cpuid = core::arch::x86_64::__cpuid(1);
    let has_rdrand = (cpuid.ecx & (1 << 30)) != 0;
    if !has_rdrand { return None; }

    let val: u64;
    let success: u8;
    unsafe {
        core::arch::asm!(
            "rdrand {val}",
            "setc {success}",
            val = out(reg) val,
            success = out(reg_byte) success,
        );
    }
    if success != 0 { Some(val) } else { None }
}

/// Fallback entropy source based on the TSC (Time Stamp Counter).
fn tsc() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe {
        core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi);
    }
    ((hi as u64) << 32) | (lo as u64)
}

use core::sync::atomic::{AtomicU64, Ordering};

/// Simple xorshift64 PRNG state.
static PRNG_STATE: AtomicU64 = AtomicU64::new(0);

fn xorshift64() -> u64 {
    let mut state = PRNG_STATE.load(Ordering::Relaxed);
    if state == 0 { state = tsc() ^ 0xdeadbeef12345678; }
    let mut x = state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    PRNG_STATE.store(x, Ordering::Relaxed);
    x
}

/// Fill a buffer with random bytes, used by TLS for values such as `client_random`.
pub fn fill_random(buf: &mut [u8]) {
    let mut offset = 0;
    while offset < buf.len() {
        let val = if let Some(r) = rdrand64() {
            r
        } else {
            xorshift64()
        };
        let bytes = val.to_le_bytes();
        let to_copy = core::cmp::min(8, buf.len() - offset);
        buf[offset..offset + to_copy].copy_from_slice(&bytes[..to_copy]);
        offset += to_copy;
    }
}

/// Generate a 32-byte random array.
pub fn random_bytes_32() -> [u8; 32] {
    let mut buf = [0u8; 32];
    fill_random(&mut buf);
    buf
}
