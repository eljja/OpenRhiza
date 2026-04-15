// src/vga.rs
use alloc::string::String;
use core::fmt;
use spin::Mutex;
use lazy_static::lazy_static;

const VGA_BUFFER_ADDR: usize = 0xb8000;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
const LOG_START_ROW: usize = 2;
const LOG_END_ROW: usize = VGA_HEIGHT - 2;
const INPUT_ROW: usize = VGA_HEIGHT - 1;
const INPUT_PROMPT: &[u8] = b"cli> ";
const INPUT_CAPACITY: usize = VGA_WIDTH - INPUT_PROMPT.len();

lazy_static! {
    pub static ref WRITER: Mutex<VgaWriter> = Mutex::new(VgaWriter {
        column_position: 0,
        row_position: LOG_START_ROW, // Leave room for bootloader logs
        color_code: 0x0A, // Light Green on Black (Matrix style)
        input_buffer: [b' '; INPUT_CAPACITY],
        input_len: 0,
        input_cursor_pos: 0,
        buffer: unsafe {
            let offset = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET;
            &mut *((offset + VGA_BUFFER_ADDR as u64) as *mut Buffer)
        },
        history: alloc::vec::Vec::new(),
        command_history: alloc::vec::Vec::new(),
        history_index: 0,
        scroll_offset: 0,
        saved_active_view: [[ScreenChar { ascii_character: b' ', color_code: 0x0A }; VGA_WIDTH]; 22],
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: u8,
}

#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; VGA_WIDTH]; VGA_HEIGHT],
}

pub struct VgaWriter {
    column_position: usize,
    row_position: usize,
    color_code: u8,
    input_buffer: [u8; INPUT_CAPACITY],
    input_len: usize,
    input_cursor_pos: usize,
    buffer: &'static mut Buffer,
    history: alloc::vec::Vec<[ScreenChar; VGA_WIDTH]>,
    command_history: alloc::vec::Vec<alloc::string::String>,
    history_index: usize,
    scroll_offset: usize,
    saved_active_view: [[ScreenChar; VGA_WIDTH]; 22],
}

impl VgaWriter {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= VGA_WIDTH {
                    self.new_line();
                }
                
                let row = self.row_position;
                let col = self.column_position;
                let color_code = self.color_code;
                
                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code,
                };
                self.column_position += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                _ => self.write_byte(0xfe),
            }
        }
    }

    fn new_line(&mut self) {
        if self.scroll_offset > 0 { self.snap_to_bottom(); }

        if self.row_position < LOG_END_ROW {
            self.row_position += 1;
        } else {
            // Push old line to history
            let mut top_line = [ScreenChar { ascii_character: b' ', color_code: self.color_code }; VGA_WIDTH];
            for col in 0..VGA_WIDTH {
                top_line[col] = self.buffer.chars[LOG_START_ROW][col];
            }
            self.history.push(top_line);
            if self.history.len() > 1000 {
                self.history.remove(0); // Arbitrary limit
            }

            for row in LOG_START_ROW + 1..=LOG_END_ROW {
                for col in 0..VGA_WIDTH {
                    let character = self.buffer.chars[row][col];
                    self.buffer.chars[row - 1][col] = character;
                }
            }
            self.clear_row(LOG_END_ROW);
        }
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..VGA_WIDTH {
            self.buffer.chars[row][col] = blank;
        }
    }
    
    pub fn backspace(&mut self) {
        if self.column_position > 0 {
            self.column_position -= 1;
            let row = self.row_position;
            let col = self.column_position;
            self.buffer.chars[row][col] = ScreenChar {
                ascii_character: b' ',
                color_code: self.color_code,
            };
        }
    }

    pub fn init_cli(&mut self) {
        self.render_input_line();
    }

    pub fn cursor_left(&mut self) {
        self.snap_to_bottom();
        if self.input_cursor_pos > 0 {
            self.input_cursor_pos -= 1;
            self.render_input_line();
        }
    }

    pub fn cursor_right(&mut self) {
        self.snap_to_bottom();
        if self.input_cursor_pos < self.input_len {
            self.input_cursor_pos += 1;
            self.render_input_line();
        }
    }

    pub fn push_input_char(&mut self, byte: u8) {
        self.snap_to_bottom();
        if self.input_len >= INPUT_CAPACITY {
            return;
        }
        for i in (self.input_cursor_pos..self.input_len).rev() {
            self.input_buffer[i + 1] = self.input_buffer[i];
        }
        self.input_buffer[self.input_cursor_pos] = byte;
        self.input_len += 1;
        self.input_cursor_pos += 1;
        self.render_input_line();
    }

    pub fn pop_input_char(&mut self) {
        self.snap_to_bottom();
        if self.input_cursor_pos == 0 {
            return;
        }
        for i in self.input_cursor_pos..self.input_len {
            self.input_buffer[i - 1] = self.input_buffer[i];
        }
        self.input_len -= 1;
        self.input_cursor_pos -= 1;
        self.input_buffer[self.input_len] = b' ';
        self.render_input_line();
    }

    pub fn delete_char(&mut self) {
        self.snap_to_bottom();
        if self.input_cursor_pos >= self.input_len {
            return;
        }
        for i in self.input_cursor_pos + 1..self.input_len {
            self.input_buffer[i - 1] = self.input_buffer[i];
        }
        self.input_len -= 1;
        self.input_buffer[self.input_len] = b' ';
        self.render_input_line();
    }

    pub fn home(&mut self) {
        self.snap_to_bottom();
        self.input_cursor_pos = 0;
        self.render_input_line();
    }

    pub fn end(&mut self) {
        self.snap_to_bottom();
        self.input_cursor_pos = self.input_len;
        self.render_input_line();
    }

    pub fn cancel_line(&mut self) {
        self.snap_to_bottom();
        self.input_buffer.fill(b' ');
        self.input_len = 0;
        self.input_cursor_pos = 0;
        self.render_input_line();
    }

    pub fn clear_before_cursor(&mut self) {
        self.snap_to_bottom();
        if self.input_cursor_pos == 0 { return; }
        
        let removed = self.input_cursor_pos;
        for i in self.input_cursor_pos..self.input_len {
            self.input_buffer[i - removed] = self.input_buffer[i];
        }
        self.input_len -= removed;
        self.input_cursor_pos = 0;
        self.render_input_line();
    }

    pub fn clear_after_cursor(&mut self) {
        self.snap_to_bottom();
        if self.input_cursor_pos >= self.input_len { return; }
        
        for i in self.input_cursor_pos..self.input_len {
            self.input_buffer[i] = b' ';
        }
        self.input_len = self.input_cursor_pos;
        self.render_input_line();
    }

    pub fn delete_word(&mut self) {
        self.snap_to_bottom();
        if self.input_cursor_pos == 0 { return; }
        
        let mut target = self.input_cursor_pos;
        while target > 0 && self.input_buffer[target - 1] == b' ' {
            target -= 1;
        }
        while target > 0 && self.input_buffer[target - 1] != b' ' {
            target -= 1;
        }
        
        let removed = self.input_cursor_pos - target;
        for i in self.input_cursor_pos..self.input_len {
            self.input_buffer[i - removed] = self.input_buffer[i];
        }
        self.input_len -= removed;
        self.input_cursor_pos = target;
        self.render_input_line();
    }

    pub fn submit_input(&mut self) -> Option<String> {
        self.snap_to_bottom();
        let mut command = String::new();
        if self.input_len > 0 {
            if let Ok(s) = core::str::from_utf8(&self.input_buffer[..self.input_len]) {
                command.push_str(s.trim());
            }
        }
        
        self.input_buffer[..self.input_len].fill(b' ');
        self.input_len = 0;
        self.input_cursor_pos = 0;
        self.render_input_line();

        if !command.is_empty() {
            self.command_history.push(command.clone());
            self.history_index = self.command_history.len();
            Some(command)
        } else {
            None
        }
    }

    pub fn history_up(&mut self) {
        self.snap_to_bottom();
        if self.command_history.is_empty() || self.history_index == 0 { return; }
        self.history_index -= 1;
        let cmd = self.command_history[self.history_index].clone();
        self.set_input_line(&cmd);
    }

    pub fn history_down(&mut self) {
        self.snap_to_bottom();
        if self.history_index < self.command_history.len() {
            self.history_index += 1;
            if self.history_index == self.command_history.len() {
                self.input_buffer.fill(b' ');
                self.input_len = 0;
                self.input_cursor_pos = 0;
                self.render_input_line();
            } else {
                let cmd = self.command_history[self.history_index].clone();
                self.set_input_line(&cmd);
            }
        }
    }

    fn set_input_line(&mut self, s: &str) {
        self.input_buffer.fill(b' ');
        let bytes = s.as_bytes();
        self.input_len = core::cmp::min(bytes.len(), INPUT_CAPACITY);
        self.input_buffer[..self.input_len].copy_from_slice(&bytes[..self.input_len]);
        self.input_cursor_pos = self.input_len;
        self.render_input_line();
    }

    pub fn snap_to_bottom(&mut self) {
        if self.scroll_offset == 0 { return; }
        self.scroll_offset = 0;
        
        let screen_height = LOG_END_ROW - LOG_START_ROW + 1;
        for r in 0..screen_height {
            for c in 0..VGA_WIDTH {
                self.buffer.chars[LOG_START_ROW + r][c] = self.saved_active_view[r][c];
            }
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.history.len();
        if max_scroll == 0 || self.scroll_offset == max_scroll { return; }
        
        // If initiating scroll, save the state of the active bottom view
        let screen_height = LOG_END_ROW - LOG_START_ROW + 1;
        if self.scroll_offset == 0 {
            for r in 0..screen_height {
                for c in 0..VGA_WIDTH {
                    self.saved_active_view[r][c] = self.buffer.chars[LOG_START_ROW + r][c];
                }
            }
        }
        
        self.scroll_offset = core::cmp::min(self.scroll_offset + lines, max_scroll);
        self.render_scroll();
    }

    pub fn scroll_down(&mut self, lines: usize) {
        if self.scroll_offset == 0 { return; }
        if self.scroll_offset <= lines {
            self.snap_to_bottom();
        } else {
            self.scroll_offset -= lines;
            self.render_scroll();
        }
    }

    fn render_scroll(&mut self) {
        let screen_height = LOG_END_ROW - LOG_START_ROW + 1;
        for r in 0..screen_height {
            let row_logic_idx = (self.history.len() + r) as i32 - self.scroll_offset as i32;
            let display_row = LOG_START_ROW + r;
            
            for c in 0..VGA_WIDTH {
                if row_logic_idx < 0 {
                    // Blank space if scrolled above history
                    self.buffer.chars[display_row][c] = ScreenChar { ascii_character: b' ', color_code: self.color_code };
                } else if (row_logic_idx as usize) < self.history.len() {
                    // Pull from history
                    self.buffer.chars[display_row][c] = self.history[row_logic_idx as usize][c];
                } else {
                    // Pull from the saved active screen for lines below history
                    let live_idx = (row_logic_idx as usize) - self.history.len();
                    if live_idx < screen_height {
                        self.buffer.chars[display_row][c] = self.saved_active_view[live_idx][c];
                    }
                }
            }
        }
    }

    pub fn clear_log_area(&mut self) {
        for row in LOG_START_ROW..=LOG_END_ROW {
            self.clear_row(row);
        }
        self.row_position = LOG_START_ROW;
        self.column_position = 0;
        self.render_input_line();
    }

    fn render_input_line(&mut self) {
        self.clear_row(INPUT_ROW);

        for (idx, byte) in INPUT_PROMPT.iter().enumerate() {
            self.buffer.chars[INPUT_ROW][idx] = ScreenChar {
                ascii_character: *byte,
                color_code: self.color_code,
            };
        }

        let prompt_len = INPUT_PROMPT.len();
        for idx in 0..INPUT_CAPACITY {
            let mut ch = if idx < self.input_len {
                self.input_buffer[idx]
            } else {
                b' '
            };
            
            let mut color = self.color_code;
            if idx == self.input_cursor_pos {
                color = 0x70; // Gray background, black text
                if ch == b' ' { ch = b'_'; }
            }

            self.buffer.chars[INPUT_ROW][prompt_len + idx] = ScreenChar {
                ascii_character: ch,
                color_code: color,
            };
        }
    }
}

impl fmt::Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
    crate::arch::x86_64::serial::_print(args);
}

pub fn init_cli() {
    WRITER.lock().init_cli();
}
