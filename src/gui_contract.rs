use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiObjectHandle(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl GuiRect {
    pub const fn contains(&self, px: usize, py: usize) -> bool {
        px >= self.x
            && py >= self.y
            && px < self.x.saturating_add(self.width)
            && py < self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiNodeKind {
    Root,
    Sidebar,
    SessionList,
    SessionItem,
    Conversation,
    Message,
    Composer,
    TextInput,
    Footer,
    Label,
    ScrollArea,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiStyleClass {
    Chrome,
    Sidebar,
    SidebarItemIdle,
    SidebarItemHover,
    SidebarItemActive,
    ConversationSurface,
    AssistantMessage,
    UserMessage,
    ComposerSurface,
    FooterSurface,
    PlainText,
    AccentText,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiInteractionState {
    Idle,
    Hovered,
    Focused,
    Active,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiEventKind {
    PointerMove,
    PointerDown,
    PointerUp,
    Scroll,
    Focus,
    Blur,
    TextInput,
    Activate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiBackendPreference {
    NativeObject,
    LvglStyle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiNode {
    pub handle: GuiObjectHandle,
    pub parent: Option<GuiObjectHandle>,
    pub kind: GuiNodeKind,
    pub style: GuiStyleClass,
    pub interaction: GuiInteractionState,
    pub bounds: GuiRect,
    pub object_ref: Option<String>,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiScene {
    pub scene_id: String,
    pub backend_preference: GuiBackendPreference,
    pub nodes: Vec<GuiNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEvent {
    pub target: Option<GuiObjectHandle>,
    pub kind: GuiEventKind,
    pub pointer_x: usize,
    pub pointer_y: usize,
    pub buttons: u8,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiMutation {
    pub target: GuiObjectHandle,
    pub new_bounds: Option<GuiRect>,
    pub new_style: Option<GuiStyleClass>,
    pub new_interaction: Option<GuiInteractionState>,
    pub new_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiObjectPolicy {
    pub isolate_failures: bool,
    pub object_local_redraw: bool,
    pub rollback_on_object_failure: bool,
    pub allow_cross_object_reads: bool,
}

pub const DEFAULT_OBJECT_POLICY: GuiObjectPolicy = GuiObjectPolicy {
    isolate_failures: true,
    object_local_redraw: true,
    rollback_on_object_failure: true,
    allow_cross_object_reads: false,
};
