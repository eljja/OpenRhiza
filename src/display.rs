use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::arch::x86_64::port::{read_port_u16, write_port_u16, write_port_u32};
use crate::gui_contract::{
    GuiBackendPreference,
    GuiInteractionState,
    GuiMutation,
    GuiNode,
    GuiNodeKind,
    GuiObjectHandle,
    GuiRect as ContractRect,
    GuiScene,
    GuiStyleClass,
};
use crate::gui_lvgl_bridge::translate_scene;
use core::sync::atomic::{AtomicBool, Ordering};
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

#[derive(Debug, Clone, Copy)]
struct BootstrapSurfaceState {
    enabled: bool,
    width: usize,
    height: usize,
    framebuffer_phys: u64,
}

const BOOTSTRAP_OVERLAY_SLOTS: usize = 6;
const BOOTSTRAP_OVERLAY_LINE_CAPACITY: usize = 120;

#[derive(Debug, Clone, Copy)]
struct BootstrapOverlayState {
    lines: [[u8; BOOTSTRAP_OVERLAY_LINE_CAPACITY]; BOOTSTRAP_OVERLAY_SLOTS],
    lens: [usize; BOOTSTRAP_OVERLAY_SLOTS],
}

#[derive(Clone)]
struct BootstrapFrameCache {
    layout_signature: u64,
    status_line: String,
    header_line: String,
    input_line: String,
    log_lines: alloc::vec::Vec<(String, u8)>,
    selected_session: GuiObjectId,
    hovered: Option<GuiObjectId>,
    focused: Option<GuiObjectId>,
}

const POINTER_BITMAP_WIDTH: usize = 12;
const POINTER_BITMAP_HEIGHT: usize = 16;
const POINTER_SCALE: usize = 1;
const POINTER_SHADOW_OFFSET: usize = 2;
const POINTER_WIDTH: usize = POINTER_BITMAP_WIDTH * POINTER_SCALE + POINTER_SHADOW_OFFSET;
const POINTER_HEIGHT: usize = POINTER_BITMAP_HEIGHT * POINTER_SCALE + POINTER_SHADOW_OFFSET;
const POINTER_SPEED_MULTIPLIER: i32 = 3;
const GUI_MESSAGE_RENDER_LINE_LIMIT: usize = 8;

#[derive(Clone, Copy)]
struct BootstrapPointerState {
    initialized: bool,
    x: usize,
    y: usize,
    buttons: u8,
}

struct PointerOverlayState {
    visible: bool,
    x: usize,
    y: usize,
    saved: [u32; POINTER_WIDTH * POINTER_HEIGHT],
}

const GUI_OBJECT_CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuiObjectId {
    SessionOpenRhiza,
    SessionSandboxGui,
    SessionWideConsole,
    SessionRecoveryShell,
    Conversation,
    Composer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuiObjectKind {
    SessionItem,
    Conversation,
    Composer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GuiRect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl GuiRect {
    const fn contains(&self, px: usize, py: usize) -> bool {
        px >= self.x
            && py >= self.y
            && px < self.x.saturating_add(self.width)
            && py < self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GuiObject {
    id: GuiObjectId,
    kind: GuiObjectKind,
    rect: GuiRect,
}

#[derive(Clone, Copy)]
struct GuiObjectRuntime {
    objects: [Option<GuiObject>; GUI_OBJECT_CAPACITY],
    len: usize,
    hovered: Option<GuiObjectId>,
    focused: Option<GuiObjectId>,
    selected_session: GuiObjectId,
    last_buttons: u8,
}

impl GuiObjectRuntime {
    const fn new() -> Self {
        Self {
            objects: [None; GUI_OBJECT_CAPACITY],
            len: 0,
            hovered: None,
            focused: None,
            selected_session: GuiObjectId::SessionOpenRhiza,
            last_buttons: 0,
        }
    }

    fn clear(&mut self) {
        self.objects = [None; GUI_OBJECT_CAPACITY];
        self.len = 0;
    }

    fn push(&mut self, object: GuiObject) {
        if self.len < GUI_OBJECT_CAPACITY {
            self.objects[self.len] = Some(object);
            self.len += 1;
        }
    }

    fn object(&self, id: GuiObjectId) -> Option<GuiObject> {
        self.objects[..self.len]
            .iter()
            .flatten()
            .copied()
            .find(|object| object.id == id)
    }

    fn hit_test(&self, px: usize, py: usize) -> Option<GuiObjectId> {
        self.objects[..self.len]
            .iter()
            .flatten()
            .copied()
            .find(|object| object.rect.contains(px, py))
            .map(|object| object.id)
    }
}

#[derive(Clone, Copy)]
struct GuiSceneRuntimeState {
    conversation_scroll_items: usize,
    composer_rows: usize,
    composer_height: usize,
}

impl GuiSceneRuntimeState {
    const fn new() -> Self {
        Self {
            conversation_scroll_items: 0,
            composer_rows: 1,
            composer_height: 72,
        }
    }
}

#[derive(Clone)]
struct GuiChatMessage {
    is_user: bool,
    style: GuiStyleClass,
    text: String,
}

const GUI_CHAT_HISTORY_LIMIT: usize = 256;

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

static BOOTSTRAP_SURFACE: Mutex<BootstrapSurfaceState> = Mutex::new(BootstrapSurfaceState {
    enabled: false,
    width: 0,
    height: 0,
    framebuffer_phys: 0,
});
static BOOTSTRAP_OVERLAY: Mutex<BootstrapOverlayState> = Mutex::new(BootstrapOverlayState {
    lines: [[0; BOOTSTRAP_OVERLAY_LINE_CAPACITY]; BOOTSTRAP_OVERLAY_SLOTS],
    lens: [0; BOOTSTRAP_OVERLAY_SLOTS],
});
static BOOTSTRAP_FRAME_CACHE: Mutex<Option<BootstrapFrameCache>> = Mutex::new(None);
static BOOTSTRAP_POINTER: Mutex<BootstrapPointerState> = Mutex::new(BootstrapPointerState {
    initialized: false,
    x: 0,
    y: 0,
    buttons: 0,
});
static POINTER_OVERLAY: Mutex<PointerOverlayState> = Mutex::new(PointerOverlayState {
    visible: false,
    x: 0,
    y: 0,
    saved: [0; POINTER_WIDTH * POINTER_HEIGHT],
});
static GUI_OBJECTS: Mutex<GuiObjectRuntime> = Mutex::new(GuiObjectRuntime::new());
static GUI_SCENE_RUNTIME: Mutex<GuiSceneRuntimeState> = Mutex::new(GuiSceneRuntimeState::new());
static GUI_MUTATIONS: Mutex<Vec<GuiMutation>> = Mutex::new(Vec::new());
static GUI_CHAT_HISTORY: Mutex<Vec<GuiChatMessage>> = Mutex::new(Vec::new());
static BOOTSTRAP_SURFACE_DIRTY: AtomicBool = AtomicBool::new(true);
static LAST_GUI_CARET_VISIBLE: AtomicBool = AtomicBool::new(true);

fn push_gui_chat_message(message: GuiChatMessage) {
    let mut history = GUI_CHAT_HISTORY.lock();
    history.push(message);
    if history.len() > GUI_CHAT_HISTORY_LIMIT {
        let overflow = history.len() - GUI_CHAT_HISTORY_LIMIT;
        history.drain(0..overflow);
    }
    drop(history);
    notify_surface_dirty();
}

pub fn record_gui_user_prompt(prompt: &str) {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return;
    }
    push_gui_chat_message(GuiChatMessage {
        is_user: true,
        style: GuiStyleClass::UserMessage,
        text: String::from(trimmed),
    });
}

pub fn record_gui_assistant_message(text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    push_gui_chat_message(GuiChatMessage {
        is_user: false,
        style: GuiStyleClass::AssistantMessage,
        text: String::from(trimmed),
    });
}

pub fn record_gui_system_message(text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    push_gui_chat_message(GuiChatMessage {
        is_user: false,
        style: GuiStyleClass::AccentText,
        text: String::from(trimmed),
    });
}

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
    notify_surface_dirty();
    crate::result_println!("[Display Runtime] Requested {}", describe_mode(mode));
}

pub fn set_display_session_target_from_wasm(target: u32) {
    let target = match target {
        1 => DisplaySessionTarget::WideConsole,
        2 => DisplaySessionTarget::GuiSession,
        _ => DisplaySessionTarget::RecoveryShell,
    };

    DISPLAY_RUNTIME.lock().target = target;
    notify_surface_dirty();
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
    notify_surface_dirty();
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
    notify_surface_dirty();
    crate::result_println!("[Display Runtime] GUI phase={}", gui_phase_name(phase));
}

pub fn notify_surface_dirty() {
    BOOTSTRAP_SURFACE_DIRTY.store(true, Ordering::Relaxed);
}

pub fn update_pointer_motion(dx: i8, dy: i8, buttons: u8, wheel: i8, input_line: &str) {
    let mode = requested_mode().unwrap_or_else(active_mode);
    let max_x = mode.pixel_width.saturating_sub(POINTER_WIDTH);
    let max_y = mode.pixel_height.saturating_sub(POINTER_HEIGHT);

    let mut pointer = BOOTSTRAP_POINTER.lock();
    if !pointer.initialized {
        pointer.initialized = true;
        pointer.x = mode.pixel_width / 2;
        pointer.y = mode.pixel_height / 2;
    }

    let next_x = pointer.x as i32 + (dx as i32 * POINTER_SPEED_MULTIPLIER);
    let next_y = pointer.y as i32 + (dy as i32 * POINTER_SPEED_MULTIPLIER);
    pointer.x = next_x.clamp(0, max_x as i32) as usize;
    pointer.y = next_y.clamp(0, max_y as i32) as usize;
    pointer.buttons = buttons & 0x07;
    let pointer_x = pointer.x;
    let pointer_y = pointer.y;
    drop(pointer);
    update_gui_pointer_interaction(
        pointer_x,
        pointer_y,
        buttons & 0x07,
        wheel,
        mode.pixel_width,
        mode.pixel_height,
        input_line,
    );
    notify_surface_dirty();
}

fn update_gui_pointer_interaction(
    pointer_x: usize,
    pointer_y: usize,
    buttons: u8,
    wheel: i8,
    width: usize,
    height: usize,
    input_line: &str,
) {
    if !matches!(session_target(), DisplaySessionTarget::GuiSession) {
        let mut objects = GUI_OBJECTS.lock();
        objects.hovered = None;
        objects.focused = None;
        objects.last_buttons = buttons;
        return;
    }

    sync_gui_objects(width, height, input_line);

    let mut objects = GUI_OBJECTS.lock();
    let hovered = objects.hit_test(pointer_x, pointer_y);
    let left_pressed = (buttons & 0x01) != 0;
    let previous_left_pressed = (objects.last_buttons & 0x01) != 0;
    let click_started = left_pressed && !previous_left_pressed;
    let mut changed = hovered != objects.hovered;

    objects.hovered = hovered;
    if click_started {
        match hovered {
            Some(
                GuiObjectId::SessionOpenRhiza
                | GuiObjectId::SessionSandboxGui
                | GuiObjectId::SessionWideConsole
                | GuiObjectId::SessionRecoveryShell,
            ) => {
                objects.selected_session = hovered.unwrap();
                objects.focused = hovered;
                changed = true;
            }
            Some(GuiObjectId::Composer) => {
                objects.focused = Some(GuiObjectId::Composer);
                changed = true;
            }
            Some(GuiObjectId::Conversation) => {
                objects.focused = Some(GuiObjectId::Conversation);
                changed = true;
            }
            None => {
                if objects.focused.is_some() {
                    objects.focused = None;
                    changed = true;
                }
            }
        }
    }
    objects.last_buttons = buttons;
    drop(objects);

    if wheel != 0 && matches!(hovered, Some(GuiObjectId::Conversation)) {
        let delta = if wheel > 0 { wheel as isize } else { -((-wheel) as isize) };
        if adjust_gui_conversation_scroll(delta) {
            changed = true;
        }
    }

    if changed {
        notify_surface_dirty();
    }
}

pub fn set_overlay_line_from_wasm(slot: u32, bytes: &[u8]) {
    let slot = slot as usize;
    if slot >= BOOTSTRAP_OVERLAY_SLOTS {
        return;
    }

    let mut overlay = BOOTSTRAP_OVERLAY.lock();
    let copy_len = bytes
        .len()
        .min(BOOTSTRAP_OVERLAY_LINE_CAPACITY.saturating_sub(1));
    overlay.lines[slot] = [0; BOOTSTRAP_OVERLAY_LINE_CAPACITY];
    overlay.lines[slot][..copy_len].copy_from_slice(&bytes[..copy_len]);
    overlay.lens[slot] = copy_len;
    drop(overlay);
    notify_surface_dirty();
}

fn overlay_line(slot: usize) -> String {
    let overlay = BOOTSTRAP_OVERLAY.lock();
    if slot >= BOOTSTRAP_OVERLAY_SLOTS {
        return String::new();
    }

    let len = overlay.lens[slot].min(BOOTSTRAP_OVERLAY_LINE_CAPACITY);
    let mut line = String::new();
    for &byte in overlay.lines[slot][..len].iter() {
        if byte == 0 {
            break;
        }
        if byte.is_ascii_graphic() || byte == b' ' {
            line.push(byte as char);
        } else {
            line.push(' ');
        }
    }
    line
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

pub fn gui_scene_report() -> String {
    if !matches!(session_target(), DisplaySessionTarget::GuiSession) {
        return String::from("GUI scene:\n- inactive: current session target is not gui-session\n");
    }

    let snapshot = crate::vga::external_surface_snapshot(24);
    let scene = build_gui_scene(
        active_mode().pixel_width,
        active_mode().pixel_height,
        snapshot.log_lines.as_slice(),
        snapshot.input_line.as_str(),
        "",
        "",
    );
    let translated = translate_scene(&scene);
    let runtime = *GUI_SCENE_RUNTIME.lock();
    let mut report = format!(
        "GUI scene:\n- id: {}\n- backend_preference: {:?}\n- nodes: {}\n- lvgl_widgets: {}\n- conversation_scroll_items: {}\n- composer_rows: {}\n- composer_height: {}\n",
        scene.scene_id,
        scene.backend_preference,
        scene.nodes.len(),
        translated.len(),
        runtime.conversation_scroll_items,
        runtime.composer_rows,
        runtime.composer_height
    );
    for node in scene.nodes.iter().take(16) {
        report.push_str(
            format!(
                "- {:?} handle={} bounds=({},{} {}x{}) interaction={:?} ref={}\n",
                node.kind,
                node.handle.0,
                node.bounds.x,
                node.bounds.y,
                node.bounds.width,
                node.bounds.height,
                node.interaction,
                node.object_ref.as_deref().unwrap_or("none")
            )
            .as_str(),
        );
    }
    for widget in translated.iter().take(8) {
        report.push_str(
            format!(
                "- lvgl handle={} widget={} style={:?} bounds=({},{} {}x{})\n",
                widget.handle.0,
                widget.widget_name,
                widget.style_hint,
                widget.bounds.x,
                widget.bounds.y,
                widget.bounds.width,
                widget.bounds.height
            )
            .as_str(),
        );
    }
    report
}

pub fn gui_mutation_report() -> String {
    let mutations = GUI_MUTATIONS.lock();
    if mutations.is_empty() {
        return String::from("GUI mutations:\n- none\n");
    }

    let mut report = format!("GUI mutations:\n- count: {}\n", mutations.len());
    for mutation in mutations.iter().take(16) {
        report.push_str(
            format!(
                "- handle={} label={} style={} interaction={} bounds={}\n",
                mutation.target.0,
                mutation
                    .new_label
                    .as_deref()
                    .unwrap_or("none"),
                mutation
                    .new_style
                    .map(gui_style_name)
                    .unwrap_or("none"),
                mutation
                    .new_interaction
                    .map(gui_interaction_name)
                    .unwrap_or("none"),
                mutation
                    .new_bounds
                    .map(|rect| format!("({},{} {}x{})", rect.x, rect.y, rect.width, rect.height))
                    .unwrap_or_else(|| String::from("none"))
            )
            .as_str(),
        );
    }
    report
}

pub fn set_gui_label(handle: u64, label: &str) -> Result<(), &'static str> {
    if !known_gui_handle(handle) {
        return Err("unknown gui handle");
    }
    upsert_gui_mutation(GuiMutation {
        target: GuiObjectHandle(handle),
        new_bounds: None,
        new_style: None,
        new_interaction: None,
        new_label: Some(String::from(label)),
    });
    notify_surface_dirty();
    Ok(())
}

pub fn set_gui_style(handle: u64, style_name: &str) -> Result<(), &'static str> {
    if !known_gui_handle(handle) {
        return Err("unknown gui handle");
    }
    let style = parse_gui_style(style_name).ok_or("unknown gui style")?;
    upsert_gui_mutation(GuiMutation {
        target: GuiObjectHandle(handle),
        new_bounds: None,
        new_style: Some(style),
        new_interaction: None,
        new_label: None,
    });
    notify_surface_dirty();
    Ok(())
}

pub fn set_gui_bounds(
    handle: u64,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> Result<(), &'static str> {
    if !known_gui_handle(handle) {
        return Err("unknown gui handle");
    }
    if width == 0 || height == 0 {
        return Err("gui bounds must have non-zero width and height");
    }
    upsert_gui_mutation(GuiMutation {
        target: GuiObjectHandle(handle),
        new_bounds: Some(ContractRect { x, y, width, height }),
        new_style: None,
        new_interaction: None,
        new_label: None,
    });
    notify_surface_dirty();
    Ok(())
}

pub fn set_gui_interaction(handle: u64, interaction_name: &str) -> Result<(), &'static str> {
    if !known_gui_handle(handle) {
        return Err("unknown gui handle");
    }
    let interaction =
        parse_gui_interaction(interaction_name).ok_or("unknown gui interaction")?;
    upsert_gui_mutation(GuiMutation {
        target: GuiObjectHandle(handle),
        new_bounds: None,
        new_style: None,
        new_interaction: Some(interaction),
        new_label: None,
    });
    notify_surface_dirty();
    Ok(())
}

pub fn reset_gui_mutations(handle: Option<u64>) {
    let mut mutations = GUI_MUTATIONS.lock();
    if let Some(handle) = handle {
        mutations.retain(|mutation| mutation.target.0 != handle);
    } else {
        mutations.clear();
    }
    drop(mutations);
    notify_surface_dirty();
}

pub fn select_gui_session(session: &str) -> Result<(), &'static str> {
    let target = match session {
        "openrhiza" | "main" => GuiObjectId::SessionOpenRhiza,
        "sandbox" | "sandbox-gui" | "lvgl" => GuiObjectId::SessionSandboxGui,
        "wide" | "wide-console" => GuiObjectId::SessionWideConsole,
        "recovery" | "recovery-shell" => GuiObjectId::SessionRecoveryShell,
        _ => return Err("unknown gui session; use openrhiza|sandbox|wide|recovery"),
    };

    let mut state = GUI_OBJECTS.lock();
    state.selected_session = target;
    state.focused = Some(target);
    drop(state);
    GUI_SCENE_RUNTIME.lock().conversation_scroll_items = 0;
    notify_surface_dirty();
    Ok(())
}

pub fn focus_gui_object(name: &str) -> Result<(), &'static str> {
    let target = match name {
        "conversation" | "chat" => Some(GuiObjectId::Conversation),
        "composer" | "input" => Some(GuiObjectId::Composer),
        "none" | "clear" => None,
        _ => return Err("unknown gui object; use conversation|composer|none"),
    };

    let mut state = GUI_OBJECTS.lock();
    state.focused = target;
    drop(state);
    notify_surface_dirty();
    Ok(())
}

pub fn scroll_gui_conversation(direction: &str, count: usize) -> Result<(), &'static str> {
    let applied = match direction {
        "up" => adjust_gui_conversation_scroll(count as isize),
        "down" => adjust_gui_conversation_scroll(-(count as isize)),
        "bottom" | "reset" => {
            GUI_SCENE_RUNTIME.lock().conversation_scroll_items = 0;
            true
        }
        _ => return Err("unknown gui scroll direction; use up|down|bottom"),
    };

    if applied {
        notify_surface_dirty();
    }
    Ok(())
}

pub fn select_gui_session_from_wasm(session: u32) {
    let name = match session {
        1 => "openrhiza",
        2 => "sandbox",
        3 => "wide",
        4 => "recovery",
        _ => return,
    };
    let _ = select_gui_session(name);
}

pub fn focus_gui_object_from_wasm(target: u32) {
    let name = match target {
        0 => "none",
        1 => "conversation",
        2 => "composer",
        _ => return,
    };
    let _ = focus_gui_object(name);
}

pub fn set_gui_label_from_wasm(handle: u32, bytes: &[u8]) {
    let Ok(label) = core::str::from_utf8(bytes) else {
        return;
    };
    let _ = set_gui_label(handle as u64, label.trim());
}

pub fn set_gui_style_code_from_wasm(handle: u32, style_code: u32) {
    let Some(style) = gui_style_from_code(style_code) else {
        return;
    };
    if !known_gui_handle(handle as u64) {
        return;
    }
    upsert_gui_mutation(GuiMutation {
        target: GuiObjectHandle(handle as u64),
        new_bounds: None,
        new_style: Some(style),
        new_interaction: None,
        new_label: None,
    });
    notify_surface_dirty();
}

pub fn set_gui_bounds_from_wasm(handle: u32, x: u32, y: u32, width: u32, height: u32) {
    let _ = set_gui_bounds(handle as u64, x as usize, y as usize, width as usize, height as usize);
}

pub fn set_gui_interaction_code_from_wasm(handle: u32, interaction_code: u32) {
    let Some(interaction) = gui_interaction_from_code(interaction_code) else {
        return;
    };
    if !known_gui_handle(handle as u64) {
        return;
    }
    upsert_gui_mutation(GuiMutation {
        target: GuiObjectHandle(handle as u64),
        new_bounds: None,
        new_style: None,
        new_interaction: Some(interaction),
        new_label: None,
    });
    notify_surface_dirty();
}

pub fn reset_gui_mutations_from_wasm(handle: u32) {
    if handle == 0 {
        reset_gui_mutations(None);
    } else {
        reset_gui_mutations(Some(handle as u64));
    }
}

fn upsert_gui_mutation(mutation: GuiMutation) {
    let mut mutations = GUI_MUTATIONS.lock();
    if let Some(existing) = mutations.iter_mut().find(|item| item.target == mutation.target) {
        if mutation.new_bounds.is_some() {
            existing.new_bounds = mutation.new_bounds;
        }
        if mutation.new_style.is_some() {
            existing.new_style = mutation.new_style;
        }
        if mutation.new_interaction.is_some() {
            existing.new_interaction = mutation.new_interaction;
        }
        if mutation.new_label.is_some() {
            existing.new_label = mutation.new_label;
        }
    } else {
        mutations.push(mutation);
    }
}

fn known_gui_handle(handle: u64) -> bool {
    matches!(
        handle,
        1 | 2 | 3 | 10 | 11 | 12 | 13 | 20 | 30 | 31 | 40 | 41
    ) || (200..=240).contains(&handle)
}

fn parse_gui_style(name: &str) -> Option<GuiStyleClass> {
    match name {
        "chrome" => Some(GuiStyleClass::Chrome),
        "sidebar" => Some(GuiStyleClass::Sidebar),
        "sidebar-idle" => Some(GuiStyleClass::SidebarItemIdle),
        "sidebar-hover" => Some(GuiStyleClass::SidebarItemHover),
        "sidebar-active" => Some(GuiStyleClass::SidebarItemActive),
        "conversation" => Some(GuiStyleClass::ConversationSurface),
        "assistant" => Some(GuiStyleClass::AssistantMessage),
        "user" => Some(GuiStyleClass::UserMessage),
        "composer" => Some(GuiStyleClass::ComposerSurface),
        "footer" => Some(GuiStyleClass::FooterSurface),
        "plain" => Some(GuiStyleClass::PlainText),
        "accent" => Some(GuiStyleClass::AccentText),
        _ => None,
    }
}

fn parse_gui_interaction(name: &str) -> Option<GuiInteractionState> {
    match name {
        "idle" => Some(GuiInteractionState::Idle),
        "hovered" | "hover" => Some(GuiInteractionState::Hovered),
        "focused" | "focus" => Some(GuiInteractionState::Focused),
        "active" => Some(GuiInteractionState::Active),
        "disabled" => Some(GuiInteractionState::Disabled),
        _ => None,
    }
}

fn gui_style_from_code(style_code: u32) -> Option<GuiStyleClass> {
    match style_code {
        0 => Some(GuiStyleClass::Chrome),
        1 => Some(GuiStyleClass::Sidebar),
        2 => Some(GuiStyleClass::SidebarItemIdle),
        3 => Some(GuiStyleClass::SidebarItemHover),
        4 => Some(GuiStyleClass::SidebarItemActive),
        5 => Some(GuiStyleClass::ConversationSurface),
        6 => Some(GuiStyleClass::AssistantMessage),
        7 => Some(GuiStyleClass::UserMessage),
        8 => Some(GuiStyleClass::ComposerSurface),
        9 => Some(GuiStyleClass::FooterSurface),
        10 => Some(GuiStyleClass::PlainText),
        11 => Some(GuiStyleClass::AccentText),
        _ => None,
    }
}

fn gui_interaction_from_code(interaction_code: u32) -> Option<GuiInteractionState> {
    match interaction_code {
        0 => Some(GuiInteractionState::Idle),
        1 => Some(GuiInteractionState::Hovered),
        2 => Some(GuiInteractionState::Focused),
        3 => Some(GuiInteractionState::Active),
        4 => Some(GuiInteractionState::Disabled),
        _ => None,
    }
}

fn gui_style_name(style: GuiStyleClass) -> &'static str {
    match style {
        GuiStyleClass::Chrome => "chrome",
        GuiStyleClass::Sidebar => "sidebar",
        GuiStyleClass::SidebarItemIdle => "sidebar-idle",
        GuiStyleClass::SidebarItemHover => "sidebar-hover",
        GuiStyleClass::SidebarItemActive => "sidebar-active",
        GuiStyleClass::ConversationSurface => "conversation",
        GuiStyleClass::AssistantMessage => "assistant",
        GuiStyleClass::UserMessage => "user",
        GuiStyleClass::ComposerSurface => "composer",
        GuiStyleClass::FooterSurface => "footer",
        GuiStyleClass::PlainText => "plain",
        GuiStyleClass::AccentText => "accent",
    }
}

fn gui_interaction_name(interaction: GuiInteractionState) -> &'static str {
    match interaction {
        GuiInteractionState::Idle => "idle",
        GuiInteractionState::Hovered => "hovered",
        GuiInteractionState::Focused => "focused",
        GuiInteractionState::Active => "active",
        GuiInteractionState::Disabled => "disabled",
    }
}

fn adjust_gui_conversation_scroll(delta: isize) -> bool {
    let mut runtime = GUI_SCENE_RUNTIME.lock();
    let previous = runtime.conversation_scroll_items;
    if delta > 0 {
        runtime.conversation_scroll_items = runtime
            .conversation_scroll_items
            .saturating_add(delta as usize)
            .min(64);
    } else if delta < 0 {
        runtime.conversation_scroll_items = runtime
            .conversation_scroll_items
            .saturating_sub((-delta) as usize);
    }
    previous != runtime.conversation_scroll_items
}

pub fn init_console() {
    crate::vga::init_cli();
}

pub fn render_runtime(seconds: u64) {
    crate::vga::render_runtime(seconds);
    notify_surface_dirty();
    refresh_if_needed();
}

pub fn refresh_if_needed() {
    let caret_visible = gui_caret_visible();
    let previous_caret_visible = LAST_GUI_CARET_VISIBLE.swap(caret_visible, Ordering::Relaxed);
    if caret_visible != previous_caret_visible && is_gui_composer_focused() {
        BOOTSTRAP_SURFACE_DIRTY.store(true, Ordering::Relaxed);
    }
    if !BOOTSTRAP_SURFACE_DIRTY.swap(false, Ordering::Relaxed) {
        return;
    }
    maybe_present_bootstrap_surface();
}

fn maybe_present_bootstrap_surface() {
    let runtime = *DISPLAY_RUNTIME.lock();
    let Some(requested) = runtime.requested_mode else {
        return;
    };

    if runtime.validation == DisplayValidationState::None || runtime.gui_phase == GuiSessionPhase::TextShell && runtime.target == DisplaySessionTarget::RecoveryShell {
        return;
    }

    if !ensure_bootstrap_surface(requested) {
        return;
    }

    let snapshot = crate::vga::external_surface_snapshot(24);
    draw_bootstrap_scene(runtime.active_mode, requested, runtime.target, runtime.validation, runtime.gui_phase, &snapshot);
}

fn ensure_bootstrap_surface(requested: DisplayModeInfo) -> bool {
    {
        let current = BOOTSTRAP_SURFACE.lock();
        if current.enabled && current.width == requested.pixel_width && current.height == requested.pixel_height {
            DISPLAY_RUNTIME.lock().active_mode = requested;
            return true;
        }
    }

    let Some(framebuffer_phys) = detect_display_framebuffer_phys() else {
        crate::result_println!("[Display Runtime] No framebuffer BAR found for bootstrap presenter.");
        return false;
    };

    if !enable_bga_mode(requested.pixel_width as u16, requested.pixel_height as u16, 32) {
        crate::result_println!("[Display Runtime] BGA mode switch failed; staying on recovery console.");
        return false;
    }

    *BOOTSTRAP_SURFACE.lock() = BootstrapSurfaceState {
        enabled: true,
        width: requested.pixel_width,
        height: requested.pixel_height,
        framebuffer_phys,
    };
    *BOOTSTRAP_FRAME_CACHE.lock() = None;
    DISPLAY_RUNTIME.lock().active_mode = requested;
    crate::result_println!(
        "[Display Runtime] Bootstrap presenter active at {}x{}.",
        requested.pixel_width,
        requested.pixel_height
    );
    true
}

fn detect_display_framebuffer_phys() -> Option<u64> {
    for bus in 0u16..=255 {
        for device in 0u16..=31 {
            for func in 0u16..=7 {
                let vendor = pci_read_word(bus as u8, device as u8, func as u8, 0x00);
                if vendor == 0xFFFF {
                    if func == 0 {
                        break;
                    }
                    continue;
                }
                let class_info = pci_read_dword(bus as u8, device as u8, func as u8, 0x08);
                let class_code = ((class_info >> 24) & 0xFF) as u8;
                if class_code != 0x03 {
                    continue;
                }
                let bar0 = pci_read_dword(bus as u8, device as u8, func as u8, 0x10);
                if (bar0 & 0x1) != 0 || bar0 == 0 {
                    continue;
                }
                return Some((bar0 & 0xFFFF_FFF0) as u64);
            }
        }
    }
    None
}

fn pci_read_word(bus: u8, device: u8, func: u8, offset: u8) -> u16 {
    let address: u32 =
        0x8000_0000 | ((bus as u32) << 16) | ((device as u32) << 11) | ((func as u32) << 8) | ((offset as u32) & 0xFC);
    write_port_u32(0xCF8, address);
    let data = crate::arch::x86_64::port::read_port_u32(0xCFC);
    ((data >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

fn pci_read_dword(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address: u32 =
        0x8000_0000 | ((bus as u32) << 16) | ((device as u32) << 11) | ((func as u32) << 8) | ((offset as u32) & 0xFC);
    write_port_u32(0xCF8, address);
    crate::arch::x86_64::port::read_port_u32(0xCFC)
}

fn enable_bga_mode(width: u16, height: u16, bpp: u16) -> bool {
    const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
    const VBE_DISPI_IOPORT_DATA: u16 = 0x01CF;
    const VBE_DISPI_INDEX_ID: u16 = 0x0;
    const VBE_DISPI_INDEX_XRES: u16 = 0x1;
    const VBE_DISPI_INDEX_YRES: u16 = 0x2;
    const VBE_DISPI_INDEX_BPP: u16 = 0x3;
    const VBE_DISPI_INDEX_ENABLE: u16 = 0x4;
    const VBE_DISPI_ENABLED: u16 = 0x01;
    const VBE_DISPI_LFB_ENABLED: u16 = 0x40;

    write_port_u16(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_ID);
    let id = read_port_u16(VBE_DISPI_IOPORT_DATA);
    if (id & 0xFFF0) != 0xB0C0 {
        return false;
    }

    write_port_u16(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_ENABLE);
    write_port_u16(VBE_DISPI_IOPORT_DATA, 0);
    write_port_u16(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_XRES);
    write_port_u16(VBE_DISPI_IOPORT_DATA, width);
    write_port_u16(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_YRES);
    write_port_u16(VBE_DISPI_IOPORT_DATA, height);
    write_port_u16(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_BPP);
    write_port_u16(VBE_DISPI_IOPORT_DATA, bpp);
    write_port_u16(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_ENABLE);
    write_port_u16(VBE_DISPI_IOPORT_DATA, VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED);
    true
}

fn draw_bootstrap_scene(
    active: DisplayModeInfo,
    requested: DisplayModeInfo,
    target: DisplaySessionTarget,
    validation: DisplayValidationState,
    gui_phase: GuiSessionPhase,
    snapshot: &crate::vga::ExternalSurfaceSnapshot,
) {
    let surface = *BOOTSTRAP_SURFACE.lock();
    if !surface.enabled || surface.width == 0 || surface.height == 0 {
        return;
    }

    let heading = overlay_line(0);
    let subheading = overlay_line(1);
    let panel_title = overlay_line(2);
    let panel_subtitle = overlay_line(3);
    let footer_primary = overlay_line(4);
    let footer_secondary = overlay_line(5);
    let layout_signature = compute_layout_signature(
        active,
        requested,
        target,
        validation,
        gui_phase,
        heading.as_str(),
        subheading.as_str(),
        panel_title.as_str(),
        panel_subtitle.as_str(),
        footer_primary.as_str(),
        footer_secondary.as_str(),
    );

    let previous = BOOTSTRAP_FRAME_CACHE.lock().clone();
    let (selected_session, hovered, focused) = current_gui_object_state();
    let layout_changed = previous
        .as_ref()
        .map(|cache| cache.layout_signature != layout_signature)
        .unwrap_or(true);
    let interaction_changed = previous
        .as_ref()
        .map(|cache| {
            cache.selected_session != selected_session
                || cache.hovered != hovered
                || cache.focused != focused
        })
        .unwrap_or(true);

    restore_pointer_overlay(surface);

    if layout_changed {
        draw_bootstrap_static_layout(
            surface,
            active,
            requested,
            target,
            validation,
            gui_phase,
            heading.as_str(),
            subheading.as_str(),
            panel_title.as_str(),
            panel_subtitle.as_str(),
            footer_primary.as_str(),
            footer_secondary.as_str(),
            snapshot.header_line.as_str(),
        );
    } else if previous
        .as_ref()
        .map(|cache| cache.header_line != snapshot.header_line)
        .unwrap_or(true)
    {
        redraw_header_line(
            surface,
            active,
            requested,
            target,
            validation,
            gui_phase,
            heading.as_str(),
            subheading.as_str(),
            snapshot.header_line.as_str(),
        );
    }

    if layout_changed
        || previous
            .as_ref()
            .map(|cache| cache.status_line != snapshot.status_line)
            .unwrap_or(true)
    {
        redraw_status_bar(surface, target, snapshot.status_line.as_str());
    }

    if layout_changed
        || previous
            .as_ref()
            .map(|cache| cache.log_lines != snapshot.log_lines)
            .unwrap_or(true)
        || (matches!(target, DisplaySessionTarget::GuiSession)
            && previous
                .as_ref()
                .map(|cache| cache.input_line != snapshot.input_line)
                .unwrap_or(true))
    {
        redraw_log_area(surface, &snapshot.log_lines);
    }

    if layout_changed
        || previous
            .as_ref()
            .map(|cache| cache.input_line != snapshot.input_line)
            .unwrap_or(true)
    {
        redraw_input_line(surface, snapshot.input_line.as_str());
    }

    if matches!(target, DisplaySessionTarget::GuiSession) && interaction_changed && !layout_changed {
        redraw_gui_interaction_delta(
            surface,
            previous
                .as_ref()
                .map(|cache| (cache.selected_session, cache.hovered, cache.focused)),
            (selected_session, hovered, focused),
            snapshot.log_lines.as_slice(),
            snapshot.input_line.as_str(),
        );
    }

    draw_pointer_overlay(surface);

    *BOOTSTRAP_FRAME_CACHE.lock() = Some(BootstrapFrameCache {
        layout_signature,
        status_line: snapshot.status_line.clone(),
        header_line: snapshot.header_line.clone(),
        input_line: snapshot.input_line.clone(),
        log_lines: snapshot.log_lines.clone(),
        selected_session,
        hovered,
        focused,
    });
}

fn compute_layout_signature(
    active: DisplayModeInfo,
    requested: DisplayModeInfo,
    target: DisplaySessionTarget,
    validation: DisplayValidationState,
    gui_phase: GuiSessionPhase,
    heading: &str,
    subheading: &str,
    panel_title: &str,
    panel_subtitle: &str,
    footer_primary: &str,
    footer_secondary: &str,
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for value in [
        active.backend as u64,
        active.text_cols as u64,
        active.text_rows as u64,
        active.pixel_width as u64,
        active.pixel_height as u64,
        requested.backend as u64,
        requested.text_cols as u64,
        requested.text_rows as u64,
        requested.pixel_width as u64,
        requested.pixel_height as u64,
        target as u64,
        validation as u64,
        gui_phase as u64,
    ] {
        hash ^= value;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for text in [
        heading,
        subheading,
        panel_title,
        panel_subtitle,
        footer_primary,
        footer_secondary,
    ] {
        for &byte in text.as_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn redraw_gui_interaction_delta(
    surface: BootstrapSurfaceState,
    previous: Option<(GuiObjectId, Option<GuiObjectId>, Option<GuiObjectId>)>,
    current: (GuiObjectId, Option<GuiObjectId>, Option<GuiObjectId>),
    log_lines: &[(String, u8)],
    input_line: &str,
) {
    let scene = build_gui_scene(surface.width, surface.height, log_lines, input_line, "", "");
    let (previous_selected, previous_hovered, previous_focused) =
        previous.unwrap_or((current.0, None, None));
    let (current_selected, current_hovered, current_focused) = current;

    let mut ids = Vec::new();
    push_unique_gui_id(&mut ids, previous_selected);
    push_unique_gui_id(&mut ids, current_selected);
    if let Some(id) = previous_hovered {
        push_unique_gui_id(&mut ids, id);
    }
    if let Some(id) = current_hovered {
        push_unique_gui_id(&mut ids, id);
    }
    if let Some(id) = previous_focused {
        push_unique_gui_id(&mut ids, id);
    }
    if let Some(id) = current_focused {
        push_unique_gui_id(&mut ids, id);
    }

    let selected_changed = previous_selected != current_selected;
    let composer_changed =
        previous_focused == Some(GuiObjectId::Composer)
            || current_focused == Some(GuiObjectId::Composer)
            || previous_hovered == Some(GuiObjectId::Composer)
            || current_hovered == Some(GuiObjectId::Composer);
    let conversation_changed =
        selected_changed
            || previous_focused == Some(GuiObjectId::Conversation)
            || current_focused == Some(GuiObjectId::Conversation)
            || previous_hovered == Some(GuiObjectId::Conversation)
            || current_hovered == Some(GuiObjectId::Conversation);

    for id in ids {
        if let Some(node) = scene
            .nodes
            .iter()
            .find(|node| matches!(node.kind, GuiNodeKind::SessionItem) && node.handle == gui_handle(session_handle_key(id)))
        {
            draw_sidebar_item(surface, node);
        }
    }

    if conversation_changed {
        redraw_gui_chat_area(surface, log_lines);
    }
    if composer_changed {
        redraw_gui_input_composer(surface, input_line);
    }
    if selected_changed {
        draw_gui_footer(surface, "", "");
    }
}

fn push_unique_gui_id(target: &mut Vec<GuiObjectId>, id: GuiObjectId) {
    if target.iter().all(|existing| *existing != id) {
        target.push(id);
    }
}

fn current_gui_object_state() -> (GuiObjectId, Option<GuiObjectId>, Option<GuiObjectId>) {
    let state = GUI_OBJECTS.lock();
    (state.selected_session, state.hovered, state.focused)
}

fn redraw_status_bar(surface: BootstrapSurfaceState, target: DisplaySessionTarget, status_line: &str) {
    let display_line = compose_status_line(target, status_line);
    fill_rect(
        surface,
        0,
        0,
        surface.width,
        40,
        if matches!(target, DisplaySessionTarget::GuiSession) {
            0x183a6b
        } else {
            0x0f2e0f
        },
    );
    draw_text(surface, 8, 8, display_line.as_str(), 0xF5F5F5, 1);
}

fn redraw_header_line(
    surface: BootstrapSurfaceState,
    active: DisplayModeInfo,
    requested: DisplayModeInfo,
    target: DisplaySessionTarget,
    validation: DisplayValidationState,
    gui_phase: GuiSessionPhase,
    heading: &str,
    subheading: &str,
    header_line: &str,
) {
    if matches!(target, DisplaySessionTarget::GuiSession) {
        fill_rect(surface, 0, 40, surface.width, 16, 0x0D1321);
        return;
    }

    fill_rect(surface, 0, 40, surface.width, 110, 0x0D1321);
    draw_text(
        surface,
        8,
        52,
        if heading.is_empty() {
            "OpenRhiza Sandbox Display Session"
        } else {
            heading
        },
        0x6ee7ff,
        2,
    );
    draw_text(
        surface,
        8,
        88,
        if subheading.is_empty() {
            header_line
        } else {
            subheading
        },
        0xffd54f,
        1,
    );
    draw_text(
        surface,
        8,
        116,
        format!("active={}  requested={}", describe_mode(active), describe_mode(requested)).as_str(),
        0xb8ffb8,
        1,
    );
    draw_text(
        surface,
        8,
        132,
        format!(
            "target={}  validation={}  gui={}",
            session_target_name(target),
            validation_state_name(validation),
            gui_phase_name(gui_phase)
        )
        .as_str(),
        0xb8ffb8,
        1,
    );
}

fn redraw_log_area(surface: BootstrapSurfaceState, log_lines: &[(String, u8)]) {
    if matches!(session_target(), DisplaySessionTarget::GuiSession) {
        redraw_gui_chat_area(surface, log_lines);
    } else {
        fill_rect(surface, 40, 300, surface.width.saturating_sub(80), surface.height.saturating_sub(380), 0x000000);
        let mut y = 300usize;
        for (line, color) in log_lines {
            if y + 18 >= surface.height.saturating_sub(40) {
                break;
            }
            draw_text(surface, 40, y, line.as_str(), vga_color_to_rgb(*color), 1);
            y += 16;
        }
    }
}

fn redraw_input_line(surface: BootstrapSurfaceState, input_line: &str) {
    if matches!(session_target(), DisplaySessionTarget::GuiSession) {
        redraw_gui_input_composer(surface, input_line);
    } else {
        fill_rect(surface, 40, surface.height.saturating_sub(40), surface.width.saturating_sub(80), 24, 0x0D1321);
        draw_text(surface, 40, surface.height.saturating_sub(36), input_line, 0x79ff79, 1);
    }
}

fn restore_pointer_overlay(surface: BootstrapSurfaceState) {
    let mut overlay = POINTER_OVERLAY.lock();
    if !overlay.visible {
        return;
    }
    for row in 0..POINTER_HEIGHT {
        for col in 0..POINTER_WIDTH {
            write_pixel(
                surface,
                overlay.x + col,
                overlay.y + row,
                overlay.saved[row * POINTER_WIDTH + col],
            );
        }
    }
    overlay.visible = false;
}

fn draw_pointer_overlay(surface: BootstrapSurfaceState) {
    let pointer = {
        let mut pointer = BOOTSTRAP_POINTER.lock();
        if !pointer.initialized {
            pointer.initialized = true;
            pointer.x = surface.width.saturating_div(2).saturating_sub(POINTER_WIDTH / 2);
            pointer.y = surface.height.saturating_div(2).saturating_sub(POINTER_HEIGHT / 2);
        }
        *pointer
    };

    let fill_color = if (pointer.buttons & 0x01) != 0 { 0xffecec } else { 0xfafafa };
    let outline_color = 0x050505;
    let shadow_color = 0x2c2c2c;

    const POINTER_ROWS: [&str; 16] = [
        "#...........",
        "##..........",
        "#w#.........",
        "#ww#........",
        "#www#.......",
        "#wwww#......",
        "#wwwww#.....",
        "#wwwwww#....",
        "#ww#####....",
        "#w#.#.......",
        "##..#.......",
        "#....#......",
        ".....#......",
        "............",
        "............",
        "............",
    ];

    let mut overlay = POINTER_OVERLAY.lock();
    overlay.x = pointer.x.min(surface.width.saturating_sub(POINTER_WIDTH));
    overlay.y = pointer.y.min(surface.height.saturating_sub(POINTER_HEIGHT));
    for row in 0..POINTER_HEIGHT {
        for col in 0..POINTER_WIDTH {
            overlay.saved[row * POINTER_WIDTH + col] = read_pixel(surface, overlay.x + col, overlay.y + row);
        }
    }

    for (row_idx, row_bits) in POINTER_ROWS.iter().enumerate() {
        for (col_idx, bit) in row_bits.as_bytes().iter().enumerate() {
            if *bit == b'.' {
                continue;
            }
            fill_rect(
                surface,
                overlay.x + 4 + col_idx * POINTER_SCALE,
                overlay.y + 4 + row_idx * POINTER_SCALE,
                POINTER_SCALE,
                POINTER_SCALE,
                shadow_color,
            );
        }
    }

    for (row_idx, row_bits) in POINTER_ROWS.iter().enumerate() {
        for (col_idx, bit) in row_bits.as_bytes().iter().enumerate() {
            let color = match *bit {
                b'#' => outline_color,
                b'w' => fill_color,
                _ => continue,
            };
            fill_rect(
                surface,
                overlay.x + col_idx * POINTER_SCALE,
                overlay.y + row_idx * POINTER_SCALE,
                POINTER_SCALE,
                POINTER_SCALE,
                color,
            );
        }
    }

    overlay.visible = true;
}

fn draw_bootstrap_static_layout(
    surface: BootstrapSurfaceState,
    active: DisplayModeInfo,
    requested: DisplayModeInfo,
    target: DisplaySessionTarget,
    validation: DisplayValidationState,
    gui_phase: GuiSessionPhase,
    heading: &str,
    subheading: &str,
    panel_title: &str,
    panel_subtitle: &str,
    footer_primary: &str,
    footer_secondary: &str,
    header_line: &str,
) {
    clear_framebuffer(
        surface,
        if matches!(target, DisplaySessionTarget::GuiSession) {
            0x0D1321
        } else {
            0x050505
        },
    );
    redraw_status_bar(surface, target, "");
    redraw_header_line(
        surface,
        active,
        requested,
        target,
        validation,
        gui_phase,
        heading,
        subheading,
        header_line,
    );

    if matches!(target, DisplaySessionTarget::GuiSession) {
        draw_gui_static_layout(surface, panel_title, panel_subtitle);
    } else {
        fill_rect(
            surface,
            24,
            180,
            surface.width.saturating_sub(48),
            surface.height.saturating_sub(220),
            0x000000,
        );
        draw_text(
            surface,
            56,
            212,
            if panel_title.is_empty() {
                "Wide console bootstrap presenter"
            } else {
                panel_title
            },
            0xffffff,
            2,
        );
        draw_text(
            surface,
            56,
            242,
            if panel_subtitle.is_empty() {
                "Sandbox display skill owns this 1920x1080 recovery-compatible console surface."
            } else {
                panel_subtitle
            },
            0xffd54f,
            1,
        );
    }

    if matches!(target, DisplaySessionTarget::GuiSession) {
        draw_gui_footer(surface, footer_primary, footer_secondary);
    } else {
        fill_rect(surface, 40, surface.height.saturating_sub(82), surface.width.saturating_sub(80), 46, 0x0D1321);
        if !footer_primary.is_empty() {
            draw_text(surface, 40, surface.height.saturating_sub(76), footer_primary, 0xb8ffb8, 1);
        }
        if !footer_secondary.is_empty() {
            draw_text(surface, 40, surface.height.saturating_sub(56), footer_secondary, 0xb8ffb8, 1);
        }
    }
}

fn sync_gui_objects(width: usize, height: usize, input_line: &str) {
    let layout = gui_layout(width, height, input_line);
    let mut objects = GUI_OBJECTS.lock();
    objects.clear();
    objects.push(GuiObject {
        id: GuiObjectId::SessionOpenRhiza,
        kind: GuiObjectKind::SessionItem,
        rect: GuiRect { x: 20, y: layout.content_top + 72, width: 220, height: 34 },
    });
    objects.push(GuiObject {
        id: GuiObjectId::SessionSandboxGui,
        kind: GuiObjectKind::SessionItem,
        rect: GuiRect { x: 20, y: layout.content_top + 116, width: 220, height: 34 },
    });
    objects.push(GuiObject {
        id: GuiObjectId::SessionWideConsole,
        kind: GuiObjectKind::SessionItem,
        rect: GuiRect { x: 20, y: layout.content_top + 160, width: 220, height: 34 },
    });
    objects.push(GuiObject {
        id: GuiObjectId::SessionRecoveryShell,
        kind: GuiObjectKind::SessionItem,
        rect: GuiRect { x: 20, y: layout.content_top + 204, width: 220, height: 34 },
    });
    objects.push(GuiObject {
        id: GuiObjectId::Conversation,
        kind: GuiObjectKind::Conversation,
        rect: GuiRect {
            x: layout.chat_x,
            y: layout.chat_y,
            width: layout.chat_w,
            height: layout.chat_h,
        },
    });
    objects.push(GuiObject {
        id: GuiObjectId::Composer,
        kind: GuiObjectKind::Composer,
        rect: GuiRect {
            x: layout.composer_x,
            y: layout.composer_y,
            width: layout.composer_w,
            height: layout.composer_h,
        },
    });
}

#[derive(Clone, Copy)]
struct GuiLayout {
    content_top: usize,
    main_x: usize,
    main_width: usize,
    chat_x: usize,
    chat_y: usize,
    chat_w: usize,
    chat_h: usize,
    composer_x: usize,
    composer_y: usize,
    composer_w: usize,
    composer_h: usize,
    footer_x: usize,
    footer_y: usize,
    footer_w: usize,
    footer_h: usize,
}

#[derive(Clone)]
struct GuiMessageSpec {
    is_user: bool,
    text: String,
    style: GuiStyleClass,
}

fn gui_layout(width: usize, height: usize, input_line: &str) -> GuiLayout {
    let sidebar_width = 260usize;
    let content_top = 64usize;
    let main_x = sidebar_width + 24;
    let main_width = width.saturating_sub(main_x + 24);
    let composer_x = 304usize;
    let composer_w = width.saturating_sub(composer_x + 24);
    let composer_h = gui_composer_height_for_width(composer_w, input_line);
    let footer_h = 28usize;
    let footer_y = height.saturating_sub(footer_h + 18);
    let composer_y = footer_y.saturating_sub(composer_h + 20);
    let chat_x = main_x;
    let chat_y = 92usize;
    let chat_w = width.saturating_sub(main_x + 24);
    let chat_h = composer_y.saturating_sub(chat_y + 16);
    let footer_x = sidebar_width + 44;
    let footer_w = width.saturating_sub(footer_x + 24);
    GuiLayout {
        content_top,
        main_x,
        main_width,
        chat_x,
        chat_y,
        chat_w,
        chat_h,
        composer_x,
        composer_y,
        composer_w,
        composer_h,
        footer_x,
        footer_y,
        footer_w,
        footer_h,
    }
}

fn draw_gui_static_layout(surface: BootstrapSurfaceState, _panel_title: &str, _panel_subtitle: &str) {
    sync_gui_objects(surface.width, surface.height, "input> ");
    let layout = gui_layout(surface.width, surface.height, "input> ");
    let scene = build_gui_scene(surface.width, surface.height, &[], "input> ", "", "");
    let sidebar_width = 260usize;
    let content_bottom = surface.height.saturating_sub(24);

    fill_rect(surface, 0, layout.content_top, sidebar_width, surface.height.saturating_sub(layout.content_top), 0x171717);
    fill_rect(surface, sidebar_width, layout.content_top, 2, surface.height.saturating_sub(layout.content_top), 0x2b2b2b);
    draw_text(surface, 20, layout.content_top + 8, "Sessions", 0xe8e8e8, 1);
    for node in scene.nodes.iter().filter(|node| matches!(node.kind, GuiNodeKind::SessionItem)) {
        draw_sidebar_item(surface, node);
    }

    fill_rect(surface, layout.main_x, layout.content_top, layout.main_width, content_bottom.saturating_sub(layout.content_top), 0x141414);
}

fn draw_sidebar_item(surface: BootstrapSurfaceState, node: &GuiNode) {
    let bg = match node.interaction {
        GuiInteractionState::Active => 0x2f3a4e,
        GuiInteractionState::Focused => 0x263042,
        GuiInteractionState::Hovered => 0x232323,
        _ => 0x171717,
    };
    fill_rect(
        surface,
        node.bounds.x,
        node.bounds.y,
        node.bounds.width,
        node.bounds.height,
        bg,
    );
    draw_text(
        surface,
        node.bounds.x + 12,
        node.bounds.y + 9,
        node.label.as_str(),
        if matches!(node.interaction, GuiInteractionState::Active | GuiInteractionState::Focused) {
            0xffffff
        } else {
            0xb0b0b0
        },
        1,
    );
}

fn draw_gui_footer(surface: BootstrapSurfaceState, footer_primary: &str, footer_secondary: &str) {
    let scene = build_gui_scene(surface.width, surface.height, &[], "input> ", footer_primary, footer_secondary);
    let Some(footer) = scene.nodes.iter().find(|node| matches!(node.kind, GuiNodeKind::Footer)) else {
        return;
    };
    fill_rect(surface, footer.bounds.x, footer.bounds.y, footer.bounds.width, footer.bounds.height, 0x111111);
    let runtime = *GUI_SCENE_RUNTIME.lock();
    let primary_source = if !footer_primary.is_empty() {
        String::from(footer_primary)
    } else if runtime.conversation_scroll_items > 0 {
        format!(
            "Scrolled {} item(s) up  |  PgUp/PgDn or wheel on conversation",
            runtime.conversation_scroll_items
        )
    } else if is_gui_conversation_focused() {
        String::from("Conversation focused  |  PgUp/PgDn or wheel to scroll")
    } else {
        String::from("Ready")
    };
    let primary = truncate_text_chars(
        primary_source.as_str(),
        footer.bounds.width.saturating_sub(24) / crate::gui_font::CHAR_ADVANCE,
    );
    draw_text(surface, footer.bounds.x + 12, footer.bounds.y + 7, primary.as_str(), 0x9cc5ff, 1);
}

fn redraw_gui_chat_area(surface: BootstrapSurfaceState, log_lines: &[(String, u8)]) {
    let scene = build_gui_scene(surface.width, surface.height, log_lines, "input> ", "", "");
    let Some(chat_object) = scene
        .nodes
        .iter()
        .find(|node| matches!(node.kind, GuiNodeKind::Conversation))
    else {
        return;
    };
    let chat_x = chat_object.bounds.x;
    let chat_y = chat_object.bounds.y;
    let chat_w = chat_object.bounds.width;
    let chat_h = chat_object.bounds.height;
    let scroll_skip = GUI_SCENE_RUNTIME.lock().conversation_scroll_items;
    let chat_bg = if matches!(chat_object.interaction, GuiInteractionState::Focused) {
        0x16181c
    } else {
        0x141414
    };
    fill_rect(surface, chat_x, chat_y, chat_w, chat_h, chat_bg);

    let message_nodes: Vec<&GuiNode> = scene
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, GuiNodeKind::Message | GuiNodeKind::Label))
        .collect();
    if message_nodes.is_empty() {
        draw_text(
            surface,
            chat_x + 20,
            chat_y + 24,
            "OpenRhiza GUI bootstrap is active. Type into the composer to start a chat session.",
            0xd4d4d4,
            1,
        );
        return;
    }

    let total_messages = session_message_specs(current_gui_object_state().0, log_lines).len();
    let visible_messages = message_nodes
        .iter()
        .filter(|node| matches!(node.kind, GuiNodeKind::Message))
        .count();

    for node in message_nodes {
        match node.style {
            GuiStyleClass::UserMessage => {
                fill_rect(
                    surface,
                    node.bounds.x,
                    node.bounds.y,
                    node.bounds.width,
                    node.bounds.height,
                    0x1c3323,
                );
                draw_wrapped_text_in_rect(surface, node, 0xe8fff0, 12, 10);
            }
            GuiStyleClass::AssistantMessage | GuiStyleClass::PlainText | GuiStyleClass::AccentText => {
                let color = if matches!(node.style, GuiStyleClass::AccentText) {
                    0xffd54f
                } else {
                    0xe6e6e6
                };
                draw_wrapped_text_in_rect(surface, node, color, 0, 4);
            }
            _ => {}
        }
    }

    draw_gui_conversation_scrollbar(
        surface,
        chat_x,
        chat_y,
        chat_w,
        chat_h,
        total_messages,
        visible_messages,
        scroll_skip,
    );
}

fn draw_gui_conversation_scrollbar(
    surface: BootstrapSurfaceState,
    chat_x: usize,
    chat_y: usize,
    chat_w: usize,
    chat_h: usize,
    total_messages: usize,
    visible_messages: usize,
    scroll_skip: usize,
) {
    if total_messages <= visible_messages || visible_messages == 0 {
        return;
    }

    let track_x = chat_x + chat_w.saturating_sub(10);
    let track_y = chat_y + 12;
    let track_h = chat_h.saturating_sub(24);
    if track_h < 24 {
        return;
    }

    fill_rect(surface, track_x, track_y, 4, track_h, 0x232323);

    let max_scroll = total_messages.saturating_sub(visible_messages).max(1);
    let thumb_h = ((visible_messages * track_h) / total_messages).clamp(24, track_h);
    let travel = track_h.saturating_sub(thumb_h);
    let thumb_offset = (scroll_skip.min(max_scroll) * travel) / max_scroll;
    let thumb_y = track_y + travel.saturating_sub(thumb_offset);

    fill_rect(surface, track_x, thumb_y, 4, thumb_h, 0x5f6f87);
}

fn redraw_gui_input_composer(surface: BootstrapSurfaceState, input_line: &str) {
    let scene = build_gui_scene(surface.width, surface.height, &[], input_line, "", "");
    let Some(composer) = scene
        .nodes
        .iter()
        .find(|node| matches!(node.kind, GuiNodeKind::Composer))
    else {
        return;
    };
    let composer_x = composer.bounds.x;
    let composer_w = composer.bounds.width;
    let composer_h = gui_composer_height(surface, input_line);
    let composer_y = composer.bounds.y;
    fill_rect(surface, composer_x, composer_y, composer_w, composer_h, 0x1c1c1c);
    fill_rect(
        surface,
        composer_x,
        composer_y,
        composer_w,
        26,
        if matches!(composer.interaction, GuiInteractionState::Focused) { 0x2c3b52 } else { 0x242424 },
    );
    let message = scene
        .nodes
        .iter()
        .find(|node| matches!(node.kind, GuiNodeKind::TextInput))
        .map(|node| node.label.as_str())
        .unwrap_or("");
        let wrapped = wrap_text(message, composer_w.saturating_sub(28) / crate::gui_font::CHAR_ADVANCE);
    let mut y = composer_y + 34;
    if message.is_empty() {
        draw_text(
            surface,
            composer_x + 12,
            y,
            "Type a prompt. The composer expands as your message wraps.",
            0x9a9a9a,
            1,
        );
        if matches!(composer.interaction, GuiInteractionState::Focused) && gui_caret_visible() {
            draw_gui_composer_caret(surface, composer_x + 12, composer_y + 34);
        }
    } else {
        let mut last_line_x = composer_x + 12;
        let mut last_line_y = y;
        let mut last_line_len = 0usize;
        for line in wrapped.iter().take(6) {
            draw_text(surface, composer_x + 12, y, line.as_str(), 0x79ff79, 1);
            last_line_x = composer_x + 12;
            last_line_y = y;
            last_line_len = crate::gui_font::text_pixel_advance(line.as_str());
            y += 16;
        }
        if matches!(composer.interaction, GuiInteractionState::Focused) && gui_caret_visible() {
            let caret_x = last_line_x + last_line_len + 2;
            draw_gui_composer_caret(surface, caret_x, last_line_y);
        }
    }
}

fn draw_gui_composer_caret(surface: BootstrapSurfaceState, x: usize, y: usize) {
    let caret_h = crate::gui_font::LINE_HEIGHT.saturating_sub(2);
    fill_rect(surface, x, y + 1, 2, caret_h, 0xe8f0ff);
}

fn gui_caret_visible() -> bool {
    let ticks = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
    let phase = (ticks / (crate::task::timer::TICKS_PER_SECOND / 2).max(1)) % 2;
    phase == 0
}

fn is_gui_composer_focused() -> bool {
    let state = GUI_OBJECTS.lock();
    matches!(state.focused, Some(GuiObjectId::Composer)) && matches!(session_target(), DisplaySessionTarget::GuiSession)
}

pub fn is_gui_conversation_focused() -> bool {
    let state = GUI_OBJECTS.lock();
    matches!(state.focused, Some(GuiObjectId::Conversation))
        && matches!(session_target(), DisplaySessionTarget::GuiSession)
}

fn gui_composer_height(surface: BootstrapSurfaceState, input_line: &str) -> usize {
    let composer_w = surface.width.saturating_sub(304 + 24);
    gui_composer_height_for_width(composer_w, input_line)
}

fn gui_composer_height_for_width(composer_w: usize, input_line: &str) -> usize {
    let line_count = wrap_text(
        composer_message_text(input_line),
        composer_w.saturating_sub(28) / crate::gui_font::CHAR_ADVANCE,
    ).len().clamp(1, 6);
    let height = 44 + line_count * crate::gui_font::LINE_HEIGHT + 12;
    let mut runtime = GUI_SCENE_RUNTIME.lock();
    runtime.composer_rows = line_count;
    runtime.composer_height = height;
    height
}

fn compose_status_line(target: DisplaySessionTarget, fallback: &str) -> String {
    let ticks = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
    let total_seconds = ticks / crate::task::timer::TICKS_PER_SECOND;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if matches!(target, DisplaySessionTarget::GuiSession) {
        return format!(
            "OpenRhiza sandbox session  |  running {:02}:{:02}:{:02}  |  1920x1080 gui",
            hours,
            minutes,
            seconds
        );
    }

    String::from(fallback)
}

fn composer_message_text(input_line: &str) -> &str {
    input_line.strip_prefix("input> ").unwrap_or(input_line)
}

fn build_gui_scene(
    width: usize,
    height: usize,
    log_lines: &[(String, u8)],
    input_line: &str,
    footer_primary: &str,
    _footer_secondary: &str,
) -> GuiScene {
    sync_gui_objects(width, height, input_line);
    let state = *GUI_OBJECTS.lock();
    let layout = gui_layout(width, height, input_line);
    let selected = state.selected_session;
    let backend_preference = if matches!(selected, GuiObjectId::SessionSandboxGui) {
        GuiBackendPreference::LvglStyle
    } else {
        GuiBackendPreference::NativeObject
    };
    let messages = session_message_specs(selected, log_lines);

    let mut nodes = Vec::new();
    nodes.push(GuiNode {
        handle: gui_handle(1),
        parent: None,
        kind: GuiNodeKind::Root,
        style: GuiStyleClass::Chrome,
        interaction: GuiInteractionState::Idle,
        bounds: ContractRect { x: 0, y: 40, width, height: height.saturating_sub(40) },
        object_ref: None,
        label: String::from("OpenRhizaRoot"),
    });
    nodes.push(GuiNode {
        handle: gui_handle(2),
        parent: Some(gui_handle(1)),
        kind: GuiNodeKind::Sidebar,
        style: GuiStyleClass::Sidebar,
        interaction: GuiInteractionState::Idle,
        bounds: ContractRect {
            x: 0,
            y: layout.content_top,
            width: 260,
            height: height.saturating_sub(layout.content_top),
        },
        object_ref: Some(String::from("gui.sidebar")),
        label: String::from("Sessions"),
    });
    nodes.push(GuiNode {
        handle: gui_handle(3),
        parent: Some(gui_handle(2)),
        kind: GuiNodeKind::SessionList,
        style: GuiStyleClass::Sidebar,
        interaction: GuiInteractionState::Idle,
        bounds: ContractRect {
            x: 20,
            y: layout.content_top + 66,
            width: 220,
            height: 188,
        },
        object_ref: Some(String::from("session.list")),
        label: String::from("Sessions"),
    });
    append_session_item_node(&mut nodes, &state, GuiObjectId::SessionOpenRhiza, "OpenRhiza", "session:openrhiza");
    append_session_item_node(&mut nodes, &state, GuiObjectId::SessionSandboxGui, "Sandbox GUI", "session:sandbox-gui");
    append_session_item_node(&mut nodes, &state, GuiObjectId::SessionWideConsole, "Wide Console", "session:wide-console");
    append_session_item_node(&mut nodes, &state, GuiObjectId::SessionRecoveryShell, "Recovery Shell", "session:recovery-shell");

    nodes.push(GuiNode {
        handle: gui_handle(20),
        parent: Some(gui_handle(1)),
        kind: GuiNodeKind::Conversation,
        style: GuiStyleClass::ConversationSurface,
        interaction: gui_interaction_for_id(&state, GuiObjectId::Conversation),
        bounds: ContractRect {
            x: layout.chat_x,
            y: layout.chat_y,
            width: layout.chat_w,
            height: layout.chat_h,
        },
        object_ref: Some(String::from(selected_session_ref(selected))),
        label: String::from(selected_session_caption()),
    });
    append_message_nodes(
        &mut nodes,
        gui_handle(20),
        layout.chat_x,
        layout.chat_y,
        layout.chat_w,
        layout.chat_h,
        &messages,
    );

    nodes.push(GuiNode {
        handle: gui_handle(30),
        parent: Some(gui_handle(1)),
        kind: GuiNodeKind::Composer,
        style: GuiStyleClass::ComposerSurface,
        interaction: gui_interaction_for_id(&state, GuiObjectId::Composer),
        bounds: ContractRect {
            x: layout.composer_x,
            y: layout.composer_y,
            width: layout.composer_w,
            height: layout.composer_h,
        },
        object_ref: Some(String::from("composer:primary")),
        label: String::new(),
    });
    nodes.push(GuiNode {
        handle: gui_handle(31),
        parent: Some(gui_handle(30)),
        kind: GuiNodeKind::TextInput,
        style: GuiStyleClass::ComposerSurface,
        interaction: gui_interaction_for_id(&state, GuiObjectId::Composer),
        bounds: ContractRect {
            x: layout.composer_x + 12,
            y: layout.composer_y + 34,
            width: layout.composer_w.saturating_sub(24),
            height: layout.composer_h.saturating_sub(40),
        },
        object_ref: Some(String::from("composer.text")),
        label: String::from(composer_message_text(input_line)),
    });
    nodes.push(GuiNode {
        handle: gui_handle(40),
        parent: Some(gui_handle(1)),
        kind: GuiNodeKind::Footer,
        style: GuiStyleClass::FooterSurface,
        interaction: GuiInteractionState::Idle,
        bounds: ContractRect {
            x: layout.footer_x,
            y: layout.footer_y,
            width: layout.footer_w,
            height: layout.footer_h,
        },
        object_ref: Some(String::from("footer.status")),
        label: if !footer_primary.is_empty() {
            String::from(footer_primary)
        } else {
            String::from(selected_session_caption())
        },
    });

    apply_gui_mutations(&mut nodes);

    GuiScene {
        scene_id: format!("bootstrap:{}:{}", selected_session_ref(selected), session_target_name(session_target())),
        backend_preference,
        nodes,
    }
}

fn apply_gui_mutations(nodes: &mut [GuiNode]) {
    let mutations = GUI_MUTATIONS.lock().clone();
    if mutations.is_empty() {
        return;
    }

    for mutation in mutations.iter() {
        if let Some(node) = nodes.iter_mut().find(|node| node.handle == mutation.target) {
            if let Some(bounds) = mutation.new_bounds {
                node.bounds = bounds;
            }
            if let Some(style) = mutation.new_style {
                node.style = style;
            }
            if let Some(interaction) = mutation.new_interaction {
                node.interaction = interaction;
            }
            if let Some(label) = mutation.new_label.as_ref() {
                node.label = label.clone();
            }
        }
    }
}

fn append_session_item_node(
    nodes: &mut Vec<GuiNode>,
    state: &GuiObjectRuntime,
    id: GuiObjectId,
    label: &str,
    object_ref: &str,
) {
    let Some(object) = state.object(id) else {
        return;
    };
    nodes.push(GuiNode {
        handle: gui_handle(session_handle_key(id)),
        parent: Some(gui_handle(3)),
        kind: GuiNodeKind::SessionItem,
        style: sidebar_style_for_id(state, id),
        interaction: gui_interaction_for_id(state, id),
        bounds: ContractRect {
            x: object.rect.x,
            y: object.rect.y,
            width: object.rect.width,
            height: object.rect.height,
        },
        object_ref: Some(String::from(object_ref)),
        label: String::from(label),
    });
}

fn append_message_nodes(
    nodes: &mut Vec<GuiNode>,
    parent: GuiObjectHandle,
    chat_x: usize,
    chat_y: usize,
    chat_w: usize,
    chat_h: usize,
    messages: &[GuiMessageSpec],
) {
    if messages.is_empty() {
        nodes.push(GuiNode {
            handle: gui_handle(100),
            parent: Some(parent),
            kind: GuiNodeKind::Label,
            style: GuiStyleClass::PlainText,
            interaction: GuiInteractionState::Idle,
            bounds: ContractRect {
                x: chat_x + 20,
                y: chat_y + 24,
                width: chat_w.saturating_sub(40),
                height: 24,
            },
            object_ref: Some(String::from("chat.empty")),
            label: String::from("OpenRhiza GUI bootstrap is active. Type into the composer to start a chat session."),
        });
        return;
    }

    let mut rendered = Vec::new();
    let mut used_height = 0usize;
    let scroll_skip = GUI_SCENE_RUNTIME.lock().conversation_scroll_items;
    for spec in messages.iter().rev().skip(scroll_skip) {
        let max_cells = if spec.is_user { 84 } else { 132 };
        let mut wrapped = wrap_text(spec.text.as_str(), max_cells);
        let mut visible_lines = wrapped.len().max(1);
        if visible_lines > GUI_MESSAGE_RENDER_LINE_LIMIT {
            wrapped.truncate(GUI_MESSAGE_RENDER_LINE_LIMIT);
            if let Some(last_line) = wrapped.last_mut() {
                let mut truncated = truncate_text_chars(last_line.as_str(), max_cells.saturating_sub(1));
                if !truncated.ends_with('…') {
                    truncated.push('…');
                }
                *last_line = truncated;
            }
            visible_lines = GUI_MESSAGE_RENDER_LINE_LIMIT;
        }
        let bubble_h = visible_lines * crate::gui_font::LINE_HEIGHT + if spec.is_user { 18 } else { 10 };
        if used_height + bubble_h + 12 > chat_h.saturating_sub(16) {
            break;
        }
        let max_line_len = wrapped
            .iter()
            .map(|line| crate::gui_font::text_display_cells(line.as_str()))
            .max()
            .unwrap_or(0);
        let natural_w = max_line_len
            .saturating_mul(crate::gui_font::CHAR_ADVANCE)
            .saturating_add(28);
        let bubble_w = if spec.is_user {
            natural_w.clamp(240, chat_w.saturating_sub(120))
        } else {
            chat_w.saturating_sub(24)
        };
        let mut rendered_spec = spec.clone();
        rendered_spec.text = wrapped.join("\n");
        rendered.push((rendered_spec, bubble_w, bubble_h));
        used_height += bubble_h + 12;
    }
    rendered.reverse();

    let mut y = chat_y + chat_h.saturating_sub(used_height + 12);
    for (index, (spec, bubble_w, bubble_h)) in rendered.into_iter().enumerate() {
        let bubble_x = if spec.is_user { chat_x + 48 } else { chat_x + 12 };
        nodes.push(GuiNode {
            handle: gui_handle(200 + index as u64),
            parent: Some(parent),
            kind: GuiNodeKind::Message,
            style: spec.style,
            interaction: GuiInteractionState::Idle,
            bounds: ContractRect {
                x: bubble_x,
                y,
                width: bubble_w,
                height: bubble_h,
            },
            object_ref: Some(String::from(if spec.is_user { "message:user" } else { "message:assistant" })),
            label: spec.text,
        });
        y += bubble_h + 12;
    }
}

fn session_message_specs(selected: GuiObjectId, log_lines: &[(String, u8)]) -> Vec<GuiMessageSpec> {
    match selected {
        GuiObjectId::SessionOpenRhiza => {
            let history = GUI_CHAT_HISTORY.lock();
            if history.is_empty() {
                collect_gui_chat_messages(log_lines, 14)
                    .into_iter()
                    .map(|(is_user, text, _color)| GuiMessageSpec {
                        is_user,
                        style: if is_user {
                            GuiStyleClass::UserMessage
                        } else {
                            GuiStyleClass::AssistantMessage
                        },
                        text,
                    })
                    .collect()
            } else {
                history
                    .iter()
                    .rev()
                    .take(20)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .map(|message| GuiMessageSpec {
                        is_user: message.is_user,
                        style: message.style,
                        text: message.text,
                    })
                    .collect()
            }
        }
        GuiObjectId::SessionSandboxGui => vec![
            GuiMessageSpec {
                is_user: false,
                style: GuiStyleClass::AccentText,
                text: String::from("LVGL-style bridge is active in parallel with the native object GUI."),
            },
            GuiMessageSpec {
                is_user: false,
                style: GuiStyleClass::AssistantMessage,
                text: String::from("The same OpenRhiza scene contract can be translated into retained widgets such as screen, container, list, button, label, and textarea."),
            },
            GuiMessageSpec {
                is_user: false,
                style: GuiStyleClass::AssistantMessage,
                text: String::from("Next step: let sandbox skills mutate the scene graph directly, then validate and promote object-local redraw behavior."),
            },
        ],
        GuiObjectId::SessionWideConsole => vec![
            GuiMessageSpec {
                is_user: false,
                style: GuiStyleClass::AccentText,
                text: String::from("Wide console session keeps the recovery shell alive while higher-level display skills are validated."),
            },
            GuiMessageSpec {
                is_user: false,
                style: GuiStyleClass::AssistantMessage,
                text: String::from("This path is intended for dense logs, driver workflow inspection, and rollback-safe operator control."),
            },
        ],
        GuiObjectId::SessionRecoveryShell => vec![
            GuiMessageSpec {
                is_user: false,
                style: GuiStyleClass::AccentText,
                text: String::from("Recovery shell is the object-safe fallback. It must survive GUI, skill, and driver failures."),
            },
            GuiMessageSpec {
                is_user: false,
                style: GuiStyleClass::AssistantMessage,
                text: String::from("If a sandbox display object fails, OpenRhiza can roll back here without corrupting unrelated runtime objects."),
            },
        ],
        GuiObjectId::Conversation | GuiObjectId::Composer => Vec::new(),
    }
}

fn collect_gui_chat_messages(log_lines: &[(String, u8)], limit: usize) -> Vec<(bool, String, u8)> {
    let mut messages: Vec<(bool, String, u8)> = Vec::new();
    let mut last_assistant_index: Option<usize> = None;

    for (line, color) in log_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let Some(index) = last_assistant_index {
                if let Some((false, existing, _)) = messages.get_mut(index) {
                    if !existing.ends_with('\n') {
                        existing.push('\n');
                    }
                }
            }
            continue;
        }
        if trimmed.starts_with("[OS Net]")
            || trimmed.starts_with("[HTTPS API]")
            || trimmed.starts_with("[TLS]")
            || trimmed.starts_with("[DNS]")
            || trimmed.starts_with("[API v1]")
        {
            last_assistant_index = None;
            continue;
        }

        if trimmed.starts_with("input> ") {
            let message = composer_message_text(trimmed);
            if !message.is_empty() {
                messages.push((true, String::from(message), *color));
            }
            last_assistant_index = None;
            continue;
        }

        if let Some(message) = trimmed.strip_prefix("[Gemini] ") {
            if message.starts_with("response status:") {
                last_assistant_index = None;
                continue;
            }
            if !message.is_empty() {
                messages.push((false, String::from(message), *color));
                last_assistant_index = Some(messages.len() - 1);
            }
            continue;
        }

        if trimmed.starts_with('[') {
            last_assistant_index = None;
            continue;
        }

        if let Some(index) = last_assistant_index {
            if let Some((false, existing, _)) = messages.get_mut(index) {
                if !existing.is_empty() {
                    existing.push('\n');
                }
                existing.push_str(trimmed);
                continue;
            }
        }
    }

    if messages.len() > limit {
        messages.split_off(messages.len() - limit)
    } else {
        messages
    }
}

fn draw_wrapped_text_in_rect(
    surface: BootstrapSurfaceState,
    node: &GuiNode,
    color: u32,
    inset_x: usize,
    inset_y: usize,
) {
    let max_chars = node
        .bounds
        .width
        .saturating_sub(inset_x.saturating_mul(2))
        / crate::gui_font::CHAR_ADVANCE;
    let wrapped = wrap_text(node.label.as_str(), max_chars);
    let mut y = node.bounds.y + inset_y;
    for line in wrapped.iter().take(GUI_MESSAGE_RENDER_LINE_LIMIT) {
        draw_text(surface, node.bounds.x + inset_x, y, line.as_str(), color, 1);
        y += crate::gui_font::LINE_HEIGHT;
        if y + crate::gui_font::LINE_HEIGHT >= node.bounds.y + node.bounds.height {
            break;
        }
    }
}

fn gui_handle(raw: u64) -> GuiObjectHandle {
    GuiObjectHandle(raw)
}

fn session_handle_key(id: GuiObjectId) -> u64 {
    match id {
        GuiObjectId::SessionOpenRhiza => 10,
        GuiObjectId::SessionSandboxGui => 11,
        GuiObjectId::SessionWideConsole => 12,
        GuiObjectId::SessionRecoveryShell => 13,
        GuiObjectId::Conversation => 20,
        GuiObjectId::Composer => 30,
    }
}

fn gui_interaction_for_id(state: &GuiObjectRuntime, id: GuiObjectId) -> GuiInteractionState {
    if state.selected_session == id {
        GuiInteractionState::Active
    } else if state.focused == Some(id) {
        GuiInteractionState::Focused
    } else if state.hovered == Some(id) {
        GuiInteractionState::Hovered
    } else {
        GuiInteractionState::Idle
    }
}

fn sidebar_style_for_id(state: &GuiObjectRuntime, id: GuiObjectId) -> GuiStyleClass {
    if state.selected_session == id {
        GuiStyleClass::SidebarItemActive
    } else if state.hovered == Some(id) || state.focused == Some(id) {
        GuiStyleClass::SidebarItemHover
    } else {
        GuiStyleClass::SidebarItemIdle
    }
}

fn selected_session_ref(id: GuiObjectId) -> &'static str {
    match id {
        GuiObjectId::SessionOpenRhiza => "session:openrhiza",
        GuiObjectId::SessionSandboxGui => "session:sandbox-gui",
        GuiObjectId::SessionWideConsole => "session:wide-console",
        GuiObjectId::SessionRecoveryShell => "session:recovery-shell",
        GuiObjectId::Conversation => "conversation",
        GuiObjectId::Composer => "composer",
    }
}

fn selected_session_caption() -> &'static str {
    match GUI_OBJECTS.lock().selected_session {
        GuiObjectId::SessionOpenRhiza => "OpenRhiza ready",
        GuiObjectId::SessionSandboxGui => "Sandbox GUI",
        GuiObjectId::SessionWideConsole => "Wide console",
        GuiObjectId::SessionRecoveryShell => "Recovery shell",
        GuiObjectId::Conversation => "Conversation",
        GuiObjectId::Composer => "Composer",
    }
}

fn truncate_text_chars(text: &str, max_cells: usize) -> String {
    if max_cells == 0 {
        return String::new();
    }

    let total = crate::gui_font::text_display_cells(text);
    if total <= max_cells {
        return String::from(text);
    }

    if max_cells <= 1 {
        return String::from("…");
    }

    let mut out = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let width = crate::gui_font::display_cells(ch);
        if used + width > max_cells.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += width;
    }
    out.push('…');
    out
}

fn wrap_text(text: &str, max_cells: usize) -> Vec<String> {
    let max_cells = max_cells.max(8);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        let mut current_width = 0usize;
        for ch in paragraph.chars() {
            let width = crate::gui_font::display_cells(ch);
            if current_width + width > max_cells && !current.is_empty() {
                lines.push(current);
                current = String::new();
                current_width = 0;
            }

            current.push(ch);
            current_width += width;

            if current_width >= max_cells {
                lines.push(current);
                current = String::new();
                current_width = 0;
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn clear_framebuffer(surface: BootstrapSurfaceState, rgb: u32) {
    fill_rect(surface, 0, 0, surface.width, surface.height, rgb);
}

fn fill_rect(surface: BootstrapSurfaceState, x: usize, y: usize, width: usize, height: usize, rgb: u32) {
    let pitch = surface.width;
    let base = unsafe {
        (crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + surface.framebuffer_phys) as usize
            as *mut u32
    };
    let x_end = (x + width).min(surface.width);
    let y_end = (y + height).min(surface.height);
    for row in y..y_end {
        for col in x..x_end {
            unsafe {
                core::ptr::write_volatile(base.add(row * pitch + col), rgb);
            }
        }
    }
}

fn read_pixel(surface: BootstrapSurfaceState, x: usize, y: usize) -> u32 {
    if x >= surface.width || y >= surface.height {
        return 0;
    }
    let pitch = surface.width;
    let base = unsafe {
        (crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + surface.framebuffer_phys) as usize
            as *mut u32
    };
    unsafe { core::ptr::read_volatile(base.add(y * pitch + x)) }
}

fn write_pixel(surface: BootstrapSurfaceState, x: usize, y: usize, rgb: u32) {
    if x >= surface.width || y >= surface.height {
        return;
    }
    let pitch = surface.width;
    let base = unsafe {
        (crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + surface.framebuffer_phys) as usize
            as *mut u32
    };
    unsafe {
        core::ptr::write_volatile(base.add(y * pitch + x), rgb);
    }
}

fn draw_text(surface: BootstrapSurfaceState, x: usize, y: usize, text: &str, rgb: u32, scale: usize) {
    let mut cursor_x = x;
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = x;
            continue;
        }
        let glyph = crate::gui_font::glyph_alpha(ch)
            .or_else(|| crate::gui_font::glyph_alpha(crate::gui_font::fallback_char()));
        if let Some(alpha) = glyph {
            draw_alpha_glyph(surface, cursor_x, y, alpha, rgb, scale);
        }
        cursor_x += crate::gui_font::pixel_advance_for_char(ch) * scale;
        if cursor_x + (crate::gui_font::CHAR_ADVANCE * scale) >= surface.width {
            break;
        }
    }
}

fn draw_alpha_glyph(
    surface: BootstrapSurfaceState,
    x: usize,
    y: usize,
    glyph_alpha: &[u8],
    rgb: u32,
    scale: usize,
) {
    for row in 0..crate::gui_font::GLYPH_HEIGHT {
        for col in 0..crate::gui_font::GLYPH_WIDTH {
            let alpha = glyph_alpha[row * crate::gui_font::GLYPH_WIDTH + col];
            if alpha == 0 {
                continue;
            }
            for sy in 0..scale {
                for sx in 0..scale {
                    let px = x + col * scale + sx;
                    let py = y + row * scale + sy;
                    let existing = read_pixel(surface, px, py);
                    write_pixel(surface, px, py, blend_rgb(existing, rgb, alpha));
                }
            }
        }
    }
}

fn blend_rgb(dst: u32, src: u32, alpha: u8) -> u32 {
    if alpha == 255 {
        return src;
    }
    let a = alpha as u32;
    let inv = 255u32.saturating_sub(a);

    let dr = (dst >> 16) & 0xff;
    let dg = (dst >> 8) & 0xff;
    let db = dst & 0xff;
    let sr = (src >> 16) & 0xff;
    let sg = (src >> 8) & 0xff;
    let sb = src & 0xff;

    let r = (sr * a + dr * inv) / 255;
    let g = (sg * a + dg * inv) / 255;
    let b = (sb * a + db * inv) / 255;
    (r << 16) | (g << 8) | b
}

fn vga_color_to_rgb(color: u8) -> u32 {
    match color & 0x0F {
        0x0 => 0x000000,
        0x1 => 0x0000AA,
        0x2 => 0x00AA00,
        0x3 => 0x00AAAA,
        0x4 => 0xAA0000,
        0x5 => 0xAA00AA,
        0x6 => 0xAA5500,
        0x7 => 0xAAAAAA,
        0x8 => 0x555555,
        0x9 => 0x5555FF,
        0xA => 0x55FF55,
        0xB => 0x55FFFF,
        0xC => 0xFF5555,
        0xD => 0xFF55FF,
        0xE => 0xFFFF55,
        _ => 0xFFFFFF,
    }
}
