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
    fn os_gui_get_object_bounds(handle: u32, component: u32) -> u32;
    fn os_gui_claim_shell_policy(owner_ptr: *const u8, owner_len: u32) -> u32;
    fn os_gui_shell_policy_heartbeat(owner_ptr: *const u8, owner_len: u32) -> u32;
}

static INIT_MSG: &[u8] =
    b"[Skill] gui_modern_shell_seed initialized. This GUI is sandbox-owned; the core only provides scene/object host calls.\n";
static RUN_MSG: &[u8] =
    b"[Skill] gui_modern_shell_seed: applied OpenRhiza Workbench layout through object-scoped sandbox mutations.\n";
static SIDEBAR_OPENRHIZA: &[u8] = b"Workspace";
static SIDEBAR_SKILLS: &[u8] = b"Skills";
static SIDEBAR_CONSOLE: &[u8] = b"Console";
static SIDEBAR_RECOVERY: &[u8] = b"Recovery";
static SIDEBAR_STATUS_A: &[u8] = b"Core: survival only";
static SIDEBAR_STATUS_B: &[u8] = b"Shell: sandbox skill";
static CHIP_SKILLS: &[u8] = b"Skills online";
static CHIP_SANDBOX: &[u8] = b"Sandbox first";
static CHIP_EVIDENCE: &[u8] = b"Evidence trail";
static CONVERSATION_LABEL: &[u8] = b"OpenRhiza Workbench";
static COMPOSER_LABEL: &[u8] = b"Ask OpenRhiza";
static FOOTER_PRIMARY: &[u8] = b"OpenRhiza Workbench | console + skills + sandbox evidence";
static FOOTER_SECONDARY: &[u8] =
    b"Flow: prompt -> plan -> skill -> sandbox -> evidence. Core stays minimal.";
static POLICY_OWNER: &[u8] = b"skill_gui_modern_shell_v1";
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
        let _ = os_gui_claim_shell_policy(POLICY_OWNER.as_ptr(), POLICY_OWNER.len() as u32);
        os_gui_select_session(1);
        os_gui_focus_object(2);

        let root_y = os_gui_get_object_bounds(1, 1);
        let root_w = os_gui_get_object_bounds(1, 2);
        let root_h = os_gui_get_object_bounds(1, 3);
        let sidebar_x: u32 = 0;
        let sidebar_y = root_y + 24;
        let sidebar_w: u32 = 280;
        let sidebar_h = root_h.saturating_sub(24);
        let list_x = sidebar_x + 20;
        let list_y = sidebar_y + 72;
        let item_w = sidebar_w.saturating_sub(40);
        let item_h: u32 = 40;
        let item_gap: u32 = 10;
        let sidebar_status_y = root_y + root_h.saturating_sub(96);

        let main_x = sidebar_w + 24;
        let main_w = root_w.saturating_sub(main_x + 28);
        let chip_w: u32 = 154;
        let chip_h: u32 = 28;
        let chip_gap: u32 = 10;
        let chip_y = root_y + 24;
        let chip_3_x = main_x + main_w.saturating_sub(chip_w);
        let chip_2_x = chip_3_x.saturating_sub(chip_w + chip_gap);
        let chip_1_x = chip_2_x.saturating_sub(chip_w + chip_gap);
        let conversation_y = root_y + 56;
        let footer_h = 42;
        let footer_y = root_y + root_h.saturating_sub(footer_h + 20);
        let composer_h = 128;
        let composer_y = footer_y.saturating_sub(composer_h + 18);
        let conversation_h = composer_y.saturating_sub(conversation_y + 18);

        // Keep the core generic: mutate already-known object handles only.
        os_gui_set_object_bounds(2, sidebar_x, sidebar_y, sidebar_w, sidebar_h);
        os_gui_set_object_bounds(3, list_x, list_y, item_w, (item_h + item_gap) * 4);

        os_gui_set_object_label(
            10,
            SIDEBAR_OPENRHIZA.as_ptr(),
            SIDEBAR_OPENRHIZA.len() as u32,
        );
        os_gui_set_object_label(11, SIDEBAR_SKILLS.as_ptr(), SIDEBAR_SKILLS.len() as u32);
        os_gui_set_object_label(13, SIDEBAR_CONSOLE.as_ptr(), SIDEBAR_CONSOLE.len() as u32);
        os_gui_set_object_label(14, SIDEBAR_RECOVERY.as_ptr(), SIDEBAR_RECOVERY.len() as u32);
        os_gui_set_object_bounds(10, list_x, list_y, item_w, item_h);
        os_gui_set_object_bounds(11, list_x, list_y + item_h + item_gap, item_w, item_h);
        os_gui_set_object_bounds(13, list_x, list_y + (item_h + item_gap) * 2, item_w, item_h);
        os_gui_set_object_bounds(14, list_x, list_y + (item_h + item_gap) * 3, item_w, item_h);
        os_gui_set_object_style(10, 4);
        os_gui_set_object_style(11, 2);
        os_gui_set_object_style(13, 2);
        os_gui_set_object_style(14, 2);

        os_gui_set_object_bounds(120, chip_1_x, chip_y, chip_w, chip_h);
        os_gui_set_object_label(120, CHIP_SKILLS.as_ptr(), CHIP_SKILLS.len() as u32);
        os_gui_set_object_style(120, 2);
        os_gui_set_object_interaction(120, 1);
        os_gui_set_object_bounds(121, chip_2_x, chip_y, chip_w, chip_h);
        os_gui_set_object_label(121, CHIP_SANDBOX.as_ptr(), CHIP_SANDBOX.len() as u32);
        os_gui_set_object_style(121, 2);
        os_gui_set_object_interaction(121, 1);
        os_gui_set_object_bounds(122, chip_3_x, chip_y, chip_w, chip_h);
        os_gui_set_object_label(122, CHIP_EVIDENCE.as_ptr(), CHIP_EVIDENCE.len() as u32);
        os_gui_set_object_style(122, 4);
        os_gui_set_object_interaction(122, 3);

        os_gui_set_object_bounds(130, list_x, sidebar_status_y, item_w, 18);
        os_gui_set_object_label(130, SIDEBAR_STATUS_A.as_ptr(), SIDEBAR_STATUS_A.len() as u32);
        os_gui_set_object_style(130, 10);
        os_gui_set_object_bounds(131, list_x, sidebar_status_y + 24, item_w, 18);
        os_gui_set_object_label(131, SIDEBAR_STATUS_B.as_ptr(), SIDEBAR_STATUS_B.len() as u32);
        os_gui_set_object_style(131, 11);

        os_gui_set_object_bounds(20, main_x, conversation_y, main_w, conversation_h);
        os_gui_set_object_style(20, 5);
        os_gui_set_object_interaction(20, 3);
        os_gui_set_object_label(
            20,
            CONVERSATION_LABEL.as_ptr(),
            CONVERSATION_LABEL.len() as u32,
        );

        os_gui_set_object_bounds(30, main_x, composer_y, main_w, composer_h);
        os_gui_set_object_style(30, 8);
        os_gui_set_object_interaction(30, 2);
        os_gui_set_object_label(30, COMPOSER_LABEL.as_ptr(), COMPOSER_LABEL.len() as u32);

        os_gui_set_object_bounds(
            31,
            main_x + 16,
            composer_y + 38,
            main_w.saturating_sub(32),
            76,
        );
        os_gui_set_object_interaction(31, 2);

        os_gui_set_object_bounds(40, main_x, footer_y, main_w, footer_h);
        os_gui_set_object_style(40, 9);
        os_gui_set_object_label(40, FOOTER_PRIMARY.as_ptr(), FOOTER_PRIMARY.len() as u32);
        os_gui_set_object_bounds(
            41,
            main_x + 16,
            footer_y + 24,
            main_w.saturating_sub(32),
            18,
        );
        os_gui_set_object_style(41, 11);
        os_gui_set_object_label(41, FOOTER_SECONDARY.as_ptr(), FOOTER_SECONDARY.len() as u32);

        os_log(RUN_MSG.as_ptr(), RUN_MSG.len() as u32);
    }
    1
}

#[no_mangle]
pub extern "C" fn poll_net() {
    unsafe {
        let _ = os_gui_shell_policy_heartbeat(POLICY_OWNER.as_ptr(), POLICY_OWNER.len() as u32);
    }
}
