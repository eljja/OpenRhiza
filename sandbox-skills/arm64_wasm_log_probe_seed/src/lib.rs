#![no_std]

use core::panic::PanicInfo;

extern "C" {
    fn os_log(ptr: *const u8, len: u32);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        core::arch::wasm32::unreachable();
    }
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn run_skill() -> i32 {
    let msg = b"[Skill][arm64-rust-log] run_skill reached host import\n";
    unsafe { os_log(msg.as_ptr(), msg.len() as u32) };
    1
}
