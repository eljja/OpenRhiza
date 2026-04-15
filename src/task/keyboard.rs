use core::{pin::Pin, task::{Poll, Context}};
use crossbeam_queue::ArrayQueue;
use alloc::sync::Arc;
use core::task::Waker;
use core::sync::atomic::AtomicBool;
use lazy_static::lazy_static;
use spin::Mutex;
use crate::vga::{WRITER};

lazy_static! {
    pub static ref SCANCODE_QUEUE: Arc<ArrayQueue<u8>> = Arc::new(ArrayQueue::new(100));
    pub static ref WAKER: Mutex<Option<Waker>> = Mutex::new(None);
    pub static ref DYNAMIC_KEYMAP: Mutex<[u8; 256]> = Mutex::new([0x3F; 256]);
    pub static ref PROMPT_QUEUE: Arc<ArrayQueue<alloc::string::String>> = Arc::new(ArrayQueue::new(10));
}

pub static KEYMAP_OVERRIDE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Called by the keyboard interrupt handler
pub fn add_scancode(scancode: u8) {
    if let Ok(_) = SCANCODE_QUEUE.push(scancode) {
        if let Some(waker) = WAKER.lock().take() {
            waker.wake();
        }
    } else {
        crate::println!("WARNING: scancode queue full; dropping keyboard input");
    }
}

pub struct ScancodeStream {}

impl ScancodeStream {
    pub fn new() -> Self {
        ScancodeStream {}
    }
}

impl core::future::Future for ScancodeStream {
    type Output = u8;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let queue = SCANCODE_QUEUE.clone();
        if let Some(scancode) = queue.pop() {
            return Poll::Ready(scancode);
        }

        *WAKER.lock() = Some(cx.waker().clone());
        if let Some(scancode) = queue.pop() {
            WAKER.lock().take();
            return Poll::Ready(scancode);
        }

        Poll::Pending
    }
}

pub async fn keyboard_task() {
    let mut keyboard = crate::keyboard::KeyboardState::new();

    crate::vga::init_cli();

    loop {
        let scancode = ScancodeStream::new().await;

        crate::serial_println!("QEMU_LOG: Received scancode -> {:#04X}", scancode);

        if let Some(event) = keyboard.process_scancode(scancode) {
            match event {
                crate::keyboard::KeyEvent::Char(byte) => handle_input_byte(byte),
                crate::keyboard::KeyEvent::Enter => submit_cli_command(),
                crate::keyboard::KeyEvent::Backspace => crate::vga::WRITER.lock().pop_input_char(),
                crate::keyboard::KeyEvent::Delete => crate::vga::WRITER.lock().delete_char(),
                crate::keyboard::KeyEvent::PageUp => crate::vga::WRITER.lock().scroll_up(10),
                crate::keyboard::KeyEvent::PageDown => crate::vga::WRITER.lock().scroll_down(10),
                crate::keyboard::KeyEvent::ArrowLeft => crate::vga::WRITER.lock().cursor_left(),
                crate::keyboard::KeyEvent::ArrowRight => crate::vga::WRITER.lock().cursor_right(),
                crate::keyboard::KeyEvent::ArrowUp => crate::vga::WRITER.lock().history_up(),
                crate::keyboard::KeyEvent::ArrowDown => crate::vga::WRITER.lock().history_down(),
                crate::keyboard::KeyEvent::Home => crate::vga::WRITER.lock().home(),
                crate::keyboard::KeyEvent::End => crate::vga::WRITER.lock().end(),
                crate::keyboard::KeyEvent::CtrlC => {
                    crate::println!("^C");
                    crate::vga::WRITER.lock().cancel_line();
                },
                crate::keyboard::KeyEvent::CtrlL => crate::vga::WRITER.lock().clear_log_area(),
                crate::keyboard::KeyEvent::CtrlU => crate::vga::WRITER.lock().clear_before_cursor(),
                crate::keyboard::KeyEvent::CtrlK => crate::vga::WRITER.lock().clear_after_cursor(),
                crate::keyboard::KeyEvent::CtrlW => crate::vga::WRITER.lock().delete_word(),
                _ => { crate::vga::WRITER.lock().snap_to_bottom(); } // Snap to bottom on any other key
            }
        }
    }
}

fn handle_input_byte(byte: u8) {
    if byte == 0x3F {
        return;
    }

    match byte {
        b'\n' => submit_cli_command(),
        0x08 => crate::vga::WRITER.lock().pop_input_char(),
        _ => crate::vga::WRITER.lock().push_input_char(byte),
    }
}

fn submit_cli_command() {
    let command_opt = WRITER.lock().submit_input();
    if let Some(command) = command_opt {
        handle_cli_command(command.as_str());
    }
}

fn handle_cli_command(command: &str) {
    if command.is_empty() {
        crate::vga::init_cli();
        return;
    }

    crate::println!("cli> {}", command);

    match command {
        "help" => crate::println!("[CLI] Commands: help, clear, status"),
        "clear" => WRITER.lock().clear_log_area(),
        "status" => {
            crate::println!("[CLI] Keyboard input ready.");
            crate::println!("[CLI] Serial debug logs remain on COM1 only.");
        }
        _ => {
            // Unrecognized native command. Assume it's an LLM Prompt.
            if let Ok(_) = PROMPT_QUEUE.push(alloc::string::String::from(command)) {
                crate::println!("[CLI] Prompt pushed to background queue. Awaiting LLM...");
            } else {
                crate::println!("[CLI] Prompt queue full! Wait for the LLM to finish.");
            }
        }
    }

    crate::vga::init_cli();
}
