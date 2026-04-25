use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;
use spin::Mutex;

use crate::sandbox_lifecycle::{PendingLoadDisposition, SandboxRuntimeState, SandboxStage};

pub const REGISTRY_LOOKUP_SKILL_ID: &str = "skill_registry_lookup_v1";
const ACTIVE_SKILL_FILES: [[u8; 11]; 1] = [*b"SKLACTV TXT"];

#[derive(Clone, Debug)]
struct SkillRuntimeEntry {
    skill_id: String,
    fat_name_text: String,
    lifecycle: SandboxRuntimeState,
}

#[derive(Clone, Debug)]
pub struct SkillRuntimeState {
    pub skill_id: String,
    pub fat_name_text: String,
    pub stage: SandboxStage,
    pub current_artifact_id: Option<String>,
    pub persisted_artifact_id: Option<String>,
    pub previous_artifact_id: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SkillRuntimeSnapshot {
    pub component: SandboxRuntimeState,
    pub cached_skill_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum SkillRuntimeCommand {
    Load { skill_id: String },
    Unload { skill_id: String },
    Run { skill_id: String },
}

lazy_static! {
    pub static ref SKILL_RUNTIME_COMMAND_QUEUE: Arc<ArrayQueue<SkillRuntimeCommand>> =
        Arc::new(ArrayQueue::new(16));
    static ref SKILL_RUNTIME_STATE: Mutex<Vec<SkillRuntimeEntry>> = Mutex::new(Vec::new());
    static ref AUTO_RUN_SKILL_IDS: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

fn module_key(skill_id: &str) -> String {
    let mut key = String::from("skill:");
    key.push_str(skill_id);
    key
}

fn upsert_entry<'a>(
    entries: &'a mut Vec<SkillRuntimeEntry>,
    skill_id: &str,
    fat_name_text: &str,
) -> &'a mut SkillRuntimeEntry {
    if let Some(index) = entries.iter().position(|entry| entry.skill_id == skill_id) {
        let entry = &mut entries[index];
        entry.fat_name_text = String::from(fat_name_text);
        return entry;
    }

    entries.push(SkillRuntimeEntry {
        skill_id: String::from(skill_id),
        fat_name_text: String::from(fat_name_text),
        lifecycle: SandboxRuntimeState::new(),
    });
    entries.last_mut().unwrap()
}

fn snapshot_entry(entry: &SkillRuntimeEntry) -> SkillRuntimeState {
    SkillRuntimeState {
        skill_id: entry.skill_id.clone(),
        fat_name_text: entry.fat_name_text.clone(),
        stage: entry.lifecycle.stage,
        current_artifact_id: entry.lifecycle.current_artifact_id.clone(),
        persisted_artifact_id: entry.lifecycle.persisted_artifact_id.clone(),
        previous_artifact_id: entry.lifecycle.previous_artifact_id.clone(),
        last_error: entry.lifecycle.last_error.clone(),
    }
}

pub fn load_cached_skills() -> Option<Vec<crate::skill_cache::CachedSkillArtifact>> {
    let records = crate::skill_cache::load_cached_skills();
    if records.is_empty() {
        None
    } else {
        Some(records)
    }
}

pub fn load_cached_skills_text() -> Option<String> {
    crate::skill_cache::load_cached_skill_map_text()
}

pub fn install_cached_skills(
    records: &[crate::skill_cache::CachedSkillArtifact],
    _source: &str,
) -> usize {
    let mut entries = SKILL_RUNTIME_STATE.lock();
    let mut installed = 0usize;

    for record in records {
        let _ = upsert_entry(&mut entries, record.skill_id.as_str(), record.fat_name_text.as_str());
        installed += 1;
    }

    installed
}

pub fn schedule_persisted_skill_restores() -> usize {
    let active_skill_ids = load_active_skill_ids();
    let mut scheduled = 0usize;

    for skill_id in active_skill_ids {
        if queue_load(skill_id.as_str()).is_ok() {
            scheduled += 1;
        }
    }

    scheduled
}

pub fn update_cached_skills(skill_ids: &[String]) -> Result<usize, &'static str> {
    let count = crate::skill_cache::update_cached_skills(skill_ids)?;
    let _ = crate::capability_cache::persist_registry_summary(
        crate::capability_cache::RegistryDomain::Skill,
        local_skill_ids_summary().as_str(),
    );
    Ok(count)
}

pub fn queue_load(skill_id: &str) -> Result<String, &'static str> {
    let cached = crate::skill_cache::find_cached_skill(skill_id)
        .ok_or("cached local skill artifact not found")?;
    let artifact_id = module_key(skill_id);

    {
        let mut entries = SKILL_RUNTIME_STATE.lock();
        let entry = upsert_entry(&mut entries, skill_id, cached.fat_name_text.as_str());
        entry
            .lifecycle
            .begin_load(artifact_id.as_str(), PendingLoadDisposition::Testing)?;
    }

    SKILL_RUNTIME_COMMAND_QUEUE
        .push(SkillRuntimeCommand::Load {
            skill_id: String::from(skill_id),
        })
        .map_err(|_| "skill runtime command queue full")?;

    Ok(cached.fat_name_text)
}

pub fn queue_run(skill_id: &str) -> Result<(), &'static str> {
    {
        let entries = SKILL_RUNTIME_STATE.lock();
        let Some(entry) = entries.iter().find(|entry| entry.skill_id == skill_id) else {
            return Err("skill is not loaded");
        };
        if entry.lifecycle.current_artifact_id.is_none() {
            return Err("skill is not loaded");
        }
    }

    SKILL_RUNTIME_COMMAND_QUEUE
        .push(SkillRuntimeCommand::Run {
            skill_id: String::from(skill_id),
        })
        .map_err(|_| "skill runtime command queue full")
}

pub fn schedule_auto_run(skill_id: &str) {
    let mut pending = AUTO_RUN_SKILL_IDS.lock();
    if pending.iter().all(|value| value != skill_id) {
        pending.push(String::from(skill_id));
    }
}

pub fn take_auto_run(skill_id: &str) -> bool {
    let mut pending = AUTO_RUN_SKILL_IDS.lock();
    if let Some(index) = pending.iter().position(|value| value == skill_id) {
        pending.remove(index);
        true
    } else {
        false
    }
}

pub fn queue_unload(skill_id: &str) -> Result<(), &'static str> {
    {
        let mut entries = SKILL_RUNTIME_STATE.lock();
        let Some(entry) = entries.iter_mut().find(|entry| entry.skill_id == skill_id) else {
            return Err("skill is not loaded");
        };
        let _ = entry.lifecycle.rollback_to_bootstrap()?;
    }

    SKILL_RUNTIME_COMMAND_QUEUE
        .push(SkillRuntimeCommand::Unload {
            skill_id: String::from(skill_id),
        })
        .map_err(|_| "skill runtime command queue full")
}

pub fn promote(skill_id: &str) -> Result<String, &'static str> {
    let mut entries = SKILL_RUNTIME_STATE.lock();
    let Some(entry) = entries.iter_mut().find(|entry| entry.skill_id == skill_id) else {
        return Err("skill is not loaded");
    };
    let artifact_id = entry.lifecycle.promote_current()?;
    drop(entries);
    persist_active_skill_ids()?;
    Ok(artifact_id)
}

pub fn finish_load_success(skill_id: &str) -> Option<SkillRuntimeState> {
    let mut entries = SKILL_RUNTIME_STATE.lock();
    let entry = entries.iter_mut().find(|entry| entry.skill_id == skill_id)?;
    entry.lifecycle.finish_load_success()?;
    Some(snapshot_entry(entry))
}

pub fn finish_load_failure(skill_id: &str, error: &str) -> Option<SkillRuntimeState> {
    let mut entries = SKILL_RUNTIME_STATE.lock();
    let entry = entries.iter_mut().find(|entry| entry.skill_id == skill_id)?;
    entry.lifecycle.finish_load_failure(error);
    Some(snapshot_entry(entry))
}

pub fn snapshot() -> SkillRuntimeSnapshot {
    let states = runtime_states();
    let component = states
        .iter()
        .find(|state| matches!(state.stage, SandboxStage::Active | SandboxStage::Testing))
        .map(|state| SandboxRuntimeState {
            stage: state.stage,
            current_artifact_id: state.current_artifact_id.clone(),
            persisted_artifact_id: state.persisted_artifact_id.clone(),
            previous_artifact_id: state.previous_artifact_id.clone(),
            last_error: state.last_error.clone(),
            pending_artifact_id: None,
            pending_disposition: None,
        })
        .unwrap_or_else(SandboxRuntimeState::new);

    SkillRuntimeSnapshot {
        component,
        cached_skill_ids: crate::skill_cache::load_cached_skills()
            .into_iter()
            .map(|artifact| artifact.skill_id)
            .collect(),
    }
}

pub fn runtime_states() -> Vec<SkillRuntimeState> {
    SKILL_RUNTIME_STATE
        .lock()
        .iter()
        .map(snapshot_entry)
        .collect()
}

pub fn module_key_for_skill(skill_id: &str) -> String {
    module_key(skill_id)
}

pub fn local_skill_ids_summary() -> String {
    let cached = crate::skill_cache::load_cached_skills();
    if cached.is_empty() {
        return String::from("none");
    }

    let mut summary = String::new();
    for (index, artifact) in cached.iter().take(4).enumerate() {
        if index > 0 {
            summary.push_str(", ");
        }
        summary.push_str(artifact.skill_id.as_str());
    }
    if cached.len() > 4 {
        summary.push_str(", ...");
    }
    summary
}

pub fn local_skill_context_block() -> Option<String> {
    let cached = crate::skill_cache::load_cached_skills();
    let states = runtime_states();

    if cached.is_empty() && states.is_empty() {
        return None;
    }

    let mut out = String::from("Local skill cache and runtime:\n");
    if cached.is_empty() {
        out.push_str("- cached skills: none\n");
    } else {
        out.push_str("- cached skills: ");
        out.push_str(local_skill_ids_summary().as_str());
        out.push('\n');
    }

    for state in states.iter().take(8) {
        out.push_str("- skill ");
        out.push_str(state.skill_id.as_str());
        out.push_str(" stage=");
        out.push_str(match state.stage {
            SandboxStage::Bootstrap => "bootstrap",
            SandboxStage::Cached => "cached",
            SandboxStage::Testing => "testing",
            SandboxStage::Active => "active",
        });
        out.push_str(" current=");
        out.push_str(state.current_artifact_id.as_deref().unwrap_or("none"));
        out.push_str(" cached_file=");
        out.push_str(state.fat_name_text.as_str());
        out.push('\n');
    }

    Some(out)
}

pub fn load_active_skills_text() -> Option<String> {
    crate::storage::read_text_file_from_secondary_fat16(&ACTIVE_SKILL_FILES)
}

fn load_active_skill_ids() -> Vec<String> {
    let Some(text) = load_active_skills_text() else {
        return Vec::new();
    };

    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                None
            } else {
                Some(String::from(trimmed))
            }
        })
        .collect()
}

fn persist_active_skill_ids() -> Result<(), &'static str> {
    let states = runtime_states();
    let mut out = String::from("# OpenRhiza active skill map\n");
    for state in states {
        if state.persisted_artifact_id.is_some() {
            out.push_str(state.skill_id.as_str());
            out.push('\n');
        }
    }

    crate::storage::write_named_file_to_secondary_fat16_preserve_size(
        &ACTIVE_SKILL_FILES,
        out.as_bytes(),
    )
}
