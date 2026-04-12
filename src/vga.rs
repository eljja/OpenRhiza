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
        buffer: unsafe {
            let offset = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET;
            &mut *((offset + VGA_BUFFER_ADDR as u64) as *mut Buffer)
        },
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
    buffer: &'static mut Buffer,
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
        if self.row_position < LOG_END_ROW {
            self.row_position += 1;
        } else {
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

    pub fn push_input_char(&mut self, byte: u8) {
        if self.input_len >= INPUT_CAPACITY {
            return;
        }
        self.input_buffer[self.input_len] = byte;
        self.input_len += 1;
        self.render_input_line();
    }

    pub fn pop_input_char(&mut self) {
        if self.input_len == 0 {
            return;
        }
        self.input_len -= 1;
        self.input_buffer[self.input_len] = b' ';
        self.render_input_line();
    }

    pub fn submit_input(&mut self) -> Option<String> {
        let line = core::str::from_utf8(&self.input_buffer[..self.input_len]).ok()?.trim();
        let command = String::from(line);
        self.input_buffer[..self.input_len].fill(b' ');
        self.input_len = 0;
        self.render_input_line();
        Some(command)
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

        for idx in 0..INPUT_CAPACITY {
            let ch = if idx < self.input_len {
                self.input_buffer[idx]
            } else {
                b' '
            };
            self.buffer.chars[INPUT_ROW][INPUT_PROMPT.len() + idx] = ScreenChar {
                ascii_character: ch,
                color_code: self.color_code,
            };
        }

        if self.input_len < INPUT_CAPACITY {
            self.buffer.chars[INPUT_ROW][INPUT_PROMPT.len() + self.input_len] = ScreenChar {
                ascii_character: b'_',
                color_code: self.color_code,
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
