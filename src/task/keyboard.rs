use core::{pin::Pin, task::{Poll, Context}};
use crossbeam_queue::ArrayQueue;
use alloc::sync::Arc;
use core::task::Waker;
use lazy_static::lazy_static;
use spin::Mutex;
use crate::print;
use crate::vga::{WRITER};

lazy_static! {
    pub static ref SCANCODE_QUEUE: Arc<ArrayQueue<u8>> = Arc::new(ArrayQueue::new(100));
    pub static ref WAKER: Mutex<Option<Waker>> = Mutex::new(None);
    pub static ref DYNAMIC_KEYMAP: Mutex<[u8; 256]> = Mutex::new([0x3F; 256]);
}

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
    let mut is_extended = false;
    let mut shift_pressed = false;

    loop {
        let scancode = ScancodeStream::new().await;
        
        crate::println!("QEMU_LOG: Received scancode -> {:#04X}", scancode);
        
        if scancode == 0xE0 {
            is_extended = true;
            continue;
        }

        let is_break = scancode >= 0x80;
        let real_scancode = scancode & 0x7F; 

        match (is_extended, real_scancode) {
            (false, 0x2A) | (false, 0x36) => { shift_pressed = !is_break; is_extended = false; continue; }, // Shift
            _ => {}
        }
        
        is_extended = false;

        if !is_break {
            // Retrieve active keymap dynamically
            let map_index = if shift_pressed { real_scancode as usize + 128 } else { real_scancode as usize };
            let char_to_print = DYNAMIC_KEYMAP.lock()[map_index];

            if char_to_print != 0x3F { 
                if char_to_print == 0x0A { 
                    crate::println!("");
                } else if char_to_print == 0x08 { 
                    WRITER.lock().backspace();
                } else { 
                    print!("{}", (char_to_print as char));
                }
            }
        }
    }
}
