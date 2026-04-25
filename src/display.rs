use alloc::string::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayBackend {
    VgaText,
    FramebufferText,
    Gui,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayModeInfo {
    pub backend: DisplayBackend,
    pub text_cols: usize,
    pub text_rows: usize,
    pub pixel_width: usize,
    pub pixel_height: usize,
}

pub fn active_mode() -> DisplayModeInfo {
    DisplayModeInfo {
        backend: DisplayBackend::VgaText,
        text_cols: 80,
        text_rows: 25,
        pixel_width: 720,
        pixel_height: 400,
    }
}

pub fn backend_name() -> &'static str {
    match active_mode().backend {
        DisplayBackend::VgaText => "vga-text",
        DisplayBackend::FramebufferText => "framebuffer-text",
        DisplayBackend::Gui => "gui",
    }
}

pub fn describe_active_mode() -> String {
    let mode = active_mode();
    let backend = backend_name();
    alloc::format!(
        "backend={} text={}x{} pixels={}x{}",
        backend,
        mode.text_cols,
        mode.text_rows,
        mode.pixel_width,
        mode.pixel_height
    )
}

pub fn init_console() {
    crate::vga::init_cli();
}

pub fn render_runtime(seconds: u64) {
    crate::vga::render_runtime(seconds);
}

