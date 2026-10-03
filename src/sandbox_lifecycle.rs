use alloc::string::String;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxStage {
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
pub struct SandboxRuntimeState {
    pub stage: SandboxStage,
    pub current_artifact_id: Option<String>,
    pub persisted_artifact_id: Option<String>,
    pub previous_artifact_id: Option<String>,
    pub last_error: Option<String>,
    pub pending_artifact_id: Option<String>,
    pub pending_disposition: Option<PendingLoadDisposition>,
}

impl SandboxRuntimeState {
    pub const fn new() -> Self {
        Self {
            stage: SandboxStage::Bootstrap,
            current_artifact_id: None,
            persisted_artifact_id: None,
            previous_artifact_id: None,
            last_error: None,
            pending_artifact_id: None,
            pending_disposition: None,
        }
    }

    pub fn begin_load(
        &mut self,
        artifact_id: &str,
        disposition: PendingLoadDisposition,
    ) -> Result<(), &'static str> {
        if self.pending_artifact_id.as_deref() == Some(artifact_id) {
            return Err("same sandbox artifact is already queued");
        }

        if self.current_artifact_id.as_deref() == Some(artifact_id) {
            return Err("same sandbox artifact is already loaded");
        }

        self.last_error = None;
        self.pending_artifact_id = Some(String::from(artifact_id));
        self.pending_disposition = Some(disposition);
        Ok(())
    }

    pub fn note_cached_artifact(&mut self, artifact_id: &str) {
        self.current_artifact_id = Some(String::from(artifact_id));
        self.last_error = None;
        self.stage = SandboxStage::Cached;
    }

    pub fn finish_load_success(&mut self) -> Option<String> {
        let artifact_id = self.pending_artifact_id.take()?;
        let disposition = self
            .pending_disposition
            .take()
            .unwrap_or(PendingLoadDisposition::Testing);

        if let Some(current) = self.current_artifact_id.as_ref() {
            if current != &artifact_id {
                self.previous_artifact_id = Some(current.clone());
            }
        }

        self.current_artifact_id = Some(artifact_id.clone());
        self.stage = match disposition {
            PendingLoadDisposition::Testing => SandboxStage::Testing,
            PendingLoadDisposition::RestoreActive => {
                self.persisted_artifact_id = Some(artifact_id.clone());
                SandboxStage::Active
            }
        };
        self.last_error = None;
        Some(artifact_id)
    }

    pub fn finish_load_failure(&mut self, error: &str) {
        self.pending_artifact_id = None;
        self.pending_disposition = None;
        self.last_error = Some(String::from(error));
        self.stage = if self.current_artifact_id.is_some() {
            if self.persisted_artifact_id.is_some() {
                SandboxStage::Active
            } else {
                SandboxStage::Testing
            }
        } else {
            SandboxStage::Bootstrap
        };
    }

    pub fn promote_current(&mut self) -> Result<String, &'static str> {
        let Some(current) = self.current_artifact_id.clone() else {
            return Err("no sandbox artifact is currently loaded");
        };

        self.persisted_artifact_id = Some(current.clone());
        self.stage = SandboxStage::Active;
        Ok(current)
    }

    pub fn rollback_to_bootstrap(&mut self) -> Result<String, &'static str> {
        let Some(current) = self.current_artifact_id.take() else {
            return Err("no sandbox artifact is active");
        };

        self.previous_artifact_id = Some(current.clone());
        self.persisted_artifact_id = None;
        self.pending_artifact_id = None;
        self.pending_disposition = None;
        self.stage = SandboxStage::Bootstrap;
        self.last_error = None;
        Ok(current)
    }

    pub fn handle_hardware_loss(&mut self, reason: &str) -> Option<String> {
        self.pending_artifact_id = None;
        self.pending_disposition = None;
        self.last_error = Some(String::from(reason));
        self.stage = SandboxStage::Bootstrap;
        self.current_artifact_id.take()
    }
}
