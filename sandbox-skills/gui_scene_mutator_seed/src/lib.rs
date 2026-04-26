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
    b"[Skill] gui_scene_mutator initialized. Ready to reshape the sandbox GUI through object-scoped mutations.\n";
static RUN_MSG: &[u8] =
    b"[Skill] gui_scene_mutator: resizing the conversation/composer/footer objects and asserting object-local focus without touching unrelated GUI objects.\n";
static FOOTER_PRIMARY: &[u8] = b"Mutator skill applied: object scene updated.";
static FOOTER_SECONDARY: &[u8] =
    b"Conversation, composer, and footer bounds were adjusted by a sandbox skill.";

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

        // conversation surface
        os_gui_set_object_style(20, 5);
        os_gui_set_object_interaction(20, 3);
        os_gui_set_object_bounds(20, 284, 92, 1612, 760);

        // composer container
        os_gui_set_object_style(30, 8);
        os_gui_set_object_interaction(30, 2);
        os_gui_set_object_bounds(30, 304, 862, 1592, 132);

        // text input child
        os_gui_set_object_bounds(31, 316, 908, 1568, 78);
        os_gui_set_object_interaction(31, 2);

        // footer
        os_gui_set_object_bounds(40, 304, 1010, 1592, 44);
        os_gui_set_object_label(40, FOOTER_PRIMARY.as_ptr(), FOOTER_PRIMARY.len() as u32);
        os_gui_set_object_bounds(41, 316, 1034, 1568, 16);
        os_gui_set_object_label(41, FOOTER_SECONDARY.as_ptr(), FOOTER_SECONDARY.len() as u32);

        os_log(RUN_MSG.as_ptr(), RUN_MSG.len() as u32);
    }
    1
}
