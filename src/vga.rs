// src/vga.rs
use alloc::string::String;
use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;

const VGA_BUFFER_ADDR: usize = 0xb8000;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
const LOG_START_ROW: usize = 2;
const STATUS_ROW: usize = 0;
const INPUT_PROMPT: &[u8] = b"input> ";
const MAX_INPUT_ROWS: usize = VGA_HEIGHT - LOG_START_ROW - 1;
const INPUT_REGION_MIN_START: usize = VGA_HEIGHT - MAX_INPUT_ROWS;
const INPUT_CAPACITY: usize = VGA_WIDTH * MAX_INPUT_ROWS - INPUT_PROMPT.len();
const MAX_LOG_LINES: usize = 2048;
const MAX_COMMAND_HISTORY: usize = 64;
const PROMPT_COLOR: u8 = 0x0A;
const LOG_COLOR: u8 = 0x08;
const USER_ECHO_COLOR: u8 = 0x0A;
const RESULT_COLOR: u8 = 0x0E;
const MOUSE_POINTER_COLOR: u8 = 0x70;
const MOUSE_SENSITIVITY_DIVISOR: i16 = 4;

lazy_static! {
    pub static ref WRITER: Mutex<VgaWriter> = Mutex::new(VgaWriter {
        column_position: 0,
        row_position: LOG_START_ROW,
        color_code: PROMPT_COLOR,
        input_buffer: [b' '; INPUT_CAPACITY],
        input_len: 0,
        input_cursor: 0,
        log_lines: [[b' '; VGA_WIDTH]; MAX_LOG_LINES],
        log_line_colors: [LOG_COLOR; MAX_LOG_LINES],
        log_line_count: 1,
        log_current_line: 0,
        scroll_offset: 0,
        command_history: [[b' '; INPUT_CAPACITY]; MAX_COMMAND_HISTORY],
        command_history_lens: [0; MAX_COMMAND_HISTORY],
        command_history_count: 0,
        command_history_write: 0,
        history_index: None,
        history_snapshot: [b' '; INPUT_CAPACITY],
        history_snapshot_len: 0,
        runtime_seconds: 0,
        mouse_enabled: false,
        mouse_col: 0,
        mouse_row: LOG_START_ROW,
        mouse_buttons: 0,
        mouse_dx_accum: 0,
        mouse_dy_accum: 0,
        scrollbar_drag_active: false,
        mouse_overlay_active: false,
        mouse_drawn_col: 0,
        mouse_drawn_row: LOG_START_ROW,
        mouse_saved_char: ScreenChar {
            ascii_character: b' ',
            color_code: LOG_COLOR,
        },
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
    input_cursor: usize,
    log_lines: [[u8; VGA_WIDTH]; MAX_LOG_LINES],
    log_line_colors: [u8; MAX_LOG_LINES],
    log_line_count: usize,
    log_current_line: usize,
    scroll_offset: usize,
    command_history: [[u8; INPUT_CAPACITY]; MAX_COMMAND_HISTORY],
    command_history_lens: [usize; MAX_COMMAND_HISTORY],
    command_history_count: usize,
    command_history_write: usize,
    history_index: Option<usize>,
    history_snapshot: [u8; INPUT_CAPACITY],
    history_snapshot_len: usize,
    runtime_seconds: u64,
    mouse_enabled: bool,
    mouse_col: usize,
    mouse_row: usize,
    mouse_buttons: u8,
    mouse_dx_accum: i16,
    mouse_dy_accum: i16,
    scrollbar_drag_active: bool,
    mouse_overlay_active: bool,
    mouse_drawn_col: usize,
    mouse_drawn_row: usize,
    mouse_saved_char: ScreenChar,
    buffer: &'static mut Buffer,
}

impl VgaWriter {
    fn active_input_rows(&self) -> usize {
        let total_cells = INPUT_PROMPT.len()
            .saturating_add(self.input_len)
            .saturating_add(1);
        ((total_cells.saturating_add(VGA_WIDTH - 1)) / VGA_WIDTH)
            .clamp(1, MAX_INPUT_ROWS)
    }

    fn input_start_row(&self) -> usize {
        VGA_HEIGHT - self.active_input_rows()
    }

    fn log_end_row(&self) -> usize {
        self.input_start_row().saturating_sub(1)
    }

    fn log_visible_rows(&self) -> usize {
        self.log_end_row().saturating_sub(LOG_START_ROW) + 1
    }

    fn clamp_mouse_to_log_area(&mut self) {
        let log_end_row = self.log_end_row();
        if self.mouse_row < LOG_START_ROW {
            self.mouse_row = LOG_START_ROW;
        } else if self.mouse_row > log_end_row {
            self.mouse_row = log_end_row;
        }
    }

    fn clear_mouse_overlay(&mut self) {
        if !self.mouse_overlay_active {
            return;
        }
        if self.mouse_drawn_row < VGA_HEIGHT && self.mouse_drawn_col < VGA_WIDTH {
            self.buffer.chars[self.mouse_drawn_row][self.mouse_drawn_col] = self.mouse_saved_char;
        }
        self.mouse_overlay_active = false;
    }

    fn write_fmt_with_color(&mut self, args: fmt::Arguments, color: u8) -> fmt::Result {
        struct ColorAdapter<'a> {
            writer: &'a mut VgaWriter,
            color: u8,
        }

        impl fmt::Write for ColorAdapter<'_> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                self.writer.write_string_with_color(s, self.color);
                Ok(())
            }
        }

        let mut adapter = ColorAdapter {
            writer: self,
            color,
        };
        fmt::Write::write_fmt(&mut adapter, args)
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.push_log_byte(byte, LOG_COLOR);
        self.render_log_view();
    }

    pub fn write_string(&mut self, s: &str) {
        self.write_string_with_color(s, LOG_COLOR);
    }

    pub fn write_string_with_color(&mut self, s: &str, color: u8) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' => self.push_log_byte(byte, color),
                _ => self.push_log_byte(0xfe, color),
            }
        }
        self.render_log_view();
    }

    fn new_line(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset = (self.scroll_offset + 1).min(self.max_scroll_offset().saturating_add(1));
        }
        if self.log_line_count < MAX_LOG_LINES {
            self.log_current_line = self.log_line_count;
            self.log_line_count += 1;
        } else {
            self.log_current_line = (self.log_current_line + 1) % MAX_LOG_LINES;
        }
        self.log_lines[self.log_current_line] = blank_line();
        self.log_line_colors[self.log_current_line] = LOG_COLOR;
        self.row_position = self.log_end_row();
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
        self.enable_hardware_cursor();
        self.runtime_seconds = 0;
        self.render_runtime(0);
        self.render_log_view();
        self.render_input_line();
    }

    fn enable_hardware_cursor(&self) {
        let mut port_3d4 = x86_64::instructions::port::Port::<u8>::new(0x3D4);
        let mut port_3d5 = x86_64::instructions::port::Port::<u8>::new(0x3D5);
        unsafe {
            port_3d4.write(0x0A);
            let val = port_3d5.read();
            port_3d5.write((val & 0xC0) | 13);
            port_3d4.write(0x0B);
            let val = port_3d5.read();
            port_3d5.write((val & 0xE0) | 15);
        }
    }

    pub fn push_input_char(&mut self, byte: u8) {
        if self.input_len >= INPUT_CAPACITY {
            return;
        }
        for i in (self.input_cursor..self.input_len).rev() {
            self.input_buffer[i + 1] = self.input_buffer[i];
        }
        self.input_buffer[self.input_cursor] = byte;
        self.input_len += 1;
        self.input_cursor += 1;
        self.render_input_line();
    }

    pub fn pop_input_char(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        for i in self.input_cursor..self.input_len {
            self.input_buffer[i - 1] = self.input_buffer[i];
        }
        self.input_len -= 1;
        self.input_cursor -= 1;
        self.input_buffer[self.input_len] = b' ';
        self.render_input_line();
    }

    pub fn submit_input(&mut self) -> Option<String> {
        let line = core::str::from_utf8(&self.input_buffer[..self.input_len]).ok()?.trim();
        let command = String::from(line);
        if !command.is_empty() {
            let should_push = !self.last_history_entry_equals(command.as_bytes());
            if should_push {
                self.push_history_entry(command.as_bytes());
            }
        }
        self.input_buffer[..self.input_len].fill(b' ');
        self.input_len = 0;
        self.input_cursor = 0;
        self.history_index = None;
        self.history_snapshot.fill(b' ');
        self.history_snapshot_len = 0;
        self.render_input_line();
        Some(command)
    }

    pub fn clear_log_area(&mut self) {
        self.log_lines = [[b' '; VGA_WIDTH]; MAX_LOG_LINES];
        self.log_line_colors = [LOG_COLOR; MAX_LOG_LINES];
        self.log_line_count = 1;
        self.log_current_line = 0;
        self.scroll_offset = 0;
        self.row_position = LOG_START_ROW;
        self.column_position = 0;
        self.render_log_view();
        self.render_input_line();
    }

    pub fn delete_char(&mut self) {
        if self.input_cursor >= self.input_len {
            return;
        }
        for i in self.input_cursor + 1..self.input_len {
            self.input_buffer[i - 1] = self.input_buffer[i];
        }
        self.input_len -= 1;
        self.input_buffer[self.input_len] = b' ';
        self.render_input_line();
    }

    pub fn cursor_left(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
            self.render_input_line();
        }
    }

    pub fn cursor_right(&mut self) {
        if self.input_cursor < self.input_len {
            self.input_cursor += 1;
            self.render_input_line();
        }
    }

    pub fn history_up(&mut self) {
        if self.command_history_count == 0 {
            return;
        }

        let next_index = match self.history_index {
            Some(0) => 0,
            Some(index) => index.saturating_sub(1),
            None => {
                self.history_snapshot.fill(b' ');
                self.history_snapshot[..self.input_len]
                    .copy_from_slice(&self.input_buffer[..self.input_len]);
                self.history_snapshot_len = self.input_len;
                self.command_history_count - 1
            }
        };

        self.history_index = Some(next_index);
        if let Some((entry, len)) = self.history_entry(next_index) {
            let mut snapshot = [b' '; INPUT_CAPACITY];
            snapshot[..len].copy_from_slice(&entry[..len]);
            self.set_input_from_bytes(&snapshot, len);
        }
    }

    pub fn history_down(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };

        if index + 1 < self.command_history_count {
            let next_index = index + 1;
            self.history_index = Some(next_index);
            if let Some((entry, len)) = self.history_entry(next_index) {
                let mut snapshot = [b' '; INPUT_CAPACITY];
                snapshot[..len].copy_from_slice(&entry[..len]);
                self.set_input_from_bytes(&snapshot, len);
            }
        } else {
            self.history_index = None;
            let snapshot = self.history_snapshot;
            let len = self.history_snapshot_len;
            self.set_input_from_bytes(&snapshot, len);
        }
    }

    pub fn home(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor = 0;
            self.render_input_line();
        }
    }

    pub fn end(&mut self) {
        if self.input_cursor < self.input_len {
            self.input_cursor = self.input_len;
            self.render_input_line();
        }
    }

    pub fn cancel_line(&mut self) {
        self.input_buffer[..self.input_len].fill(b' ');
        self.input_len = 0;
        self.input_cursor = 0;
        self.render_input_line();
    }

    pub fn clear_before_cursor(&mut self) {
        self.cancel_line();
    }

    pub fn clear_after_cursor(&mut self) {}

    pub fn delete_word(&mut self) {
        while self.input_cursor > 0 && self.input_buffer[self.input_cursor - 1] == b' ' {
            self.pop_input_char();
        }
        while self.input_cursor > 0 && self.input_buffer[self.input_cursor - 1] != b' ' {
            self.pop_input_char();
        }
    }

    pub fn snap_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.render_log_view();
    }

    pub fn scroll_up(&mut self, lines: usize) {
        let max_offset = self.max_scroll_offset();
        self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
        self.render_log_view();
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        self.render_log_view();
    }

    fn render_input_line(&mut self) {
        let input_start_row = self.input_start_row();
        let active_input_rows = self.active_input_rows();
        let total_cells = VGA_WIDTH * active_input_rows;

        for row in INPUT_REGION_MIN_START..VGA_HEIGHT {
            self.clear_row(row);
        }

        self.render_log_view();

        for (idx, byte) in INPUT_PROMPT.iter().enumerate() {
            let absolute = idx;
            let row = input_start_row + absolute / VGA_WIDTH;
            let col = absolute % VGA_WIDTH;
            self.buffer.chars[row][col] = ScreenChar {
                ascii_character: *byte,
                color_code: self.color_code,
            };
        }

        for idx in 0..self.input_len {
            let absolute = INPUT_PROMPT.len() + idx;
            if absolute >= total_cells {
                break;
            }

            let row = input_start_row + absolute / VGA_WIDTH;
            let col = absolute % VGA_WIDTH;
            self.buffer.chars[row][col] = ScreenChar {
                ascii_character: self.input_buffer[idx],
                color_code: self.color_code,
            };
        }

        self.update_hardware_cursor();
    }

    fn update_hardware_cursor(&self) {
        let input_start_row = self.input_start_row();
        let cursor_absolute = INPUT_PROMPT.len() + self.input_cursor;
        let row = input_start_row + cursor_absolute / VGA_WIDTH;
        let col = cursor_absolute % VGA_WIDTH;

        let pos = (row * VGA_WIDTH + col) as u16;
        let mut port_3d4 = x86_64::instructions::port::Port::<u8>::new(0x3D4);
        let mut port_3d5 = x86_64::instructions::port::Port::<u8>::new(0x3D5);
        unsafe {
            port_3d4.write(0x0F);
            port_3d5.write((pos & 0x00FF) as u8);
            port_3d4.write(0x0E);
            port_3d5.write((pos >> 8) as u8);
        }
    }

    pub fn render_runtime(&mut self, total_seconds: u64) {
        self.runtime_seconds = total_seconds;
        self.render_status_row();
        self.render_mouse_overlay();
        self.update_hardware_cursor();
    }

    fn render_status_row(&mut self) {
        let total_seconds = self.runtime_seconds;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        let label = alloc::format!("running {:02}:{:02}:{:02}", hours, minutes, seconds);
        let bytes = label.as_bytes();
        let start_col = VGA_WIDTH.saturating_sub(bytes.len());

        for col in 0..VGA_WIDTH {
            self.buffer.chars[STATUS_ROW][col] = ScreenChar {
                ascii_character: b' ',
                color_code: self.color_code,
            };
        }

        for (idx, byte) in bytes.iter().enumerate() {
            let col = start_col + idx;
            if col < VGA_WIDTH {
                self.buffer.chars[STATUS_ROW][col] = ScreenChar {
                    ascii_character: *byte,
                    color_code: self.color_code,
                };
            }
        }

        let mouse_label = alloc::format!(
            "mouse {:03},{:02} {:03b}",
            self.mouse_col,
            self.mouse_row.saturating_sub(LOG_START_ROW),
            self.mouse_buttons & 0x07
        );
        for (idx, byte) in mouse_label.as_bytes().iter().enumerate() {
            if idx >= start_col.saturating_sub(1) {
                break;
            }
            self.buffer.chars[STATUS_ROW][idx] = ScreenChar {
                ascii_character: *byte,
                color_code: self.color_code,
            };
        }

        self.render_mouse_overlay();
        self.update_hardware_cursor();
    }

    fn push_log_byte(&mut self, byte: u8, color: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= VGA_WIDTH {
                    self.new_line();
                }

                self.log_line_colors[self.log_current_line] = color;
                self.log_lines[self.log_current_line][self.column_position] = byte;
                self.column_position += 1;
            }
        }
    }

    fn render_log_view(&mut self) {
        let log_end_row = self.log_end_row();
        let log_visible_rows = self.log_visible_rows();

        self.clear_mouse_overlay();
        self.clamp_mouse_to_log_area();

        for row in LOG_START_ROW..=log_end_row {
            self.clear_row(row);
        }

        let total_lines = self.log_line_count;
        let start_idx = if total_lines > log_visible_rows {
            total_lines - log_visible_rows - self.scroll_offset.min(self.max_scroll_offset())
        } else {
            0
        };

        for row_offset in 0..log_visible_rows {
            let row = LOG_START_ROW + row_offset;
            let Some(line) = self.log_line_by_logical_index(start_idx + row_offset) else {
                continue;
            };
            let line = *line;
            let color = self
                .log_line_color_by_logical_index(start_idx + row_offset)
                .unwrap_or(LOG_COLOR);

            for (col, byte) in line.iter().enumerate() {
                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: *byte,
                    color_code: color,
                };
            }
        }

        let max_offset = self.max_scroll_offset();
        if max_offset > 0 {
            let track_len = log_visible_rows.saturating_sub(1).max(1);
            let thumb_y = track_len - (self.scroll_offset * track_len) / max_offset;
            for row_offset in 0..log_visible_rows {
                let row = LOG_START_ROW + row_offset;
                let is_thumb = row_offset == thumb_y;
                let ch = if is_thumb { b'#' } else { b'|' };
                let color = if is_thumb { 0x0F } else { 0x08 };
                self.buffer.chars[row][VGA_WIDTH - 1] = ScreenChar {
                    ascii_character: ch,
                    color_code: color,
                };
            }
        }
    }

    fn max_scroll_offset(&self) -> usize {
        self.log_line_count.saturating_sub(self.log_visible_rows())
    }

    fn set_input_from_bytes(&mut self, value: &[u8], value_len: usize) {
        self.input_buffer.fill(b' ');
        let copy_len = value_len.min(INPUT_CAPACITY).min(value.len());
        self.input_buffer[..copy_len].copy_from_slice(&value[..copy_len]);
        self.input_len = copy_len;
        self.input_cursor = copy_len;
        self.render_input_line();
    }

    pub fn update_mouse_state(&mut self, dx: i8, dy: i8, buttons: u8, wheel: i8) {
        let mut view_changed = false;
        if wheel > 0 {
            self.scroll_up(wheel as usize);
            view_changed = true;
        } else if wheel < 0 {
            self.scroll_down((-wheel) as usize);
            view_changed = true;
        }

        self.mouse_enabled = true;
        self.mouse_dx_accum += dx as i16;
        self.mouse_dy_accum += dy as i16;

        let mut step_x = 0isize;
        let mut step_y = 0isize;

        while self.mouse_dx_accum >= MOUSE_SENSITIVITY_DIVISOR {
            self.mouse_dx_accum -= MOUSE_SENSITIVITY_DIVISOR;
            step_x += 1;
        }
        while self.mouse_dx_accum <= -MOUSE_SENSITIVITY_DIVISOR {
            self.mouse_dx_accum += MOUSE_SENSITIVITY_DIVISOR;
            step_x -= 1;
        }
        while self.mouse_dy_accum >= MOUSE_SENSITIVITY_DIVISOR {
            self.mouse_dy_accum -= MOUSE_SENSITIVITY_DIVISOR;
            step_y += 1;
        }
        while self.mouse_dy_accum <= -MOUSE_SENSITIVITY_DIVISOR {
            self.mouse_dy_accum += MOUSE_SENSITIVITY_DIVISOR;
            step_y -= 1;
        }

        let next_col = self.mouse_col as isize + step_x;
        let next_row = self.mouse_row as isize + step_y;
        let log_end_row = self.log_end_row();
        self.mouse_col = next_col.clamp(0, (VGA_WIDTH - 1) as isize) as usize;
        self.mouse_row = next_row.clamp(LOG_START_ROW as isize, log_end_row as isize) as usize;
        self.mouse_buttons = buttons & 0x07;

        let max_offset = self.max_scroll_offset();
        let left_click = (self.mouse_buttons & 0x01) != 0;

        if left_click {
            if !self.scrollbar_drag_active && self.mouse_col == VGA_WIDTH - 1 && max_offset > 0 {
                self.scrollbar_drag_active = true;
            }
            if self.scrollbar_drag_active && max_offset > 0 {
                let relative_y = self.mouse_row.saturating_sub(LOG_START_ROW);
                let track_len = self.log_visible_rows().saturating_sub(1).max(1);
                let inverse_y = track_len.saturating_sub(relative_y);
                let new_offset = (inverse_y * max_offset) / track_len;
                if new_offset.min(max_offset) != self.scroll_offset {
                    view_changed = true;
                }
                self.scroll_offset = new_offset.min(max_offset);
            }
        } else {
            self.scrollbar_drag_active = false;
        }

        if view_changed {
            self.render_runtime(self.runtime_seconds);
            self.render_log_view();
            self.render_input_line();
        } else {
            self.render_status_row();
            self.render_mouse_overlay();
            self.update_hardware_cursor();
        }
    }

    fn render_mouse_overlay(&mut self) {
        self.clear_mouse_overlay();
        if !self.mouse_enabled {
            return;
        }
        if self.mouse_row >= VGA_HEIGHT || self.mouse_col >= VGA_WIDTH {
            return;
        }

        self.mouse_saved_char = self.buffer.chars[self.mouse_row][self.mouse_col];
        self.mouse_drawn_row = self.mouse_row;
        self.mouse_drawn_col = self.mouse_col;
        self.buffer.chars[self.mouse_row][self.mouse_col] = ScreenChar {
            ascii_character: b'X',
            color_code: MOUSE_POINTER_COLOR,
        };
        self.mouse_overlay_active = true;
    }

    fn history_entry(&self, logical_index: usize) -> Option<(&[u8], usize)> {
        if logical_index >= self.command_history_count {
            return None;
        }
        let physical = self.history_physical_index(logical_index);
        Some((&self.command_history[physical], self.command_history_lens[physical]))
    }

    fn history_physical_index(&self, logical_index: usize) -> usize {
        let oldest = if self.command_history_count < MAX_COMMAND_HISTORY {
            0
        } else {
            self.command_history_write
        };
        (oldest + logical_index) % MAX_COMMAND_HISTORY
    }

    fn push_history_entry(&mut self, value: &[u8]) {
        let physical = self.command_history_write;
        self.command_history[physical].fill(b' ');
        let copy_len = value.len().min(INPUT_CAPACITY);
        self.command_history[physical][..copy_len].copy_from_slice(&value[..copy_len]);
        self.command_history_lens[physical] = copy_len;

        if self.command_history_count < MAX_COMMAND_HISTORY {
            self.command_history_count += 1;
        }
        self.command_history_write = (self.command_history_write + 1) % MAX_COMMAND_HISTORY;
    }

    fn last_history_entry_equals(&self, value: &[u8]) -> bool {
        if self.command_history_count == 0 {
            return false;
        }

        let logical = self.command_history_count - 1;
        let physical = self.history_physical_index(logical);
        let len = self.command_history_lens[physical];
        len == value.len() && &self.command_history[physical][..len] == value
    }

    fn log_line_by_logical_index(&self, logical_index: usize) -> Option<&[u8; VGA_WIDTH]> {
        if logical_index >= self.log_line_count {
            return None;
        }

        let oldest = (self.log_current_line + MAX_LOG_LINES + 1 - self.log_line_count) % MAX_LOG_LINES;
        let physical = (oldest + logical_index) % MAX_LOG_LINES;
        Some(&self.log_lines[physical])
    }

    fn log_line_color_by_logical_index(&self, logical_index: usize) -> Option<u8> {
        if logical_index >= self.log_line_count {
            return None;
        }

        let oldest = (self.log_current_line + MAX_LOG_LINES + 1 - self.log_line_count) % MAX_LOG_LINES;
        let physical = (oldest + logical_index) % MAX_LOG_LINES;
        Some(self.log_line_colors[physical])
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

#[macro_export]
macro_rules! user_println {
    ($($arg:tt)*) => ($crate::vga::_print_with_color(format_args!("{}\n", format_args!($($arg)*)), $crate::vga::user_echo_color()));
}

#[macro_export]
macro_rules! result_println {
    ($($arg:tt)*) => ($crate::vga::_print_with_color(format_args!("{}\n", format_args!($($arg)*)), $crate::vga::result_color()));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    crate::arch::x86_64::serial::_print(args);
}

#[doc(hidden)]
pub fn _print_with_color(args: fmt::Arguments, color: u8) {
    if color != LOG_COLOR {
        let mut writer = WRITER.lock();
        writer.write_fmt_with_color(args, color).unwrap();
    }
    crate::arch::x86_64::serial::_print(args);
}

pub fn init_cli() {
    WRITER.lock().init_cli();
}

pub fn render_runtime(total_seconds: u64) {
    WRITER.lock().render_runtime(total_seconds);
}

pub const fn user_echo_color() -> u8 {
    USER_ECHO_COLOR
}

pub const fn result_color() -> u8 {
    RESULT_COLOR
}

fn blank_line() -> [u8; VGA_WIDTH] {
    [b' '; VGA_WIDTH]
}
