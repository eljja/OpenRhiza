use alloc::format;
use alloc::sync::Arc;
use alloc::string::String;

use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;

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
const OPENRHIZA_SYSTEM_INSTRUCTION: &str =
    "You are OpenRhiza OS. Be concise, safe, incremental, and technically explicit.";

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
    All,
}

lazy_static! {
    pub static ref SERVICE_API_QUEUE: Arc<ArrayQueue<ServiceApiCommand>> =
        Arc::new(ArrayQueue::new(8));
    pub static ref GEMINI_PROMPT_QUEUE: Arc<ArrayQueue<String>> = Arc::new(ArrayQueue::new(4));
}

pub fn build_node_register_request(profile: &NodeProfile) -> String {
    format!(
        "{{\"protocol_version\":\"{}\",\"node_id\":\"{}\",\"public_key\":\"{}\",\"identity_type\":\"software_key\",\"tpm_present\":{},\"os_version\":\"{}\",\"transport_capabilities\":[\"tls\",\"http_json\",\"signed_wasm\"]}}",
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

pub fn log_request_previews(profile: &NodeProfile) {
    let register = build_node_register_request(profile);
    let hardware = build_hardware_report_request(profile);
    let driver_query = build_driver_query_request(profile);

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
    format!(
        "{{\"system_instruction\":{{\"parts\":[{{\"text\":\"{}\"}}]}},\"contents\":[{{\"role\":\"user\",\"parts\":[{{\"text\":\"{}\"}}]}}]}}",
        json_escape(OPENRHIZA_SYSTEM_INSTRUCTION),
        json_escape(prompt)
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
