#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_gui_select_session(session: u32);
    fn os_gui_focus_object(target: u32);
    fn os_gui_set_object_label(handle: u32, ptr: *const u8, len: u32);
    fn os_gui_set_object_style(handle: u32, style: u32);
    fn os_gui_set_object_bounds(handle: u32, x: u32, y: u32, width: u32, height: u32);
    fn os_gui_set_object_interaction(handle: u32, interaction: u32);
}

static INIT_MSG: &[u8] =
    b"[Skill] gui_modern_shell_seed initialized. This GUI is sandbox-owned; the core only provides scene/object host calls.\n";
static RUN_MSG: &[u8] =
    b"[Skill] gui_modern_shell_seed: applied object-scoped modern shell layout without adding GUI policy to the core.\n";
static FOOTER_PRIMARY: &[u8] = b"Modern shell skill active.";
static FOOTER_SECONDARY: &[u8] =
    b"Core boundary preserved: scene, input, display, rollback only; GUI decisions stay in sandbox skills.";
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
        os_gui_select_session(1);
        os_gui_focus_object(2);

        // Keep the core generic: mutate already-known object handles only.
        os_gui_set_object_style(10, 4);
        os_gui_set_object_style(11, 2);
        os_gui_set_object_style(13, 2);
        os_gui_set_object_style(14, 2);

        os_gui_set_object_bounds(20, 304, 118, 1588, 708);
        os_gui_set_object_style(20, 5);
        os_gui_set_object_interaction(20, 3);

        os_gui_set_object_bounds(30, 304, 842, 1588, 142);
        os_gui_set_object_style(30, 8);
        os_gui_set_object_interaction(30, 2);

        os_gui_set_object_bounds(31, 320, 884, 1552, 88);
        os_gui_set_object_interaction(31, 2);

        os_gui_set_object_bounds(40, 304, 1004, 1588, 52);
        os_gui_set_object_label(40, FOOTER_PRIMARY.as_ptr(), FOOTER_PRIMARY.len() as u32);
        os_gui_set_object_bounds(41, 320, 1028, 1552, 18);
        os_gui_set_object_label(41, FOOTER_SECONDARY.as_ptr(), FOOTER_SECONDARY.len() as u32);

        os_log(RUN_MSG.as_ptr(), RUN_MSG.len() as u32);
    }
    1
}
