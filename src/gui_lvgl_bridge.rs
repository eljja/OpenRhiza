use alloc::vec::Vec;
use crate::gui_contract::{
    GuiBackendPreference,
    GuiNode,
    GuiNodeKind,
    GuiObjectHandle,
    GuiRect,
    GuiScene,
    GuiStyleClass,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LvglBridgeMode {
    NativeOnly,
    LvglStyleSandbox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LvglWidgetMapping {
    pub kind: GuiNodeKind,
    pub widget_name: &'static str,
    pub style_hint: GuiStyleClass,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LvglWidgetDescriptor {
    pub handle: GuiObjectHandle,
    pub parent: Option<GuiObjectHandle>,
    pub widget_name: &'static str,
    pub style_hint: GuiStyleClass,
    pub bounds: GuiRect,
    pub label: alloc::string::String,
}

pub fn recommended_widget_mapping(kind: GuiNodeKind) -> LvglWidgetMapping {
    match kind {
        GuiNodeKind::Root => LvglWidgetMapping { kind, widget_name: "screen", style_hint: GuiStyleClass::Chrome },
        GuiNodeKind::Sidebar => LvglWidgetMapping { kind, widget_name: "container", style_hint: GuiStyleClass::Sidebar },
        GuiNodeKind::SessionList => LvglWidgetMapping { kind, widget_name: "list", style_hint: GuiStyleClass::Sidebar },
        GuiNodeKind::SessionItem => LvglWidgetMapping { kind, widget_name: "button", style_hint: GuiStyleClass::SidebarItemIdle },
        GuiNodeKind::Conversation => LvglWidgetMapping { kind, widget_name: "container", style_hint: GuiStyleClass::ConversationSurface },
        GuiNodeKind::Message => LvglWidgetMapping { kind, widget_name: "label", style_hint: GuiStyleClass::PlainText },
        GuiNodeKind::Composer => LvglWidgetMapping { kind, widget_name: "container", style_hint: GuiStyleClass::ComposerSurface },
        GuiNodeKind::TextInput => LvglWidgetMapping { kind, widget_name: "textarea", style_hint: GuiStyleClass::ComposerSurface },
        GuiNodeKind::Footer => LvglWidgetMapping { kind, widget_name: "container", style_hint: GuiStyleClass::FooterSurface },
        GuiNodeKind::Label => LvglWidgetMapping { kind, widget_name: "label", style_hint: GuiStyleClass::PlainText },
        GuiNodeKind::ScrollArea => LvglWidgetMapping { kind, widget_name: "container", style_hint: GuiStyleClass::ConversationSurface },
        GuiNodeKind::Custom => LvglWidgetMapping { kind, widget_name: "container", style_hint: GuiStyleClass::Chrome },
    }
}

pub fn scene_requests_lvgl_style(scene: &GuiScene) -> bool {
    matches!(scene.backend_preference, GuiBackendPreference::LvglStyle)
}

pub fn translate_node(node: &GuiNode) -> LvglWidgetDescriptor {
    let mapping = recommended_widget_mapping(node.kind);
    LvglWidgetDescriptor {
        handle: node.handle,
        parent: node.parent,
        widget_name: mapping.widget_name,
        style_hint: node.style,
        bounds: node.bounds,
        label: node.label.clone(),
    }
}

pub fn translate_scene(scene: &GuiScene) -> Vec<LvglWidgetDescriptor> {
    scene.nodes.iter().map(translate_node).collect()
}
