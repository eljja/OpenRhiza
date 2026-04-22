use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::ServiceApiPhase;

#[derive(Clone, Debug)]
pub struct PromptOrchestrationPlan {
    pub phases: Vec<ServiceApiPhase>,
    pub next_index: usize,
}

pub fn build_prompt_orchestration_plan(prompt: &str) -> PromptOrchestrationPlan {
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
        phases.push(ServiceApiPhase::DriverQuery);
    }
    if software_intent {
        phases.push(ServiceApiPhase::SoftwareQuery);
    }
    phases.push(ServiceApiPhase::SkillQuery);
    if skill_intent || driver_intent || software_intent {
        phases.push(ServiceApiPhase::WorkflowQuery);
    }
    phases.push(ServiceApiPhase::PolicyQuery);
    if evaluation_intent {
        phases.push(ServiceApiPhase::EvaluationQuery);
    }

    PromptOrchestrationPlan {
        phases,
        next_index: 0,
    }
}

pub fn is_prompt_orchestration_phase(phase: ServiceApiPhase) -> bool {
    matches!(
        phase,
        ServiceApiPhase::DriverQuery
            | ServiceApiPhase::SoftwareQuery
            | ServiceApiPhase::SkillQuery
            | ServiceApiPhase::WorkflowQuery
            | ServiceApiPhase::PolicyQuery
            | ServiceApiPhase::EvaluationQuery
    )
}

pub fn build_prompt_execution_context(prompt: &str) -> Option<String> {
    let mut out = String::new();

    let plan = build_prompt_orchestration_plan(prompt);
    if !plan.phases.is_empty() {
        out.push_str("Prompt orchestration plan:\n");
        for phase in &plan.phases {
            out.push_str("- ");
            out.push_str(service_phase_label(*phase));
            out.push('\n');
        }
    }

    if let Some(local_cache) = crate::capability_cache::current_local_registry_context_block() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(local_cache.trim());
        out.push('\n');
    }

    if let Some(skill_context) = crate::skill_runtime::current_skill_context_block() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(skill_context.trim());
        out.push('\n');
    }

    let bindings = crate::runtime_bindings::snapshot();
    if !bindings.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("Live driver bindings:\n");
        for binding in bindings.iter().take(6) {
            out.push_str("- ");
            out.push_str(binding.match_key.as_str());
            out.push_str(" -> ");
            out.push_str(binding.driver_id.as_str());
            out.push('\n');
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn prompt_has_any(prompt_lower: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| prompt_lower.contains(needle))
}

fn service_phase_label(phase: ServiceApiPhase) -> &'static str {
    match phase {
        ServiceApiPhase::DriverQuery => "driver_query",
        ServiceApiPhase::SoftwareQuery => "software_query",
        ServiceApiPhase::SkillQuery => "skill_query",
        ServiceApiPhase::WorkflowQuery => "workflow_query",
        ServiceApiPhase::PolicyQuery => "policy_query",
        ServiceApiPhase::EvaluationQuery => "evaluation_query",
        _ => "other",
    }
}
