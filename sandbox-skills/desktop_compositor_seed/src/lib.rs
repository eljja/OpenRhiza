#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_request_display_mode(backend: u32, cols: u32, rows: u32, px_w: u32, px_h: u32);
    fn os_set_gui_session_state(state: u32);
    fn os_set_display_session_target(target: u32);
    fn os_set_display_validation_state(state: u32);
    fn os_gui_select_session(session: u32);
    fn os_gui_focus_object(target: u32);
    fn os_gui_set_object_bounds(handle: u32, x: u32, y: u32, w: u32, h: u32);
    fn os_gui_set_object_style(handle: u32, style: u32);
    fn os_gui_set_object_label(handle: u32, ptr: *const u8, len: u32);
    fn os_gui_set_object_interaction(handle: u32, interaction: u32);
}

const STYLE_CHROME: u32 = 0;
const STYLE_SIDEBAR: u32 = 1;
const STYLE_PLAIN: u32 = 10;
const STYLE_ACCENT: u32 = 11;
const STYLE_WINDOW: u32 = 12;

const INTERACTION_IDLE: u32 = 0;
const INTERACTION_HOVERED: u32 = 1;
const INTERACTION_FOCUSED: u32 = 2;

const DESKTOP_BG: u32 = 500;
const DOCK: u32 = 790;
const DOCK_LABEL: u32 = 791;
const WINDOW_BASE: u32 = 510;
const WINDOW_STRIDE: u32 = 20;
const MAX_WINDOWS: usize = 4;

const INIT_MSG: &[u8] =
    b"[Desktop Compositor] initialized compact sandbox-owned desktop compositor.\n";
const RUN_MSG: &[u8] =
    b"[Desktop Compositor] rendered desktop background, dock, and four default app windows.\n";
const DEFAULT_APPS_MSG: &[u8] =
    b"[Desktop Apps] selftest rendered Terminal + LLM, System Monitor, Skill Store, and VFS Explorer.\n";
const ACTION_FOCUS_TERMINAL_MSG: &[u8] =
    b"[Desktop Action] focused Terminal + LLM via bounded action route.\n";
const ACTION_FOCUS_MONITOR_MSG: &[u8] =
    b"[Desktop Action] focused System Monitor via bounded action route.\n";
const ACTION_FOCUS_STORE_MSG: &[u8] =
    b"[Desktop Action] focused Skill Store via bounded action route.\n";
const ACTION_FOCUS_VFS_MSG: &[u8] =
    b"[Desktop Action] focused VFS Explorer via bounded action route.\n";
const PROTOCOL_SELFTEST_MSG: &[u8] =
    b"[Desktop Protocol] selftest moved window 2 and focused it.\n";
const PROTOCOL_RESTORE_MSG: &[u8] =
    b"[Desktop Protocol] selftest restored the default four-app layout.\n";
const PROTOCOL_CLOSE_MSG: &[u8] =
    b"[Desktop Protocol] selftest closed window 2 through the window protocol.\n";
const CLOSE_LABEL: &[u8] = b"[x]";
const DOCK_TEXT: &[u8] = b"OpenRhiza Dock | command shell active | recovery ready";
const EMPTY_LABEL: &[u8] = b"";

#[derive(Clone, Copy)]
struct Window {
    id: u32,
    title: &'static [u8],
    line_a: &'static [u8],
    line_b: &'static [u8],
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    closed: bool,
}

impl Window {
    const fn empty() -> Self {
        Self {
            id: 0,
            title: b"",
            line_a: b"",
            line_b: b"",
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            closed: true,
        }
    }
}

static mut WINDOWS: [Window; MAX_WINDOWS] = [Window::empty(); MAX_WINDOWS];
static mut FOCUSED_WINDOW: u32 = 1;
static mut INITIALIZED: bool = false;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn log(bytes: &[u8]) {
    unsafe {
        os_log(bytes.as_ptr(), bytes.len() as u32);
    }
}

unsafe fn init_windows_once() {
    if INITIALIZED {
        return;
    }
    WINDOWS[0] = Window {
        id: 1,
        title: b"Terminal + LLM",
        line_a: b"Plain prompts route to the LLM path.",
        line_b: b"Slash commands stay local and recovery-safe.",
        x: 96,
        y: 96,
        w: 620,
        h: 360,
        closed: false,
    };
    WINDOWS[1] = Window {
        id: 2,
        title: b"System Monitor",
        line_a: b"Runtime: core, sandbox skills, display, input.",
        line_b: b"Health checks stay visible before promotion.",
        x: 760,
        y: 176,
        w: 560,
        h: 320,
        closed: false,
    };
    WINDOWS[2] = Window {
        id: 3,
        title: b"Skill Store",
        line_a: b"Local cache, registry lookup, install candidates.",
        line_b: b"Future: promote skill after QEMU validation.",
        x: 96,
        y: 520,
        w: 620,
        h: 300,
        closed: false,
    };
    WINDOWS[3] = Window {
        id: 4,
        title: b"VFS Explorer",
        line_a: b"Boot disk, driver cache, documents, artifacts.",
        line_b: b"Future: bounded file actions for the agent.",
        x: 760,
        y: 520,
        w: 560,
        h: 300,
        closed: false,
    };
    FOCUSED_WINDOW = 1;
    INITIALIZED = true;
}

unsafe fn render_label(handle: u32, x: u32, y: u32, w: u32, h: u32, style: u32, label: &[u8]) {
    os_gui_set_object_bounds(handle, x, y, w, h);
    os_gui_set_object_style(handle, style);
    os_gui_set_object_label(handle, label.as_ptr(), label.len() as u32);
}

unsafe fn clear_window_slot(slot: usize) {
    let base = WINDOW_BASE + (slot as u32 * WINDOW_STRIDE);
    for offset in 0..5 {
        render_label(base + offset, 0, 0, 0, 0, STYLE_PLAIN, EMPTY_LABEL);
        os_gui_set_object_interaction(base + offset, INTERACTION_IDLE);
    }
}

unsafe fn render_window(slot: usize, window: Window) {
    if window.closed {
        clear_window_slot(slot);
        return;
    }

    let base = WINDOW_BASE + (slot as u32 * WINDOW_STRIDE);
    let focused = window.id == FOCUSED_WINDOW;
    let title_style = if focused { STYLE_ACCENT } else { STYLE_CHROME };
    let interaction = if focused {
        INTERACTION_FOCUSED
    } else {
        INTERACTION_IDLE
    };

    render_label(base, window.x, window.y, window.w, window.h, STYLE_WINDOW, b" ");
    os_gui_set_object_interaction(base, interaction);

    render_label(base + 1, window.x, window.y, window.w, 34, title_style, window.title);
    os_gui_set_object_interaction(base + 1, interaction);

    render_label(
        base + 2,
        window.x + window.w.saturating_sub(44),
        window.y + 7,
        30,
        20,
        STYLE_SIDEBAR,
        CLOSE_LABEL,
    );
    os_gui_set_object_interaction(base + 2, INTERACTION_HOVERED);

    render_label(
        base + 3,
        window.x + 24,
        window.y + 64,
        window.w.saturating_sub(48),
        28,
        STYLE_ACCENT,
        window.line_a,
    );
    render_label(
        base + 4,
        window.x + 24,
        window.y + 104,
        window.w.saturating_sub(48),
        28,
        STYLE_PLAIN,
        window.line_b,
    );
}

unsafe fn render_desktop() {
    render_label(DESKTOP_BG, 0, 0, 1920, 1080, STYLE_PLAIN, b"OpenRhiza Desktop");
    os_gui_set_object_interaction(DESKTOP_BG, INTERACTION_IDLE);

    for index in 0..MAX_WINDOWS {
        render_window(index, WINDOWS[index]);
    }

    render_label(DOCK, 0, 1032, 1920, 48, STYLE_CHROME, b" ");
    render_label(DOCK_LABEL, 24, 1044, 900, 24, STYLE_ACCENT, DOCK_TEXT);
}

unsafe fn focus_window(win_id: u32) -> u32 {
    for index in 0..MAX_WINDOWS {
        if WINDOWS[index].id == win_id && !WINDOWS[index].closed {
            FOCUSED_WINDOW = win_id;
            render_desktop();
            return 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn init_driver() {
    unsafe {
        init_windows_once();
    }
    log(INIT_MSG);
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    unsafe {
        init_windows_once();
        os_set_display_session_target(2);
        os_set_display_validation_state(3);
        os_request_display_mode(2, 240, 67, 1920, 1080);
        os_set_gui_session_state(2);
        os_gui_select_session(2);
        os_gui_focus_object(2);
        render_desktop();
    }
    log(RUN_MSG);
    1
}

#[no_mangle]
pub extern "C" fn compositor_handle_click(click_x: u32, click_y: u32) -> u32 {
    unsafe {
        for index in (0..MAX_WINDOWS).rev() {
            let window = WINDOWS[index];
            if window.closed {
                continue;
            }
            if click_x < window.x
                || click_y < window.y
                || click_x >= window.x.saturating_add(window.w)
                || click_y >= window.y.saturating_add(window.h)
            {
                continue;
            }
            if click_y < window.y + 34 && click_x >= window.x + window.w.saturating_sub(44) {
                WINDOWS[index].closed = true;
            } else {
                FOCUSED_WINDOW = window.id;
            }
            render_desktop();
            return 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn agent_cmd_close_window(win_id: u32) -> u32 {
    unsafe {
        for index in 0..MAX_WINDOWS {
            if WINDOWS[index].id == win_id {
                WINDOWS[index].closed = true;
                render_desktop();
                return 1;
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn agent_cmd_move_window(win_id: u32, x: u32, y: u32) -> u32 {
    unsafe {
        for index in 0..MAX_WINDOWS {
            if WINDOWS[index].id == win_id && !WINDOWS[index].closed {
                WINDOWS[index].x = x;
                WINDOWS[index].y = y;
                FOCUSED_WINDOW = win_id;
                render_desktop();
                return 1;
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn desktop_window_protocol_selftest() -> i32 {
    unsafe {
        init_windows_once();
        os_set_display_session_target(2);
        os_set_display_validation_state(3);
        os_request_display_mode(2, 240, 67, 1920, 1080);
        os_set_gui_session_state(2);
        os_gui_select_session(2);
        os_gui_focus_object(2);
        let _ = agent_cmd_move_window(2, 980, 220);
        render_desktop();
    }
    log(PROTOCOL_SELFTEST_MSG);
    1
}

#[no_mangle]
pub extern "C" fn desktop_window_close_selftest() -> i32 {
    unsafe {
        init_windows_once();
        let _ = agent_cmd_close_window(2);
        render_desktop();
    }
    log(PROTOCOL_CLOSE_MSG);
    1
}

#[no_mangle]
pub extern "C" fn desktop_window_restore_selftest() -> i32 {
    unsafe {
        INITIALIZED = false;
        init_windows_once();
        os_set_display_session_target(2);
        os_set_display_validation_state(3);
        os_request_display_mode(2, 240, 67, 1920, 1080);
        os_set_gui_session_state(2);
        os_gui_select_session(2);
        os_gui_focus_object(2);
        render_desktop();
    }
    log(PROTOCOL_RESTORE_MSG);
    1
}

#[no_mangle]
pub extern "C" fn desktop_default_apps_selftest() -> i32 {
    unsafe {
        INITIALIZED = false;
        init_windows_once();
        os_set_display_session_target(2);
        os_set_display_validation_state(3);
        os_request_display_mode(2, 240, 67, 1920, 1080);
        os_set_gui_session_state(2);
        os_gui_select_session(2);
        os_gui_focus_object(2);
        render_desktop();
    }
    log(DEFAULT_APPS_MSG);
    1
}

#[no_mangle]
pub extern "C" fn desktop_action_focus_terminal() -> i32 {
    unsafe {
        init_windows_once();
        let _ = focus_window(1);
    }
    log(ACTION_FOCUS_TERMINAL_MSG);
    1
}

#[no_mangle]
pub extern "C" fn desktop_action_focus_system_monitor() -> i32 {
    unsafe {
        init_windows_once();
        let _ = focus_window(2);
    }
    log(ACTION_FOCUS_MONITOR_MSG);
    1
}

#[no_mangle]
pub extern "C" fn desktop_action_focus_skill_store() -> i32 {
    unsafe {
        init_windows_once();
        let _ = focus_window(3);
    }
    log(ACTION_FOCUS_STORE_MSG);
    1
}

#[no_mangle]
pub extern "C" fn desktop_action_focus_vfs_explorer() -> i32 {
    unsafe {
        init_windows_once();
        let _ = focus_window(4);
    }
    log(ACTION_FOCUS_VFS_MSG);
    1
}

#[no_mangle]
pub extern "C" fn desktop_action_route_selftest() -> i32 {
    unsafe {
        INITIALIZED = false;
        init_windows_once();
        let _ = focus_window(4);
    }
    log(ACTION_FOCUS_VFS_MSG);
    1
}
