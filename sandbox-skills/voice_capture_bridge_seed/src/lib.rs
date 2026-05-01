#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
}

static INIT_MSG: &[u8] =
    b"[Skill] voice_capture_bridge initialized. Waiting for bounded audio-frame host ABI.\n";
static RUN_MSG: &[u8] =
    b"[Skill] voice_capture_bridge: capture -> VAD -> transcript -> confirmation -> prompt path. No direct action from raw audio.\n";

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn init_driver() {
    unsafe {
        os_log(INIT_MSG.as_ptr(), INIT_MSG.len() as u32);
    }
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    unsafe {
        os_log(RUN_MSG.as_ptr(), RUN_MSG.len() as u32);
    }
    1
}

