#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
}

static INIT_MSG: &[u8] =
    b"[Skill] gui_session bootstrap initialized. Ready to coordinate text-shell to GUI handoff through sandbox components.\n";
static RUN_MSG: &[u8] =
    b"[Skill] gui_session bootstrap: discover display skills, load compositor and input policies, keep rollback path to text console alive.\n";

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
