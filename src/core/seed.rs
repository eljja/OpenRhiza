// src/core/seed.rs
use crate::arch::x86_64::discovery::SystemIdentity;

pub enum ExecutionResult<'a> {
    Success(&'a str),
    Panic(&'a str), // 하드웨어 예외 발생 시 AI가 학습할 데이터
}

pub struct OpenRhizaSeed<'a> {
    pub identity: SystemIdentity,
    pub log_buffer: [&'a str; 10], // 동적 할당(Vec) 대신 고정 크기 배열 사용 (No-std 제약)
    pub log_count: usize,
}

impl<'a> OpenRhizaSeed<'a> {
    pub fn new(identity: SystemIdentity) -> Self {
        Self {
            identity,
            log_buffer: [""; 10],
            log_count: 0,
        }
    }

    pub fn execute_instruction(&mut self, generated_code: &str) -> ExecutionResult<'a> {
        // AI가 생성한 하위 드라이버/명령을 격리된 환경(Layer 0)에서 실행
        match self.run_in_sandbox(generated_code) {
            Ok(output) => ExecutionResult::Success(output),
            Err(e) => {
                // 에러 발생 시 로그 버퍼에 기록 (포맷팅 없이 직접 참조 저장)
                if self.log_count < self.log_buffer.len() {
                    self.log_buffer[self.log_count] = e;
                    self.log_count += 1;
                }
                ExecutionResult::Panic(e)
            }
        }
    }

    fn run_in_sandbox(&self, _code: &str) -> Result<&'static str, &'static str> {
        // TODO: 향후 이 곳에 IOMMU 및 Ring 3 전환 로직이 들어갑니다.
        // 현재는 모의 에러를 반환하여 AI 피드백 루프를 테스트합니다.
        Err("Exception: Page Fault at 0xDEADBEEF")
    }

    /// AI가 샌드박스 내부에서 하드웨어 입력(키보드 큐)을 읽기 위해 호출하는 원시 함수
    pub fn poll_hardware_event(&self) -> Option<u8> {
        crate::arch::x86_64::interrupts::KEYBOARD_QUEUE.lock().pop()
    }

    /// 외부 AI(Host)로부터 전송된 드라이버 데이터(매핑 테이블)를 읽어오는 원시 함수
    pub fn poll_host_data(&self) -> Option<u8> {
        crate::arch::x86_64::serial::poll_receive()
    }
}