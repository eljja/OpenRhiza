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
    fn os_set_gui_session_state(state: u32);
    fn os_set_display_session_target(target: u32);
    fn os_set_display_validation_state(state: u32);
    fn os_set_display_overlay_line(slot: u32, ptr: *const u8, len: u32);
    fn os_gui_select_session(session: u32);
    fn os_gui_focus_object(target: u32);
    fn os_gui_set_object_label(handle: u32, ptr: *const u8, len: u32);
    fn os_gui_set_object_style(handle: u32, style: u32);
}

static INIT_MSG: &[u8] =
    b"[Skill] gui_compositor_seed initialized. Ready to assemble a sandbox GUI compositor stack for a 1920x1080 session.\n";
static RUN_MSG: &[u8] =
    b"[Skill] gui_compositor_seed: mark the 1920x1080 GUI session ready, attach display/input policies, and preserve rollback to the text shell if validation fails.\n";
static TITLE: &[u8] = b"OpenRhiza Sandbox GUI Session";
static SUBTITLE: &[u8] =
    b"Recovery shell remains available while sandbox display sessions are validated.";
static PANEL_TITLE: &[u8] = b"GUI compositor seed";
static PANEL_SUBTITLE: &[u8] =
    b"Sandbox compositor session is live. The next skill can replace this bootstrap layer with a richer desktop.";
static FOOTER_PRIMARY: &[u8] = b"Validation: ready";
static FOOTER_SECONDARY: &[u8] = b"Session: compositor seed attached, rollback preserved";
static EMPTY: &[u8] = b"";
static GUI_FOOTER_PRIMARY: &[u8] = b"OpenRhiza AI session is live.";
static GUI_FOOTER_SECONDARY: &[u8] = b"Composer focused. Object-scoped GUI mutations are now sandbox-owned.";

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
        os_set_display_session_target(2);
        os_set_display_validation_state(3);
        os_set_display_overlay_line(0, EMPTY.as_ptr(), 0);
        os_set_display_overlay_line(1, EMPTY.as_ptr(), 0);
        os_set_display_overlay_line(2, EMPTY.as_ptr(), 0);
        os_set_display_overlay_line(3, EMPTY.as_ptr(), 0);
        os_set_display_overlay_line(4, EMPTY.as_ptr(), 0);
        os_set_display_overlay_line(5, EMPTY.as_ptr(), 0);
        os_request_display_mode(2, 240, 67, 1920, 1080);
        os_set_gui_session_state(2);
        os_gui_select_session(1);
        os_gui_focus_object(2);
        os_gui_set_object_style(20, 5);
        os_gui_set_object_style(30, 8);
        os_gui_set_object_label(40, GUI_FOOTER_PRIMARY.as_ptr(), GUI_FOOTER_PRIMARY.len() as u32);
        os_gui_set_object_label(41, GUI_FOOTER_SECONDARY.as_ptr(), GUI_FOOTER_SECONDARY.len() as u32);
        os_log(RUN_MSG.as_ptr(), RUN_MSG.len() as u32);
    }
    1
}
