// src/arch/discovery.rs

/// OpenRhiza가 인식한 하드웨어의 기본 정보
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct SystemIdentity {
    pub cpu_cores: u32,
    pub total_memory: usize,
    pub storage_detected: bool,
}

impl SystemIdentity {
    /// 시스템 하드웨어를 스캔하여 기초 정보를 수집합니다.
    pub fn scan() -> Self {
        SystemIdentity {
            cpu_cores: Self::get_cpu_count(),
            total_memory: Self::detect_memory_limit(),
            storage_detected: Self::check_storage_interface(),
        }
    }

    fn get_cpu_count() -> u32 {
        1 // 임시(Dummy) 코어 수 반환
    }

    fn detect_memory_limit() -> usize {
        1024 * 1024 * 16 // 임시 16MB 반환
    }

    fn check_storage_interface() -> bool {
        false // 임시 반환
    }
}