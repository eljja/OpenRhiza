pub const GLYPH_WIDTH: usize = 16;
pub const GLYPH_HEIGHT: usize = 24;
pub const GLYPH_BYTES: usize = GLYPH_WIDTH * GLYPH_HEIGHT;
pub const CHAR_ADVANCE: usize = 12;
pub const LINE_HEIGHT: usize = 22;

const ASCII_START: u32 = 0x20;
const ASCII_END: u32 = 0x7E;
const ASCII_COUNT: usize = (ASCII_END - ASCII_START + 1) as usize;
const COMPAT_JAMO_START: u32 = 0x3131;
const COMPAT_JAMO_END: u32 = 0x3163;
const COMPAT_JAMO_COUNT: usize = (COMPAT_JAMO_END - COMPAT_JAMO_START + 1) as usize;
const HANGUL_START: u32 = 0xAC00;
const HANGUL_END: u32 = 0xD7A3;
const HANGUL_COUNT: usize = (HANGUL_END - HANGUL_START + 1) as usize;

static FONT_ATLAS: &[u8] = include_bytes!("../assets/fonts/noto_sans_kr_ui_16x24.bin");

pub fn has_glyph(ch: char) -> bool {
    glyph_index(ch).is_some()
}

pub fn is_wide(ch: char) -> bool {
    let code = ch as u32;
    (COMPAT_JAMO_START..=COMPAT_JAMO_END).contains(&code)
        || (HANGUL_START..=HANGUL_END).contains(&code)
}

pub fn display_cells(ch: char) -> usize {
    if is_wide(ch) { 2 } else { 1 }
}

pub fn text_display_cells(text: &str) -> usize {
    text.chars().map(display_cells).sum()
}

pub fn pixel_advance_for_char(ch: char) -> usize {
    if is_wide(ch) {
        GLYPH_WIDTH
    } else {
        CHAR_ADVANCE
    }
}

pub fn text_pixel_advance(text: &str) -> usize {
    text.chars().map(pixel_advance_for_char).sum()
}

pub fn glyph_alpha(ch: char) -> Option<&'static [u8]> {
    let index = glyph_index(ch)?;
    let start = index.checked_mul(GLYPH_BYTES)?;
    let end = start.checked_add(GLYPH_BYTES)?;
    FONT_ATLAS.get(start..end)
}

pub fn fallback_char() -> char {
    '?'
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

pub fn atlas_expected_len() -> usize {
    (ASCII_COUNT + COMPAT_JAMO_COUNT + HANGUL_COUNT) * GLYPH_BYTES
}
