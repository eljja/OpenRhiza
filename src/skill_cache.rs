use alloc::string::String;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

const SKILL_MAP_FILES: [[u8; 11]; 3] = [*b"SKILLCCHTXT", *b"SKILLMAPTXT", *b"SKMAP   TXT"];
const SKILL_SLOT_TEXT: [&str; 9] = [
    "SK000.WAS",
    "SK001.WAS",
    "SK002.WAS",
    "SK003.WAS",
    "SK004.WAS",
    "SK005.WAS",
    "SK006.WAS",
    "SK007.WAS",
    "SK008.WAS",
];
const SEED_SKILL_MAP: [(&str, &str); 9] = [
    ("skill_display_console_mode_v1", "SK000.WAS"),
    ("skill_gui_session_bootstrap_v1", "SK001.WAS"),
    ("skill_display_framebuffer_mode_v1", "SK002.WAS"),
    ("skill_gui_compositor_seed_v1", "SK003.WAS"),
    ("skill_registry_lookup_v1", "SK004.WAS"),
    ("skill_gui_scene_mutator_v1", "SK005.WAS"),
    ("skill_fs_image_probe_v1", "SK006.WAS"),
    ("skill_gui_modern_shell_v1", "SK007.WAS"),
    ("skill_qemu_driver_pack_v1", "SK008.WAS"),
];

#[derive(Clone, Debug)]
pub struct CachedSkillArtifact {
    pub skill_id: String,
    pub fat_name_text: String,
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
        .or_else(|| {
            SEED_SKILL_MAP
                .iter()
                .find(|(seed_skill_id, _)| *seed_skill_id == skill_id)
                .map(|(seed_skill_id, fat_name_text)| CachedSkillArtifact {
                    skill_id: String::from(*seed_skill_id),
                    fat_name_text: String::from(*fat_name_text),
                })
        })
}

pub fn parse_cached_skills(text: &str) -> Vec<CachedSkillArtifact> {
    let mut cached = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((skill_id, fat_name_text)) = trimmed.split_once('=') else {
            continue;
        };

        let skill_id = skill_id.trim();
        let fat_name_text = fat_name_text.trim();
        if skill_id.is_empty() || fat_name_text.is_empty() {
            continue;
        }
        if !fat_name_text.ends_with(".WAS") {
            continue;
        }

        cached.push(CachedSkillArtifact {
            skill_id: String::from(skill_id),
            fat_name_text: String::from(fat_name_text),
        });
    }

    cached
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
        if let Some(cached) = cache.iter().find(|cached| cached.skill_id == record.skill_id) {
            return Some(cached.payload.clone());
        }
    }

    let fat_name = fat_name_bytes_from_text(record.fat_name_text.as_str())?;
    let payload = crate::storage::read_named_file_from_secondary_fat16(&[fat_name])?;
    remember_skill_payload(record.skill_id.as_str(), record.fat_name_text.as_str(), payload.as_slice());
    Some(payload)
}

fn remember_skill_payload(skill_id: &str, fat_name_text: &str, payload: &[u8]) {
    let mut cache = SKILL_PAYLOAD_CACHE.lock();
    if let Some(existing) = cache.iter_mut().find(|cached| cached.skill_id == skill_id) {
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
        out.push('\n');
    }

    crate::storage::write_named_file_to_secondary_fat16_preserve_size(
        &SKILL_MAP_FILES,
        out.as_bytes(),
    )
}

pub fn update_cached_skills(skill_ids: &[String]) -> Result<usize, &'static str> {
    let mut cached = load_cached_skills();

    for skill_id in skill_ids {
        if cached.iter().any(|record| record.skill_id == *skill_id) {
            continue;
        }

        let fat_name_text =
            allocate_skill_slot_text(&cached).ok_or("no free preallocated skill slot is available")?;
        cached.push(CachedSkillArtifact {
            skill_id: skill_id.clone(),
            fat_name_text,
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
    let mut cached = load_cached_skills();
    let fat_name_text = if let Some(existing) = cached.iter().find(|record| record.skill_id == skill_id) {
        existing.fat_name_text.clone()
    } else {
        allocate_skill_slot_text(&cached).ok_or("no free preallocated skill slot is available")?
    };

    let fat_name = fat_name_bytes_from_text(fat_name_text.as_str())
        .ok_or("invalid FAT file name for downloaded skill payload")?;
    crate::storage::write_named_file_to_secondary_fat16_existing(&[fat_name], payload)?;
    remember_skill_payload(skill_id, fat_name_text.as_str(), payload);

    if cached.iter().all(|record| record.skill_id != skill_id) {
        cached.push(CachedSkillArtifact {
            skill_id: String::from(skill_id),
            fat_name_text: fat_name_text.clone(),
        });
        persist_cached_skills(&cached)?;
    }

    Ok(fat_name_text)
}
