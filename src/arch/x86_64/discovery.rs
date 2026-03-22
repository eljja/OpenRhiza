// src/arch/x86_64/discovery.rs
use core::arch::x86_64::__cpuid;

pub struct SystemIdentity {
    pub cpu_cores: u32,
    pub total_memory: usize,
    pub storage_detected: bool,
}

impl SystemIdentity {
    pub fn scan() -> Self {
        // 1. CPUID 스캔 (실제 코어 수 확인)
        let cores = Self::get_cpu_count();
        
        // 2. Memory & Storage는 부트로더와 PCI 스캔(Phase 2)에서 받아올 예정이므로 초기값 0/false 할당
        let mem = 0; 
        let storage = false;
        
        SystemIdentity {
            cpu_cores: cores,
            total_memory: mem,
            storage_detected: storage,
        }
    }

    fn get_cpu_count() -> u32 {
        // CPUID EAX=1 기능 호출
        let result = __cpuid(1);
        // EBX 레지스터의 [23:16] 비트에 논리 프로세서(Logical Processor) 개수가 담겨 있음
        ((result.ebx >> 16) & 0xFF) as u32
    }
}