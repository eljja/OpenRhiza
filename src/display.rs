use alloc::format;
use alloc::string::String;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayBackend {
    VgaText,
    FramebufferText,
    Gui,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiSessionPhase {
    TextShell,
    BootstrapRequested,
    SandboxSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaySessionTarget {
    RecoveryShell,
    WideConsole,
    GuiSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayValidationState {
    None,
    Requested,
    Testing,
    Ready,
    Promoted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayModeInfo {
    pub backend: DisplayBackend,
    pub text_cols: usize,
    pub text_rows: usize,
    pub pixel_width: usize,
    pub pixel_height: usize,
}

#[derive(Debug, Clone, Copy)]
struct DisplayRuntimeState {
    active_mode: DisplayModeInfo,
    requested_mode: Option<DisplayModeInfo>,
    gui_phase: GuiSessionPhase,
    target: DisplaySessionTarget,
    validation: DisplayValidationState,
}

static DISPLAY_RUNTIME: Mutex<DisplayRuntimeState> = Mutex::new(DisplayRuntimeState {
    active_mode: DisplayModeInfo {
        backend: DisplayBackend::VgaText,
        text_cols: 80,
        text_rows: 25,
        pixel_width: 720,
        pixel_height: 400,
    },
    requested_mode: None,
    gui_phase: GuiSessionPhase::TextShell,
    target: DisplaySessionTarget::RecoveryShell,
    validation: DisplayValidationState::None,
});

pub fn active_mode() -> DisplayModeInfo {
    DISPLAY_RUNTIME.lock().active_mode
}

pub fn requested_mode() -> Option<DisplayModeInfo> {
    DISPLAY_RUNTIME.lock().requested_mode
}

pub fn gui_phase() -> GuiSessionPhase {
    DISPLAY_RUNTIME.lock().gui_phase
}

pub fn session_target() -> DisplaySessionTarget {
    DISPLAY_RUNTIME.lock().target
}

pub fn validation_state() -> DisplayValidationState {
    DISPLAY_RUNTIME.lock().validation
}

pub fn backend_name(backend: DisplayBackend) -> &'static str {
    match backend {
        DisplayBackend::VgaText => "vga-text",
        DisplayBackend::FramebufferText => "framebuffer-text",
        DisplayBackend::Gui => "gui",
    }
}

pub fn describe_mode(mode: DisplayModeInfo) -> String {
    format!(
        "backend={} text={}x{} pixels={}x{}",
        backend_name(mode.backend),
        mode.text_cols,
        mode.text_rows,
        mode.pixel_width,
        mode.pixel_height
    )
}

pub fn describe_active_mode() -> String {
    describe_mode(active_mode())
}

pub fn describe_requested_mode() -> Option<String> {
    requested_mode().map(describe_mode)
}

pub fn gui_phase_name(phase: GuiSessionPhase) -> &'static str {
    match phase {
        GuiSessionPhase::TextShell => "text-shell",
        GuiSessionPhase::BootstrapRequested => "bootstrap-requested",
        GuiSessionPhase::SandboxSession => "sandbox-session",
    }
}

pub fn session_target_name(target: DisplaySessionTarget) -> &'static str {
    match target {
        DisplaySessionTarget::RecoveryShell => "recovery-shell",
        DisplaySessionTarget::WideConsole => "wide-console",
        DisplaySessionTarget::GuiSession => "gui-session",
    }
}

pub fn validation_state_name(state: DisplayValidationState) -> &'static str {
    match state {
        DisplayValidationState::None => "none",
        DisplayValidationState::Requested => "requested",
        DisplayValidationState::Testing => "testing",
        DisplayValidationState::Ready => "ready",
        DisplayValidationState::Promoted => "promoted",
    }
}

pub fn request_mode_from_wasm(
    backend: u32,
    text_cols: u32,
    text_rows: u32,
    pixel_width: u32,
    pixel_height: u32,
) {
    let backend = match backend {
        1 => DisplayBackend::FramebufferText,
        2 => DisplayBackend::Gui,
        _ => DisplayBackend::VgaText,
    };

    let mode = DisplayModeInfo {
        backend,
        text_cols: text_cols as usize,
        text_rows: text_rows as usize,
        pixel_width: pixel_width as usize,
        pixel_height: pixel_height as usize,
    };

    DISPLAY_RUNTIME.lock().requested_mode = Some(mode);
    crate::result_println!("[Display Runtime] Requested {}", describe_mode(mode));
}

pub fn set_display_session_target_from_wasm(target: u32) {
    let target = match target {
        1 => DisplaySessionTarget::WideConsole,
        2 => DisplaySessionTarget::GuiSession,
        _ => DisplaySessionTarget::RecoveryShell,
    };

    DISPLAY_RUNTIME.lock().target = target;
    crate::result_println!(
        "[Display Runtime] Session target={}",
        session_target_name(target)
    );
}

pub fn set_display_validation_state_from_wasm(state: u32) {
    let validation = match state {
        1 => DisplayValidationState::Requested,
        2 => DisplayValidationState::Testing,
        3 => DisplayValidationState::Ready,
        4 => DisplayValidationState::Promoted,
        _ => DisplayValidationState::None,
    };

    DISPLAY_RUNTIME.lock().validation = validation;
    crate::result_println!(
        "[Display Runtime] Validation={}",
        validation_state_name(validation)
    );
}

pub fn set_gui_session_state_from_wasm(state: u32) {
    let phase = match state {
        1 => GuiSessionPhase::BootstrapRequested,
        2 => GuiSessionPhase::SandboxSession,
        _ => GuiSessionPhase::TextShell,
    };

    DISPLAY_RUNTIME.lock().gui_phase = phase;
    crate::result_println!("[Display Runtime] GUI phase={}", gui_phase_name(phase));
}

pub fn status_block() -> String {
    let active = describe_active_mode();
    let requested = describe_requested_mode().unwrap_or_else(|| String::from("none"));
    let gui = gui_phase_name(gui_phase());
    let target = session_target_name(session_target());
    let validation = validation_state_name(validation_state());
    format!(
        "Display runtime:\n- active: {}\n- requested: {}\n- target: {}\n- validation: {}\n- gui: {}\n",
        active, requested, target, validation, gui
    )
}

pub fn init_console() {
    crate::vga::init_cli();
}

pub fn render_runtime(seconds: u64) {
    crate::vga::render_runtime(seconds);
}
