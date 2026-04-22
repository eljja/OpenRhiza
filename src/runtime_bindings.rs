use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lazy_static::lazy_static;
use spin::Mutex;

#[derive(Clone, Debug)]
pub struct RuntimeDriverBinding {
    pub match_key: String,
    pub driver_id: String,
    pub source: String,
    pub previous_driver_id: Option<String>,
    pub rollback_to_none_allowed: bool,
}

#[derive(Clone, Debug)]
pub struct ActivationOutcome {
    pub changed: bool,
    pub previous_driver_id: Option<String>,
}

lazy_static! {
    static ref ACTIVE_RUNTIME_BINDINGS: Mutex<Vec<RuntimeDriverBinding>> = Mutex::new(Vec::new());
}

pub fn install_local_bindings(bindings: &[crate::driver_cache::ActiveDriverBinding]) -> usize {
    let mut active = ACTIVE_RUNTIME_BINDINGS.lock();
    active.clear();
    for binding in bindings {
        active.push(RuntimeDriverBinding {
            match_key: binding.match_key.clone(),
            driver_id: binding.driver_id.clone(),
            source: String::from("local-cache"),
            previous_driver_id: None,
            rollback_to_none_allowed: false,
        });
    }
    active.len()
}

pub fn activate_binding(match_key: &str, driver_id: &str, source: &str) -> ActivationOutcome {
    let mut active = ACTIVE_RUNTIME_BINDINGS.lock();

    if let Some(binding) = active.iter_mut().find(|binding| binding.match_key == match_key) {
        if binding.driver_id == driver_id {
            binding.source = String::from(source);
            return ActivationOutcome {
                changed: false,
                previous_driver_id: binding.previous_driver_id.clone(),
            };
        }

        let previous = binding.driver_id.clone();
        binding.previous_driver_id = Some(previous.clone());
        binding.driver_id = String::from(driver_id);
        binding.source = String::from(source);
        binding.rollback_to_none_allowed = false;
        return ActivationOutcome {
            changed: true,
            previous_driver_id: Some(previous),
        };
    }

    active.push(RuntimeDriverBinding {
        match_key: String::from(match_key),
        driver_id: String::from(driver_id),
        source: String::from(source),
        previous_driver_id: None,
        rollback_to_none_allowed: true,
    });

    ActivationOutcome {
        changed: true,
        previous_driver_id: None,
    }
}

pub fn rollback_binding(match_key: &str) -> Result<String, &'static str> {
    let mut active = ACTIVE_RUNTIME_BINDINGS.lock();
    let Some(index) = active.iter().position(|binding| binding.match_key == match_key) else {
        return Err("no live binding exists for this match key");
    };

    if active[index].previous_driver_id.is_none() && active[index].rollback_to_none_allowed {
        let removed = active.remove(index);
        return Ok(format!("(removed live binding {})", removed.driver_id).into());
    }

    let binding = &mut active[index];
    let Some(previous) = binding.previous_driver_id.clone() else {
        return Err("no rollback target is available for this match key");
    };

    let current = binding.driver_id.clone();
    binding.driver_id = previous.clone();
    binding.previous_driver_id = Some(current);
    binding.source = String::from("rollback");
    binding.rollback_to_none_allowed = false;
    Ok(previous)
}

pub fn current_driver(match_key: &str) -> Option<String> {
    ACTIVE_RUNTIME_BINDINGS
        .lock()
        .iter()
        .find(|binding| binding.match_key == match_key)
        .map(|binding| binding.driver_id.clone())
}

pub fn deactivate_binding(match_key: &str) -> Option<String> {
    let mut active = ACTIVE_RUNTIME_BINDINGS.lock();
    let index = active
        .iter()
        .position(|binding| binding.match_key == match_key)?;
    Some(active.remove(index).driver_id)
}

pub fn snapshot() -> Vec<RuntimeDriverBinding> {
    ACTIVE_RUNTIME_BINDINGS.lock().clone()
}
