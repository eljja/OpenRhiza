use alloc::format;
use alloc::string::String;
use lazy_static::lazy_static;
use spin::Mutex;

const VOICE_CONFIG_FILES: [[u8; 11]; 1] = [*b"VOICECFGTXT"];
const VOICE_INPUT_FILES: [[u8; 11]; 1] = [*b"VOICEIN TXT"];
const DEFAULT_MODEL: &str = "gemini-3.1-pro-preview";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceMode {
    Off,
    PushToTalk,
    AlwaysListen,
}

impl VoiceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            VoiceMode::Off => "off",
            VoiceMode::PushToTalk => "push-to-talk",
            VoiceMode::AlwaysListen => "always-listen",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim() {
            "off" => Some(VoiceMode::Off),
            "on" | "push" | "push-to-talk" => Some(VoiceMode::PushToTalk),
            "always" | "always-listen" => Some(VoiceMode::AlwaysListen),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceRoute {
    TextFirst,
    DirectAudio,
    Hybrid,
}

impl VoiceRoute {
    pub fn as_str(self) -> &'static str {
        match self {
            VoiceRoute::TextFirst => "text-first",
            VoiceRoute::DirectAudio => "direct-audio",
            VoiceRoute::Hybrid => "hybrid",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim() {
            "text" | "text-first" | "transcript" => Some(VoiceRoute::TextFirst),
            "direct" | "audio" | "direct-audio" => Some(VoiceRoute::DirectAudio),
            "hybrid" | "auto" => Some(VoiceRoute::Hybrid),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct VoiceConfig {
    configured: bool,
    mode: VoiceMode,
    route: VoiceRoute,
    model: String,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            configured: false,
            mode: VoiceMode::Off,
            route: VoiceRoute::TextFirst,
            model: String::from(DEFAULT_MODEL),
        }
    }
}

#[derive(Clone, Debug)]
struct VoiceState {
    config: VoiceConfig,
    last_status: String,
    buffered_transcript: String,
}

impl VoiceState {
    fn new() -> Self {
        Self {
            config: VoiceConfig::default(),
            last_status: String::from("voice input is disabled"),
            buffered_transcript: String::new(),
        }
    }
}

lazy_static! {
    static ref VOICE_STATE: Mutex<VoiceState> = Mutex::new(VoiceState::new());
}

pub fn load_persisted_config() {
    let Some(text) = crate::storage::read_text_file_from_secondary_fat16(&VOICE_CONFIG_FILES) else {
        return;
    };

    let mut config = VoiceConfig::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "configured" => config.configured = value.trim() == "1" || value.trim() == "true",
            "mode" => {
                if let Some(mode) = VoiceMode::from_str(value) {
                    config.mode = mode;
                }
            }
            "route" => {
                if let Some(route) = VoiceRoute::from_str(value) {
                    config.route = route;
                }
            }
            "model" => {
                let model = value.trim();
                if !model.is_empty() && model.len() <= 96 {
                    config.model = String::from(model);
                }
            }
            _ => {}
        }
    }

    let mut state = VOICE_STATE.lock();
    state.last_status = format!("loaded voice config: mode={}", config.mode.as_str());
    state.config = config;
}

fn persist_config(config: &VoiceConfig) -> Result<(), &'static str> {
    let text = format!(
        "# OpenRhiza voice input config\nconfigured={}\nmode={}\nroute={}\nmodel={}\n",
        if config.configured { 1 } else { 0 },
        config.mode.as_str(),
        config.route.as_str(),
        config.model
    );
    crate::storage::write_named_file_to_secondary_fat16_preserve_size(
        &VOICE_CONFIG_FILES,
        text.as_bytes(),
    )
}

pub fn status_block() -> String {
    let state = VOICE_STATE.lock();
    format!(
        "Voice input:\n- configured: {}\n- mode: {}\n- route: {}\n- model: {}\n- capture: sandbox skill required\n- buffered transcript bytes: {}\n- last status: {}",
        state.config.configured,
        state.config.mode.as_str(),
        state.config.route.as_str(),
        state.config.model,
        state.buffered_transcript.len(),
        state.last_status
    )
}

pub fn set_mode(mode_text: &str) -> Result<String, &'static str> {
    let mode = VoiceMode::from_str(mode_text).ok_or("expected off, on, push-to-talk, or always-listen")?;
    let mut state = VOICE_STATE.lock();
    state.config.configured = true;
    state.config.mode = mode;
    state.last_status = match mode {
        VoiceMode::Off => String::from("voice capture disabled"),
        VoiceMode::PushToTalk => String::from("voice capture armed for explicit push-to-talk flows"),
        VoiceMode::AlwaysListen => {
            String::from("always-listen requested; future capture still requires visible recording state")
        }
    };
    let config = state.config.clone();
    let status = state.last_status.clone();
    drop(state);
    match persist_config(&config) {
        Ok(()) => Ok(format!("[Voice] mode set to {}", mode.as_str())),
        Err(error) => {
            let mut state = VOICE_STATE.lock();
            state.last_status = format!("{}; persistence failed: {}", status, error);
            Ok(format!(
                "[Voice] mode set to {} for this session; persistence failed: {}",
                mode.as_str(),
                error
            ))
        }
    }
}

pub fn set_route(route_text: &str) -> Result<String, &'static str> {
    let route = VoiceRoute::from_str(route_text).ok_or("expected text-first, direct-audio, or hybrid")?;
    let mut state = VOICE_STATE.lock();
    state.config.configured = true;
    state.config.route = route;
    state.last_status = match route {
        VoiceRoute::TextFirst => {
            String::from("voice route uses transcript first; audio is not uploaded unless needed")
        }
        VoiceRoute::DirectAudio => {
            String::from("voice route prefers bounded compressed audio to a multimodal LLM")
        }
        VoiceRoute::Hybrid => {
            String::from("voice route uses transcript by default and direct audio only when confidence or intent requires it")
        }
    };
    let config = state.config.clone();
    let status = state.last_status.clone();
    drop(state);
    match persist_config(&config) {
        Ok(()) => Ok(format!("[Voice] route set to {}", route.as_str())),
        Err(error) => {
            let mut state = VOICE_STATE.lock();
            state.last_status = format!("{}; persistence failed: {}", status, error);
            Ok(format!(
                "[Voice] route set to {} for this session; persistence failed: {}",
                route.as_str(),
                error
            ))
        }
    }
}

pub fn set_model(model: &str) -> Result<String, &'static str> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return Err("model name is empty");
    }
    if trimmed.len() > 96 {
        return Err("model name is too long");
    }

    let mut state = VOICE_STATE.lock();
    state.config.configured = true;
    state.config.model = String::from(trimmed);
    state.last_status = format!("voice transcription model set to {}", trimmed);
    let config = state.config.clone();
    let status = state.last_status.clone();
    drop(state);
    match persist_config(&config) {
        Ok(()) => Ok(format!("[Voice] model set to {}", trimmed)),
        Err(error) => {
            let mut state = VOICE_STATE.lock();
            state.last_status = format!("{}; persistence failed: {}", status, error);
            Ok(format!(
                "[Voice] model set to {} for this session; persistence failed: {}",
                trimmed,
                error
            ))
        }
    }
}

pub fn clear_buffer() -> String {
    let mut state = VOICE_STATE.lock();
    state.buffered_transcript.clear();
    state.last_status = String::from("voice transcript buffer cleared");
    String::from("[Voice] transcript buffer cleared")
}

pub fn import_transcript_to_composer() -> Result<String, &'static str> {
    let Some(mut transcript) = crate::storage::read_text_file_from_secondary_fat16(&VOICE_INPUT_FILES) else {
        return Err("VOICEIN.TXT not found on the driver disk");
    };

    transcript = transcript.trim_matches(|ch| ch == '\r' || ch == '\n' || ch == '\0').into();
    if transcript.trim().is_empty() {
        return Err("VOICEIN.TXT has no transcript text");
    }
    if transcript.len() > 4096 {
        transcript.truncate(4096);
    }

    crate::vga::commit_input_text(transcript.as_str());
    let mut state = VOICE_STATE.lock();
    state.buffered_transcript = transcript;
    state.last_status = String::from("imported transcript into composer for confirmation");
    Ok(String::from(
        "[Voice] imported transcript into composer. Review it, then press Enter to submit.",
    ))
}

pub fn queue_capture_bridge_test() -> Result<String, &'static str> {
    {
        let mut state = VOICE_STATE.lock();
        if state.config.mode == VoiceMode::Off {
            state.last_status = String::from("voice-test refused because voice mode is off");
            return Err("voice is off; use /voice on first");
        }
        state.last_status = String::from("queued voice capture bridge skill load");
    }

    match crate::skill_runtime::queue_load("skill_voice_capture_bridge_v1") {
        Ok(fat_name) => Ok(format!(
            "[Voice] queued skill_voice_capture_bridge_v1 from {}. Audio frames are not captured yet; this validates the sandbox voice path.",
            fat_name
        )),
        Err(error) => Err(error),
    }
}
