use alloc::string::String;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentStage {
    Bootstrap,
    Cached,
    Testing,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingLoadDisposition {
    Testing,
    RestoreActive,
}

#[derive(Clone, Debug)]
pub struct ComponentRuntimeState {
    pub stage: ComponentStage,
    pub current_component_id: Option<String>,
    pub persisted_component_id: Option<String>,
    pub previous_component_id: Option<String>,
    pub last_error: Option<String>,
    pub pending_component_id: Option<String>,
    pub pending_disposition: Option<PendingLoadDisposition>,
}

impl ComponentRuntimeState {
    pub const fn bootstrap() -> Self {
        Self {
            stage: ComponentStage::Bootstrap,
            current_component_id: None,
            persisted_component_id: None,
            previous_component_id: None,
            last_error: None,
            pending_component_id: None,
            pending_disposition: None,
        }
    }

    pub fn reject_duplicate_load(&self, component_id: &str) -> Result<(), &'static str> {
        if self.pending_component_id.as_deref() == Some(component_id) {
            return Err("same sandbox input driver is already queued");
        }

        if self.current_component_id.as_deref() == Some(component_id)
            && matches!(
                self.stage,
                ComponentStage::Cached | ComponentStage::Testing | ComponentStage::Active
            )
        {
            return Err("same sandbox input driver is already loaded");
        }

        Ok(())
    }

    pub fn begin_load(
        &mut self,
        component_id: &str,
        disposition: PendingLoadDisposition,
    ) {
        self.last_error = None;
        self.pending_component_id = Some(String::from(component_id));
        self.pending_disposition = Some(disposition);
    }

    pub fn finish_load_success(&mut self) -> Option<String> {
        let component_id = self.pending_component_id.take()?;
        let disposition = self
            .pending_disposition
            .take()
            .unwrap_or(PendingLoadDisposition::Testing);

        if let Some(current) = self.current_component_id.as_ref() {
            if current != &component_id {
                self.previous_component_id = Some(current.clone());
            }
        }

        self.current_component_id = Some(component_id.clone());
        self.stage = match disposition {
            PendingLoadDisposition::Testing => ComponentStage::Testing,
            PendingLoadDisposition::RestoreActive => {
                self.persisted_component_id = Some(component_id.clone());
                ComponentStage::Active
            }
        };
        self.last_error = None;
        Some(component_id)
    }

    pub fn finish_load_failure(&mut self, error: &str) {
        self.pending_component_id = None;
        self.pending_disposition = None;
        self.last_error = Some(String::from(error));
        self.stage = if self.current_component_id.is_some() {
            if self.persisted_component_id.is_some() {
                ComponentStage::Active
            } else {
                ComponentStage::Testing
            }
        } else {
            ComponentStage::Bootstrap
        };
    }

    pub fn promote_loaded(&mut self) -> Result<String, &'static str> {
        let component_id = self
            .current_component_id
            .clone()
            .ok_or("no sandbox component is currently loaded")?;
        self.persisted_component_id = Some(component_id.clone());
        self.stage = ComponentStage::Active;
        Ok(component_id)
    }

    pub fn rollback_to_bootstrap(&mut self) -> Result<String, &'static str> {
        let Some(current) = self.current_component_id.take() else {
            return Err("no sandbox component is active");
        };

        self.previous_component_id = Some(current.clone());
        self.persisted_component_id = None;
        self.pending_component_id = None;
        self.pending_disposition = None;
        self.stage = ComponentStage::Bootstrap;
        self.last_error = None;
        Ok(current)
    }

    pub fn handle_component_loss(&mut self, reason: &str) -> Option<String> {
        self.pending_component_id = None;
        self.pending_disposition = None;
        self.last_error = Some(String::from(reason));
        self.stage = ComponentStage::Bootstrap;
        self.current_component_id.take()
    }

    pub fn queue_restore_candidate(&self) -> Option<String> {
        if self.current_component_id.is_some() || self.pending_component_id.is_some() {
            return None;
        }

        self.persisted_component_id.clone()
    }

    pub fn note_cached_component(&mut self, component_id: &str) {
        self.current_component_id = Some(String::from(component_id));
        self.last_error = None;
        self.stage = ComponentStage::Cached;
    }
}
