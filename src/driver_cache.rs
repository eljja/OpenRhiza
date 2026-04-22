use alloc::string::String;
use alloc::vec::Vec;

const ACTIVE_DRIVER_MAP_FILES: [[u8; 11]; 2] = [*b"DRVMAP  TXT", *b"ACTIVE  MAP"];
const LAST_GENERATED_FILES: [[u8; 11]; 2] = [*b"LASTGEN TXT", *b"DRVLAST TXT"];

#[derive(Clone, Debug)]
pub struct ActiveDriverBinding {
    pub match_key: String,
    pub driver_id: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct DriverManifestRecord {
    pub driver_id: String,
    pub match_key: String,
    pub version: String,
    pub artifact_hash: String,
    pub status: String,
    pub validated: bool,
    pub rollback_target: String,
}

pub fn load_active_driver_map() -> Option<Vec<ActiveDriverBinding>> {
    let text = crate::storage::read_text_file_from_secondary_fat16(&ACTIVE_DRIVER_MAP_FILES)?;
    let bindings = parse_active_driver_map(&text);
    if bindings.is_empty() {
        None
    } else {
        Some(bindings)
    }
}

pub fn load_active_driver_map_text() -> Option<String> {
    crate::storage::read_text_file_from_secondary_fat16(&ACTIVE_DRIVER_MAP_FILES)
}

pub fn parse_active_driver_map(text: &str) -> Vec<ActiveDriverBinding> {
    let mut bindings = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((match_key, driver_id)) = trimmed.split_once('=') else {
            continue;
        };

        let match_key = match_key.trim();
        let driver_id = driver_id.trim();
        if match_key.is_empty() || driver_id.is_empty() {
            continue;
        }

        bindings.push(ActiveDriverBinding {
            match_key: String::from(match_key),
            driver_id: String::from(driver_id),
        });
    }

    bindings
}

pub fn find_active_driver<'a>(
    bindings: &'a [ActiveDriverBinding],
    match_key: &str,
) -> Option<&'a str> {
    bindings
        .iter()
        .find(|binding| binding.match_key == match_key)
        .map(|binding| binding.driver_id.as_str())
}

pub fn persist_active_driver_binding(
    match_key: &str,
    driver_id: &str,
) -> Result<(), &'static str> {
    let mut bindings = load_active_driver_map().unwrap_or_default();

    if let Some(existing) = bindings
        .iter_mut()
        .find(|binding| binding.match_key == match_key)
    {
        existing.driver_id = String::from(driver_id);
    } else {
        bindings.push(ActiveDriverBinding {
            match_key: String::from(match_key),
            driver_id: String::from(driver_id),
        });
    }

    let serialized = serialize_active_driver_map(&bindings);
    crate::storage::write_named_file_to_secondary_fat16_preserve_size(
        &ACTIVE_DRIVER_MAP_FILES,
        serialized.as_bytes(),
    )
}

pub fn remove_active_driver_binding(match_key: &str) -> Result<(), &'static str> {
    let mut bindings = load_active_driver_map().unwrap_or_default();
    let original_len = bindings.len();
    bindings.retain(|binding| binding.match_key != match_key);
    if bindings.len() == original_len {
        return Ok(());
    }

    let serialized = serialize_active_driver_map(&bindings);
    crate::storage::write_named_file_to_secondary_fat16_preserve_size(
        &ACTIVE_DRIVER_MAP_FILES,
        serialized.as_bytes(),
    )
}

pub fn persist_last_generated_driver_note(
    match_key: &str,
    driver_id: &str,
    text: &str,
) -> Result<(), &'static str> {
    let mut note = String::new();
    note.push_str("# OpenRhiza Last Generated Driver\n");
    note.push_str("match_key=");
    note.push_str(match_key);
    note.push('\n');
    note.push_str("driver_id=");
    note.push_str(driver_id);
    note.push('\n');
    note.push_str("text=\n");
    note.push_str(text);
    note.push('\n');

    crate::storage::write_named_file_to_secondary_fat16_existing(
        &LAST_GENERATED_FILES,
        note.as_bytes(),
    )
}

fn serialize_active_driver_map(bindings: &[ActiveDriverBinding]) -> String {
    let mut out = String::from("# OpenRhiza active driver map\n");
    for binding in bindings {
        out.push_str(binding.match_key.as_str());
        out.push('=');
        out.push_str(binding.driver_id.as_str());
        out.push('\n');
    }
    out
}
