// src/core/seed.rs
use crate::arch::x86_64::discovery::SystemIdentity;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use wasmi::{Caller, Engine, Linker, Module, Store, Instance};

pub enum ExecutionResult {
    Success(String),
    Panic(String),
}

pub struct WasmState {
    pub engine: Engine,
    pub store: Store<()>,
    pub instance: Instance,
}

pub struct OpenRhizaSeed {
    pub identity: SystemIdentity,
    pub log_buffer: Vec<String>,
    pub wasm_state: Option<WasmState>,
}

impl OpenRhizaSeed {
    pub fn new(identity: SystemIdentity) -> Self {
        Self {
            identity,
            log_buffer: Vec::new(),
            wasm_state: None,
        }
    }

    pub fn execute_wasm_sandbox(&mut self, wasm_bytes: &[u8]) -> ExecutionResult {
        match self.instantiate_and_run(wasm_bytes) {
            Ok(_) => {
                let msg = format!("Wasm Execution Success! init_e1000 completed. Engine running.");
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

    fn instantiate_and_run(&mut self, wasm_bytes: &[u8]) -> Result<(), String> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm_bytes).map_err(|e| format!("Failed to parse Wasm: {}", e))?;
        let mut store = Store::new(&engine, ());
        let mut linker = <Linker<()>>::new(&engine);

        linker.func_wrap("env", "read_mmio", |_caller: Caller<'_, ()>, addr: u32| -> u32 {
            unsafe { 
                let virt_addr = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + (addr as u64);
                core::ptr::read_volatile(virt_addr as usize as *const u32) 
            }
        }).map_err(|_| String::from("Failed to link read_mmio"))?;

        linker.func_wrap("env", "write_mmio", |_caller: Caller<'_, ()>, addr: u32, val: u32| {
            unsafe { 
                let virt_addr = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + (addr as u64);
                core::ptr::write_volatile(virt_addr as usize as *mut u32, val) 
            }
        }).map_err(|_| String::from("Failed to link write_mmio"))?;

        linker.func_wrap("env", "alloc_dma_page", |_caller: Caller<'_, ()>| -> u32 {
            unsafe {
                let phys_addr = crate::arch::x86_64::discovery::DMA_BASE + crate::arch::x86_64::discovery::DMA_OFFSET;
                crate::arch::x86_64::discovery::DMA_OFFSET += 4096;
                let virt_addr = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET + (phys_addr as u64);
                core::ptr::write_bytes(virt_addr as usize as *mut u8, 0, 4096);
                crate::println!("[OS DMA] Allocated 4KB physical page at {:#010X} for AI Wasm", phys_addr);
                phys_addr
            }
        }).map_err(|_| String::from("Failed to link alloc_dma_page"))?;

        linker.func_wrap("env", "os_rx_packet", |caller: Caller<'_, ()>, ptr: u32, len: u32| {
            if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                let mut packet = alloc::vec![0u8; len as usize];
                if memory.read(&caller, ptr as usize, &mut packet).is_ok() {
                    crate::net::RX_QUEUE.lock().push(packet);
                }
            }
        }).map_err(|_| String::from("Failed to link os_rx_packet"))?;

        linker.func_wrap("env", "os_fetch_tx_packet", |mut caller: Caller<'_, ()>, ptr: u32, max_len: u32| -> u32 {
            let mut tx_queue = crate::net::TX_QUEUE.lock();
            if !tx_queue.is_empty() {
                let packet = tx_queue.remove(0);
                if (packet.len() as u32) <= max_len {
                    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                        if memory.write(&mut caller, ptr as usize, &packet).is_ok() {
                            return packet.len() as u32;
                        }
                    }
                }
            }
            0
        }).map_err(|_| String::from("Failed to link os_fetch_tx_packet"))?;

        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| format!("Instantiate failed: {}", e))?
            .start(&mut store)
            .map_err(|e| format!("Start instance failed: {}", e))?;
        
        let init = instance.get_typed_func::<(), ()>(&store, "init_driver")
            .map_err(|_| String::from("Export 'init_driver' not found"))?;
        init.call(&mut store, ()).map_err(|e| format!("Init trapped: {}", e))?;

        self.wasm_state = Some(WasmState { engine, store, instance });
        Ok(())
    }

    pub fn poll_wasm_network(&mut self) {
        if let Some(state) = &mut self.wasm_state {
            if let Ok(poll) = state.instance.get_typed_func::<(), ()>(&state.store, "poll_net") {
                let _ = poll.call(&mut state.store, ());
            }
        }
    }

    pub fn poll_hardware_event(&self) -> Option<u8> {
        crate::arch::x86_64::interrupts::KEYBOARD_QUEUE.lock().pop()
    }

    pub fn poll_host_data(&self) -> Option<u8> {
        crate::arch::x86_64::serial::poll_receive()
    }
}
