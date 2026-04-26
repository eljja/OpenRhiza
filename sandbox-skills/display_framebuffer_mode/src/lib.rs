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
    fn os_set_display_overlay_line(slot: u32, ptr: *const u8, len: u32);
    fn os_gui_select_session(session: u32);
    fn os_gui_focus_object(target: u32);
    fn os_gui_set_object_label(handle: u32, ptr: *const u8, len: u32);
    fn os_gui_set_object_style(handle: u32, style: u32);
}

static INIT_MSG: &[u8] =
    b"[Skill] display_framebuffer_mode initialized. Ready to negotiate a 1920x1080 framebuffer-backed console target with rollback to the text shell.\n";
static RUN_MSG: &[u8] =
    b"[Skill] display_framebuffer_mode: request a 1920x1080 framebuffer-console session, validate the display path in sandbox, and keep text-shell recovery visible.\n";
static TITLE: &[u8] = b"OpenRhiza Framebuffer Console Session";
static SUBTITLE: &[u8] =
    b"Sandbox validation is actively driving a 1920x1080 framebuffer-text session with rollback intact.";
static PANEL_TITLE: &[u8] = b"Framebuffer console presenter";
static PANEL_SUBTITLE: &[u8] =
    b"Display skill owns the larger console surface while recovery text mode stays available underneath.";
static FOOTER_PRIMARY: &[u8] = b"Validation state: testing framebuffer-backed 240x67 console";
static FOOTER_SECONDARY: &[u8] = b"Rollback path: live VGA text shell remains preserved";
static GUI_FOOTER_PRIMARY: &[u8] = b"Framebuffer console validation in progress.";
static GUI_FOOTER_SECONDARY: &[u8] = b"Wide console scene is active while the text-shell rollback path stays preserved.";

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
        os_set_display_validation_state(2);
        os_set_display_overlay_line(0, TITLE.as_ptr(), TITLE.len() as u32);
        os_set_display_overlay_line(1, SUBTITLE.as_ptr(), SUBTITLE.len() as u32);
        os_set_display_overlay_line(2, PANEL_TITLE.as_ptr(), PANEL_TITLE.len() as u32);
        os_set_display_overlay_line(3, PANEL_SUBTITLE.as_ptr(), PANEL_SUBTITLE.len() as u32);
        os_set_display_overlay_line(4, FOOTER_PRIMARY.as_ptr(), FOOTER_PRIMARY.len() as u32);
        os_set_display_overlay_line(5, FOOTER_SECONDARY.as_ptr(), FOOTER_SECONDARY.len() as u32);
        os_request_display_mode(1, 240, 67, 1920, 1080);
        os_gui_select_session(3);
        os_gui_focus_object(1);
        os_gui_set_object_style(20, 5);
        os_gui_set_object_label(40, GUI_FOOTER_PRIMARY.as_ptr(), GUI_FOOTER_PRIMARY.len() as u32);
        os_gui_set_object_label(41, GUI_FOOTER_SECONDARY.as_ptr(), GUI_FOOTER_SECONDARY.len() as u32);
        os_log(RUN_MSG.as_ptr(), RUN_MSG.len() as u32);
    }
    1
}
