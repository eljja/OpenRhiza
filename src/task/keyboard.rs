use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};
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
    pub static ref DYNAMIC_KEYMAP: Mutex<[u8; 256]> = Mutex::new([0; 256]);
}

pub static KEYMAP_OVERRIDE_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static KEYBOARD_DEBUG_ACTIVE: AtomicBool = AtomicBool::new(false);

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
    let mut hangul = crate::hangul::HangulIme::new();
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

        if KEYMAP_OVERRIDE_ACTIVE.load(Ordering::Relaxed) {
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
                (_, 0x2A) | (_, 0x36) => {
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
                if char_to_print != 0 {
                    handle_input_byte(char_to_print);
                }
            }
            continue;
        }

        let event = keyboard.process_scancode(scancode);
        if KEYBOARD_DEBUG_ACTIVE.load(Ordering::Relaxed) {
            crate::serial_println!(
                "[Keyboard Debug] sc={:#04X} event={:?} shift={} lshift={} rshift={} ctrl={} alt={} num={} caps={}",
                scancode,
                event,
                keyboard.shift_pressed as u8,
                keyboard.left_shift_pressed as u8,
                keyboard.right_shift_pressed as u8,
                keyboard.ctrl_pressed as u8,
                keyboard.alt_pressed as u8,
                keyboard.num_lock as u8,
                keyboard.caps_lock as u8
            );
        }

        if let Some(event) = event {
            match event {
                crate::keyboard::KeyEvent::Char(byte) => handle_key_char(byte, &mut hangul),
                crate::keyboard::KeyEvent::Enter => {
                    commit_hangul_pending(&mut hangul);
                    submit_cli_command();
                }
                crate::keyboard::KeyEvent::Backspace => {
                    if hangul.enabled() && hangul.backspace() {
                        update_hangul_preview(&hangul);
                    } else {
                        WRITER.lock().pop_input_char();
                    }
                }
                crate::keyboard::KeyEvent::ArrowUp => WRITER.lock().history_up(),
                crate::keyboard::KeyEvent::ArrowDown => WRITER.lock().history_down(),
                crate::keyboard::KeyEvent::ArrowLeft => {
                    commit_hangul_pending(&mut hangul);
                    WRITER.lock().cursor_left();
                }
                crate::keyboard::KeyEvent::ArrowRight => {
                    commit_hangul_pending(&mut hangul);
                    WRITER.lock().cursor_right();
                }
                crate::keyboard::KeyEvent::Home => {
                    commit_hangul_pending(&mut hangul);
                    WRITER.lock().home();
                }
                crate::keyboard::KeyEvent::End => {
                    commit_hangul_pending(&mut hangul);
                    WRITER.lock().end();
                }
                crate::keyboard::KeyEvent::Delete => {
                    commit_hangul_pending(&mut hangul);
                    WRITER.lock().delete_char();
                }
                crate::keyboard::KeyEvent::PageUp => {
                    if crate::display::is_gui_conversation_focused() {
                        let _ = crate::display::scroll_gui_conversation("up", 3);
                    } else {
                        WRITER.lock().scroll_up(10);
                    }
                }
                crate::keyboard::KeyEvent::PageDown => {
                    if crate::display::is_gui_conversation_focused() {
                        let _ = crate::display::scroll_gui_conversation("down", 3);
                    } else {
                        WRITER.lock().scroll_down(10);
                    }
                }
                crate::keyboard::KeyEvent::ToggleHangul => toggle_hangul_mode(&mut hangul),
                crate::keyboard::KeyEvent::CtrlC => {
                    commit_hangul_pending(&mut hangul);
                    crate::println!("^C");
                    WRITER.lock().cancel_line();
                }
                crate::keyboard::KeyEvent::CtrlL => {
                    commit_hangul_pending(&mut hangul);
                    WRITER.lock().clear_log_area();
                }
                crate::keyboard::KeyEvent::CtrlU => {
                    commit_hangul_pending(&mut hangul);
                    WRITER.lock().clear_before_cursor();
                }
                crate::keyboard::KeyEvent::CtrlK => {
                    commit_hangul_pending(&mut hangul);
                    WRITER.lock().clear_after_cursor();
                }
                crate::keyboard::KeyEvent::CtrlW => {
                    commit_hangul_pending(&mut hangul);
                    WRITER.lock().delete_word();
                }
                _ => {}
            }
        }
    }
}

fn handle_key_char(byte: u8, hangul: &mut crate::hangul::HangulIme) {
    let ch = byte as char;
    if !hangul.enabled() {
        handle_input_byte(byte);
        return;
    }

    let step = hangul.process_ascii(ch);
    if !step.commit.is_empty() {
        crate::vga::commit_input_text(step.commit.as_str());
    }
    apply_hangul_preview(step.preview);
}

fn apply_hangul_preview(preview: Option<char>) {
    if let Some(ch) = preview {
        let mut text = alloc::string::String::new();
        text.push(ch);
        crate::vga::set_ime_preview(text.as_str());
    } else {
        crate::vga::clear_ime_preview();
    }
}

fn update_hangul_preview(hangul: &crate::hangul::HangulIme) {
    apply_hangul_preview(hangul.preview_char());
}

fn commit_hangul_pending(hangul: &mut crate::hangul::HangulIme) {
    let committed = hangul.commit_pending();
    if !committed.is_empty() {
        crate::vga::commit_input_text(committed.as_str());
    }
    crate::vga::clear_ime_preview();
}

fn toggle_hangul_mode(hangul: &mut crate::hangul::HangulIme) {
    let committed = hangul.take_commit_before_toggle();
    if !committed.is_empty() {
        crate::vga::commit_input_text(committed.as_str());
    }
    let enabled = hangul.toggle();
    crate::vga::clear_ime_preview();
    crate::result_println!(
        "[IME] {} mode",
        if enabled { "Hangul" } else { "English" }
    );
}

fn handle_input_byte(byte: u8) {
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
            "help" => crate::result_println!("[CLI] Local commands: /help, /clear, /status, /platform-status, /keyboard-debug <on|off>, /keyboard-selftest, /scheduler-status, /smp-status, /wasm-status, /semantic-status, /registry-context, /autonomy-status, /autonomy-mode <off|assist|council>, /autonomy-interval <minutes>, /autonomy-run-now, /voice-status, /voice <off|on|push-to-talk|always-listen>, /voice-route <text-first|direct-audio|hybrid>, /voice-model <model>, /voice-test, /voice-import, /voice-clear-buffer, /display-status, /gui-scene, /gui-mutations, /gui-session <openrhiza|sandbox|wide|recovery>, /gui-focus <conversation|composer|none>, /gui-scroll <up|down|bottom> [count], /gui-compose-demo, /gui-label <handle> <text>, /gui-style <handle> <style>, /gui-bounds <handle> <x> <y> <width> <height>, /gui-interaction <handle> <idle|hovered|focused|active|disabled>, /gui-reset <handle|all>, /fs-harness-status, /fs-harness-probe, /fs-bridge-status, /driver-host-status, /nexus-fetch, /api-register, /api-register-http, /http-health, /https-health, /https-root, /api-hw, /api-driver, /api-software, /api-skill, /api-workflow, /api-policy, /api-eval, /api-all, /gemini-test, /gemini-gui-test, /driver-map, /driver-runtime-status, /driver-promote <match_key>, /skill-cache, /skill-download <skill_id>, /skill-load <skill_id>, /skill-run <skill_id>, /skill-unload <skill_id>, /skill-activate <skill_id>, /driver-generate <match_key>, /driver-upload <match_key>, /driver-download <driver_id> [match_key], /driver-comment <driver_id> <text>, /driver-vote <driver_id> up|down, /driver-bindings, /driver-activate <match_key> <driver_id>, /driver-rollback <match_key>, /sandbox-mouse-load, /sandbox-keyboard-load, /input-routing-status, /input-activate <keyboard|mouse>, /input-rollback <keyboard|mouse>"),
            "clear" => WRITER.lock().clear_log_area(),
            "status" => {
                crate::result_println!("[CLI] Keyboard input ready.");
                crate::result_println!("[CLI] Serial debug logs remain on COM1 only.");
                crate::result_println!("[CLI] Plain text without '/' is sent to Gemini.");
            }
            "keyboard-selftest" => keyboard_selftest(),
            "platform-status" => show_platform_status(),
            _ if local_command.starts_with("keyboard-debug ") => {
                let mode = local_command["keyboard-debug ".len()..].trim();
                set_keyboard_debug(mode);
            }
            "scheduler-status" => show_scheduler_status(),
            "smp-status" => show_smp_status(),
            "wasm-status" => show_wasm_status(),
            "semantic-status" => show_semantic_status(),
            "registry-context" => show_registry_context(),
            "autonomy-status" => show_autonomy_status(),
            "voice-status" => show_voice_status(),
            "voice-test" => request_voice_test(),
            "voice-import" => import_voice_transcript(),
            "voice-clear-buffer" => clear_voice_buffer(),
            "display-status" => show_display_status(),
            "fs-harness-status" => show_fs_harness_status(),
            "fs-harness-probe" => run_fs_harness_probe(),
            "fs-bridge-status" => show_fs_bridge_status(),
            "driver-host-status" => show_driver_host_status(),
            "gui-scene" => show_gui_scene(),
            "gui-mutations" => show_gui_mutations(),
            _ if local_command.starts_with("autonomy-mode ") => {
                let mode = local_command["autonomy-mode ".len()..].trim();
                set_autonomy_mode(mode);
            }
            _ if local_command.starts_with("autonomy-interval ") => {
                let minutes = local_command["autonomy-interval ".len()..].trim();
                set_autonomy_interval(minutes);
            }
            "autonomy-run-now" => request_autonomy_run_now(),
            _ if local_command.starts_with("voice ") => {
                let mode = local_command["voice ".len()..].trim();
                set_voice_mode(mode);
            }
            _ if local_command.starts_with("voice-route ") => {
                let route = local_command["voice-route ".len()..].trim();
                set_voice_route(route);
            }
            _ if local_command.starts_with("voice-model ") => {
                let model = local_command["voice-model ".len()..].trim();
                set_voice_model(model);
            }
            _ if local_command.starts_with("gui-session ") => {
                let name = local_command["gui-session ".len()..].trim();
                select_gui_session(name);
            }
            _ if local_command.starts_with("gui-focus ") => {
                let name = local_command["gui-focus ".len()..].trim();
                focus_gui_object(name);
            }
            _ if local_command.starts_with("gui-scroll ") => {
                let rest = local_command["gui-scroll ".len()..].trim();
                scroll_gui_conversation(rest);
            }
            "gui-compose-demo" => set_gui_composer_demo(),
            _ if local_command.starts_with("gui-label ") => {
                let rest = local_command["gui-label ".len()..].trim();
                set_gui_label(rest);
            }
            _ if local_command.starts_with("gui-style ") => {
                let rest = local_command["gui-style ".len()..].trim();
                set_gui_style(rest);
            }
            _ if local_command.starts_with("gui-bounds ") => {
                let rest = local_command["gui-bounds ".len()..].trim();
                set_gui_bounds(rest);
            }
            _ if local_command.starts_with("gui-interaction ") => {
                let rest = local_command["gui-interaction ".len()..].trim();
                set_gui_interaction(rest);
            }
            _ if local_command.starts_with("gui-reset ") => {
                let target = local_command["gui-reset ".len()..].trim();
                reset_gui_mutations(target);
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
            "gemini-gui-test" => queue_gemini_gui_test(),
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
        crate::display::record_gui_user_prompt(command);
        queue_gemini_prompt(alloc::string::String::from(command));
    }

    crate::vga::init_cli();
}

pub fn execute_virtual_cli_command(command: &str) {
    handle_cli_command(command);
}

fn set_keyboard_debug(mode: &str) {
    match mode {
        "on" => {
            KEYBOARD_DEBUG_ACTIVE.store(true, Ordering::Relaxed);
            crate::result_println!("[Keyboard Debug] decoded-event serial logging enabled.");
        }
        "off" => {
            KEYBOARD_DEBUG_ACTIVE.store(false, Ordering::Relaxed);
            crate::result_println!("[Keyboard Debug] decoded-event serial logging disabled.");
        }
        _ => crate::result_println!("[Keyboard Debug] usage: /keyboard-debug <on|off>"),
    }
}

fn keyboard_selftest() {
    use crate::keyboard::{KeyEvent, KeyboardState};

    let mut state = KeyboardState::new();
    let rshift_down = state.process_scancode(0x36);
    let rshift_slash = state.process_scancode(0x35);
    let rshift_up = state.process_scancode(0xB6);
    let slash_plain = state.process_scancode(0x35);

    let mut left_state = KeyboardState::new();
    let lshift_down = left_state.process_scancode(0x2A);
    let lshift_slash = left_state.process_scancode(0x35);
    let lshift_up = left_state.process_scancode(0xAA);

    let mut extended_state = KeyboardState::new();
    let ext_prefix = extended_state.process_scancode(0xE0);
    let ext_shift_down = extended_state.process_scancode(0x36);
    let ext_shift_slash = extended_state.process_scancode(0x35);
    let ext_up_prefix = extended_state.process_scancode(0xE0);
    let ext_shift_up = extended_state.process_scancode(0xB6);

    let ok = matches!(rshift_down, Some(KeyEvent::ModifierOnly))
        && matches!(rshift_slash, Some(KeyEvent::Char(b'?')))
        && rshift_up.is_none()
        && matches!(slash_plain, Some(KeyEvent::Char(b'/')))
        && matches!(lshift_down, Some(KeyEvent::ModifierOnly))
        && matches!(lshift_slash, Some(KeyEvent::Char(b'?')))
        && lshift_up.is_none()
        && ext_prefix.is_none()
        && matches!(ext_shift_down, Some(KeyEvent::ModifierOnly))
        && matches!(ext_shift_slash, Some(KeyEvent::Char(b'?')))
        && ext_up_prefix.is_none()
        && ext_shift_up.is_none();

    crate::result_println!(
        "[Keyboard Selftest] right_shift_down={:?} right_shift_slash={:?} right_shift_up={:?}",
        rshift_down,
        rshift_slash,
        rshift_up
    );
    crate::result_println!(
        "[Keyboard Selftest] left_shift_down={:?} left_shift_slash={:?} left_shift_up={:?}",
        lshift_down,
        lshift_slash,
        lshift_up
    );
    crate::result_println!(
        "[Keyboard Selftest] slash_plain={:?} result={}",
        slash_plain,
        if ok { "pass" } else { "fail" }
    );
    crate::result_println!(
        "[Keyboard Selftest] extended_shift_down={:?} extended_shift_slash={:?} extended_shift_up={:?}",
        ext_shift_down,
        ext_shift_slash,
        ext_shift_up
    );
    if ok {
        crate::result_println!(
            "[Keyboard Selftest] OS decoder accepts RShift scancode 0x36/0xB6 and Shift+/ -> '?'."
        );
    }
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

fn queue_gemini_gui_test() {
    queue_gemini_prompt(
        "[gui-selftest] Adjust the current OpenRhiza GUI scene. Emit only compact JSON action objects with no prose and no markdown fences. Prefer these exact actions: gui_select_session, gui_focus, gui_set_bounds, gui_set_label, gui_set_style, gui_set_interaction, gui_reset. Select the openrhiza session, focus the composer, slightly widen the conversation region, increase the composer height for a richer Codex-like layout, and update the footer labels to mention a Gemini GUI self-test.".into(),
    );
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
    for state in states.iter().take(20) {
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

fn show_display_status() {
    for line in crate::display::status_block().lines() {
        crate::result_println!("{}", line);
    }
}

fn show_scheduler_status() {
    let snapshot = crate::task::executor::scheduler_metrics_snapshot();
    crate::result_println!(
        "[Scheduler] wake_events={} wake_drops={} total_polls={} completed={} batch_yields={} idle_halts={} max_queue_depth={}",
        snapshot.wake_events,
        snapshot.wake_drops,
        snapshot.total_polls,
        snapshot.completed_tasks,
        snapshot.batch_yields,
        snapshot.idle_halts,
        snapshot.max_queue_depth
    );
}

fn show_smp_status() {
    crate::result_println!("[SMP] {}", crate::smp::status_block());
}

fn show_platform_status() {
    for line in crate::platform::status_block().lines() {
        crate::result_println!("{}", line);
    }
}

fn show_wasm_status() {
    for line in crate::os_core_seed::wasm_health_report().lines() {
        crate::result_println!("{}", line);
    }
}

fn show_semantic_status() {
    for line in crate::semantic_graph::status_block().lines() {
        crate::result_println!("{}", line);
    }
}

fn show_registry_context() {
    match crate::api_v1::current_registry_context_block() {
        Some(context) => {
            for line in context.lines().take(24) {
                crate::result_println!("{}", line);
            }
        }
        None => crate::result_println!(
            "[Registry Context] not available yet. Run /api-all or ask OpenRhiza to inspect registry context."
        ),
    }
}

fn show_autonomy_status() {
    for line in crate::autonomy::status_block().lines() {
        crate::result_println!("{}", line);
    }
}

fn set_autonomy_mode(mode: &str) {
    match crate::autonomy::set_mode(mode) {
        Ok(message) => crate::result_println!("{}", message),
        Err(error) => crate::result_println!("[Autonomy] {}", error),
    }
}

fn set_autonomy_interval(minutes: &str) {
    match minutes.parse::<u32>() {
        Ok(value) => match crate::autonomy::set_interval_minutes(value) {
            Ok(message) => crate::result_println!("{}", message),
            Err(error) => crate::result_println!("[Autonomy] {}", error),
        },
        Err(_) => crate::result_println!("[Autonomy] usage: /autonomy-interval <minutes>"),
    }
}

fn request_autonomy_run_now() {
    match crate::autonomy::request_run_now() {
        Ok(message) => crate::result_println!("{}", message),
        Err(error) => crate::result_println!("[Autonomy] {}", error),
    }
}

fn show_voice_status() {
    for line in crate::voice::status_block().lines() {
        crate::result_println!("{}", line);
    }
}

fn set_voice_mode(mode: &str) {
    match crate::voice::set_mode(mode) {
        Ok(message) => crate::result_println!("{}", message),
        Err(error) => crate::result_println!("[Voice] {}", error),
    }
}

fn set_voice_route(route: &str) {
    match crate::voice::set_route(route) {
        Ok(message) => crate::result_println!("{}", message),
        Err(error) => crate::result_println!("[Voice] {}", error),
    }
}

fn set_voice_model(model: &str) {
    match crate::voice::set_model(model) {
        Ok(message) => crate::result_println!("{}", message),
        Err(error) => crate::result_println!("[Voice] {}", error),
    }
}

fn request_voice_test() {
    match crate::voice::queue_capture_bridge_test() {
        Ok(message) => crate::result_println!("{}", message),
        Err(error) => crate::result_println!("[Voice] {}", error),
    }
}

fn import_voice_transcript() {
    match crate::voice::import_transcript_to_composer() {
        Ok(message) => crate::result_println!("{}", message),
        Err(error) => crate::result_println!("[Voice] {}", error),
    }
}

fn clear_voice_buffer() {
    crate::result_println!("{}", crate::voice::clear_buffer());
}

fn show_fs_harness_status() {
    crate::result_println!("{}", crate::storage_host::status_block());
}

fn run_fs_harness_probe() {
    crate::result_println!("{}", crate::storage_host::probe_report());
    match crate::skill_runtime::queue_load("skill_fs_image_probe_v1") {
        Ok(fat_name) => crate::result_println!(
            "[Storage Host] Queued filesystem harness probe skill from {}",
            fat_name
        ),
        Err(error) => crate::result_println!("[Storage Host] {}", error),
    }
    match crate::skill_runtime::queue_run("skill_fs_image_probe_v1") {
        Ok(()) => crate::result_println!("[Storage Host] Queued filesystem harness probe skill run."),
        Err(error) => crate::result_println!("[Storage Host] {}", error),
    }
}

fn show_fs_bridge_status() {
    crate::result_println!("[FS Bridge] interface=skill_fs_bridge_v1");
    crate::result_println!("[FS Bridge] core_scope=raw bounded block transport only");
    crate::result_println!("[FS Bridge] skill_target=skill_fs_image_probe_v1");
    crate::result_println!("[FS Bridge] families=fat32,exfat,ntfs,ext2,ext3,ext4");
    crate::result_println!("{}", crate::storage_host::status_block());
    crate::result_println!("[FS Bridge] semantic_graph_root=.openrhiza/semantic-graph/");
}

fn show_driver_host_status() {
    for line in crate::driver_host::status_block().lines() {
        crate::result_println!("{}", line);
    }
}

fn show_gui_scene() {
    for line in crate::display::gui_scene_report().lines() {
        crate::result_println!("{}", line);
    }
}

fn show_gui_mutations() {
    for line in crate::display::gui_mutation_report().lines() {
        crate::result_println!("{}", line);
    }
}

fn select_gui_session(name: &str) {
    match crate::display::select_gui_session(name) {
        Ok(()) => crate::result_println!("[GUI] selected session: {}", name),
        Err(error) => crate::result_println!("[GUI] {}", error),
    }
}

fn focus_gui_object(name: &str) {
    match crate::display::focus_gui_object(name) {
        Ok(()) => crate::result_println!("[GUI] focus: {}", name),
        Err(error) => crate::result_println!("[GUI] {}", error),
    }
}

fn scroll_gui_conversation(rest: &str) {
    let mut parts = rest.split_whitespace();
    let Some(direction) = parts.next() else {
        crate::result_println!("[CLI] Usage: /gui-scroll <up|down|bottom> [count]");
        return;
    };
    let count = parts
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1);

    match crate::display::scroll_gui_conversation(direction, count) {
        Ok(()) => crate::result_println!("[CLI] GUI conversation scroll updated."),
        Err(error) => crate::result_println!("[CLI] {}", error),
    }
}

fn set_gui_composer_demo() {
    crate::vga::debug_set_input_line(
        "input> Design a codex-like multi-session GUI with a wider assistant conversation surface, a taller adaptive composer, and object-local scroll behavior that does not affect unrelated GUI objects.",
    );
    crate::result_println!("[CLI] GUI composer demo text applied.");
}

fn set_gui_label(rest: &str) {
    let mut parts = rest.splitn(2, ' ');
    let Some(handle_text) = parts.next() else {
        crate::result_println!("[GUI] Usage: /gui-label <handle> <text>");
        return;
    };
    let Some(label) = parts.next() else {
        crate::result_println!("[GUI] Usage: /gui-label <handle> <text>");
        return;
    };
    let Ok(handle) = handle_text.parse::<u64>() else {
        crate::result_println!("[GUI] handle must be a number");
        return;
    };
    match crate::display::set_gui_label(handle, label.trim()) {
        Ok(()) => crate::result_println!("[GUI] label updated for handle {}", handle),
        Err(error) => crate::result_println!("[GUI] {}", error),
    }
}

fn set_gui_style(rest: &str) {
    let mut parts = rest.split_whitespace();
    let Some(handle_text) = parts.next() else {
        crate::result_println!("[GUI] Usage: /gui-style <handle> <style>");
        return;
    };
    let Some(style) = parts.next() else {
        crate::result_println!("[GUI] Usage: /gui-style <handle> <style>");
        return;
    };
    let Ok(handle) = handle_text.parse::<u64>() else {
        crate::result_println!("[GUI] handle must be a number");
        return;
    };
    match crate::display::set_gui_style(handle, style) {
        Ok(()) => crate::result_println!("[GUI] style updated for handle {}", handle),
        Err(error) => crate::result_println!("[GUI] {}", error),
    }
}

fn set_gui_bounds(rest: &str) {
    let parts: alloc::vec::Vec<&str> = rest.split_whitespace().collect();
    if parts.len() != 5 {
        crate::result_println!("[GUI] Usage: /gui-bounds <handle> <x> <y> <width> <height>");
        return;
    }
    let Ok(handle) = parts[0].parse::<u64>() else {
        crate::result_println!("[GUI] handle must be a number");
        return;
    };
    let Ok(x) = parts[1].parse::<usize>() else {
        crate::result_println!("[GUI] x must be a number");
        return;
    };
    let Ok(y) = parts[2].parse::<usize>() else {
        crate::result_println!("[GUI] y must be a number");
        return;
    };
    let Ok(width) = parts[3].parse::<usize>() else {
        crate::result_println!("[GUI] width must be a number");
        return;
    };
    let Ok(height) = parts[4].parse::<usize>() else {
        crate::result_println!("[GUI] height must be a number");
        return;
    };
    match crate::display::set_gui_bounds(handle, x, y, width, height) {
        Ok(()) => crate::result_println!("[GUI] bounds updated for handle {}", handle),
        Err(error) => crate::result_println!("[GUI] {}", error),
    }
}

fn set_gui_interaction(rest: &str) {
    let mut parts = rest.split_whitespace();
    let Some(handle_text) = parts.next() else {
        crate::result_println!("[GUI] Usage: /gui-interaction <handle> <idle|hovered|focused|active|disabled>");
        return;
    };
    let Some(interaction) = parts.next() else {
        crate::result_println!("[GUI] Usage: /gui-interaction <handle> <idle|hovered|focused|active|disabled>");
        return;
    };
    let Ok(handle) = handle_text.parse::<u64>() else {
        crate::result_println!("[GUI] handle must be a number");
        return;
    };
    match crate::display::set_gui_interaction(handle, interaction) {
        Ok(()) => crate::result_println!("[GUI] interaction updated for handle {}", handle),
        Err(error) => crate::result_println!("[GUI] {}", error),
    }
}

fn reset_gui_mutations(target: &str) {
    if target == "all" {
        crate::display::reset_gui_mutations(None);
        crate::result_println!("[GUI] cleared all gui mutations");
        return;
    }
    let Ok(handle) = target.parse::<u64>() else {
        crate::result_println!("[GUI] Usage: /gui-reset <handle|all>");
        return;
    };
    crate::display::reset_gui_mutations(Some(handle));
    crate::result_println!("[GUI] cleared gui mutations for handle {}", handle);
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

