use core::ptr::{read_volatile, write_volatile};

const GUI_WIDTH: usize = 1920;
const GUI_HEIGHT: usize = 1080;
const GUI_STRIDE: usize = GUI_WIDTH * 4;
const GUI_PIXELS: usize = GUI_WIDTH * GUI_HEIGHT;

const FW_CFG_BASE: usize = 0x0902_0000;
const FW_CFG_DATA: usize = FW_CFG_BASE;
const FW_CFG_SELECTOR: usize = FW_CFG_BASE + 0x08;
const FW_CFG_DMA: usize = FW_CFG_BASE + 0x10;
const FW_CFG_FILE_DIR: u16 = 0x0019;
const FW_CFG_DMA_CTL_ERROR: u32 = 0x01;
const FW_CFG_DMA_CTL_SELECT: u32 = 0x08;
const FW_CFG_DMA_CTL_WRITE: u32 = 0x10;
const FW_CFG_MAX_FILES: usize = 128;
const RAMFB_NAME: &[u8] = b"etc/ramfb";

const GLYPH_WIDTH: usize = 16;
const GLYPH_HEIGHT: usize = 24;
const GLYPH_BYTES: usize = GLYPH_WIDTH * GLYPH_HEIGHT;
const CHAR_ADVANCE: usize = 12;
const ASCII_START: u32 = 0x20;
const ASCII_END: u32 = 0x7e;
const ASCII_COUNT: usize = (ASCII_END - ASCII_START + 1) as usize;
const COMPAT_JAMO_START: u32 = 0x3131;
const COMPAT_JAMO_END: u32 = 0x3163;
const COMPAT_JAMO_COUNT: usize = (COMPAT_JAMO_END - COMPAT_JAMO_START + 1) as usize;
const HANGUL_START: u32 = 0xac00;
const HANGUL_END: u32 = 0xd7a3;

const COLOR_BG: u32 = 0x000f_1117;
const COLOR_PANEL: u32 = 0x0016_1820;
const COLOR_PANEL_SOFT: u32 = 0x001d_2028;
const COLOR_SIDEBAR: u32 = 0x000b_101b;
const COLOR_TOPBAR: u32 = 0x0020_416c;
const COLOR_ACTIVE: u32 = 0x002c_466c;
const COLOR_BORDER: u32 = 0x0031_3644;
const COLOR_TEXT: u32 = 0x00d7_dce6;
const COLOR_MUTED: u32 = 0x0084_8b98;
const COLOR_GREEN: u32 = 0x008d_d596;
const COLOR_YELLOW: u32 = 0x00df_c96b;
const COLOR_BLUE: u32 = 0x0054_bce9;

static FONT_ATLAS: &[u8] = include_bytes!("../../../assets/fonts/noto_sans_kr_ui_16x24.bin");

#[repr(align(4096))]
struct GuiFramebuffer([u32; GUI_PIXELS]);

#[repr(C, align(16))]
struct DmaAccess {
    control: u32,
    length: u32,
    address: u64,
}

#[repr(C, packed)]
struct RamfbConfig {
    addr: u64,
    fourcc: u32,
    flags: u32,
    width: u32,
    height: u32,
    stride: u32,
}

static mut GUI_FRAMEBUFFER: GuiFramebuffer = GuiFramebuffer([0; GUI_PIXELS]);
static mut DMA_ACCESS: DmaAccess = DmaAccess {
    control: 0,
    length: 0,
    address: 0,
};
static mut RAMFB_CONFIG: RamfbConfig = RamfbConfig {
    addr: 0,
    fourcc: 0,
    flags: 0,
    width: 0,
    height: 0,
    stride: 0,
};
static mut GUI_ACTIVE: bool = false;
static mut RAMFB_SELECTOR: u16 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiStartResult {
    Rendered,
    RamfbFileNotFound,
    DmaError,
}

pub fn start_gui() -> GuiStartResult {
    let Some(selector) = find_ramfb_selector() else {
        return GuiStartResult::RamfbFileNotFound;
    };

    unsafe {
        RAMFB_SELECTOR = selector;
    }

    draw_bootstrap_scene();

    if !configure_ramfb(selector) {
        return GuiStartResult::DmaError;
    }

    unsafe {
        GUI_ACTIVE = true;
    }
    GuiStartResult::Rendered
}

pub fn redraw_gui() -> bool {
    if !is_active() {
        return false;
    }
    draw_bootstrap_scene();
    true
}

pub fn is_active() -> bool {
    unsafe { GUI_ACTIVE }
}

pub fn selector() -> u16 {
    unsafe { RAMFB_SELECTOR }
}

pub fn width() -> usize {
    GUI_WIDTH
}

pub fn height() -> usize {
    GUI_HEIGHT
}

fn configure_ramfb(selector: u16) -> bool {
    let fb_addr = unsafe { core::ptr::addr_of!(GUI_FRAMEBUFFER.0) as *const u32 as u64 };
    unsafe {
        RAMFB_CONFIG = RamfbConfig {
            addr: fb_addr.to_be(),
            fourcc: fourcc(*b"XR24").to_be(),
            flags: 0u32.to_be(),
            width: (GUI_WIDTH as u32).to_be(),
            height: (GUI_HEIGHT as u32).to_be(),
            stride: (GUI_STRIDE as u32).to_be(),
        };
    }
    fw_cfg_dma_write(
        selector,
        core::ptr::addr_of!(RAMFB_CONFIG) as u64,
        core::mem::size_of::<RamfbConfig>() as u32,
    )
}

fn find_ramfb_selector() -> Option<u16> {
    fw_cfg_select(FW_CFG_FILE_DIR);
    let file_count = read_be_u32() as usize;
    let capped = if file_count > FW_CFG_MAX_FILES {
        FW_CFG_MAX_FILES
    } else {
        file_count
    };
    for _ in 0..capped {
        let _size = read_be_u32();
        let selector = read_be_u16();
        let _reserved = read_be_u16();
        let mut name = [0u8; 56];
        for byte in &mut name {
            *byte = fw_cfg_read_u8();
        }
        if fw_cfg_name_matches(&name, RAMFB_NAME) {
            return Some(selector);
        }
    }
    None
}

fn fw_cfg_name_matches(entry: &[u8; 56], expected: &[u8]) -> bool {
    let mut i = 0usize;
    while i < expected.len() {
        if entry[i] != expected[i] {
            return false;
        }
        i += 1;
    }
    entry[expected.len()] == 0
}

fn fw_cfg_select(selector: u16) {
    unsafe {
        write_volatile(FW_CFG_SELECTOR as *mut u16, selector.to_be());
    }
}

fn fw_cfg_read_u8() -> u8 {
    unsafe { read_volatile(FW_CFG_DATA as *const u8) }
}

fn read_be_u16() -> u16 {
    let hi = fw_cfg_read_u8() as u16;
    let lo = fw_cfg_read_u8() as u16;
    (hi << 8) | lo
}

fn read_be_u32() -> u32 {
    let b0 = fw_cfg_read_u8() as u32;
    let b1 = fw_cfg_read_u8() as u32;
    let b2 = fw_cfg_read_u8() as u32;
    let b3 = fw_cfg_read_u8() as u32;
    (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
}

fn fw_cfg_dma_write(selector: u16, guest_addr: u64, len: u32) -> bool {
    unsafe {
        DMA_ACCESS.control =
            (((selector as u32) << 16) | FW_CFG_DMA_CTL_SELECT | FW_CFG_DMA_CTL_WRITE).to_be();
        DMA_ACCESS.length = len.to_be();
        DMA_ACCESS.address = guest_addr.to_be();
        let dma_addr = core::ptr::addr_of!(DMA_ACCESS) as u64;
        write_volatile(FW_CFG_DMA as *mut u32, ((dma_addr >> 32) as u32).to_be());
        write_volatile((FW_CFG_DMA + 4) as *mut u32, (dma_addr as u32).to_be());
        for _ in 0..1_000_000 {
            let control = u32::from_be(read_volatile(core::ptr::addr_of!(DMA_ACCESS.control)));
            if control == 0 {
                return true;
            }
            if control & FW_CFG_DMA_CTL_ERROR != 0 {
                return false;
            }
            core::hint::spin_loop();
        }
        false
    }
}

const fn fourcc(code: [u8; 4]) -> u32 {
    (code[0] as u32) | ((code[1] as u32) << 8) | ((code[2] as u32) << 16) | ((code[3] as u32) << 24)
}

fn draw_bootstrap_scene() {
    fill_rect(0, 0, GUI_WIDTH, GUI_HEIGHT, COLOR_BG);
    fill_rect(0, 0, GUI_WIDTH, 46, COLOR_TOPBAR);
    draw_text(
        12,
        12,
        "OpenRhiza ARM64 sandbox session  |  1920x1080 ramfb  |  x86 GUI parity bootstrap",
        COLOR_TEXT,
    );

    fill_rect(0, 46, 288, GUI_HEIGHT - 46, COLOR_SIDEBAR);
    draw_vline(288, 46, GUI_HEIGHT - 46, COLOR_BORDER);
    draw_text(28, 92, "OpenRhiza", COLOR_BLUE);
    draw_text(28, 124, "AI-native capability sessions", COLOR_MUTED);
    sidebar_item(24, 172, "OpenRhiza", true);
    sidebar_item(24, 220, "Sandbox GUI", false);
    sidebar_item(24, 268, "Wide Console", false);
    sidebar_item(24, 316, "Recovery Shell", false);

    let main_x = 318;
    let main_w = GUI_WIDTH - main_x - 28;
    draw_text(main_x, 76, "OpenRhiza ready", COLOR_TEXT);
    draw_text(
        main_x,
        110,
        "ARM64 uses a minimal ramfb backend; GUI policy and interaction stay sandbox-owned.",
        COLOR_MUTED,
    );

    fill_rect(main_x, 156, main_w, 668, COLOR_PANEL);
    draw_rect(main_x, 156, main_w, 668, COLOR_BORDER);
    draw_text(
        main_x + 28,
        190,
        "OpenRhiza conversation session",
        COLOR_GREEN,
    );
    draw_text(
        main_x + 28,
        230,
        "OpenRhiza ARM64 GUI bootstrap is active. The session/sidebar/conversation/composer layout mirrors x86_64.",
        COLOR_TEXT,
    );
    draw_text(
        main_x + 28,
        264,
        "Recovery serial CLI remains available while sandbox display, input, and compositor skills mature.",
        COLOR_TEXT,
    );
    draw_text(
        main_x + 28,
        310,
        "ARM64 GUI 경로가 준비되었습니다.",
        COLOR_GREEN,
    );

    fill_rect(main_x + 24, 844, main_w - 48, 86, COLOR_PANEL_SOFT);
    fill_rect(main_x + 24, 844, main_w - 48, 28, COLOR_ACTIVE);
    draw_text(main_x + 40, 848, "Composer", COLOR_TEXT);
    draw_text(
        main_x + 40,
        886,
        "Type a prompt. The composer expands as your message wraps.",
        COLOR_MUTED,
    );

    fill_rect(main_x + 24, 962, main_w - 48, 72, COLOR_PANEL);
    draw_text(main_x + 40, 980, "Status", COLOR_GREEN);
    draw_text(
        main_x + 40,
        1008,
        "GUI visible path: ramfb. Input path: PL011 recovery CLI until virtio-input sandbox promotion.",
        COLOR_YELLOW,
    );
}

fn sidebar_item(x: usize, y: usize, label: &str, active: bool) {
    let color = if active { COLOR_ACTIVE } else { COLOR_PANEL };
    fill_rect(x, y, 234, 34, color);
    draw_rect(
        x,
        y,
        234,
        34,
        if active { COLOR_BLUE } else { COLOR_BORDER },
    );
    draw_text(
        x + 14,
        y + 6,
        label,
        if active { COLOR_TEXT } else { COLOR_MUTED },
    );
}

fn fill_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    let x_end = min_usize(x.saturating_add(w), GUI_WIDTH);
    let y_end = min_usize(y.saturating_add(h), GUI_HEIGHT);
    let mut yy = y;
    while yy < y_end {
        let mut xx = x;
        while xx < x_end {
            put_pixel(xx, yy, color);
            xx += 1;
        }
        yy += 1;
    }
}

fn draw_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    if w == 0 || h == 0 {
        return;
    }
    fill_rect(x, y, w, 1, color);
    fill_rect(x, y + h - 1, w, 1, color);
    fill_rect(x, y, 1, h, color);
    fill_rect(x + w - 1, y, 1, h, color);
}

fn draw_vline(x: usize, y: usize, h: usize, color: u32) {
    fill_rect(x, y, 1, h, color);
}

fn draw_text(mut x: usize, y: usize, text: &str, color: u32) {
    for ch in text.chars() {
        if ch == '\n' {
            x = 0;
            continue;
        }
        let draw_ch = if glyph_alpha(ch).is_some() { ch } else { '?' };
        draw_glyph(x, y, draw_ch, color);
        x = x.saturating_add(pixel_advance_for_char(draw_ch));
    }
}

fn draw_glyph(x: usize, y: usize, ch: char, color: u32) {
    let Some(alpha) = glyph_alpha(ch) else {
        return;
    };
    let mut gy = 0usize;
    while gy < GLYPH_HEIGHT {
        let mut gx = 0usize;
        while gx < GLYPH_WIDTH {
            let a = alpha[gy * GLYPH_WIDTH + gx];
            if a != 0 {
                let px = x + gx;
                let py = y + gy;
                if px < GUI_WIDTH && py < GUI_HEIGHT {
                    let dst = get_pixel(px, py);
                    put_pixel(px, py, blend_rgb(dst, color, a));
                }
            }
            gx += 1;
        }
        gy += 1;
    }
}

fn glyph_alpha(ch: char) -> Option<&'static [u8]> {
    let index = glyph_index(ch)?;
    let start = index.checked_mul(GLYPH_BYTES)?;
    let end = start.checked_add(GLYPH_BYTES)?;
    FONT_ATLAS.get(start..end)
}

fn glyph_index(ch: char) -> Option<usize> {
    let code = ch as u32;
    if (ASCII_START..=ASCII_END).contains(&code) {
        return Some((code - ASCII_START) as usize);
    }
    if (COMPAT_JAMO_START..=COMPAT_JAMO_END).contains(&code) {
        return Some(ASCII_COUNT + (code - COMPAT_JAMO_START) as usize);
    }
    if (HANGUL_START..=HANGUL_END).contains(&code) {
        return Some(ASCII_COUNT + COMPAT_JAMO_COUNT + (code - HANGUL_START) as usize);
    }
    None
}

fn pixel_advance_for_char(ch: char) -> usize {
    let code = ch as u32;
    if (COMPAT_JAMO_START..=COMPAT_JAMO_END).contains(&code)
        || (HANGUL_START..=HANGUL_END).contains(&code)
    {
        GLYPH_WIDTH
    } else {
        CHAR_ADVANCE
    }
}

fn blend_rgb(dst: u32, src: u32, alpha: u8) -> u32 {
    let a = alpha as u32;
    let inv = 255 - a;
    let rb = (((src & 0x00ff_00ff) * a + (dst & 0x00ff_00ff) * inv) >> 8) & 0x00ff_00ff;
    let g = (((src & 0x0000_ff00) * a + (dst & 0x0000_ff00) * inv) >> 8) & 0x0000_ff00;
    rb | g
}

fn put_pixel(x: usize, y: usize, color: u32) {
    unsafe {
        let ptr = core::ptr::addr_of_mut!(GUI_FRAMEBUFFER.0) as *mut u32;
        write_volatile(ptr.add(y * GUI_WIDTH + x), color);
    }
}

fn get_pixel(x: usize, y: usize) -> u32 {
    unsafe {
        let ptr = core::ptr::addr_of!(GUI_FRAMEBUFFER.0) as *const u32;
        read_volatile(ptr.add(y * GUI_WIDTH + x))
    }
}

fn min_usize(a: usize, b: usize) -> usize {
    if a < b {
        a
    } else {
        b
    }
}
