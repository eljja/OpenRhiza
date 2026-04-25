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
    let mut scancode_log_count = 0usize;

    crate::vga::init_cli();

    loop {
        let scancode = ScancodeStream::new().await;

        if scancode_log_count == 0 {
            crate::serial_print!("QEMU_LOG: Scancodes");
        }
        crate::serial_print!(" {:#04X}", scancode);
        scancode_log_count += 1;
        if scancode_log_count >= 16 || scancode == 0x1C || scancode == 0x9C {
            crate::serial_println!("");
            scancode_log_count = 0;
        }

        if KEYMAP_OVERRIDE_ACTIVE.load(core::sync::atomic::Ordering::Relaxed) {
            if scancode == 0xE0 {
                is_extended = true;
                continue;
            }

            let is_break = scancode >= 0x80;
            let real_scancode = scancode & 0x7F;

            // Even if dynamic keymap is active, preserve extended navigation keys.
            if is_extended && !is_break {
                is_extended = false;
                match real_scancode {
                    0x4B => { WRITER.lock().cursor_left(); continue; } // ArrowLeft
                    0x4D => { WRITER.lock().cursor_right(); continue; } // ArrowRight
                    0x48 => { WRITER.lock().history_up(); continue; } // ArrowUp
                    0x50 => { WRITER.lock().history_down(); continue; } // ArrowDown
                    0x47 => { WRITER.lock().home(); continue; } // Home
                    0x4F => { WRITER.lock().end(); continue; } // End
                    0x49 => { WRITER.lock().scroll_up(10); continue; } // PageUp
                    0x51 => { WRITER.lock().scroll_down(10); continue; } // PageDown
                    0x53 => { WRITER.lock().delete_char(); continue; } // Delete
                    _ => {}
                }
            }

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
                crate::keyboard::KeyEvent::ArrowLeft => WRITER.lock().cursor_left(),
                crate::keyboard::KeyEvent::ArrowRight => WRITER.lock().cursor_right(),
                crate::keyboard::KeyEvent::Home => WRITER.lock().home(),
                crate::keyboard::KeyEvent::End => WRITER.lock().end(),
                crate::keyboard::KeyEvent::Delete => WRITER.lock().delete_char(),
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

    crate::user_println!("input> {}", command);

    if let Some(local_command) = command.strip_prefix('/') {
        match local_command {
            "help" => crate::result_println!("[CLI] Local commands: /help, /clear, /status, /nexus-fetch, /api-register, /api-register-http, /http-health, /https-health, /https-root, /api-hw, /api-driver, /api-software, /api-skill, /api-workflow, /api-policy, /api-eval, /api-all, /gemini-test, /driver-map, /driver-runtime-status, /driver-promote <match_key>, /skill-cache, /skill-download <skill_id>, /skill-load <skill_id>, /skill-run <skill_id>, /skill-unload <skill_id>, /skill-activate <skill_id>, /driver-generate <match_key>, /driver-upload <match_key>, /driver-download <driver_id> [match_key], /driver-comment <driver_id> <text>, /driver-vote <driver_id> up|down, /driver-bindings, /driver-activate <match_key> <driver_id>, /driver-rollback <match_key>, /sandbox-mouse-load, /sandbox-keyboard-load, /input-routing-status, /input-activate <keyboard|mouse>, /input-rollback <keyboard|mouse>"),
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
            "api-software" => queue_api_command(crate::api_v1::ServiceApiCommand::SoftwareQuery, "software_query"),
            "api-skill" => queue_api_command(crate::api_v1::ServiceApiCommand::SkillQuery, "skill_query"),
            "api-workflow" => queue_api_command(crate::api_v1::ServiceApiCommand::WorkflowQuery, "workflow_query"),
            "api-policy" => queue_api_command(crate::api_v1::ServiceApiCommand::PolicyQuery, "policy_query"),
            "api-eval" => queue_api_command(crate::api_v1::ServiceApiCommand::EvaluationQuery, "evaluation_query"),
            "api-all" => queue_api_command(crate::api_v1::ServiceApiCommand::All, "full_api_sequence"),
            "gemini-test" => queue_gemini_prompt("Summarize the current role of OpenRhiza OS in one short sentence.".into()),
            "driver-map" => show_driver_map(),
            "driver-runtime-status" => show_driver_runtime_status(),
            "skill-cache" => show_skill_cache(),
            _ if local_command.starts_with("skill-download ") => {
                let skill_id = local_command["skill-download ".len()..].trim();
                queue_skill_download(skill_id);
            }
            _ if local_command.starts_with("skill-load ") => {
                let skill_id = local_command["skill-load ".len()..].trim();
                queue_skill_load(skill_id);
            }
            _ if local_command.starts_with("skill-run ") => {
                let skill_id = local_command["skill-run ".len()..].trim();
                queue_skill_run(skill_id);
            }
            _ if local_command.starts_with("skill-unload ") => {
                let skill_id = local_command["skill-unload ".len()..].trim();
                queue_skill_unload(skill_id);
            }
            _ if local_command.starts_with("skill-activate ") => {
                let skill_id = local_command["skill-activate ".len()..].trim();
                activate_skill(skill_id);
            }
            _ if local_command.starts_with("driver-generate ") => {
                let match_key = local_command["driver-generate ".len()..].trim();
                queue_driver_generate(match_key);
            }
            _ if local_command.starts_with("driver-upload ") => {
                let match_key = local_command["driver-upload ".len()..].trim();
                queue_driver_upload(match_key);
            }
            _ if local_command.starts_with("driver-download ") => {
                let rest = &local_command["driver-download ".len()..];
                queue_driver_download(rest);
            }
            _ if local_command.starts_with("driver-comment ") => {
                let rest = &local_command["driver-comment ".len()..];
                queue_driver_comment(rest);
            }
            _ if local_command.starts_with("driver-vote ") => {
                let rest = &local_command["driver-vote ".len()..];
                queue_driver_vote(rest);
            }
            _ if local_command.starts_with("driver-promote ") => {
                let match_key = local_command["driver-promote ".len()..].trim();
                promote_driver_binding(match_key);
            }
            "driver-bindings" => show_driver_bindings(),
            "sandbox-mouse-load" => queue_sandbox_mouse_load(),
            "sandbox-keyboard-load" => queue_sandbox_keyboard_load(),
            "input-routing-status" => show_input_routing_status(),
            _ if local_command.starts_with("input-activate ") => {
                let target = local_command["input-activate ".len()..].trim();
                activate_input_driver(target);
            }
            _ if local_command.starts_with("input-rollback ") => {
                let target = local_command["input-rollback ".len()..].trim();
                rollback_input_driver(target);
            }
            _ if local_command.starts_with("driver-activate ") => {
                let rest = &local_command["driver-activate ".len()..];
                activate_driver_binding(rest);
            }
            _ if local_command.starts_with("driver-rollback ") => {
                let match_key = local_command["driver-rollback ".len()..].trim();
                rollback_driver_binding(match_key);
            }
            _ => crate::result_println!("[CLI] Unknown local command. Use /help."),
        }
    } else {
        queue_gemini_prompt(alloc::string::String::from(command));
    }

    crate::vga::init_cli();
}

pub fn execute_virtual_cli_command(command: &str) {
    handle_cli_command(command);
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

fn queue_driver_download(rest: &str) {
    let mut parts = rest.split_whitespace();
    let Some(driver_id) = parts.next() else {
        crate::result_println!("[CLI] Usage: /driver-download <driver_id> [match_key]");
        return;
    };
    let match_key = parts.next().unwrap_or("");

    match crate::api_v1::queue_driver_registry_command(
        crate::api_v1::DriverRegistryCommand::DownloadCandidate {
            driver_id: alloc::string::String::from(driver_id),
            match_key: alloc::string::String::from(match_key),
        },
    ) {
        Ok(()) => crate::result_println!("[CLI] Queued driver download for {}", driver_id),
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

fn show_driver_bindings() {
    let states = crate::driver_runtime::snapshot();
    if states.is_empty() {
        crate::result_println!("[Driver Runtime] No tracked driver runtime entries exist.");
        return;
    }

    crate::result_println!(
        "[Driver Runtime] Tracked runtime entries: {}",
        states.len()
    );
    for state in states.iter().take(12) {
        crate::result_println!(
            "[Driver Runtime] {} stage={:?} current={} persisted={} source={}",
            state.match_key,
            state.component.stage,
            state.component.current_artifact_id.as_deref().unwrap_or("none"),
            state.component.persisted_artifact_id.as_deref().unwrap_or("none"),
            state.source
        );
    }
}

fn show_driver_runtime_status() {
    let states = crate::driver_runtime::snapshot();
    if states.is_empty() {
        crate::result_println!("[Driver Runtime] No tracked driver runtime entries exist.");
        return;
    }

    for state in states.iter().take(16) {
        crate::result_println!(
            "[Driver Runtime] {} stage={:?} current={} persisted={} previous={} source={}",
            state.match_key,
            state.component.stage,
            state.component.current_artifact_id.as_deref().unwrap_or("none"),
            state.component.persisted_artifact_id.as_deref().unwrap_or("none"),
            state.component.previous_artifact_id.as_deref().unwrap_or("none"),
            state.source
        );
        if let Some(error) = state.component.last_error.as_deref() {
            crate::result_println!(
                "[Driver Runtime] {} last_error={}",
                state.match_key,
                error
            );
        }
    }
}

fn show_driver_map() {
    match crate::driver_cache::load_active_driver_map_text() {
        Some(text) => {
            crate::result_println!("[Driver Cache] DRVMAP.TXT contents:");
            let mut persisted_entries = 0usize;
            for line in text.lines() {
                crate::result_println!("{}", line);
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('#') && trimmed.contains('=') {
                    persisted_entries += 1;
                }
            }
            if persisted_entries == 0 {
                crate::result_println!("[Driver Cache] No persisted preferred bindings yet.");
            }
        }
        None => crate::result_println!("[Driver Cache] DRVMAP.TXT not found on the secondary driver disk."),
    }

    let bindings = crate::runtime_bindings::snapshot();
    if bindings.is_empty() {
        crate::result_println!("[Driver Runtime] No live driver bindings are active.");
        return;
    }

    crate::result_println!("[Driver Runtime] Live bindings:");
    for binding in bindings.iter().take(16) {
        crate::result_println!("{}={}", binding.match_key, binding.driver_id);
    }
}

fn show_skill_cache() {
    match crate::skill_runtime::load_cached_skills_text() {
        Some(text) => {
            crate::result_println!("[Skill Cache] SKILLCCH.TXT contents:");
            for line in text.lines() {
                crate::result_println!("{}", line);
            }
        }
        None => crate::result_println!("[Skill Cache] SKILLCCH.TXT not found on the secondary driver disk."),
    }

    match crate::skill_runtime::load_active_skills_text() {
        Some(text) => {
            crate::result_println!("[Skill Cache] SKLACTV.TXT contents:");
            for line in text.lines() {
                crate::result_println!("{}", line);
            }
        }
        None => crate::result_println!("[Skill Cache] SKLACTV.TXT not found on the secondary driver disk."),
    }

    let snapshot = crate::skill_runtime::snapshot();
    crate::result_println!(
        "[Skill Runtime] stage={:?} current={} persisted={}",
        snapshot.component.stage,
        snapshot
            .component
            .current_artifact_id
            .as_deref()
            .unwrap_or("none"),
        snapshot
            .component
            .persisted_artifact_id
            .as_deref()
            .unwrap_or("none")
    );
    crate::result_println!(
        "[Skill Runtime] cached_skill_count={}",
        snapshot.cached_skill_ids.len()
    );
    for skill_id in snapshot.cached_skill_ids.iter().take(8) {
        crate::result_println!("[Skill Runtime] {}", skill_id);
    }

    for state in crate::skill_runtime::runtime_states().iter().take(8) {
        crate::result_println!(
            "[Skill Runtime] loaded {} stage={:?} current={} persisted={} cached_file={}",
            state.skill_id,
            state.stage,
            state.current_artifact_id.as_deref().unwrap_or("none"),
            state.persisted_artifact_id.as_deref().unwrap_or("none"),
            state.fat_name_text
        );
    }
}

fn queue_skill_load(skill_id: &str) {
    if skill_id.is_empty() {
        crate::result_println!("[CLI] Usage: /skill-load <skill_id>");
        return;
    }

    match crate::skill_runtime::queue_load(skill_id) {
        Ok(fat_name) => crate::result_println!(
            "[CLI] Queued skill load {} from {}",
            skill_id,
            fat_name
        ),
        Err(error) => crate::result_println!("[CLI] {}", error),
    }
}

fn queue_skill_download(skill_id: &str) {
    if skill_id.is_empty() {
        crate::result_println!("[CLI] Usage: /skill-download <skill_id>");
        return;
    }

    match crate::api_v1::queue_skill_registry_command(
        crate::api_v1::SkillRegistryCommand::DownloadCandidate {
            skill_id: alloc::string::String::from(skill_id),
            auto_load: false,
            auto_run: false,
        },
    ) {
        Ok(()) => crate::result_println!("[CLI] Queued skill download for {}", skill_id),
        Err(_) => crate::result_println!("[CLI] Skill registry queue full."),
    }
}

fn queue_skill_run(skill_id: &str) {
    if skill_id.is_empty() {
        crate::result_println!("[CLI] Usage: /skill-run <skill_id>");
        return;
    }

    match crate::skill_runtime::queue_run(skill_id) {
        Ok(()) => crate::result_println!("[CLI] Queued skill run {}", skill_id),
        Err(error) => crate::result_println!("[CLI] {}", error),
    }
}

fn queue_skill_unload(skill_id: &str) {
    if skill_id.is_empty() {
        crate::result_println!("[CLI] Usage: /skill-unload <skill_id>");
        return;
    }

    match crate::skill_runtime::queue_unload(skill_id) {
        Ok(()) => crate::result_println!("[CLI] Queued skill unload {}", skill_id),
        Err(error) => crate::result_println!("[CLI] {}", error),
    }
}

fn activate_skill(skill_id: &str) {
    if skill_id.is_empty() {
        crate::result_println!("[CLI] Usage: /skill-activate <skill_id>");
        return;
    }

    match crate::skill_runtime::promote(skill_id) {
        Ok(artifact_id) => crate::result_println!(
            "[Skill Runtime] Promoted {} -> {}",
            skill_id,
            artifact_id
        ),
        Err(error) => crate::result_println!("[Skill Runtime] {}", error),
    }
}

fn queue_sandbox_mouse_load() {
    match crate::input_runtime::queue_testing_load(crate::input_handoff::HidDeviceKind::Mouse) {
        Ok(driver_id) => crate::result_println!(
            "[CLI] Queued sandbox mouse driver load: {}",
            driver_id
        ),
        Err(error) => crate::result_println!("[CLI] {}", error),
    }
}

fn queue_sandbox_keyboard_load() {
    match crate::input_runtime::queue_testing_load(crate::input_handoff::HidDeviceKind::Keyboard) {
        Ok(driver_id) => crate::result_println!(
            "[CLI] Queued sandbox keyboard driver load: {}",
            driver_id
        ),
        Err(error) => crate::result_println!("[CLI] {}", error),
    }
}

fn show_input_routing_status() {
    let k_mode = crate::input_handoff::routing_mode_for_kind(crate::input_handoff::HidDeviceKind::Keyboard);
    let m_mode = crate::input_handoff::routing_mode_for_kind(crate::input_handoff::HidDeviceKind::Mouse);
    let k_active = crate::input_handoff::sandbox_input_active_for_kind(crate::input_handoff::HidDeviceKind::Keyboard);
    let m_active = crate::input_handoff::sandbox_input_active_for_kind(crate::input_handoff::HidDeviceKind::Mouse);
    let states = crate::input_runtime::snapshot();
    crate::result_println!(
        "[Input Routing] keyboard mode={:?} sandbox_active={} | mouse mode={:?} sandbox_active={}",
        k_mode,
        k_active as u8,
        m_mode,
        m_active as u8
    );
    for state in states {
        crate::result_println!(
            "[Input Runtime] {} stage={:?} current={} persisted={} previous={}",
            crate::input_runtime::kind_label(state.kind),
            state.component.stage,
            state.component.current_artifact_id.as_deref().unwrap_or("none"),
            state.component.persisted_artifact_id.as_deref().unwrap_or("none"),
            state.component.previous_artifact_id.as_deref().unwrap_or("none")
        );
        if let Some(error) = state.component.last_error.as_deref() {
            crate::result_println!(
                "[Input Runtime] {} last_error={}",
                crate::input_runtime::kind_label(state.kind),
                error
            );
        }
    }
}

fn parse_input_kind(target: &str) -> Option<crate::input_handoff::HidDeviceKind> {
    match target {
        "keyboard" => Some(crate::input_handoff::HidDeviceKind::Keyboard),
        "mouse" => Some(crate::input_handoff::HidDeviceKind::Mouse),
        _ => None,
    }
}

fn activate_input_driver(target: &str) {
    let Some(kind) = parse_input_kind(target) else {
        crate::result_println!("[CLI] Usage: /input-activate <keyboard|mouse>");
        return;
    };

    match crate::input_runtime::promote(kind) {
        Ok(driver_id) => crate::result_println!(
            "[Input Runtime] Promoted {} -> {} and persisted for future boots.",
            crate::input_runtime::kind_label(kind),
            driver_id
        ),
        Err(error) => crate::result_println!("[Input Runtime] Promote failed: {}", error),
    }
}

fn rollback_input_driver(target: &str) {
    let Some(kind) = parse_input_kind(target) else {
        crate::result_println!("[CLI] Usage: /input-rollback <keyboard|mouse>");
        return;
    };

    match crate::input_runtime::rollback_to_bootstrap(kind) {
        Ok(driver_id) => crate::result_println!(
            "[Input Runtime] Rolled back {} from {} to bootstrap fallback.",
            crate::input_runtime::kind_label(kind),
            driver_id
        ),
        Err(error) => crate::result_println!("[Input Runtime] Rollback failed: {}", error),
    }
}

fn activate_driver_binding(rest: &str) {
    let mut parts = rest.splitn(2, ' ');
    let Some(match_key) = parts.next() else {
        crate::result_println!("[CLI] Usage: /driver-activate <match_key> <driver_id>");
        return;
    };
    let Some(driver_id) = parts.next() else {
        crate::result_println!("[CLI] Usage: /driver-activate <match_key> <driver_id>");
        return;
    };

    let outcome = crate::driver_runtime::activate_binding(
        match_key.trim(),
        driver_id.trim(),
        "manual-cli",
    );
    if outcome.changed {
        if let Some(previous) = outcome.previous_driver_id.as_deref() {
            crate::result_println!(
                "[Driver Runtime] Activated {} -> {} (previously {})",
                match_key.trim(),
                driver_id.trim(),
                previous
            );
        } else {
            crate::result_println!(
                "[Driver Runtime] Activated {} -> {}",
                match_key.trim(),
                driver_id.trim()
            );
        }
    } else {
        crate::result_println!(
            "[Driver Runtime] {} is already active for {}",
            driver_id.trim(),
            match_key.trim()
        );
    }
}

fn rollback_driver_binding(match_key: &str) {
    if match_key.is_empty() {
        crate::result_println!("[CLI] Usage: /driver-rollback <match_key>");
        return;
    }

    match crate::driver_runtime::rollback_binding(match_key) {
        Ok(driver_id) => crate::result_println!(
            "[Driver Runtime] Rolled back {} -> {}",
            match_key,
            driver_id
        ),
        Err(error) => crate::result_println!("[Driver Runtime] Rollback failed: {}", error),
    }
}

fn promote_driver_binding(match_key: &str) {
    if match_key.is_empty() {
        crate::result_println!("[CLI] Usage: /driver-promote <match_key>");
        return;
    }

    match crate::driver_runtime::promote_binding(match_key) {
        Ok(driver_id) => crate::result_println!(
            "[Driver Runtime] Promoted {} -> {} and persisted for future boots.",
            match_key,
            driver_id
        ),
        Err(error) => crate::result_println!("[Driver Runtime] Promote failed: {}", error),
    }
}

