use alloc::string::String;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

const SKILL_MAP_FILES: [[u8; 11]; 3] = [*b"SKILLCCHTXT", *b"SKILLMAPTXT", *b"SKMAP   TXT"];
const SKILL_SLOT_TEXT: [&str; 25] = [
    "SK000.WAS",
    "SK001.WAS",
    "SK002.WAS",
    "SK003.WAS",
    "SK004.WAS",
    "SK005.WAS",
    "SK006.WAS",
    "SK007.WAS",
    "SK008.WAS",
    "SK009.WAS",
    "SK010.WAS",
    "SK011.WAS",
    "SK012.WAS",
    "SK013.WAS",
    "SK014.WAS",
    "SK015.WAS",
    "SK016.WAS",
    "SK017.WAS",
    "SK018.WAS",
    "SK019.WAS",
    "SK020.WAS",
    "SK021.WAS",
    "SK022.WAS",
    "SK023.WAS",
    "SK024.WAS",
];
const SEED_SKILL_MAP: [(&str, &str); 28] = [
    ("skill_display_console_mode_v1", "SK000.WAS"),
    ("skill_gui_session_bootstrap_v1", "SK001.WAS"),
    ("skill_display_framebuffer_mode_v1", "SK002.WAS"),
    ("skill_gui_compositor_seed_v1", "SK003.WAS"),
    ("skill_registry_lookup_v1", "SK004.WAS"),
    ("skill_gui_scene_mutator_v1", "SK005.WAS"),
    ("skill_fs_image_probe_v1", "SK006.WAS"),
    ("skill_gui_modern_shell_v1", "SK007.WAS"),
    ("skill_qemu_driver_pack_v1", "SK008.WAS"),
    ("skill_voice_capture_bridge_v1", "SK009.WAS"),
    ("skill_voice_router_policy_v1", "SK009.WAS"),
    ("skill_voice_audio_llm_bridge_v1", "SK009.WAS"),
    ("skill_e1000_sandbox_probe_v1", "SK010.WAS"),
    ("skill_storage_host_probe_v1", "SK011.WAS"),
    ("skill_piix_ide_sandbox_probe_v1", "SK012.WAS"),
    ("skill_xhci_sandbox_probe_v1", "SKXHCI.WAS"),
    ("skill_text_web_browser_v1", "SK014.WAS"),
    ("skill_gui_web_browser_v1", "SK015.WAS"),
    ("skill_network_diagnostics_v1", "SK016.WAS"),
    ("skill_system_monitor_v1", "SK017.WAS"),
    ("skill_virtio_mmio_probe_v1", "SK018.WAS"),
    ("skill_virtio_net_driver_v1", "SK019.WAS"),
    ("skill_virtio_block_driver_v1", "SK020.WAS"),
    ("skill_virtio_input_driver_v1", "SK021.WAS"),
    ("skill_virtio_gpu_driver_v1", "SK022.WAS"),
    ("skill_arm64_virtio_gpu_payload_v1", "SK023.WAS"),
    ("skill_desktop_compositor_v1", "SK024.WAS"),
    ("skill_doom_port_v1", "SKDOGEN.WAS"),
];

#[derive(Clone, Debug)]
pub struct CachedSkillArtifact {
    pub skill_id: String,
    pub fat_name_text: String,
    pub metadata: SkillArtifactMetadata,
}

#[derive(Clone, Debug, Default)]
pub struct SkillArtifactMetadata {
    pub artifact_id: Option<String>,
    pub version: Option<u64>,
    pub updated_at: Option<String>,
    pub signature_verified: bool,
}

#[derive(Clone, Debug)]
struct CachedSkillPayload {
    skill_id: String,
    fat_name_text: String,
    payload: Vec<u8>,
}

lazy_static! {
    static ref SKILL_PAYLOAD_CACHE: Mutex<Vec<CachedSkillPayload>> = Mutex::new(Vec::new());
}

pub fn load_cached_skill_map_text() -> Option<String> {
    crate::storage::read_text_file_from_secondary_fat16(&SKILL_MAP_FILES)
}

pub fn load_cached_skills() -> Vec<CachedSkillArtifact> {
    let Some(text) = load_cached_skill_map_text() else {
        return Vec::new();
    };

    parse_cached_skills(&text)
}

pub fn find_cached_skill(skill_id: &str) -> Option<CachedSkillArtifact> {
    load_cached_skills()
        .into_iter()
        .find(|artifact| artifact.skill_id == skill_id)
        .or_else(|| find_seed_skill(skill_id))
}

pub fn find_recovery_cached_skill(skill_id: &str) -> Option<CachedSkillArtifact> {
    select_recovery_candidate_from_records(load_cached_skills(), skill_id)
        .or_else(|| find_seed_skill(skill_id))
}

fn select_recovery_candidate_from_records(
    records: Vec<CachedSkillArtifact>,
    skill_id: &str,
) -> Option<CachedSkillArtifact> {
    let mut selected: Option<(usize, CachedSkillArtifact)> = None;

    for (index, artifact) in records.into_iter().enumerate() {
        if artifact.skill_id != skill_id {
            continue;
        }
        let replace = match selected.as_ref() {
            Some((selected_index, selected_artifact)) => {
                recovery_candidate_is_better(index, &artifact, *selected_index, selected_artifact)
            }
            None => true,
        };
        if replace {
            selected = Some((index, artifact));
        }
    }

    selected.map(|(_, artifact)| artifact)
}

fn find_seed_skill(skill_id: &str) -> Option<CachedSkillArtifact> {
    SEED_SKILL_MAP
        .iter()
        .find(|(seed_skill_id, _)| *seed_skill_id == skill_id)
        .map(|(seed_skill_id, fat_name_text)| CachedSkillArtifact {
            skill_id: String::from(*seed_skill_id),
            fat_name_text: String::from(*fat_name_text),
            metadata: SkillArtifactMetadata::default(),
        })
}

fn recovery_candidate_is_better(
    candidate_index: usize,
    candidate: &CachedSkillArtifact,
    selected_index: usize,
    selected: &CachedSkillArtifact,
) -> bool {
    if candidate.metadata.signature_verified != selected.metadata.signature_verified {
        return candidate.metadata.signature_verified;
    }

    let candidate_version = candidate.metadata.version.unwrap_or(0);
    let selected_version = selected.metadata.version.unwrap_or(0);
    if candidate_version != selected_version {
        return candidate_version > selected_version;
    }

    let candidate_updated = candidate.metadata.updated_at.as_deref().unwrap_or("");
    let selected_updated = selected.metadata.updated_at.as_deref().unwrap_or("");
    if candidate_updated != selected_updated {
        return candidate_updated > selected_updated;
    }

    candidate_index > selected_index
}

pub fn parse_cached_skills(text: &str) -> Vec<CachedSkillArtifact> {
    let mut cached = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((skill_id, artifact_text)) = trimmed.split_once('=') else {
            continue;
        };

        let skill_id = skill_id.trim();
        let mut artifact_fields = artifact_text.split('|');
        let Some(fat_name_text) = artifact_fields.next() else {
            continue;
        };
        let fat_name_text = fat_name_text.trim();
        if skill_id.is_empty() || fat_name_text.is_empty() {
            continue;
        }
        if !fat_name_text.ends_with(".WAS") {
            continue;
        }
        let mut metadata = SkillArtifactMetadata::default();
        for field in artifact_fields {
            apply_metadata_field(&mut metadata, field.trim());
        }

        cached.push(CachedSkillArtifact {
            skill_id: String::from(skill_id),
            fat_name_text: String::from(fat_name_text),
            metadata,
        });
    }

    cached
}

pub fn recovery_ranking_selftest() -> Result<CachedSkillArtifact, &'static str> {
    let fixture = "\
skill_rank_selftest_v1=SK020.WAS|artifact_id=unsigned-new|version=99|updated_at=2026-06-01T00:00:00Z\n\
skill_rank_selftest_v1=SK021.WAS|artifact_id=signed-old|version=1|updated_at=2026-01-01T00:00:00Z|signature_verified=1\n\
skill_rank_selftest_v1=SK022.WAS|artifact_id=signed-v2|version=2|updated_at=2026-02-01T00:00:00Z|signature_verified=1\n\
skill_rank_selftest_v1=SK023.WAS|artifact_id=signed-v2-newer|version=2|updated_at=2026-03-01T00:00:00Z|signature_verified=1\n\
skill_rank_selftest_v1=SK024.WAS|artifact_id=signed-v2-newer-order|version=2|updated_at=2026-03-01T00:00:00Z|signature_verified=1\n";

    let records = parse_cached_skills(fixture);
    let selected = select_recovery_candidate_from_records(records, "skill_rank_selftest_v1")
        .ok_or("fixture did not produce a recovery candidate")?;

    if selected.fat_name_text != "SK024.WAS" {
        return Err("recovery ranking selected the wrong artifact");
    }
    if !selected.metadata.signature_verified {
        return Err("recovery ranking did not prefer a signed artifact");
    }
    if selected.metadata.version != Some(2) {
        return Err("recovery ranking did not preserve version ordering");
    }
    if selected.metadata.artifact_id.as_deref() != Some("signed-v2-newer-order") {
        return Err("recovery ranking did not preserve deterministic tie ordering");
    }

    Ok(selected)
}

fn apply_metadata_field(metadata: &mut SkillArtifactMetadata, field: &str) {
    let Some((key, value)) = field.split_once('=') else {
        return;
    };
    let key = key.trim();
    let value = value.trim();
    if value.is_empty() {
        return;
    }

    match key {
        "artifact_id" => metadata.artifact_id = Some(String::from(value)),
        "version" => {
            if let Ok(version) = value.parse::<u64>() {
                metadata.version = Some(version);
            }
        }
        "updated_at" => metadata.updated_at = Some(String::from(value)),
        "signature_verified" => {
            metadata.signature_verified = value == "1" || value.eq_ignore_ascii_case("true");
        }
        _ => {}
    }
}

pub fn fat_name_bytes_from_text(name: &str) -> Option<[u8; 11]> {
    let mut parts = name.split('.');
    let base = parts.next()?.trim().to_ascii_uppercase();
    let ext = parts.next().unwrap_or("").trim().to_ascii_uppercase();
    if parts.next().is_some() {
        return None;
    }
    if base.is_empty() || base.len() > 8 || ext.len() > 3 {
        return None;
    }

    let mut fat = [b' '; 11];
    for (index, byte) in base.as_bytes().iter().enumerate() {
        fat[index] = *byte;
    }
    for (index, byte) in ext.as_bytes().iter().enumerate() {
        fat[8 + index] = *byte;
    }
    Some(fat)
}

pub fn preload_cached_skill_payloads(records: &[CachedSkillArtifact]) -> usize {
    let mut loaded = 0usize;
    for record in records {
        if load_cached_skill_payload(record).is_some() {
            loaded += 1;
        }
    }
    loaded
}

pub fn load_cached_skill_payload(record: &CachedSkillArtifact) -> Option<Vec<u8>> {
    {
        let cache = SKILL_PAYLOAD_CACHE.lock();
        if let Some(cached) = cache
            .iter()
            .find(|cached| {
                cached.skill_id == record.skill_id && cached.fat_name_text == record.fat_name_text
            })
        {
            return Some(cached.payload.clone());
        }
    }

    let fat_name = fat_name_bytes_from_text(record.fat_name_text.as_str())?;
    let payload = crate::storage::read_named_file_from_secondary_fat16(&[fat_name])?;
    remember_skill_payload(
        record.skill_id.as_str(),
        record.fat_name_text.as_str(),
        payload.as_slice(),
    );
    Some(payload)
}

fn remember_skill_payload(skill_id: &str, fat_name_text: &str, payload: &[u8]) {
    let mut cache = SKILL_PAYLOAD_CACHE.lock();
    if let Some(existing) = cache.iter_mut().find(|cached| {
        cached.skill_id == skill_id && cached.fat_name_text == fat_name_text
    }) {
        existing.fat_name_text = String::from(fat_name_text);
        existing.payload.clear();
        existing.payload.extend_from_slice(payload);
        return;
    }

    cache.push(CachedSkillPayload {
        skill_id: String::from(skill_id),
        fat_name_text: String::from(fat_name_text),
        payload: payload.to_vec(),
    });
}

pub fn persist_cached_skills(records: &[CachedSkillArtifact]) -> Result<(), &'static str> {
    let mut out = String::from("# OpenRhiza local skill cache\n");
    for record in records {
        out.push_str(record.skill_id.as_str());
        out.push('=');
        out.push_str(record.fat_name_text.as_str());
        append_metadata_text(&mut out, &record.metadata);
        out.push('\n');
    }

    crate::storage::write_named_file_to_secondary_fat16_preserve_size(
        &SKILL_MAP_FILES,
        out.as_bytes(),
    )
}

fn append_metadata_text(out: &mut String, metadata: &SkillArtifactMetadata) {
    if let Some(artifact_id) = metadata.artifact_id.as_deref() {
        out.push_str("|artifact_id=");
        out.push_str(artifact_id);
    }
    if let Some(version) = metadata.version {
        out.push_str("|version=");
        push_u64_decimal(out, version);
    }
    if let Some(updated_at) = metadata.updated_at.as_deref() {
        out.push_str("|updated_at=");
        out.push_str(updated_at);
    }
    if metadata.signature_verified {
        out.push_str("|signature_verified=1");
    }
}

fn push_u64_decimal(out: &mut String, mut value: u64) {
    let mut digits = [0u8; 20];
    let mut len = 0usize;
    loop {
        digits[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    while len > 0 {
        len -= 1;
        out.push(digits[len] as char);
    }
}

pub fn update_cached_skills(skill_ids: &[String]) -> Result<usize, &'static str> {
    let mut cached = load_cached_skills();

    for skill_id in skill_ids {
        if cached.iter().any(|record| record.skill_id == *skill_id) {
            continue;
        }

        let fat_name_text = allocate_skill_slot_text(&cached)
            .ok_or("no free preallocated skill slot is available")?;
        cached.push(CachedSkillArtifact {
            skill_id: skill_id.clone(),
            fat_name_text,
            metadata: SkillArtifactMetadata::default(),
        });
    }

    persist_cached_skills(&cached)?;
    Ok(cached.len())
}

fn allocate_skill_slot_text(cached: &[CachedSkillArtifact]) -> Option<String> {
    for slot in SKILL_SLOT_TEXT {
        if cached.iter().all(|record| record.fat_name_text != slot) {
            return Some(String::from(slot));
        }
    }

    None
}

pub fn persist_downloaded_skill(skill_id: &str, payload: &[u8]) -> Result<String, &'static str> {
    persist_downloaded_skill_with_metadata(skill_id, payload, SkillArtifactMetadata::default())
}

pub fn persist_downloaded_skill_with_metadata(
    skill_id: &str,
    payload: &[u8],
    metadata: SkillArtifactMetadata,
) -> Result<String, &'static str> {
    let mut cached = load_cached_skills();
    let existing_index = find_download_target_index(&cached, skill_id, &metadata);
    let fat_name_text = match existing_index {
        Some(index) => cached[index].fat_name_text.clone(),
        None => allocate_skill_slot_text(&cached)
            .ok_or("no free preallocated skill slot is available")?,
    };

    let fat_name = fat_name_bytes_from_text(fat_name_text.as_str())
        .ok_or("invalid FAT file name for downloaded skill payload")?;
    crate::storage::write_named_file_to_secondary_fat16_existing(&[fat_name], payload)?;
    remember_skill_payload(skill_id, fat_name_text.as_str(), payload);

    match existing_index {
        Some(index) => {
            cached[index].metadata = metadata;
        }
        None => cached.push(CachedSkillArtifact {
            skill_id: String::from(skill_id),
            fat_name_text: fat_name_text.clone(),
            metadata,
        }),
    }
    persist_cached_skills(&cached)?;

    Ok(fat_name_text)
}

fn find_download_target_index(
    cached: &[CachedSkillArtifact],
    skill_id: &str,
    metadata: &SkillArtifactMetadata,
) -> Option<usize> {
    if let Some(artifact_id) = metadata.artifact_id.as_deref() {
        if let Some(index) = cached.iter().position(|record| {
            record.skill_id == skill_id
                && record.metadata.artifact_id.as_deref() == Some(artifact_id)
        }) {
            return Some(index);
        }
        return None;
    }

    cached.iter().position(|record| record.skill_id == skill_id)
}
