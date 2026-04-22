use alloc::string::String;
use alloc::vec::Vec;

const SKILL_MAP_FILES: [[u8; 11]; 3] = [*b"SKILLCCHTXT", *b"SKILLMAPTXT", *b"SKMAP   TXT"];

#[derive(Clone, Debug)]
pub struct CachedSkillArtifact {
    pub skill_id: String,
    pub fat_name_text: String,
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

        cached.push(CachedSkillArtifact {
            skill_id: skill_id.clone(),
            fat_name_text: derive_fat_name_text(skill_id),
        });
    }

    persist_cached_skills(&cached)?;
    Ok(cached.len())
}

fn derive_fat_name_text(skill_id: &str) -> String {
    let digest = crate::identity::sha256_hex(skill_id.as_bytes());
    let mut base = String::from("SK");
    base.push_str(&digest[..6]);
    base.push_str(".WAS");
    base
}
