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
    let mut is_extended = false;
    let mut shift_pressed = false;

    crate::vga::init_cli();

    loop {
        let scancode = ScancodeStream::new().await;

        crate::serial_println!("QEMU_LOG: Received scancode -> {:#04X}", scancode);

        if KEYMAP_OVERRIDE_ACTIVE.load(core::sync::atomic::Ordering::Relaxed) {
            if scancode == 0xE0 {
                is_extended = true;
                continue;
            }

            let is_break = scancode >= 0x80;
            let real_scancode = scancode & 0x7F;

            match (is_extended, real_scancode) {
                (false, 0x2A) | (false, 0x36) => {
                    shift_pressed = !is_break;
                    is_extended = false;
                    continue;
                }
                _ => {}
            }

            is_extended = false;

            if !is_break {
                let map_index = if shift_pressed { real_scancode as usize + 128 } else { real_scancode as usize };
                let char_to_print = DYNAMIC_KEYMAP.lock()[map_index];
                handle_input_byte(char_to_print);
            }
            continue;
        }

        if let Some(event) = keyboard.process_scancode(scancode) {
            match event {
                crate::keyboard::KeyEvent::Char(byte) => handle_input_byte(byte),
                crate::keyboard::KeyEvent::Enter => submit_cli_command(),
                crate::keyboard::KeyEvent::Backspace => WRITER.lock().pop_input_char(),
                _ => {}
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
        0x08 => WRITER.lock().pop_input_char(),
        _ => WRITER.lock().push_input_char(byte),
    }
}

fn submit_cli_command() {
    if let Some(command) = WRITER.lock().submit_input() {
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
        _ => crate::println!("[CLI] Unknown command: {}", command),
    }

    crate::vga::init_cli();
}
