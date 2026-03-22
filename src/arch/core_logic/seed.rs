// src/arch/core_logic/seed.rs

use crate::arch::discovery::SystemIdentity;

/// AI 엔진이 생성한 코드를 실행한 후의 결과를 나타냅니다.
#[allow(dead_code)]
pub enum ExecutionResult {
    Success(&'static str),
    Panic(&'static str),
}

/// OpenRhiza가 스스로 명령을 해석하고 실행 결과를 피드백받는 핵심 구조체
#[allow(dead_code)]
pub struct OpenRhizaSeed {
    pub identity: SystemIdentity,
    // 동적 할당을 사용할 수 없는 환경을 위한 고정 크기 로그 버퍼
    pub log_buffer: [&'static str; 10],
    pub log_count: usize,
}

impl OpenRhizaSeed {
    /// 시스템 초기 환경(Identity)을 부여받아 Seed를 생성합니다.
    pub fn new(identity: SystemIdentity) -> Self {
        Self {
            identity,
            log_buffer: [""; 10],
            log_count: 0,
        }
    }

    /// AI가 다음에 시도할 하드웨어 제어 명령을 요청합니다.
    pub fn request_next_action(&self) -> &'static str {
        "INIT_VGA_BUFFER"
    }

    /// AI가 생성한 명령을 격리된 샌드박스에서 모의 실행합니다.
    pub fn execute_instruction(&mut self, _generated_code: &str) -> ExecutionResult {
        ExecutionResult::Success("Command executed without hardware fault.")
    }

    /// 실행 결과를 시스템 로그에 기록하고 다음 학습(루프)에 반영합니다.
    pub fn report_and_learn(&mut self, result: ExecutionResult) {
        match result {
            ExecutionResult::Success(msg) => self.add_log(msg),
            ExecutionResult::Panic(err) => self.add_log(err),
        }
    }

    /// 내부 로그 버퍼에 메시지를 안전하게 기록합니다.
    fn add_log(&mut self, message: &'static str) {
        let index = self.log_count % self.log_buffer.len();
        self.log_buffer[index] = message;
        self.log_count = self.log_count.wrapping_add(1);
    }
}