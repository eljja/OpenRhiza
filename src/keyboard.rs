// src/keyboard.rs
// Native full-QWERTY keyboard implementation (no serial injection required)
// Based on PS/2 Scancode Set 1 and supports the standard key set.

/// Keyboard event produced after decoding an input scancode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    /// Printable ASCII character (a-z, A-Z, 0-9, symbols, and so on)
    Char(u8),
    /// Enter / Keypad Enter
    Enter,
    /// Backspace
    Backspace,
    /// Tab
    Tab,
    /// Escape
    Escape,
    /// Arrow keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    /// Navigation keys
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    CtrlC,
    CtrlL,
    CtrlW,
    CtrlU,
    CtrlK,
    /// Function keys (F1-F12, encoded as 1-12)
    FunctionKey(u8),
    /// Modifier-only event (Shift, Ctrl, Alt by themselves), usually ignored
    ModifierOnly,
}

/// State machine that tracks modifier and lock-key state.
pub struct KeyboardState {
    pub shift_pressed: bool,
    pub ctrl_pressed: bool,
    pub alt_pressed: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
    pub is_extended: bool,   // E0 extended-key prefix flag
}

impl KeyboardState {
    pub const fn new() -> Self {
        Self {
            shift_pressed: false,
            ctrl_pressed: false,
            alt_pressed: false,
            caps_lock: false,
            num_lock: false,
            is_extended: false,
        }
    }

    /// Convert a scancode into a `KeyEvent`.
    /// Returns `None` for scancodes that should be ignored, such as break codes and prefixes.
    pub fn process_scancode(&mut self, scancode: u8) -> Option<KeyEvent> {
        // Handle the E0 extended-key prefix.
        if scancode == 0xE0 {
            self.is_extended = true;
            return None;
        }

        let is_break = scancode >= 0x80;
        let make_code = scancode & 0x7F;
        let extended = self.is_extended;
        self.is_extended = false; // Consume the prefix once, then clear it.

        // --- Modifier keys (both make and break) ---
        match (extended, make_code) {
            // Left Shift (0x2A), Right Shift (0x36)
            (false, 0x2A) | (false, 0x36) => {
                self.shift_pressed = !is_break;
                return if is_break { None } else { Some(KeyEvent::ModifierOnly) };
            }
            // Left Ctrl (0x1D), Right Ctrl (E0 1D)
            (false, 0x1D) | (true, 0x1D) => {
                self.ctrl_pressed = !is_break;
                return if is_break { None } else { Some(KeyEvent::ModifierOnly) };
            }
            // Left Alt (0x38), Right Alt (E0 38)
            (false, 0x38) | (true, 0x38) => {
                self.alt_pressed = !is_break;
                return if is_break { None } else { Some(KeyEvent::ModifierOnly) };
            }
            // Caps Lock (0x3A) — toggle only on make
            (false, 0x3A) => {
                if !is_break {
                    self.caps_lock = !self.caps_lock;
                }
                return None;
            }
            // Num Lock (0x45) — toggle only on make
            (false, 0x45) => {
                if !is_break {
                    self.num_lock = !self.num_lock;
                }
                return None;
            }
            // Scroll Lock (0x46) — ignore
            (false, 0x46) => return None,
            _ => {}
        }

        // Ignore break/release events. Characters are produced on make only.
        if is_break {
            return None;
        }

        // --- Extended keys (E0-prefixed) ---
        if extended {
            return match make_code {
                0x48 => Some(KeyEvent::ArrowUp),
                0x50 => Some(KeyEvent::ArrowDown),
                0x4B => Some(KeyEvent::ArrowLeft),
                0x4D => Some(KeyEvent::ArrowRight),
                0x47 => Some(KeyEvent::Home),
                0x4F => Some(KeyEvent::End),
                0x49 => Some(KeyEvent::PageUp),
                0x51 => Some(KeyEvent::PageDown),
                0x52 => Some(KeyEvent::Insert),
                0x53 => Some(KeyEvent::Delete),
                0x1C => Some(KeyEvent::Enter), // Keypad Enter
                0x35 => Some(KeyEvent::Char(b'/')), // Keypad /
                _ => None,
            };
        }

        // --- Special non-printable keys ---
        match make_code {
            0x01 => return Some(KeyEvent::Escape),
            0x0E => return Some(KeyEvent::Backspace),
            0x0F => return Some(KeyEvent::Tab),
            0x1C => return Some(KeyEvent::Enter),
            // Function Keys F1~F10
            0x3B => return Some(KeyEvent::FunctionKey(1)),
            0x3C => return Some(KeyEvent::FunctionKey(2)),
            0x3D => return Some(KeyEvent::FunctionKey(3)),
            0x3E => return Some(KeyEvent::FunctionKey(4)),
            0x3F => return Some(KeyEvent::FunctionKey(5)),
            0x40 => return Some(KeyEvent::FunctionKey(6)),
            0x41 => return Some(KeyEvent::FunctionKey(7)),
            0x42 => return Some(KeyEvent::FunctionKey(8)),
            0x43 => return Some(KeyEvent::FunctionKey(9)),
            0x44 => return Some(KeyEvent::FunctionKey(10)),
            0x57 => return Some(KeyEvent::FunctionKey(11)),
            0x58 => return Some(KeyEvent::FunctionKey(12)),
            _ => {}
        }

        // --- Numpad keys (behavior depends on Num Lock) ---
        if self.num_lock {
            match make_code {
                0x47 => return Some(KeyEvent::Char(b'7')),
                0x48 => return Some(KeyEvent::Char(b'8')),
                0x49 => return Some(KeyEvent::Char(b'9')),
                0x4B => return Some(KeyEvent::Char(b'4')),
                0x4C => return Some(KeyEvent::Char(b'5')),
                0x4D => return Some(KeyEvent::Char(b'6')),
                0x4F => return Some(KeyEvent::Char(b'1')),
                0x50 => return Some(KeyEvent::Char(b'2')),
                0x51 => return Some(KeyEvent::Char(b'3')),
                0x52 => return Some(KeyEvent::Char(b'0')),
                0x53 => return Some(KeyEvent::Char(b'.')),
                _ => {}
            }
        } else {
            match make_code {
                0x47 => return Some(KeyEvent::Home),
                0x48 => return Some(KeyEvent::ArrowUp),
                0x49 => return Some(KeyEvent::PageUp),
                0x4B => return Some(KeyEvent::ArrowLeft),
                0x4C => return None, // Numpad 5 (no navigation mapping when Num Lock is off)
                0x4D => return Some(KeyEvent::ArrowRight),
                0x4F => return Some(KeyEvent::End),
                0x50 => return Some(KeyEvent::ArrowDown),
                0x51 => return Some(KeyEvent::PageDown),
                0x52 => return Some(KeyEvent::Insert),
                0x53 => return Some(KeyEvent::Delete),
                _ => {}
            }
        }
        // Numpad operators
        match make_code {
            0x37 => return Some(KeyEvent::Char(b'*')), // Keypad *
            0x4A => return Some(KeyEvent::Char(b'-')), // Keypad -
            0x4E => return Some(KeyEvent::Char(b'+')), // Keypad +
            _ => {}
        }

        // --- Printable characters (QWERTY mapping) ---
        let effective_shift = self.shift_pressed;

        // Alphabetic keys combine Caps Lock and Shift through XOR.
        let alpha_upper = self.shift_pressed ^ self.caps_lock;

        let ch = match make_code {
            // Number row (` 1 2 3 4 5 6 7 8 9 0 - =)
            0x29 => if effective_shift { b'~' } else { b'`' },
            0x02 => if effective_shift { b'!' } else { b'1' },
            0x03 => if effective_shift { b'@' } else { b'2' },
            0x04 => if effective_shift { b'#' } else { b'3' },
            0x05 => if effective_shift { b'$' } else { b'4' },
            0x06 => if effective_shift { b'%' } else { b'5' },
            0x07 => if effective_shift { b'^' } else { b'6' },
            0x08 => if effective_shift { b'&' } else { b'7' },
            0x09 => if effective_shift { b'*' } else { b'8' },
            0x0A => if effective_shift { b'(' } else { b'9' },
            0x0B => if effective_shift { b')' } else { b'0' },
            0x0C => if effective_shift { b'_' } else { b'-' },
            0x0D => if effective_shift { b'+' } else { b'=' },

            // QWERTY row 1 (q w e r t y u i o p [ ] \)
            0x10 => if alpha_upper { b'Q' } else { b'q' },
            0x11 => if alpha_upper { b'W' } else { b'w' },
            0x12 => if alpha_upper { b'E' } else { b'e' },
            0x13 => if alpha_upper { b'R' } else { b'r' },
            0x14 => if alpha_upper { b'T' } else { b't' },
            0x15 => if alpha_upper { b'Y' } else { b'y' },
            0x16 => if alpha_upper { b'U' } else { b'u' },
            0x17 => if alpha_upper { b'I' } else { b'i' },
            0x18 => if alpha_upper { b'O' } else { b'o' },
            0x19 => if alpha_upper { b'P' } else { b'p' },
            0x1A => if effective_shift { b'{' } else { b'[' },
            0x1B => if effective_shift { b'}' } else { b']' },
            0x2B => if effective_shift { b'|' } else { b'\\' },

            // QWERTY row 2 (a s d f g h j k l ; ')
            0x1E => if alpha_upper { b'A' } else { b'a' },
            0x1F => if alpha_upper { b'S' } else { b's' },
            0x20 => if alpha_upper { b'D' } else { b'd' },
            0x21 => if alpha_upper { b'F' } else { b'f' },
            0x22 => if alpha_upper { b'G' } else { b'g' },
            0x23 => if alpha_upper { b'H' } else { b'h' },
            0x24 => if alpha_upper { b'J' } else { b'j' },
            0x25 => if alpha_upper { b'K' } else { b'k' },
            0x26 => if alpha_upper { b'L' } else { b'l' },
            0x27 => if effective_shift { b':' } else { b';' },
            0x28 => if effective_shift { b'"' } else { b'\'' },

            // QWERTY row 3 (z x c v b n m , . /)
            0x2C => if alpha_upper { b'Z' } else { b'z' },
            0x2D => if alpha_upper { b'X' } else { b'x' },
            0x2E => if alpha_upper { b'C' } else { b'c' },
            0x2F => if alpha_upper { b'V' } else { b'v' },
            0x30 => if alpha_upper { b'B' } else { b'b' },
            0x31 => if alpha_upper { b'N' } else { b'n' },
            0x32 => if alpha_upper { b'M' } else { b'm' },
            0x33 => if effective_shift { b'<' } else { b',' },
            0x34 => if effective_shift { b'>' } else { b'.' },
            0x35 => if effective_shift { b'?' } else { b'/' },

            // Space
            0x39 => b' ',

            _ => return None,
        };

        if self.ctrl_pressed {
            match ch {
                b'a' | b'A' => return Some(KeyEvent::Home),
                b'e' | b'E' => return Some(KeyEvent::End),
                b'c' | b'C' => return Some(KeyEvent::CtrlC),
                b'l' | b'L' => return Some(KeyEvent::CtrlL),
                b'w' | b'W' => return Some(KeyEvent::CtrlW),
                b'u' | b'U' => return Some(KeyEvent::CtrlU),
                b'k' | b'K' => return Some(KeyEvent::CtrlK),
                _ => {}
            }
        }

        Some(KeyEvent::Char(ch))
    }
}
