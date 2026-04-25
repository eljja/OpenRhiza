#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_request_display_mode(
        backend: u32,
        text_cols: u32,
        text_rows: u32,
        pixel_width: u32,
        pixel_height: u32,
    );
    fn os_set_display_session_target(target: u32);
    fn os_set_display_validation_state(state: u32);
}

static INIT_MSG: &[u8] =
    b"[Skill] display_console bootstrap initialized. Ready to negotiate a sandbox-owned 1920x1080 text console without moving display policy into the core.\n";
static RUN_MSG: &[u8] =
    b"[Skill] display_console bootstrap: request a 1920x1080 wide-console session, keep the recovery shell alive, and continue through registry workflow before promotion.\n";

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
        os_set_display_session_target(1);
        os_set_display_validation_state(1);
        os_request_display_mode(1, 240, 67, 1920, 1080);
        os_log(RUN_MSG.as_ptr(), RUN_MSG.len() as u32);
    }
    1
}
