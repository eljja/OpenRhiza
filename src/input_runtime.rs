use alloc::string::String;

use lazy_static::lazy_static;
use spin::Mutex;

use crate::input_handoff::{HidDeviceKind, InputRoutingMode, SandboxInputCommand};
use crate::sandbox_lifecycle::{PendingLoadDisposition, SandboxRuntimeState};

pub const INPUT_KEYBOARD_MATCH_KEY: &str = "input:keyboard";
pub const INPUT_MOUSE_MATCH_KEY: &str = "input:mouse";
pub const INPUT_KEYBOARD_DRIVER_ID: &str = "snd_input_keyboard_bootstrap_v1";
pub const INPUT_MOUSE_DRIVER_ID: &str = "snd_input_mouse_bootstrap_v1";

const KEYBOARD_DRIVER_FILES: [[u8; 11]; 1] = [*b"KEYBDRV WAS"];
const MOUSE_DRIVER_FILES: [[u8; 11]; 1] = [*b"MOUSEDRVWAS"];

pub use crate::sandbox_lifecycle::SandboxStage as InputDriverStage;

#[derive(Clone, Debug)]
struct InputRuntimeEntry {
    kind: HidDeviceKind,
    lifecycle: SandboxRuntimeState,
}

#[derive(Clone, Debug)]
pub struct InputRuntimeState {
    pub kind: HidDeviceKind,
    pub component: SandboxRuntimeState,
}

lazy_static! {
    static ref INPUT_RUNTIME_STATE: Mutex<[InputRuntimeEntry; 2]> = Mutex::new([
        InputRuntimeEntry {
            kind: HidDeviceKind::Keyboard,
            lifecycle: SandboxRuntimeState::new(),
        },
        InputRuntimeEntry {
            kind: HidDeviceKind::Mouse,
            lifecycle: SandboxRuntimeState::new(),
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
    Some(snapshot_entry(state))
}

pub fn finish_load_failure(kind: HidDeviceKind, error: &str) -> InputRuntimeState {
    let mut states = INPUT_RUNTIME_STATE.lock();
    let state = &mut states[kind_index(kind)];
    state.lifecycle.finish_load_failure(error);
    snapshot_entry(state)
}

pub fn promote(kind: HidDeviceKind) -> Result<String, &'static str> {
    let driver_id = {
        let mut states = INPUT_RUNTIME_STATE.lock();
        let state = &mut states[kind_index(kind)];
        state.lifecycle.promote_current()?
    };

    crate::driver_cache::persist_active_driver_binding(match_key_for_kind(kind), driver_id.as_str())?;
    crate::runtime_bindings::activate_binding(match_key_for_kind(kind), driver_id.as_str(), "input-runtime");
    Ok(driver_id)
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

    begin_load(kind, driver_id.as_str(), PendingLoadDisposition::RestoreActive)?;
    Ok(Some(driver_id))
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

        match queue_restore_load(kind, driver_id.as_str()) {
            Ok(()) => crate::println!(
                "[Sandbox Input] Scheduled persisted {} driver {} for auto-restore.",
                kind_label(kind),
                driver_id
            ),
            Err(error) => crate::println!(
                "[Sandbox Input] Could not restore persisted {} driver {}: {}",
                kind_label(kind),
                driver_id,
                error
            ),
        }
    }
}
