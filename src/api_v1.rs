use alloc::format;
use alloc::sync::Arc;
use alloc::string::String;
use alloc::vec::Vec;

use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;
use spin::Mutex;

use crate::identity::{HardwareDeviceSummary, NodeProfile, format_mac};

const PROTOCOL_VERSION: &str = "v1";
const OS_VERSION: &str = "0.1.0";
const OPENRHIZA_HOST: &str = "openrhiza.com";
const GEMINI_HOST: &str = "generativelanguage.googleapis.com";
const DEFAULT_GEMINI_MODELS: [&str; 4] = [
    "gemini-3-flash-preview",
    "gemini-3.1-flash-lite-preview",
    "gemini-2.5-flash",
    "gemini-2.5-flash-lite",
];
const OPENRHIZA_SYSTEM_INSTRUCTION_HEADER: &str =
    "You are OpenRhiza OS. Follow the baseline below. Be concise, safe, incremental, and technically explicit.";
const OPENRHIZA_OS_BASELINE_MD: &str = include_str!("../OS.md");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceApiCommand {
    NexusFetch,
    Register,
    RegisterHttp,
    HealthHttp,
    HealthHttps,
    RootHttps,
    HardwareReport,
    DriverQuery,
    SoftwareQuery,
    SkillQuery,
    WorkflowQuery,
    PolicyQuery,
    EvaluationQuery,
    All,
}

#[derive(Debug, Clone)]
pub enum DriverRegistryCommand {
    UploadGenerated { match_key: String },
    DownloadCandidate { driver_id: String, match_key: String },
    Comment { driver_id: String, comment: String },
    Vote { driver_id: String, vote: DriverVote },
    UploadEvaluation {
        driver_id: String,
        match_key: String,
        note: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverVote {
    Up,
    Down,
}

#[derive(Debug, Clone)]
pub struct DriverRegistryCandidate {
    pub driver_id: String,
    pub match_key: String,
}

lazy_static! {
    pub static ref SERVICE_API_QUEUE: Arc<ArrayQueue<ServiceApiCommand>> =
        Arc::new(ArrayQueue::new(8));
    pub static ref GEMINI_PROMPT_QUEUE: Arc<ArrayQueue<String>> = Arc::new(ArrayQueue::new(4));
    pub static ref DRIVER_REGISTRY_QUEUE: Arc<ArrayQueue<DriverRegistryCommand>> =
        Arc::new(ArrayQueue::new(8));
    static ref LAST_GEMINI_PROMPT: Mutex<Option<String>> = Mutex::new(None);
    static ref LAST_GEMINI_TEXT: Mutex<Option<String>> = Mutex::new(None);
    static ref LAST_GEMINI_MODEL: Mutex<Option<String>> = Mutex::new(None);
    static ref LAST_DRIVER_UPLOAD_MATCH_KEY: Mutex<Option<String>> = Mutex::new(None);
    static ref LAST_DRIVER_DOWNLOAD_TARGET: Mutex<Option<(String, String)>> = Mutex::new(None);
    static ref LAST_DRIVER_REGISTRY_SUMMARY: Mutex<Option<String>> = Mutex::new(None);
    static ref LAST_DRIVER_REGISTRY_CANDIDATES: Mutex<Vec<DriverRegistryCandidate>> =
        Mutex::new(Vec::new());
    static ref LAST_SOFTWARE_REGISTRY_SUMMARY: Mutex<Option<String>> = Mutex::new(None);
    static ref LAST_SKILL_REGISTRY_SUMMARY: Mutex<Option<String>> = Mutex::new(None);
    static ref LAST_WORKFLOW_REGISTRY_SUMMARY: Mutex<Option<String>> = Mutex::new(None);
    static ref LAST_POLICY_REGISTRY_SUMMARY: Mutex<Option<String>> = Mutex::new(None);
    static ref LAST_EVALUATION_REGISTRY_SUMMARY: Mutex<Option<String>> = Mutex::new(None);
}

pub fn build_node_register_request(profile: &NodeProfile) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"public_key\":\"{}\",\"identity_type\":\"software_key\",\"tpm_present\":{},\"os_version\":\"{}\",\"transport_capabilities\":[\"tls\",\"http_json\",\"signed_wasm\",\"driver_download\"]}}",
        PROTOCOL_VERSION,
        profile.node_id_hex(),
        profile.identity_key_hex(),
        json_bool(profile.machine_profile.tpm_present),
        OS_VERSION,
    )
}

pub fn build_hardware_report_request(profile: &NodeProfile) -> String {
    let mut body = String::new();
    body.push_str("{\"protocol_version\":\"");
    body.push_str(PROTOCOL_VERSION);
    body.push_str("\",\"node_id\":\"");
    body.push_str(&profile.node_id_hex());
    body.push_str("\",\"hardware_fingerprint\":\"sha256:");
    body.push_str(&profile.hardware_fingerprint_hex());
    body.push_str("\",\"machine_profile\":{");

    let cpu_vendor = json_escape(&profile.machine_profile.cpu.vendor_string());
    body.push_str("\"cpu\":{");
    body.push_str(&format!(
        "\"vendor\":\"{}\",\"family\":{},\"model\":{},\"stepping\":{},\"logical_cores\":{}",
        cpu_vendor,
        profile.machine_profile.cpu.family,
        profile.machine_profile.cpu.model,
        profile.machine_profile.cpu.stepping,
        profile.machine_profile.cpu.logical_cores
    ));
    body.push_str("},");

    body.push_str(&format!(
        "\"memory\":{{\"total_bytes\":{}}},",
        profile.machine_profile.total_memory_bytes
    ));

    body.push_str("\"network\":{\"mac_addresses\":[");
    for (index, mac) in profile.machine_profile.mac_addresses.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push('"');
        body.push_str(&format_mac(*mac));
        body.push('"');
    }
    body.push_str("]},");

    body.push_str(&format!(
        "\"tpm\":{{\"present\":{},\"attestation_ready\":false}}",
        json_bool(profile.machine_profile.tpm_present)
    ));

    body.push_str("},\"devices\":[");
    for (index, device) in profile.machine_profile.pci_devices.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push_str(&device_json(device, true));
    }
    body.push_str("]}");
    body
}

pub fn build_driver_query_request(profile: &NodeProfile) -> String {
    let mut body = String::new();
    body.push_str("{\"protocol_version\":\"");
    body.push_str(PROTOCOL_VERSION);
    body.push_str("\",\"node_id\":\"");
    body.push_str(&profile.node_id_hex());
    body.push_str("\",\"devices\":[");

    for (index, device) in profile.machine_profile.pci_devices.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push_str(&device_json(device, false));
    }

    body.push_str("]}");
    body
}

pub fn build_software_query_request(profile: &NodeProfile) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"ui_mode\":\"cli\",\"capabilities\":[\"registry\",\"gemini\",\"sandbox\",\"driver_runtime\",\"hot_swap\"]}}",
        PROTOCOL_VERSION,
        profile.node_id_hex()
    )
}

pub fn build_skill_query_request(profile: &NodeProfile) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"capabilities\":[\"registry\",\"gemini\",\"sandbox\",\"driver_runtime\",\"hot_swap\"],\"preferred_domains\":[\"driver\",\"skill\",\"workflow\",\"policy\"]}}",
        PROTOCOL_VERSION,
        profile.node_id_hex()
    )
}

pub fn build_workflow_query_request(profile: &NodeProfile) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"goal\":\"capability_registry_lookup\",\"available_skills\":[\"registry_lookup\",\"sandbox\",\"gemini\"]}}",
        PROTOCOL_VERSION,
        profile.node_id_hex()
    )
}

pub fn build_policy_query_request(profile: &NodeProfile) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"scope\":\"all\"}}",
        PROTOCOL_VERSION,
        profile.node_id_hex()
    )
}

pub fn build_evaluation_query_request(profile: &NodeProfile) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"subject_type\":\"all\"}}",
        PROTOCOL_VERSION,
        profile.node_id_hex()
    )
}

pub fn build_driver_evaluation_upload_request(
    profile: &NodeProfile,
    driver_id: &str,
    match_key: &str,
    note: &str,
) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"subject_type\":\"driver\",\"subject_id\":\"{}\",\"subject_label\":\"{}\",\"driver_id\":\"{}\",\"hardware_match_key\":\"{}\",\"stability_score\":70,\"performance_score\":60,\"notes\":[\"{}\"]}}",
        PROTOCOL_VERSION,
        profile.node_id_hex(),
        json_escape(driver_id),
        json_escape(driver_id),
        json_escape(driver_id),
        json_escape(match_key),
        json_escape(note)
    )
}

pub fn log_request_previews(profile: &NodeProfile) {
    let register = build_node_register_request(profile);
    let hardware = build_hardware_report_request(profile);
    let driver_query = build_driver_query_request(profile);
    let software_query = build_software_query_request(profile);
    let skill_query = build_skill_query_request(profile);
    let workflow_query = build_workflow_query_request(profile);
    let policy_query = build_policy_query_request(profile);
    let evaluation_query = build_evaluation_query_request(profile);

    crate::println!(
        "[API v1] Prepared register request: {} bytes",
        register.len()
    );
    crate::println!(
        "[API v1] Prepared hardware report: {} bytes",
        hardware.len()
    );
    crate::println!(
        "[API v1] Prepared driver query: {} bytes",
        driver_query.len()
    );
    crate::println!(
        "[API v1] Prepared software query: {} bytes",
        software_query.len()
    );
    crate::println!(
        "[API v1] Prepared skill query: {} bytes",
        skill_query.len()
    );
    crate::println!(
        "[API v1] Prepared workflow query: {} bytes",
        workflow_query.len()
    );
    crate::println!(
        "[API v1] Prepared policy query: {} bytes",
        policy_query.len()
    );
    crate::println!(
        "[API v1] Prepared evaluation query: {} bytes",
        evaluation_query.len()
    );

    if let Some(device) = profile.machine_profile.pci_devices.first() {
        crate::println!(
            "[API v1] First device match key: {}",
            crate::identity::stable_device_match_key(device)
        );
    }
}

pub fn queue_service_api_command(command: ServiceApiCommand) -> Result<(), ServiceApiCommand> {
    SERVICE_API_QUEUE.push(command)
}

pub fn queue_gemini_prompt(prompt: String) -> Result<(), String> {
    GEMINI_PROMPT_QUEUE.push(prompt)
}

pub fn queue_driver_registry_command(
    command: DriverRegistryCommand,
) -> Result<(), DriverRegistryCommand> {
    DRIVER_REGISTRY_QUEUE.push(command)
}

pub fn record_last_gemini_prompt(prompt: &str) {
    *LAST_GEMINI_PROMPT.lock() = Some(String::from(prompt));
}

pub fn record_last_gemini_response(model: &str, text: &str) {
    *LAST_GEMINI_MODEL.lock() = Some(String::from(model));
    *LAST_GEMINI_TEXT.lock() = Some(String::from(text));
}

pub fn last_gemini_text() -> Option<String> {
    LAST_GEMINI_TEXT.lock().clone()
}

pub fn last_gemini_prompt() -> Option<String> {
    LAST_GEMINI_PROMPT.lock().clone()
}

pub fn last_gemini_model() -> Option<String> {
    LAST_GEMINI_MODEL.lock().clone()
}

pub fn record_pending_driver_upload(match_key: &str) {
    *LAST_DRIVER_UPLOAD_MATCH_KEY.lock() = Some(String::from(match_key));
}

pub fn take_pending_driver_upload_match_key() -> Option<String> {
    LAST_DRIVER_UPLOAD_MATCH_KEY.lock().take()
}

pub fn record_pending_driver_download(driver_id: &str, match_key: &str) {
    *LAST_DRIVER_DOWNLOAD_TARGET.lock() = Some((String::from(driver_id), String::from(match_key)));
}

pub fn take_pending_driver_download() -> Option<(String, String)> {
    LAST_DRIVER_DOWNLOAD_TARGET.lock().take()
}

pub fn clear_registry_context() {
    *LAST_DRIVER_REGISTRY_SUMMARY.lock() = None;
    LAST_DRIVER_REGISTRY_CANDIDATES.lock().clear();
    *LAST_SOFTWARE_REGISTRY_SUMMARY.lock() = None;
    *LAST_SKILL_REGISTRY_SUMMARY.lock() = None;
    *LAST_WORKFLOW_REGISTRY_SUMMARY.lock() = None;
    *LAST_POLICY_REGISTRY_SUMMARY.lock() = None;
    *LAST_EVALUATION_REGISTRY_SUMMARY.lock() = None;
}

pub fn record_driver_registry_summary(summary: &str) {
    *LAST_DRIVER_REGISTRY_SUMMARY.lock() = Some(String::from(summary));
}

pub fn record_driver_registry_candidates(candidates: &[DriverRegistryCandidate]) {
    *LAST_DRIVER_REGISTRY_CANDIDATES.lock() = candidates.to_vec();
}

pub fn find_driver_registry_match_key(driver_id: &str) -> Option<String> {
    LAST_DRIVER_REGISTRY_CANDIDATES
        .lock()
        .iter()
        .find(|candidate| candidate.driver_id == driver_id)
        .map(|candidate| candidate.match_key.clone())
}

pub fn record_software_registry_summary(summary: &str) {
    *LAST_SOFTWARE_REGISTRY_SUMMARY.lock() = Some(String::from(summary));
    let _ = crate::capability_cache::persist_registry_summary(
        crate::capability_cache::RegistryDomain::Software,
        summary,
    );
}

pub fn record_skill_registry_summary(summary: &str) {
    *LAST_SKILL_REGISTRY_SUMMARY.lock() = Some(String::from(summary));
    let _ = crate::capability_cache::persist_registry_summary(
        crate::capability_cache::RegistryDomain::Skill,
        summary,
    );
}

pub fn record_workflow_registry_summary(summary: &str) {
    *LAST_WORKFLOW_REGISTRY_SUMMARY.lock() = Some(String::from(summary));
    let _ = crate::capability_cache::persist_registry_summary(
        crate::capability_cache::RegistryDomain::Workflow,
        summary,
    );
}

pub fn record_policy_registry_summary(summary: &str) {
    *LAST_POLICY_REGISTRY_SUMMARY.lock() = Some(String::from(summary));
    let _ = crate::capability_cache::persist_registry_summary(
        crate::capability_cache::RegistryDomain::Policy,
        summary,
    );
}

pub fn record_evaluation_registry_summary(summary: &str) {
    *LAST_EVALUATION_REGISTRY_SUMMARY.lock() = Some(String::from(summary));
    let _ = crate::capability_cache::persist_registry_summary(
        crate::capability_cache::RegistryDomain::Evaluation,
        summary,
    );
}

pub fn current_registry_context_block() -> Option<String> {
    let driver = LAST_DRIVER_REGISTRY_SUMMARY.lock().clone();
    let software = LAST_SOFTWARE_REGISTRY_SUMMARY.lock().clone();
    let skill = LAST_SKILL_REGISTRY_SUMMARY.lock().clone();
    let workflow = LAST_WORKFLOW_REGISTRY_SUMMARY.lock().clone();
    let policy = LAST_POLICY_REGISTRY_SUMMARY.lock().clone();
    let evaluation = LAST_EVALUATION_REGISTRY_SUMMARY.lock().clone();
    let software = software.or_else(|| {
        crate::capability_cache::load_registry_summary(
            crate::capability_cache::RegistryDomain::Software,
        )
    });
    let skill = skill.or_else(|| {
        crate::capability_cache::load_registry_summary(crate::capability_cache::RegistryDomain::Skill)
    });
    let workflow = workflow.or_else(|| {
        crate::capability_cache::load_registry_summary(
            crate::capability_cache::RegistryDomain::Workflow,
        )
    });
    let policy = policy.or_else(|| {
        crate::capability_cache::load_registry_summary(
            crate::capability_cache::RegistryDomain::Policy,
        )
    });
    let evaluation = evaluation.or_else(|| {
        crate::capability_cache::load_registry_summary(
            crate::capability_cache::RegistryDomain::Evaluation,
        )
    });

    if driver.is_none()
        && software.is_none()
        && skill.is_none()
        && workflow.is_none()
        && policy.is_none()
        && evaluation.is_none()
    {
        return None;
    }

    let mut out = String::from("Recent capability registry context:\n");
    if let Some(driver) = driver {
        out.push_str("- drivers: ");
        out.push_str(driver.as_str());
        out.push('\n');
    }
    if let Some(software) = software {
        out.push_str("- software: ");
        out.push_str(software.as_str());
        out.push('\n');
    }
    if let Some(skill) = skill {
        out.push_str("- skills: ");
        out.push_str(skill.as_str());
        out.push('\n');
    }
    if let Some(workflow) = workflow {
        out.push_str("- workflows: ");
        out.push_str(workflow.as_str());
        out.push('\n');
    }
    if let Some(policy) = policy {
        out.push_str("- policies: ");
        out.push_str(policy.as_str());
        out.push('\n');
    }
    if let Some(evaluation) = evaluation {
        out.push_str("- evaluations: ");
        out.push_str(evaluation.as_str());
        out.push('\n');
    }

    Some(out)
}

pub fn openrhiza_host() -> &'static str {
    OPENRHIZA_HOST
}

pub fn gemini_host() -> &'static str {
    GEMINI_HOST
}

pub fn gemini_models() -> &'static [&'static str] {
    &DEFAULT_GEMINI_MODELS
}

pub fn gemini_api_key() -> Option<&'static str> {
    option_env!("OPENRHIZA_GEMINI_API_KEY")
}

pub fn build_gemini_generate_path(model: &str) -> String {
    format!("/v1beta/models/{}:generateContent", model)
}

pub fn build_gemini_generate_request(prompt: &str) -> String {
    build_gemini_generate_request_with_context(prompt, None)
}

pub fn build_gemini_generate_request_with_context(
    prompt: &str,
    extra_context: Option<&str>,
) -> String {
    let mut system_instruction = String::from(OPENRHIZA_SYSTEM_INSTRUCTION_HEADER);
    system_instruction.push_str("\n\n");
    system_instruction.push_str(OPENRHIZA_OS_BASELINE_MD.trim());
    if let Some(context) = current_registry_context_block() {
        system_instruction.push_str("\n\n");
        system_instruction.push_str(context.trim());
    }
    if let Some(extra_context) = extra_context {
        let trimmed = extra_context.trim();
        if !trimmed.is_empty() {
            system_instruction.push_str("\n\n");
            system_instruction.push_str(trimmed);
        }
    }

    format!(
        "{{\"system_instruction\":{{\"parts\":[{{\"text\":\"{}\"}}]}},\"contents\":[{{\"role\":\"user\",\"parts\":[{{\"text\":\"{}\"}}]}}]}}",
        json_escape(&system_instruction),
        json_escape(prompt)
    )
}

pub fn build_driver_generation_prompt(match_key: &str) -> Option<String> {
    let profile = crate::identity::current_profile()?;
    let hardware = driver_hardware_label(&profile, match_key);
    Some(format!(
        "Generate a concise Rust no_std driver candidate for OpenRhiza.\nTarget match key: {}\nHardware: {}\nConstraints: text-first OS, kernel-adjacent environment, no_std, incremental validation.\nReturn: 1) driver summary, 2) init path, 3) MMIO/PIO or protocol assumptions, 4) interrupt/polling model, 5) sandbox smoke tests, 6) minimal candidate code or pseudocode that OpenRhiza can refine.",
        match_key,
        hardware
    ))
}

pub fn build_driver_upload_request(
    profile: &NodeProfile,
    match_key: &str,
    payload_text: &str,
) -> String {
    let prompt = last_gemini_prompt().unwrap_or_default();
    let model = last_gemini_model().unwrap_or_else(|| String::from(gemini_models()[0]));
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"match_key\":\"{}\",\"display_name\":\"{}\",\"hardware\":\"{}\",\"source_type\":\"gemini_generated\",\"model\":\"{}\",\"prompt_hash\":\"sha256:{}\",\"payload_text\":\"{}\"}}",
        PROTOCOL_VERSION,
        profile.node_id_hex(),
        json_escape(match_key),
        json_escape(&display_name_from_match_key(match_key)),
        json_escape(&driver_hardware_label(profile, match_key)),
        json_escape(&model),
        crate::identity::sha256_hex(prompt.as_bytes()),
        json_escape(payload_text)
    )
}

pub fn build_driver_download_request(
    profile: &NodeProfile,
    driver_id: &str,
    match_key: &str,
) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"driver_id\":\"{}\",\"match_key\":\"{}\"}}",
        PROTOCOL_VERSION,
        profile.node_id_hex(),
        json_escape(driver_id),
        json_escape(match_key)
    )
}

pub fn build_driver_comment_request(
    profile: &NodeProfile,
    driver_id: &str,
    comment: &str,
) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"driver_id\":\"{}\",\"comment\":\"{}\"}}",
        PROTOCOL_VERSION,
        profile.node_id_hex(),
        json_escape(driver_id),
        json_escape(comment)
    )
}

pub fn build_driver_vote_request(
    profile: &NodeProfile,
    driver_id: &str,
    vote: DriverVote,
) -> String {
    let vote = match vote {
        DriverVote::Up => "up",
        DriverVote::Down => "down",
    };
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"driver_id\":\"{}\",\"vote\":\"{}\"}}",
        PROTOCOL_VERSION,
        profile.node_id_hex(),
        json_escape(driver_id),
        vote
    )
}

fn device_json(device: &HardwareDeviceSummary, include_topology: bool) -> String {
    if include_topology {
        format!(
            "{{\"bus_type\":\"{}\",\"vendor_id\":\"{:04x}\",\"device_id\":\"{:04x}\",\"class_code\":\"{:02x}\",\"subclass\":\"{:02x}\",\"prog_if\":\"{:02x}\",\"bus\":{},\"slot\":{}}}",
            device.bus_type,
            device.vendor_id,
            device.device_id,
            device.class_code,
            device.subclass,
            device.prog_if,
            device.bus,
            device.slot
        )
    } else {
        format!(
            "{{\"bus_type\":\"{}\",\"vendor_id\":\"{:04x}\",\"device_id\":\"{:04x}\",\"class_code\":\"{:02x}\",\"subclass\":\"{:02x}\",\"prog_if\":\"{:02x}\"}}",
            device.bus_type,
            device.vendor_id,
            device.device_id,
            device.class_code,
            device.subclass,
            device.prog_if
        )
    }
}

fn display_name_from_match_key(match_key: &str) -> String {
    format!("Generated Driver {}", match_key)
}

fn driver_hardware_label(profile: &NodeProfile, match_key: &str) -> String {
    if let Some(device) = profile
        .machine_profile
        .pci_devices
        .iter()
        .find(|device| crate::identity::stable_device_match_key(device) == match_key)
    {
        return format!(
            "PCI {:04x}:{:04x} class {:02x}:{:02x}",
            device.vendor_id, device.device_id, device.class_code, device.subclass
        );
    }
    String::from(match_key)
}

fn json_escape(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn json_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}


