use alloc::string::String;

use lazy_static::lazy_static;
use spin::Mutex;

use crate::input_handoff::{HidDeviceKind, InputRoutingMode, SandboxInputCommand};
use crate::sandbox_lifecycle::{PendingLoadDisposition, SandboxRuntimeState};

pub const INPUT_KEYBOARD_MATCH_KEY: &str = "input:keyboard";
pub const INPUT_MOUSE_MATCH_KEY: &str = "input:mouse";
pub const INPUT_KEYBOARD_DRIVER_ID: &str = "snd_input_keyboard_bootstrap_v1";
pub const INPUT_MOUSE_DRIVER_ID: &str = "snd_input_mouse_bootstrap_v1";

pub const INPUT_HEALTH_LOAD_SUCCESS: u32 = 0x01;
pub const INPUT_HEALTH_RAW_FETCH: u32 = 0x02;
pub const INPUT_HEALTH_EVENT_EMIT: u32 = 0x04;
pub const INPUT_HEALTH_KIND_FILTERED: u32 = 0x08;
pub const INPUT_HEALTH_FALLBACK_PRESERVED: u32 = 0x10;
pub const INPUT_HEALTH_USER_APPROVED: u32 = 0x20;

const INPUT_HEALTH_ALLOWED: u32 = INPUT_HEALTH_LOAD_SUCCESS
    | INPUT_HEALTH_RAW_FETCH
    | INPUT_HEALTH_EVENT_EMIT
    | INPUT_HEALTH_KIND_FILTERED
    | INPUT_HEALTH_FALLBACK_PRESERVED
    | INPUT_HEALTH_USER_APPROVED;
const INPUT_HEALTH_REQUIRED_FOR_SELF_PROMOTION: u32 = INPUT_HEALTH_ALLOWED;

const KEYBOARD_DRIVER_FILES: [[u8; 11]; 1] = [*b"KEYBDRV WAS"];
const MOUSE_DRIVER_FILES: [[u8; 11]; 1] = [*b"MOUSEDRVWAS"];

pub use crate::sandbox_lifecycle::SandboxStage as InputDriverStage;

#[derive(Clone, Debug)]
struct InputRuntimeEntry {
    kind: HidDeviceKind,
    lifecycle: SandboxRuntimeState,
    raw_fetched_at_load: u64,
    events_emitted_at_load: u64,
    health_driver_id: Option<String>,
    health_flags: u32,
    last_health_tick: u64,
    promote_attempts: u32,
    promote_denied: u32,
    promote_ok: u32,
}

#[derive(Clone, Debug)]
pub struct InputRuntimeState {
    pub kind: HidDeviceKind,
    pub component: SandboxRuntimeState,
    pub health_driver_id: Option<String>,
    pub health_flags: u32,
    pub last_health_tick: u64,
    pub promote_attempts: u32,
    pub promote_denied: u32,
    pub promote_ok: u32,
}

lazy_static! {
    static ref INPUT_RUNTIME_STATE: Mutex<[InputRuntimeEntry; 2]> = Mutex::new([
        InputRuntimeEntry {
            kind: HidDeviceKind::Keyboard,
            lifecycle: SandboxRuntimeState::new(),
            raw_fetched_at_load: 0,
            events_emitted_at_load: 0,
            health_driver_id: None,
            health_flags: 0,
            last_health_tick: 0,
            promote_attempts: 0,
            promote_denied: 0,
            promote_ok: 0,
        },
        InputRuntimeEntry {
            kind: HidDeviceKind::Mouse,
            lifecycle: SandboxRuntimeState::new(),
            raw_fetched_at_load: 0,
            events_emitted_at_load: 0,
            health_driver_id: None,
            health_flags: 0,
            last_health_tick: 0,
            promote_attempts: 0,
            promote_denied: 0,
            promote_ok: 0,
        },
    ]);
}

fn kind_index(kind: HidDeviceKind) -> usize {
    match kind {
        HidDeviceKind::Keyboard => 0,
        HidDeviceKind::Mouse => 1,
    }
}

fn snapshot_entry(entry: &InputRuntimeEntry) -> InputRuntimeState {
    InputRuntimeState {
        kind: entry.kind,
        component: entry.lifecycle.clone(),
        health_driver_id: entry.health_driver_id.clone(),
        health_flags: entry.health_flags,
        last_health_tick: entry.last_health_tick,
        promote_attempts: entry.promote_attempts,
        promote_denied: entry.promote_denied,
        promote_ok: entry.promote_ok,
    }
}

pub fn kind_label(kind: HidDeviceKind) -> &'static str {
    match kind {
        HidDeviceKind::Keyboard => "keyboard",
        HidDeviceKind::Mouse => "mouse",
    }
}

pub fn match_key_for_kind(kind: HidDeviceKind) -> &'static str {
    match kind {
        HidDeviceKind::Keyboard => INPUT_KEYBOARD_MATCH_KEY,
        HidDeviceKind::Mouse => INPUT_MOUSE_MATCH_KEY,
    }
}

pub fn default_driver_id_for_kind(kind: HidDeviceKind) -> &'static str {
    match kind {
        HidDeviceKind::Keyboard => INPUT_KEYBOARD_DRIVER_ID,
        HidDeviceKind::Mouse => INPUT_MOUSE_DRIVER_ID,
    }
}

pub fn local_driver_files_for_kind(kind: HidDeviceKind) -> &'static [[u8; 11]] {
    match kind {
        HidDeviceKind::Keyboard => &KEYBOARD_DRIVER_FILES,
        HidDeviceKind::Mouse => &MOUSE_DRIVER_FILES,
    }
}

pub fn is_supported_local_driver(kind: HidDeviceKind, driver_id: &str) -> bool {
    driver_id == default_driver_id_for_kind(kind)
}

fn queue_command_for_kind(kind: HidDeviceKind) -> Result<(), &'static str> {
    let command = match kind {
        HidDeviceKind::Keyboard => SandboxInputCommand::LoadKeyboardDriver,
        HidDeviceKind::Mouse => SandboxInputCommand::LoadMouseDriver,
    };

    crate::input_handoff::queue_sandbox_input_command(command)
        .map_err(|_| "sandbox input command queue full")
}

fn queue_unload_command_for_kind(kind: HidDeviceKind) -> Result<(), &'static str> {
    let command = match kind {
        HidDeviceKind::Keyboard => SandboxInputCommand::UnloadKeyboardDriver,
        HidDeviceKind::Mouse => SandboxInputCommand::UnloadMouseDriver,
    };

    crate::input_handoff::queue_sandbox_input_command(command)
        .map_err(|_| "sandbox input command queue full")
}

fn begin_load(
    kind: HidDeviceKind,
    driver_id: &str,
    disposition: PendingLoadDisposition,
) -> Result<(), &'static str> {
    {
        let mut states = INPUT_RUNTIME_STATE.lock();
        let state = &mut states[kind_index(kind)];
        state.lifecycle.begin_load(driver_id, disposition)?;
        state.health_driver_id = None;
        state.health_flags = 0;
        state.last_health_tick = 0;
        state.promote_attempts = 0;
        state.promote_denied = 0;
        state.promote_ok = 0;
    }

    queue_command_for_kind(kind)
}

pub fn queue_testing_load(kind: HidDeviceKind) -> Result<&'static str, &'static str> {
    let driver_id = default_driver_id_for_kind(kind);
    begin_load(kind, driver_id, PendingLoadDisposition::Testing)?;
    Ok(driver_id)
}

pub fn queue_restore_load(kind: HidDeviceKind, driver_id: &str) -> Result<(), &'static str> {
    if !is_supported_local_driver(kind, driver_id) {
        return Err("persisted input driver is not available on the local driver disk");
    }

    {
        let mut states = INPUT_RUNTIME_STATE.lock();
        let state = &mut states[kind_index(kind)];
        state.lifecycle.persisted_artifact_id = Some(String::from(driver_id));
    }

    begin_load(kind, driver_id, PendingLoadDisposition::RestoreActive)
}

pub fn finish_load_success(kind: HidDeviceKind) -> Option<InputRuntimeState> {
    let mut states = INPUT_RUNTIME_STATE.lock();
    let state = &mut states[kind_index(kind)];
    state.lifecycle.finish_load_success()?;
    state.raw_fetched_at_load = crate::input_handoff::raw_packets_fetched_for_kind(kind);
    state.events_emitted_at_load = crate::input_handoff::input_events_emitted_for_kind(kind);
    if let Some(driver_id) = state.lifecycle.current_artifact_id.as_deref() {
        state.health_driver_id = Some(String::from(driver_id));
        state.health_flags |= INPUT_HEALTH_LOAD_SUCCESS | INPUT_HEALTH_FALLBACK_PRESERVED;
        state.last_health_tick = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
    }
    Some(snapshot_entry(state))
}

pub fn finish_load_failure(kind: HidDeviceKind, error: &str) -> InputRuntimeState {
    let mut states = INPUT_RUNTIME_STATE.lock();
    let state = &mut states[kind_index(kind)];
    state.lifecycle.finish_load_failure(error);
    snapshot_entry(state)
}

pub fn promote(kind: HidDeviceKind) -> Result<String, &'static str> {
    {
        let states = INPUT_RUNTIME_STATE.lock();
        let state = &states[kind_index(kind)];
        let raw_delta = crate::input_handoff::raw_packets_fetched_for_kind(kind)
            .saturating_sub(state.raw_fetched_at_load);
        let event_delta = crate::input_handoff::input_events_emitted_for_kind(kind)
            .saturating_sub(state.events_emitted_at_load);
        if raw_delta == 0 || event_delta == 0 {
            return Err("sandbox input validation needs raw fetch and emitted event evidence after load");
        }
    }

    let driver_id = {
        let mut states = INPUT_RUNTIME_STATE.lock();
        let state = &mut states[kind_index(kind)];
        state.lifecycle.promote_current()?
    };

    crate::input_handoff::set_sandbox_input_active_for_kind(kind, true);
    crate::input_handoff::set_routing_mode_for_kind(kind, InputRoutingMode::SandboxPreferred);
    crate::driver_cache::persist_active_driver_binding(match_key_for_kind(kind), driver_id.as_str())?;
    crate::runtime_bindings::activate_binding(match_key_for_kind(kind), driver_id.as_str(), "input-runtime");
    Ok(driver_id)
}

pub fn report_sandbox_input_health(
    kind: HidDeviceKind,
    driver_id: &str,
    flags: u32,
) -> u32 {
    let mut states = INPUT_RUNTIME_STATE.lock();
    let state = &mut states[kind_index(kind)];
    let driver_id = driver_id.trim();
    if driver_id.is_empty() {
        return 0;
    }
    if state.lifecycle.current_artifact_id.as_deref() != Some(driver_id) {
        return 0;
    }

    state.health_driver_id = Some(String::from(driver_id));
    state.health_flags |= flags & INPUT_HEALTH_ALLOWED;
    state.last_health_tick = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
    1
}

pub fn request_sandbox_input_promotion(kind: HidDeviceKind, driver_id: &str) -> u32 {
    {
        let mut states = INPUT_RUNTIME_STATE.lock();
        let state = &mut states[kind_index(kind)];
        state.promote_attempts = state.promote_attempts.saturating_add(1);
        let driver_id = driver_id.trim();
        if driver_id.is_empty()
            || state.lifecycle.current_artifact_id.as_deref() != Some(driver_id)
            || state.health_driver_id.as_deref() != Some(driver_id)
            || (state.health_flags & INPUT_HEALTH_REQUIRED_FOR_SELF_PROMOTION)
                != INPUT_HEALTH_REQUIRED_FOR_SELF_PROMOTION
        {
            state.promote_denied = state.promote_denied.saturating_add(1);
            return 0;
        }
    }

    match promote(kind) {
        Ok(_) => {
            let mut states = INPUT_RUNTIME_STATE.lock();
            states[kind_index(kind)].promote_ok =
                states[kind_index(kind)].promote_ok.saturating_add(1);
            1
        }
        Err(_) => {
            let mut states = INPUT_RUNTIME_STATE.lock();
            states[kind_index(kind)].promote_denied =
                states[kind_index(kind)].promote_denied.saturating_add(1);
            0
        }
    }
}

pub fn allow_wasm_active_request(kind: HidDeviceKind, active: bool) -> bool {
    if !active {
        crate::input_handoff::set_sandbox_input_active_for_kind(kind, false);
        return true;
    }

    false
}

pub fn rollback_to_bootstrap(kind: HidDeviceKind) -> Result<String, &'static str> {
    let removed_driver_id = {
        let mut states = INPUT_RUNTIME_STATE.lock();
        let state = &mut states[kind_index(kind)];
        state.lifecycle.rollback_to_bootstrap()?
    };

    crate::input_handoff::set_sandbox_input_active_for_kind(kind, false);
    crate::input_handoff::set_routing_mode_for_kind(kind, InputRoutingMode::HandoffMirror);
    crate::runtime_bindings::deactivate_binding(match_key_for_kind(kind));
    let _ = crate::driver_cache::remove_active_driver_binding(match_key_for_kind(kind));
    let _ = queue_unload_command_for_kind(kind);

    Ok(removed_driver_id)
}

pub fn handle_hardware_loss(kind: HidDeviceKind, reason: &str) -> Option<String> {
    let removed_driver_id = {
        let mut states = INPUT_RUNTIME_STATE.lock();
        let state = &mut states[kind_index(kind)];
        state.lifecycle.handle_hardware_loss(reason)
    };

    crate::input_handoff::set_sandbox_input_active_for_kind(kind, false);
    crate::input_handoff::set_routing_mode_for_kind(kind, InputRoutingMode::HandoffMirror);
    crate::runtime_bindings::deactivate_binding(match_key_for_kind(kind));
    let _ = queue_unload_command_for_kind(kind);

    removed_driver_id
}

pub fn queue_restore_if_persisted(kind: HidDeviceKind) -> Result<Option<String>, &'static str> {
    let persisted_driver_id = {
        let states = INPUT_RUNTIME_STATE.lock();
        let state = &states[kind_index(kind)];
        if state.lifecycle.current_artifact_id.is_some() || state.lifecycle.pending_artifact_id.is_some() {
            return Ok(None);
        }
        state.lifecycle.persisted_artifact_id.clone()
    };

    let Some(driver_id) = persisted_driver_id else {
        return Ok(None);
    };

    crate::println!(
        "[Sandbox Input] Persisted {} driver {} is available after re-enumeration, but automatic runtime restore remains disabled to preserve the guaranteed local input path.",
        kind_label(kind),
        driver_id
    );
    Ok(None)
}

pub fn watchdog() {
    const INPUT_SANDBOX_LEASE_TIMEOUT_TICKS: u64 = 3_000;

    let now = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
    let mut expired: alloc::vec::Vec<(HidDeviceKind, String)> = alloc::vec::Vec::new();
    {
        let mut states = INPUT_RUNTIME_STATE.lock();
        for state in states.iter_mut() {
            if state.lifecycle.stage != InputDriverStage::Active {
                continue;
            }
            if state.last_health_tick == 0
                || now.saturating_sub(state.last_health_tick) <= INPUT_SANDBOX_LEASE_TIMEOUT_TICKS
            {
                continue;
            }
            if let Some(driver_id) =
                state.lifecycle.handle_hardware_loss("sandbox input lease expired")
            {
                expired.push((state.kind, driver_id));
            }
        }
    }

    for (kind, driver_id) in expired {
        crate::input_handoff::set_sandbox_input_active_for_kind(kind, false);
        crate::input_handoff::set_routing_mode_for_kind(kind, InputRoutingMode::HandoffMirror);
        crate::runtime_bindings::deactivate_binding(match_key_for_kind(kind));
        let _ = queue_unload_command_for_kind(kind);
        crate::serial_println!(
            "[Input Runtime] sandbox {} lease expired for {}; restored native fallback.",
            kind_label(kind),
            driver_id
        );
    }
}

pub fn snapshot() -> [InputRuntimeState; 2] {
    let states = INPUT_RUNTIME_STATE.lock();
    [snapshot_entry(&states[0]), snapshot_entry(&states[1])]
}

pub fn schedule_persisted_restores() {
    for kind in [HidDeviceKind::Keyboard, HidDeviceKind::Mouse] {
        let Some(driver_id) = crate::runtime_bindings::current_driver(match_key_for_kind(kind)) else {
            continue;
        };

        crate::println!(
            "[Sandbox Input] Persisted {} driver {} detected, but auto-restore is disabled to preserve a guaranteed local input path.",
            kind_label(kind),
            driver_id
        );
    }
}
