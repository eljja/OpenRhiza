use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

const AUTONOMY_FILES: [[u8; 11]; 1] = [*b"AUTONOMYTXT"];
const DEFAULT_INTERVAL_MINUTES: u32 = 10;
const MIN_INTERVAL_MINUTES: u32 = 1;
const MAX_INTERVAL_MINUTES: u32 = 24 * 60;
const COUNCIL_CYCLE_TIMEOUT_TICKS: u64 = 120 * crate::task::timer::TICKS_PER_SECOND as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutonomyMode {
    Off,
    Assist,
    Council,
}

impl AutonomyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AutonomyMode::Off => "off",
            AutonomyMode::Assist => "assist",
            AutonomyMode::Council => "council",
        }
    }

    pub fn from_str(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "assist" => Some(Self::Assist),
            "council" => Some(Self::Council),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AutonomyConfig {
    pub configured: bool,
    pub mode: AutonomyMode,
    pub interval_minutes: u32,
}

impl Default for AutonomyConfig {
    fn default() -> Self {
        Self {
            configured: false,
            mode: AutonomyMode::Off,
            interval_minutes: DEFAULT_INTERVAL_MINUTES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AutonomyStance {
    Hold,
    Suggest,
    Prepare,
    AskApproval,
}

impl AutonomyStance {
    fn as_str(self) -> &'static str {
        match self {
            AutonomyStance::Hold => "hold",
            AutonomyStance::Suggest => "suggest",
            AutonomyStance::Prepare => "prepare",
            AutonomyStance::AskApproval => "ask-approval",
        }
    }

    fn from_str(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "hold" => Self::Hold,
            "prepare" => Self::Prepare,
            "ask-approval" => Self::AskApproval,
            _ => Self::Suggest,
        }
    }
}

#[derive(Clone, Debug)]
struct CouncilVote {
    role: String,
    stance: AutonomyStance,
    intent: String,
    goal: String,
    blocker: String,
    proposal: String,
    evidence: String,
    approval_needed: bool,
    confidence_percent: u8,
}

#[derive(Clone, Debug)]
struct CouncilCycle {
    cycle_id: u64,
    mode: AutonomyMode,
    started_tick: u64,
    expected_roles: Vec<String>,
    votes: Vec<CouncilVote>,
}

struct AutonomyState {
    config: AutonomyConfig,
    next_cycle_id: u64,
    last_cycle_started_tick: u64,
    active_cycle: Option<CouncilCycle>,
    first_boot_notice_shown: bool,
    force_cycle: bool,
    last_presented_digest: Option<String>,
}

impl AutonomyState {
    fn new() -> Self {
        Self {
            config: AutonomyConfig::default(),
            next_cycle_id: 1,
            last_cycle_started_tick: 0,
            active_cycle: None,
            first_boot_notice_shown: false,
            force_cycle: false,
            last_presented_digest: None,
        }
    }
}

lazy_static! {
    static ref AUTONOMY_STATE: Mutex<AutonomyState> = Mutex::new(AutonomyState::new());
}

fn interval_ticks(config: &AutonomyConfig) -> u64 {
    config.interval_minutes as u64
        * 60
        * crate::task::timer::TICKS_PER_SECOND as u64
}

fn normalize_interval(value: u32) -> u32 {
    value.clamp(MIN_INTERVAL_MINUTES, MAX_INTERVAL_MINUTES)
}

pub fn load_persisted_config() {
    let Some(text) = crate::storage::read_text_file_from_secondary_fat16(&AUTONOMY_FILES) else {
        return;
    };

    let mut config = AutonomyConfig::default();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        match key.trim() {
            "configured" => {
                config.configured = value.trim() == "1" || value.trim().eq_ignore_ascii_case("true");
            }
            "mode" => {
                if let Some(mode) = AutonomyMode::from_str(value.trim()) {
                    config.mode = mode;
                }
            }
            "interval_minutes" => {
                if let Ok(minutes) = value.trim().parse::<u32>() {
                    config.interval_minutes = normalize_interval(minutes);
                }
            }
            _ => {}
        }
    }

    AUTONOMY_STATE.lock().config = config;
}

fn persist_config(config: &AutonomyConfig) -> Result<(), &'static str> {
    let text = format!(
        "# OpenRhiza autonomy config\nconfigured={}\nmode={}\ninterval_minutes={}\n",
        if config.configured { 1 } else { 0 },
        config.mode.as_str(),
        config.interval_minutes
    );
    crate::storage::write_named_file_to_secondary_fat16_preserve_size(&AUTONOMY_FILES, text.as_bytes())
}

pub fn status_block() -> String {
    let state = AUTONOMY_STATE.lock();
    let mut out = format!(
        "Autonomy runtime:\n- configured: {}\n- mode: {}\n- interval_minutes: {}\n- interval_ticks: {}\n",
        if state.config.configured { "yes" } else { "no" },
        state.config.mode.as_str(),
        state.config.interval_minutes,
        interval_ticks(&state.config)
    );

    if let Some(cycle) = state.active_cycle.as_ref() {
        let ticks = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
        let age = ticks.saturating_sub(cycle.started_tick);
        out.push_str(
            format!(
                "- active_cycle: {} mode={} votes={}/{} started_tick={} age_ticks={} timeout_ticks={}\n",
                cycle.cycle_id,
                cycle.mode.as_str(),
                cycle.votes.len(),
                cycle.expected_roles.len(),
                cycle.started_tick,
                age,
                COUNCIL_CYCLE_TIMEOUT_TICKS
            )
            .as_str(),
        );
        for vote in &cycle.votes {
            out.push_str(
                format!(
                    "  - {} stance={} confidence={} proposal={}\n",
                    vote.role,
                    vote.stance.as_str(),
                    vote.confidence_percent,
                    vote.proposal
                )
                .as_str(),
            );
        }
    } else {
        out.push_str("- active_cycle: none\n");
    }

    out
}

pub fn set_mode(mode_text: &str) -> Result<String, &'static str> {
    let mode = AutonomyMode::from_str(mode_text).ok_or("usage: /autonomy-mode <off|assist|council>")?;
    let mut state = AUTONOMY_STATE.lock();
    state.config.mode = mode;
    state.config.configured = true;
    state.active_cycle = None;
    state.force_cycle = false;
    persist_config(&state.config)?;
    Ok(format!(
        "[Autonomy] mode set to {} (interval={} minute(s))",
        mode.as_str(),
        state.config.interval_minutes
    ))
}

pub fn set_interval_minutes(minutes: u32) -> Result<String, &'static str> {
    let normalized = normalize_interval(minutes);
    let mut state = AUTONOMY_STATE.lock();
    state.config.interval_minutes = normalized;
    state.config.configured = true;
    persist_config(&state.config)?;
    Ok(format!(
        "[Autonomy] interval set to {} minute(s). AI cannot change this on its own.",
        normalized
    ))
}

pub fn request_run_now() -> Result<String, &'static str> {
    let mut state = AUTONOMY_STATE.lock();
    if state.config.mode == AutonomyMode::Off {
        return Err("autonomy is off; set /autonomy-mode assist or /autonomy-mode council first");
    }
    state.force_cycle = true;
    Ok(String::from("[Autonomy] queued an immediate autonomy cycle."))
}

pub fn current_mode() -> AutonomyMode {
    AUTONOMY_STATE.lock().config.mode
}

fn should_show_first_boot_notice(ticks: u64) -> bool {
    let mut state = AUTONOMY_STATE.lock();
    if state.first_boot_notice_shown || state.config.configured || ticks < 300 {
        return false;
    }
    state.first_boot_notice_shown = true;
    true
}

fn role_labels_for_mode(mode: AutonomyMode) -> Vec<&'static str> {
    match mode {
        AutonomyMode::Off => Vec::new(),
        AutonomyMode::Assist => Vec::from(["practical"]),
        AutonomyMode::Council => Vec::from(["practical", "analytical", "bold"]),
    }
}

fn role_system_style(role: &str) -> &'static str {
    match role {
        "practical" => "You optimize for reliability, immediate usefulness, and minimal disruption.",
        "analytical" => "You optimize for correctness, evidence, constraints, and hidden failure detection.",
        "bold" => "You optimize for capability growth, ambitious improvement, and high-upside options that remain reversible.",
        _ => "You optimize for practical system assistance.",
    }
}

fn recent_context_block() -> Option<String> {
    let recent_chat = crate::display::recent_gui_chat_context(6, 900)?;
    let mut out = String::from("Recent GUI conversation:\n");
    out.push_str(recent_chat.as_str());
    out.push_str("\n\nCurrent runtime summary:\n");
    out.push_str(crate::display::status_block().trim());
    if let Some(registry) = crate::api_v1::current_registry_context_block() {
        out.push_str("\n\n");
        out.push_str(registry.trim());
    }
    if let Some(skill_context) = crate::skill_runtime::local_skill_context_block() {
        out.push_str("\n\n");
        out.push_str(skill_context.trim());
    }
    Some(out)
}

fn build_council_prompt(role: &str, context: &str) -> String {
    format!(
        "You are one member of the OpenRhiza autonomy council.\nRole: {}.\n{}\n\
         Decide how the OS should help the user next without acting unilaterally.\n\
         You may infer intent, goal, blockers, and safe bounded evidence-gathering steps.\n\
         Never emit machine-action JSON. Never change autonomy mode or interval. Do not output markdown.\n\
         Return exactly one compact JSON object with keys:\n\
         intent, goal, blocker, stance, proposal, evidence, approval_needed, confidence\n\
         where stance is one of hold, suggest, prepare, ask-approval and confidence is between 0 and 1.\n\
         Keep every field short and concrete.\n\nContext:\n{}",
        role,
        role_system_style(role),
        context
    )
}

fn begin_cycle_if_needed(ticks: u64) {
    let (mode, cycle_id, roles) = {
        let mut state = AUTONOMY_STATE.lock();
        if state.config.mode == AutonomyMode::Off {
            return;
        }
        if state.active_cycle.is_some() {
            return;
        }

        let interval_ready = ticks.saturating_sub(state.last_cycle_started_tick) >= interval_ticks(&state.config);
        if !state.force_cycle && !interval_ready {
            return;
        }

        let roles = role_labels_for_mode(state.config.mode);
        if roles.is_empty() {
            return;
        }

        let cycle_id = state.next_cycle_id;
        state.next_cycle_id = state.next_cycle_id.saturating_add(1);
        state.last_cycle_started_tick = ticks;
        state.force_cycle = false;
        (state.config.mode, cycle_id, roles)
    };

    let Some(context) = recent_context_block() else {
        AUTONOMY_STATE.lock().active_cycle = None;
        return;
    };

    let mut queued = 0usize;
    let mut queued_roles = Vec::new();
    for role in roles {
        let prompt = build_council_prompt(role, context.as_str());
        if crate::api_v1::queue_autonomy_gemini_prompt(prompt, cycle_id, role).is_ok() {
            queued += 1;
            queued_roles.push(String::from(role));
        } else {
            crate::result_println!("[Autonomy] failed to queue {} council request.", role);
        }
    }

    if queued == 0 {
        return;
    }

    AUTONOMY_STATE.lock().active_cycle = Some(CouncilCycle {
        cycle_id,
        mode,
        started_tick: ticks,
        expected_roles: queued_roles,
        votes: Vec::new(),
    });

    crate::result_println!(
        "[Autonomy] started {} cycle {} with {} agent(s).",
        mode.as_str(),
        cycle_id,
        queued
    );
}

fn expire_stale_cycle_if_needed(ticks: u64) -> Option<String> {
    let mut state = AUTONOMY_STATE.lock();
    let Some(cycle) = state.active_cycle.as_ref() else {
        return None;
    };
    if ticks.saturating_sub(cycle.started_tick) < COUNCIL_CYCLE_TIMEOUT_TICKS {
        return None;
    }

    let mut cycle = state.active_cycle.take().unwrap();
    for role in cycle.expected_roles.clone() {
        if cycle.votes.iter().any(|vote| vote.role == role) {
            continue;
        }
        cycle.votes.push(CouncilVote {
            role,
            stance: AutonomyStance::Hold,
            intent: String::from("autonomy timeout recovery"),
            goal: String::from("keep the OS responsive"),
            blocker: String::from("council member did not respond before timeout"),
            proposal: String::from("Clear the stale cycle and wait for the next user-controlled interval."),
            evidence: String::from("Autonomy council cycle exceeded its timeout budget."),
            approval_needed: false,
            confidence_percent: 75,
        });
    }

    let message = summarize_cycle(&cycle);
    let digest = crate::identity::sha256_hex(message.as_bytes());
    if state.last_presented_digest.as_deref() == Some(digest.as_str()) {
        None
    } else {
        state.last_presented_digest = Some(digest);
        Some(format!(
            "[Autonomy] stale cycle {} timed out after {} ticks.\n{}",
            cycle.cycle_id,
            ticks.saturating_sub(cycle.started_tick),
            message
        ))
    }
}

pub async fn autonomy_task() {
    crate::println!("[Task] autonomy_task started");
    loop {
        crate::task::timer::sleep_ticks(crate::task::timer::TICKS_PER_SECOND).await;
        let ticks = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
        if should_show_first_boot_notice(ticks) {
            crate::display::record_gui_system_message(
                "Autonomy is off by default. Use /autonomy-mode off|assist|council and /autonomy-interval <minutes> to configure it."
            );
        }
        if let Some(message) = expire_stale_cycle_if_needed(ticks) {
            crate::display::record_gui_system_message(message.as_str());
            for line in message.lines() {
                crate::result_println!("{}", line);
            }
        }
        begin_cycle_if_needed(ticks);
    }
}

pub fn handle_council_response(cycle_id: u64, role: &str, text: &str) {
    let vote = parse_vote(role, text);
    finish_vote(cycle_id, role, vote);
}

pub fn handle_council_failure(cycle_id: u64, role: &str, reason: &str) {
    let vote = CouncilVote {
        role: String::from(role),
        stance: AutonomyStance::Hold,
        intent: String::from("bounded failure handling"),
        goal: String::from("avoid acting without a complete council response"),
        blocker: String::from(reason),
        proposal: String::from("Hold for now and keep the current user-directed flow unchanged."),
        evidence: String::from("The council member did not return a usable response."),
        approval_needed: false,
        confidence_percent: 40,
    };
    finish_vote(cycle_id, role, vote);
}

fn finish_vote(cycle_id: u64, role: &str, vote: CouncilVote) {
    let outcome = {
        let mut state = AUTONOMY_STATE.lock();
        let Some(cycle) = state.active_cycle.as_mut() else {
            return;
        };
        if cycle.cycle_id != cycle_id {
            return;
        }
        if cycle.votes.iter().any(|existing| existing.role == role) {
            return;
        }
        cycle.votes.push(vote);
        if cycle.votes.len() < cycle.expected_roles.len() {
            return;
        }

        let cycle = state.active_cycle.take().unwrap();
        let message = summarize_cycle(&cycle);
        let digest = crate::identity::sha256_hex(message.as_bytes());
        if state.last_presented_digest.as_deref() == Some(digest.as_str()) {
            None
        } else {
            state.last_presented_digest = Some(digest);
            Some(message)
        }
    };

    if let Some(message) = outcome {
        crate::display::record_gui_system_message(message.as_str());
        for line in message.lines() {
            crate::result_println!("{}", line);
        }
    }
}

fn summarize_cycle(cycle: &CouncilCycle) -> String {
    if cycle.votes.is_empty() {
        return format!("[Autonomy] cycle {} produced no votes.", cycle.cycle_id);
    }

    let mut stance_counts = [0usize; 4];
    let mut approval_yes = 0usize;
    for vote in &cycle.votes {
        let index = match vote.stance {
            AutonomyStance::Hold => 0,
            AutonomyStance::Suggest => 1,
            AutonomyStance::Prepare => 2,
            AutonomyStance::AskApproval => 3,
        };
        stance_counts[index] += 1;
        if vote.approval_needed {
            approval_yes += 1;
        }
    }

    let winner_index = stance_counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| *count)
        .map(|(index, _)| index)
        .unwrap_or(1);
    let winner = match winner_index {
        0 => AutonomyStance::Hold,
        1 => AutonomyStance::Suggest,
        2 => AutonomyStance::Prepare,
        _ => AutonomyStance::AskApproval,
    };

    let winner_count = stance_counts[winner_index];
    let has_majority = winner_count * 2 > cycle.votes.len();

    let representative = cycle
        .votes
        .iter()
        .filter(|vote| vote.stance == winner)
        .max_by_key(|vote| vote.confidence_percent)
        .unwrap_or(&cycle.votes[0]);

    let approval_text = if approval_yes * 2 >= cycle.votes.len() {
        "yes"
    } else {
        "no"
    };

    if has_majority {
        format!(
            "[Autonomy Council] Inferred intent: {}\n[Autonomy Council] Inferred goal: {}\n[Autonomy Council] Majority stance: {} ({}/{})\n[Autonomy Council] Recommended next step: {}\n[Autonomy Council] Likely blocker: {}\n[Autonomy Council] Evidence prepared: {}\n[Autonomy Council] Approval needed: {}",
            representative.intent,
            representative.goal,
            winner.as_str(),
            winner_count,
            cycle.votes.len(),
            representative.proposal,
            representative.blocker,
            representative.evidence,
            approval_text
        )
    } else {
        let mut details = String::new();
        for vote in &cycle.votes {
            if !details.is_empty() {
                details.push_str(" | ");
            }
            details.push_str(vote.role.as_str());
            details.push(':');
            details.push_str(vote.stance.as_str());
            details.push(' ');
            details.push_str(vote.proposal.as_str());
        }
        format!(
            "[Autonomy Council] No majority this cycle.\n[Autonomy Council] Intent candidate: {}\n[Autonomy Council] Goal candidates: {}\n[Autonomy Council] Agent positions: {}\n[Autonomy Council] Approval needed: {}",
            representative.intent,
            representative.goal,
            details,
            approval_text
        )
    }
}

fn parse_vote(role: &str, text: &str) -> CouncilVote {
    let stance = AutonomyStance::from_str(
        extract_json_like_string(text, "stance")
            .unwrap_or_else(|| String::from("suggest"))
            .as_str(),
    );
    let confidence = extract_json_like_number_percent(text, "confidence").unwrap_or(60);
    let proposal = extract_json_like_string(text, "proposal")
        .unwrap_or_else(|| fallback_first_line(text));
    CouncilVote {
        role: String::from(role),
        stance,
        intent: extract_json_like_string(text, "intent").unwrap_or_else(|| String::from("helpful assistance")),
        goal: extract_json_like_string(text, "goal").unwrap_or_else(|| String::from("improve the user's current task")),
        blocker: extract_json_like_string(text, "blocker").unwrap_or_else(|| String::from("uncertain constraints")),
        evidence: extract_json_like_string(text, "evidence").unwrap_or_else(|| String::from("current GUI/runtime context inspected")),
        approval_needed: extract_json_like_bool(text, "approval_needed").unwrap_or(matches!(stance, AutonomyStance::AskApproval)),
        confidence_percent: confidence,
        proposal,
    }
}

fn fallback_first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or("suggest next helpful step").trim();
    if line.is_empty() {
        String::from("suggest next helpful step")
    } else {
        String::from(line)
    }
}

fn extract_json_like_string(body: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = body.find(pattern.as_str())? + pattern.len();
    let rest = &body[start..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let quote = rest.find('"')? + 1;
    let mut chars = rest[quote..].chars();
    let mut out = String::new();
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            match ch {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'u' => {
                    let mut value = 0u32;
                    let mut ok = true;
                    for _ in 0..4 {
                        let Some(hex) = chars.next() else {
                            ok = false;
                            break;
                        };
                        let Some(digit) = hex.to_digit(16) else {
                            ok = false;
                            break;
                        };
                        value = (value << 4) | digit;
                    }
                    if ok {
                        if let Some(decoded) = core::char::from_u32(value) {
                            out.push(decoded);
                        }
                    }
                }
                _ => out.push(ch),
            }
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return Some(out),
            _ => out.push(ch),
        }
    }
    None
}

fn extract_json_like_bool(body: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\"", key);
    let start = body.find(pattern.as_str())? + pattern.len();
    let rest = &body[start..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn extract_json_like_number_percent(body: &str, key: &str) -> Option<u8> {
    let pattern = format!("\"{}\"", key);
    let start = body.find(pattern.as_str())? + pattern.len();
    let rest = &body[start..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    let allowed = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
        .count();
    if allowed == 0 {
        return None;
    }
    let number = rest[..allowed].parse::<f32>().ok()?;
    let normalized = if number <= 1.0 { number * 100.0 } else { number };
    Some(normalized.clamp(0.0, 100.0) as u8)
}
