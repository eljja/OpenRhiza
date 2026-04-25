use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;

use crate::sandbox_lifecycle::SandboxStage;

const BOOT_AUTORUN_FILES: [[u8; 11]; 1] = [*b"BOOTAUTOMD "];
const AUTORUN_POLL_TICKS: u64 = 25;
const AUTORUN_TIMEOUT_TICKS: u64 = 30_000;

enum BootAutorunStep {
    Command(String),
    WaitTicks(u64),
    WaitSkillCached(String),
    WaitSkillStage { skill_id: String, stage: SandboxStage },
}

pub async fn boot_autorun_task() {
    crate::println!("[Task] boot_autorun_task started");
    crate::task::timer::sleep_ticks(250).await;

    let Some(text) = crate::storage::read_text_file_from_secondary_fat16(&BOOT_AUTORUN_FILES) else {
        return;
    };

    let steps = parse_boot_autorun_markdown(text.as_str());
    if steps.is_empty() {
        crate::result_println!("[Boot Autorun] No executable steps found in BOOTAUTO.MD.");
        return;
    }

    crate::result_println!("[Boot Autorun] Loaded {} boot steps from BOOTAUTO.MD.", steps.len());

    for step in steps {
        match step {
            BootAutorunStep::Command(command) => {
                crate::result_println!("[Boot Autorun] Running {}", command);
                crate::task::keyboard::execute_virtual_cli_command(command.as_str());
                crate::task::timer::sleep_ticks(50).await;
            }
            BootAutorunStep::WaitTicks(ticks) => {
                crate::result_println!("[Boot Autorun] Waiting {} ticks.", ticks);
                crate::task::timer::sleep_ticks(ticks).await;
            }
            BootAutorunStep::WaitSkillCached(skill_id) => {
                wait_for_skill_cached(skill_id.as_str()).await;
            }
            BootAutorunStep::WaitSkillStage { skill_id, stage } => {
                wait_for_skill_stage(skill_id.as_str(), stage).await;
            }
        }
    }

    crate::result_println!("[Boot Autorun] Completed boot script.");
}

fn parse_boot_autorun_markdown(text: &str) -> Vec<BootAutorunStep> {
    let mut steps = Vec::new();
    let mut in_code_fence = false;

    for raw_line in text.lines() {
        let trimmed = raw_line.trim();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }

        let Some(candidate) = normalize_markdown_step(trimmed, in_code_fence) else {
            continue;
        };

        if let Some(step) = parse_step(candidate.as_str()) {
            steps.push(step);
        }
    }

    steps
}

fn normalize_markdown_step(line: &str, in_code_fence: bool) -> Option<String> {
    if line.is_empty() {
        return None;
    }

    if in_code_fence {
        return Some(String::from(line));
    }

    if line.starts_with('#') || line.starts_with('>') {
        return None;
    }

    for prefix in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(String::from(rest.trim()));
        }
    }

    let mut digit_prefix_len = 0usize;
    for ch in line.chars() {
        if ch.is_ascii_digit() {
            digit_prefix_len += ch.len_utf8();
        } else {
            break;
        }
    }
    if digit_prefix_len > 0 {
        let remainder = &line[digit_prefix_len..];
        if let Some(rest) = remainder.strip_prefix(". ") {
            return Some(String::from(rest.trim()));
        }
    }

    if line.starts_with('/') || line.starts_with("@wait ") || line.starts_with("@sleep ") {
        return Some(String::from(line));
    }

    None
}

fn parse_step(line: &str) -> Option<BootAutorunStep> {
    if let Some(rest) = line.strip_prefix("@sleep ") {
        let ticks = parse_u64(rest.trim())?;
        return Some(BootAutorunStep::WaitTicks(ticks));
    }

    if let Some(rest) = line.strip_prefix("@wait ") {
        let mut parts = rest.split_whitespace();
        match parts.next()? {
            "skill-cached" => {
                let skill_id = parts.next()?.trim();
                if skill_id.is_empty() {
                    None
                } else {
                    Some(BootAutorunStep::WaitSkillCached(String::from(skill_id)))
                }
            }
            "skill-stage" => {
                let skill_id = parts.next()?.trim();
                let stage = parse_stage(parts.next()?)?;
                if skill_id.is_empty() {
                    None
                } else {
                    Some(BootAutorunStep::WaitSkillStage {
                        skill_id: String::from(skill_id),
                        stage,
                    })
                }
            }
            "ticks" => {
                let ticks = parse_u64(parts.next()?.trim())?;
                Some(BootAutorunStep::WaitTicks(ticks))
            }
            _ => None,
        }
    } else if line.starts_with('/') {
        Some(BootAutorunStep::Command(String::from(line)))
    } else {
        None
    }
}

fn parse_stage(text: &str) -> Option<SandboxStage> {
    match text {
        "bootstrap" => Some(SandboxStage::Bootstrap),
        "cached" => Some(SandboxStage::Cached),
        "testing" => Some(SandboxStage::Testing),
        "active" => Some(SandboxStage::Active),
        _ => None,
    }
}

fn parse_u64(text: &str) -> Option<u64> {
    let mut out = 0u64;
    if text.is_empty() {
        return None;
    }
    for byte in text.as_bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        out = out.saturating_mul(10).saturating_add((byte - b'0') as u64);
    }
    Some(out)
}

async fn wait_for_skill_cached(skill_id: &str) {
    crate::result_println!("[Boot Autorun] Waiting for cached skill {}.", skill_id);
    let deadline = crate::task::timer::TICKS
        .load(Ordering::Relaxed)
        .saturating_add(AUTORUN_TIMEOUT_TICKS);

    loop {
        if crate::skill_cache::find_cached_skill(skill_id).is_some() {
            crate::result_println!("[Boot Autorun] Cached skill {} is ready.", skill_id);
            return;
        }

        if crate::task::timer::TICKS.load(Ordering::Relaxed) >= deadline {
            crate::result_println!(
                "[Boot Autorun] Timeout waiting for cached skill {}.",
                skill_id
            );
            return;
        }

        crate::task::timer::sleep_ticks(AUTORUN_POLL_TICKS).await;
    }
}

async fn wait_for_skill_stage(skill_id: &str, stage: SandboxStage) {
    crate::result_println!(
        "[Boot Autorun] Waiting for {} to reach {:?}.",
        skill_id,
        stage
    );
    let deadline = crate::task::timer::TICKS
        .load(Ordering::Relaxed)
        .saturating_add(AUTORUN_TIMEOUT_TICKS);

    loop {
        let reached = crate::skill_runtime::runtime_states()
            .iter()
            .any(|state| state.skill_id == skill_id && state.stage == stage);
        if reached {
            crate::result_println!(
                "[Boot Autorun] {} reached {:?}.",
                skill_id,
                stage
            );
            return;
        }

        if crate::task::timer::TICKS.load(Ordering::Relaxed) >= deadline {
            crate::result_println!(
                "[Boot Autorun] Timeout waiting for {} to reach {:?}.",
                skill_id,
                stage
            );
            return;
        }

        crate::task::timer::sleep_ticks(AUTORUN_POLL_TICKS).await;
    }
}
