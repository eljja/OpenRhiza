// src/core/seed.rs
use crate::arch::x86_64::discovery::SystemIdentity;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use wasmi::{Caller, Engine, Linker, Module, Store};

pub enum ExecutionResult {
    Success(String),
    Panic(String),
}

pub struct OpenRhizaSeed {
    pub identity: SystemIdentity,
    pub log_buffer: Vec<String>, // 동적 할당(Vec)을 이용한 무제한 로그 버퍼
}

impl OpenRhizaSeed {
    pub fn new(identity: SystemIdentity) -> Self {
        Self {
            identity,
            log_buffer: Vec::new(),
        }
    }

    /// Layer 0 샌드박스 내에서 Wasm 바이너리를 안전하게 실행합니다.
    pub fn execute_wasm_sandbox(&mut self, wasm_bytes: &[u8]) -> ExecutionResult {
        match self.run_wasm_internal(wasm_bytes) {
            Ok(_) => {
                let msg = format!("Wasm Execution Success! init_e1000 completed.");
                self.log_buffer.push(msg.clone());
                ExecutionResult::Success(msg)
            },
            Err(e) => {
                let err_msg = format!("Wasm Sandbox Trap (Panic): {}", e);
                self.log_buffer.push(err_msg.clone());
                ExecutionResult::Panic(err_msg)
            }
        }
    }

    fn run_wasm_internal(&self, wasm_bytes: &[u8]) -> Result<(), &'static str> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm_bytes).map_err(|_| "Failed to parse Wasm module")?;
        let mut store = Store::new(&engine, ());
        let mut linker = <Linker<()>>::new(&engine);

        // Host Function: read_mmio (Wasm 샌드박스에서 물리 메모리 읽기 허용)
        linker.func_wrap("env", "read_mmio", |_caller: Caller<'_, ()>, addr: u32| -> u32 {
            unsafe { core::ptr::read_volatile(addr as usize as *const u32) }
        }).map_err(|_| "Failed to link read_mmio")?;

        // Host Function: write_mmio (Wasm 샌드박스에서 물리 메모리 쓰기 허용)
        linker.func_wrap("env", "write_mmio", |_caller: Caller<'_, ()>, addr: u32, val: u32| {
            unsafe { core::ptr::write_volatile(addr as usize as *mut u32, val) }
        }).map_err(|_| "Failed to link write_mmio")?;

        let instance = linker.instantiate(&mut store, &module).map_err(|_| "Failed to instantiate")?
            .start(&mut store).map_err(|_| "Failed to start instance")?;
        
        let init = instance.get_typed_func::<(), ()>(&store, "init_e1000").map_err(|_| "Export 'init_e1000' not found")?;
        init.call(&mut store, ()).map_err(|_| "Wasm execution trapped!")
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