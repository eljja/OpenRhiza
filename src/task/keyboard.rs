use alloc::sync::Arc;
use core::sync::atomic::AtomicBool;
use core::task::Waker;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;
use spin::Mutex;

use crate::vga::WRITER;

lazy_static! {
    pub static ref SCANCODE_QUEUE: Arc<ArrayQueue<u8>> = Arc::new(ArrayQueue::new(100));
    pub static ref WAKER: Mutex<Option<Waker>> = Mutex::new(None);
    pub static ref DYNAMIC_KEYMAP: Mutex<[u8; 256]> = Mutex::new([0x3F; 256]);
}

pub static KEYMAP_OVERRIDE_ACTIVE: AtomicBool = AtomicBool::new(false);

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
    crate::println!("[Task] keyboard_task started");
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
                let map_index = if shift_pressed {
                    real_scancode as usize + 128
                } else {
                    real_scancode as usize
                };
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
                crate::keyboard::KeyEvent::ArrowUp => WRITER.lock().history_up(),
                crate::keyboard::KeyEvent::ArrowDown => WRITER.lock().history_down(),
                crate::keyboard::KeyEvent::Home => WRITER.lock().home(),
                crate::keyboard::KeyEvent::End => WRITER.lock().end(),
                crate::keyboard::KeyEvent::PageUp => WRITER.lock().scroll_up(10),
                crate::keyboard::KeyEvent::PageDown => WRITER.lock().scroll_down(10),
                crate::keyboard::KeyEvent::CtrlC => {
                    crate::println!("^C");
                    WRITER.lock().cancel_line();
                }
                crate::keyboard::KeyEvent::CtrlL => WRITER.lock().clear_log_area(),
                crate::keyboard::KeyEvent::CtrlU => WRITER.lock().clear_before_cursor(),
                crate::keyboard::KeyEvent::CtrlK => WRITER.lock().clear_after_cursor(),
                crate::keyboard::KeyEvent::CtrlW => WRITER.lock().delete_word(),
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

    crate::user_println!("cli> {}", command);

    if let Some(local_command) = command.strip_prefix('/') {
        match local_command {
            "help" => crate::result_println!("[CLI] Local commands: /help, /clear, /status, /nexus-fetch, /api-register, /api-register-http, /http-health, /https-health, /https-root, /api-hw, /api-driver, /api-all, /gemini-test, /driver-generate <match_key>, /driver-upload <match_key>, /driver-comment <driver_id> <text>, /driver-vote <driver_id> up|down"),
            "clear" => WRITER.lock().clear_log_area(),
            "status" => {
                crate::result_println!("[CLI] Keyboard input ready.");
                crate::result_println!("[CLI] Serial debug logs remain on COM1 only.");
                crate::result_println!("[CLI] Plain text without '/' is sent to Gemini.");
            }
            "nexus-fetch" => queue_api_command(crate::api_v1::ServiceApiCommand::NexusFetch, "nexus_fetch"),
            "api-register" => queue_api_command(crate::api_v1::ServiceApiCommand::Register, "register"),
            "api-register-http" => queue_api_command(crate::api_v1::ServiceApiCommand::RegisterHttp, "register_http"),
            "http-health" => queue_api_command(crate::api_v1::ServiceApiCommand::HealthHttp, "health_http"),
            "https-health" => queue_api_command(crate::api_v1::ServiceApiCommand::HealthHttps, "health_https"),
            "https-root" => queue_api_command(crate::api_v1::ServiceApiCommand::RootHttps, "root_https"),
            "api-hw" => queue_api_command(crate::api_v1::ServiceApiCommand::HardwareReport, "hardware_report"),
            "api-driver" => queue_api_command(crate::api_v1::ServiceApiCommand::DriverQuery, "driver_query"),
            "api-all" => queue_api_command(crate::api_v1::ServiceApiCommand::All, "full_api_sequence"),
            "gemini-test" => queue_gemini_prompt("Summarize the current role of OpenRhiza OS in one short sentence.".into()),
            _ if local_command.starts_with("driver-generate ") => {
                let match_key = local_command["driver-generate ".len()..].trim();
                queue_driver_generate(match_key);
            }
            _ if local_command.starts_with("driver-upload ") => {
                let match_key = local_command["driver-upload ".len()..].trim();
                queue_driver_upload(match_key);
            }
            _ if local_command.starts_with("driver-comment ") => {
                let rest = &local_command["driver-comment ".len()..];
                queue_driver_comment(rest);
            }
            _ if local_command.starts_with("driver-vote ") => {
                let rest = &local_command["driver-vote ".len()..];
                queue_driver_vote(rest);
            }
            _ => crate::result_println!("[CLI] Unknown local command. Use /help."),
        }
    } else {
        queue_gemini_prompt(alloc::string::String::from(command));
    }

    crate::vga::init_cli();
}

fn queue_api_command(command: crate::api_v1::ServiceApiCommand, label: &str) {
    match crate::api_v1::queue_service_api_command(command) {
        Ok(()) => crate::result_println!("[CLI] Queued API command: {}", label),
        Err(_) => crate::result_println!("[CLI] API command queue full."),
    }
}

fn queue_gemini_prompt(prompt: alloc::string::String) {
    crate::api_v1::record_last_gemini_prompt(prompt.as_str());
    match crate::api_v1::queue_gemini_prompt(prompt) {
        Ok(()) => crate::result_println!("[CLI] Queued Gemini prompt."),
        Err(_) => crate::result_println!("[CLI] Gemini prompt queue full."),
    }
}

fn queue_driver_generate(match_key: &str) {
    if match_key.is_empty() {
        crate::result_println!("[CLI] Usage: /driver-generate <match_key>");
        return;
    }

    match crate::api_v1::build_driver_generation_prompt(match_key) {
        Some(prompt) => {
            crate::api_v1::record_last_gemini_prompt(prompt.as_str());
            match crate::api_v1::queue_gemini_prompt(prompt) {
                Ok(()) => crate::result_println!("[CLI] Queued driver generation for {}", match_key),
                Err(_) => crate::result_println!("[CLI] Gemini prompt queue full."),
            }
        }
        None => crate::result_println!("[CLI] Node profile unavailable; cannot build driver prompt."),
    }
}

fn queue_driver_upload(match_key: &str) {
    if match_key.is_empty() {
        crate::result_println!("[CLI] Usage: /driver-upload <match_key>");
        return;
    }

    match crate::api_v1::queue_driver_registry_command(
        crate::api_v1::DriverRegistryCommand::UploadGenerated {
            match_key: alloc::string::String::from(match_key),
        },
    ) {
        Ok(()) => crate::result_println!("[CLI] Queued driver upload for {}", match_key),
        Err(_) => crate::result_println!("[CLI] Driver registry queue full."),
    }
}

fn queue_driver_comment(rest: &str) {
    let mut parts = rest.splitn(2, ' ');
    let Some(driver_id) = parts.next() else {
        crate::result_println!("[CLI] Usage: /driver-comment <driver_id> <text>");
        return;
    };
    let Some(comment) = parts.next() else {
        crate::result_println!("[CLI] Usage: /driver-comment <driver_id> <text>");
        return;
    };

    match crate::api_v1::queue_driver_registry_command(
        crate::api_v1::DriverRegistryCommand::Comment {
            driver_id: alloc::string::String::from(driver_id),
            comment: alloc::string::String::from(comment.trim()),
        },
    ) {
        Ok(()) => crate::result_println!("[CLI] Queued driver comment for {}", driver_id),
        Err(_) => crate::result_println!("[CLI] Driver registry queue full."),
    }
}

fn queue_driver_vote(rest: &str) {
    let mut parts = rest.split_whitespace();
    let Some(driver_id) = parts.next() else {
        crate::result_println!("[CLI] Usage: /driver-vote <driver_id> up|down");
        return;
    };
    let Some(vote_text) = parts.next() else {
        crate::result_println!("[CLI] Usage: /driver-vote <driver_id> up|down");
        return;
    };

    let vote = match vote_text {
        "up" => crate::api_v1::DriverVote::Up,
        "down" => crate::api_v1::DriverVote::Down,
        _ => {
            crate::result_println!("[CLI] Vote must be up or down.");
            return;
        }
    };

    match crate::api_v1::queue_driver_registry_command(
        crate::api_v1::DriverRegistryCommand::Vote {
            driver_id: alloc::string::String::from(driver_id),
            vote,
        },
    ) {
        Ok(()) => crate::result_println!("[CLI] Queued driver vote for {}", driver_id),
        Err(_) => crate::result_println!("[CLI] Driver registry queue full."),
    }
}
