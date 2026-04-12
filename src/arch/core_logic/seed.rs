// src/arch/core_logic/seed.rs

use crate::arch::discovery::SystemIdentity;

/// Result returned after executing AI-generated code.
#[allow(dead_code)]
pub enum ExecutionResult {
    Success(&'static str),
    Panic(&'static str),
}

/// Core structure that stores system identity and feedback for the seed loop.
#[allow(dead_code)]
pub struct OpenRhizaSeed {
    pub identity: SystemIdentity,
    // Fixed-size log buffer for environments without dynamic allocation.
    pub log_buffer: [&'static str; 10],
    pub log_count: usize,
}

impl OpenRhizaSeed {
    /// Create a seed with the discovered system identity.
    pub fn new(identity: SystemIdentity) -> Self {
        Self {
            identity,
            log_buffer: [""; 10],
            log_count: 0,
        }
    }

    /// Return the next hardware action the AI should attempt.
    pub fn request_next_action(&self) -> &'static str {
        "INIT_VGA_BUFFER"
    }

    /// Simulate execution of AI-generated code inside an isolated sandbox.
    pub fn execute_instruction(&mut self, _generated_code: &str) -> ExecutionResult {
        ExecutionResult::Success("Command executed without hardware fault.")
    }

    /// Record the result and feed it back into the next learning loop.
    pub fn report_and_learn(&mut self, result: ExecutionResult) {
        match result {
            ExecutionResult::Success(msg) => self.add_log(msg),
            ExecutionResult::Panic(err) => self.add_log(err),
        }
    }

    /// Safely append a message to the internal log buffer.
    fn add_log(&mut self, message: &'static str) {
        let index = self.log_count % self.log_buffer.len();
        self.log_buffer[index] = message;
        self.log_count = self.log_count.wrapping_add(1);
    }
}
