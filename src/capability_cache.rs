use alloc::string::String;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryDomain {
    Software,
    Skill,
    Workflow,
    Policy,
    Evaluation,
}

const SOFTWARE_CACHE_FILES: [[u8; 11]; 1] = [*b"SOFTCCH TXT"];
const SKILL_CACHE_FILES: [[u8; 11]; 1] = [*b"SKILLCCHTXT"];
const WORKFLOW_CACHE_FILES: [[u8; 11]; 1] = [*b"WORKCCH TXT"];
const POLICY_CACHE_FILES: [[u8; 11]; 1] = [*b"POLICCH TXT"];
const EVALUATION_CACHE_FILES: [[u8; 11]; 1] = [*b"EVALCCH TXT"];

fn files_for_domain(domain: RegistryDomain) -> &'static [[u8; 11]] {
    match domain {
        RegistryDomain::Software => &SOFTWARE_CACHE_FILES,
        RegistryDomain::Skill => &SKILL_CACHE_FILES,
        RegistryDomain::Workflow => &WORKFLOW_CACHE_FILES,
        RegistryDomain::Policy => &POLICY_CACHE_FILES,
        RegistryDomain::Evaluation => &EVALUATION_CACHE_FILES,
    }
}

fn label_for_domain(domain: RegistryDomain) -> &'static str {
    match domain {
        RegistryDomain::Software => "software",
        RegistryDomain::Skill => "skills",
        RegistryDomain::Workflow => "workflows",
        RegistryDomain::Policy => "policies",
        RegistryDomain::Evaluation => "evaluations",
    }
}

pub fn persist_registry_summary(
    domain: RegistryDomain,
    summary: &str,
) -> Result<(), &'static str> {
    let mut out = String::from("# OpenRhiza capability cache\n");
    out.push_str("domain=");
    out.push_str(label_for_domain(domain));
    out.push('\n');
    out.push_str("summary=");
    out.push_str(summary.trim());
    out.push('\n');

    crate::storage::write_named_file_to_secondary_fat16_preserve_size(
        files_for_domain(domain),
        out.as_bytes(),
    )
}

pub fn load_registry_summary(domain: RegistryDomain) -> Option<String> {
    let text = crate::storage::read_text_file_from_secondary_fat16(files_for_domain(domain))?;
    for line in text.lines() {
        if let Some(summary) = line.strip_prefix("summary=") {
            let trimmed = summary.trim();
            if !trimmed.is_empty() {
                return Some(String::from(trimmed));
            }
        }
    }
    None
}

pub fn current_local_registry_context_block() -> Option<String> {
    let mut out = String::new();

    for domain in [
        RegistryDomain::Software,
        RegistryDomain::Skill,
        RegistryDomain::Workflow,
        RegistryDomain::Policy,
        RegistryDomain::Evaluation,
    ] {
        if let Some(summary) = load_registry_summary(domain) {
            if out.is_empty() {
                out.push_str("Local capability cache:\n");
            }
            out.push_str("- ");
            out.push_str(label_for_domain(domain));
            out.push_str(": ");
            out.push_str(summary.as_str());
            out.push('\n');
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
