use alloc::string::String;
use alloc::vec::Vec;

use lazy_static::lazy_static;
use spin::Mutex;

use crate::sandbox_lifecycle::{SandboxRuntimeState, SandboxStage};

#[derive(Clone, Debug)]
struct DriverRuntimeEntry {
    match_key: String,
    source: String,
    lifecycle: SandboxRuntimeState,
}

#[derive(Clone, Debug)]
pub struct DriverRuntimeState {
    pub match_key: String,
    pub source: String,
    pub component: SandboxRuntimeState,
}

lazy_static! {
    static ref DRIVER_RUNTIME_STATE: Mutex<Vec<DriverRuntimeEntry>> = Mutex::new(Vec::new());
}

fn snapshot_entry(entry: &DriverRuntimeEntry) -> DriverRuntimeState {
    DriverRuntimeState {
        match_key: entry.match_key.clone(),
        source: entry.source.clone(),
        component: entry.lifecycle.clone(),
    }
}

fn ensure_entry<'a>(
    entries: &'a mut Vec<DriverRuntimeEntry>,
    match_key: &str,
    source: &str,
) -> &'a mut DriverRuntimeEntry {
    if let Some(index) = entries.iter().position(|entry| entry.match_key == match_key) {
        let entry = &mut entries[index];
        entry.source = String::from(source);
        return entry;
    }

    entries.push(DriverRuntimeEntry {
        match_key: String::from(match_key),
        source: String::from(source),
        lifecycle: SandboxRuntimeState::new(),
    });
    entries.last_mut().unwrap()
}

pub fn install_local_bindings(bindings: &[crate::driver_cache::ActiveDriverBinding]) -> usize {
    let installed = crate::runtime_bindings::install_local_bindings(bindings);
    let mut entries = DRIVER_RUNTIME_STATE.lock();
    entries.clear();

    for binding in bindings {
        let entry = ensure_entry(&mut entries, binding.match_key.as_str(), "local-cache");
        entry.lifecycle.current_artifact_id = Some(binding.driver_id.clone());
        entry.lifecycle.persisted_artifact_id = Some(binding.driver_id.clone());
        entry.lifecycle.previous_artifact_id = None;
        entry.lifecycle.pending_artifact_id = None;
        entry.lifecycle.pending_disposition = None;
        entry.lifecycle.last_error = None;
        entry.lifecycle.stage = SandboxStage::Active;
    }

    installed
}

pub fn activate_binding(
    match_key: &str,
    driver_id: &str,
    source: &str,
) -> crate::runtime_bindings::ActivationOutcome {
    let outcome = crate::runtime_bindings::activate_binding(match_key, driver_id, source);
    let mut entries = DRIVER_RUNTIME_STATE.lock();
    let entry = ensure_entry(&mut entries, match_key, source);

    if outcome.changed {
        if let Some(current) = entry.lifecycle.current_artifact_id.as_ref() {
            if current != driver_id {
                entry.lifecycle.previous_artifact_id = Some(current.clone());
            }
        }
    }

    entry.lifecycle.current_artifact_id = Some(String::from(driver_id));
    entry.lifecycle.pending_artifact_id = None;
    entry.lifecycle.pending_disposition = None;
    entry.lifecycle.last_error = None;
    entry.lifecycle.stage = if entry.lifecycle.persisted_artifact_id.as_deref() == Some(driver_id) {
        SandboxStage::Active
    } else {
        SandboxStage::Testing
    };

    outcome
}

pub fn promote_binding(match_key: &str) -> Result<String, &'static str> {
    let driver_id = crate::runtime_bindings::current_driver(match_key)
        .ok_or("no live binding exists for this match key")?;

    {
        let mut entries = DRIVER_RUNTIME_STATE.lock();
        let entry = ensure_entry(&mut entries, match_key, "promote");
        entry.lifecycle.current_artifact_id = Some(driver_id.clone());
        entry.lifecycle.persisted_artifact_id = Some(driver_id.clone());
        entry.lifecycle.pending_artifact_id = None;
        entry.lifecycle.pending_disposition = None;
        entry.lifecycle.last_error = None;
        entry.lifecycle.stage = SandboxStage::Active;
    }

    crate::driver_cache::persist_active_driver_binding(match_key, driver_id.as_str())?;
    Ok(driver_id)
}

pub fn rollback_binding(match_key: &str) -> Result<String, &'static str> {
    let previous = crate::runtime_bindings::rollback_binding(match_key)?;
    let mut entries = DRIVER_RUNTIME_STATE.lock();
    let entry = ensure_entry(&mut entries, match_key, "rollback");

    if previous.starts_with("(removed live binding") {
        entry.lifecycle.current_artifact_id = None;
        entry.lifecycle.pending_artifact_id = None;
        entry.lifecycle.pending_disposition = None;
        entry.lifecycle.last_error = None;
        entry.lifecycle.stage = SandboxStage::Bootstrap;
        return Ok(previous);
    }

    if let Some(current) = entry.lifecycle.current_artifact_id.as_ref() {
        if current != &previous {
            entry.lifecycle.previous_artifact_id = Some(current.clone());
        }
    }

    entry.lifecycle.current_artifact_id = Some(previous.clone());
    entry.lifecycle.pending_artifact_id = None;
    entry.lifecycle.pending_disposition = None;
    entry.lifecycle.last_error = None;
    entry.lifecycle.stage = if entry.lifecycle.persisted_artifact_id.as_deref() == Some(previous.as_str()) {
        SandboxStage::Active
    } else {
        SandboxStage::Testing
    };

    Ok(previous)
}

pub fn note_load_failure(match_key: &str, source: &str, error: &str) {
    let mut entries = DRIVER_RUNTIME_STATE.lock();
    let entry = ensure_entry(&mut entries, match_key, source);
    entry.lifecycle.last_error = Some(String::from(error));
    if entry.lifecycle.current_artifact_id.is_some() {
        entry.lifecycle.stage = if entry.lifecycle.persisted_artifact_id.is_some() {
            SandboxStage::Active
        } else {
            SandboxStage::Testing
        };
    } else {
        entry.lifecycle.stage = SandboxStage::Bootstrap;
    }
}

pub fn cache_binding_candidate(match_key: &str, driver_id: &str, source: &str) {
    let mut entries = DRIVER_RUNTIME_STATE.lock();
    let entry = ensure_entry(&mut entries, match_key, source);
    entry.source = String::from(source);
    entry.lifecycle.note_cached_artifact(driver_id);
}

pub fn snapshot() -> Vec<DriverRuntimeState> {
    DRIVER_RUNTIME_STATE
        .lock()
        .iter()
        .map(snapshot_entry)
        .collect()
}

pub fn context_block() -> Option<String> {
    let states = snapshot();
    if states.is_empty() {
        return None;
    }

    let mut out = String::from("Driver runtime state:\n");
    for state in states.iter().take(12) {
        out.push_str("- ");
        out.push_str(state.match_key.as_str());
        out.push_str(" stage=");
        out.push_str(match state.component.stage {
            SandboxStage::Bootstrap => "bootstrap",
            SandboxStage::Cached => "cached",
            SandboxStage::Testing => "testing",
            SandboxStage::Active => "active",
        });
        out.push_str(" current=");
        out.push_str(state.component.current_artifact_id.as_deref().unwrap_or("none"));
        out.push_str(" persisted=");
        out.push_str(state.component.persisted_artifact_id.as_deref().unwrap_or("none"));
        out.push_str(" source=");
        out.push_str(state.source.as_str());
        out.push('\n');
    }

    Some(out)
}

