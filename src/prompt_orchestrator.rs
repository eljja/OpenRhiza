use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryQueryPhase {
    Driver,
    Software,
    Skill,
    Workflow,
    Policy,
    Evaluation,
}

pub type PromptRegistryStep = RegistryQueryPhase;

#[derive(Clone, Debug)]
pub struct PromptOrchestrationPlan {
    pub phases: Vec<RegistryQueryPhase>,
    pub next_index: usize,
    pub local_context_block: String,
    pub summary: String,
}

pub fn build_plan(prompt: &str) -> PromptOrchestrationPlan {
    let prompt_lower = prompt.to_ascii_lowercase();
    let driver_intent = prompt_has_any(
        prompt_lower.as_str(),
        &[
            "driver", "device", "hardware", "pci", "usb", "xhci", "keyboard", "mouse",
            "network", "nic", "storage", "disk", "filesystem", "ata", "e1000",
        ],
    );
    let software_intent = prompt_has_any(
        prompt_lower.as_str(),
        &[
            "program", "software", "tool", "app", "application", "package", "install",
            "download", "run",
        ],
    );
    let skill_intent = prompt_has_any(
        prompt_lower.as_str(),
        &[
            "skill", "search", "python", "test", "benchmark", "validate", "workflow",
            "policy", "plan",
        ],
    );
    let evaluation_intent = prompt_has_any(
        prompt_lower.as_str(),
        &[
            "evaluate", "evaluation", "score", "stable", "stability", "performance",
            "reliable", "quality", "vote", "comment", "improvement",
        ],
    ) || driver_intent || software_intent || skill_intent;

    let mut phases = vec![];
    if driver_intent {
        phases.push(RegistryQueryPhase::Driver);
    }
    if software_intent {
        phases.push(RegistryQueryPhase::Software);
    }
    phases.push(RegistryQueryPhase::Skill);
    if skill_intent || driver_intent || software_intent {
        phases.push(RegistryQueryPhase::Workflow);
    }
    phases.push(RegistryQueryPhase::Policy);
    if evaluation_intent {
        phases.push(RegistryQueryPhase::Evaluation);
    }

    let local_context_block = build_local_context_block(prompt);
    let summary = build_summary(driver_intent, software_intent, skill_intent, evaluation_intent);

    PromptOrchestrationPlan {
        phases,
        next_index: 0,
        local_context_block,
        summary,
    }
}

pub fn build_prompt_orchestration_plan(prompt: &str) -> PromptOrchestrationPlan {
    build_plan(prompt)
}

pub fn build_enriched_prompt(prompt: &str, plan: &PromptOrchestrationPlan) -> String {
    format!(
        "OpenRhiza prompt orchestration summary: {}\n\n{}\n\nUser request:\n{}",
        plan.summary,
        plan.local_context_block.trim(),
        prompt.trim()
    )
}

pub fn build_prompt_execution_context(prompt: &str) -> Option<String> {
    let plan = build_plan(prompt);
    if plan.local_context_block.trim().is_empty() {
        None
    } else {
        Some(plan.local_context_block)
    }
}

fn build_summary(
    driver_intent: bool,
    software_intent: bool,
    skill_intent: bool,
    evaluation_intent: bool,
) -> String {
    let mut labels = Vec::new();
    if driver_intent {
        labels.push("driver");
    }
    if software_intent {
        labels.push("software");
    }
    if skill_intent {
        labels.push("skill");
    }
    if evaluation_intent {
        labels.push("evaluation");
    }

    if labels.is_empty() {
        return String::from("generic prompt");
    }

    let mut out = String::from("intent:");
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(label);
    }
    out
}

fn build_local_context_block(prompt: &str) -> String {
    let mut out = String::from("Local machine context:\n");
    out.push_str("- prompt: ");
    out.push_str(prompt.trim());
    out.push('\n');

    if let Some(driver_map) = crate::driver_cache::load_active_driver_map_text() {
        out.push_str("Local preferred driver map:\n");
        for line in driver_map.lines().take(12) {
            out.push_str("- ");
            out.push_str(line);
            out.push('\n');
        }
    }

    if let Some(local_cache) = crate::capability_cache::current_local_registry_context_block() {
        out.push('\n');
        out.push_str(local_cache.trim());
        out.push('\n');
    }

    if let Some(skill_block) = crate::skill_runtime::local_skill_context_block() {
        out.push('\n');
        out.push_str(skill_block.trim());
        out.push('\n');
    }

    if let Some(driver_runtime) = crate::driver_runtime::context_block() {
        out.push('\n');
        out.push_str(driver_runtime.trim());
        out.push('\n');
    }

    let bindings = crate::runtime_bindings::snapshot();
    if !bindings.is_empty() {
        out.push_str("Live driver bindings:\n");
        for binding in bindings.iter().take(12) {
            out.push_str("- ");
            out.push_str(binding.match_key.as_str());
            out.push('=');
            out.push_str(binding.driver_id.as_str());
            out.push_str(" [");
            out.push_str(binding.source.as_str());
            out.push_str("]\n");
        }
    }

    let input_states = crate::input_runtime::snapshot();
    out.push_str("Input runtime state:\n");
    for state in input_states {
        out.push_str("- ");
        out.push_str(crate::input_runtime::kind_label(state.kind));
        out.push_str(" stage=");
        out.push_str(match state.component.stage {
            crate::input_runtime::InputDriverStage::Bootstrap => "bootstrap",
            crate::input_runtime::InputDriverStage::Cached => "cached",
            crate::input_runtime::InputDriverStage::Testing => "testing",
            crate::input_runtime::InputDriverStage::Active => "active",
        });
        out.push_str(" current=");
        out.push_str(state.component.current_artifact_id.as_deref().unwrap_or("none"));
        out.push_str(" persisted=");
        out.push_str(state.component.persisted_artifact_id.as_deref().unwrap_or("none"));
        out.push('\n');
    }

    out
}

fn prompt_has_any(prompt_lower: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| prompt_lower.contains(needle))
}
