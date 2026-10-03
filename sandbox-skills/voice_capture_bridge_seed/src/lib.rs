#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
}

static INIT_MSG: &[u8] =
    b"[Skill] voice_capture_bridge initialized. Route policy and audio-LLM bridge are sandbox peers.\n";
static RUN_MSG: &[u8] =
    b"[Skill] voice_capture_bridge: bounded capture -> route policy -> transcript/direct-audio bridge -> confirmation. No raw-audio action without policy.\n";

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
