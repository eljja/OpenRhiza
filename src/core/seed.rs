// src/core/seed.rs
use crate::arch::x86_64::discovery::SystemIdentity;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use wasmi::{Caller, Engine, Linker, Module, Store, Instance};
use crate::input_handoff::HidDeviceKind;

pub enum ExecutionResult {
    Success(String),
    Panic(String),
}

pub struct WasmState {
    pub engine: Engine,
    pub store: Store<()>,
    pub instance: Instance,
}

pub struct LoadedWasmModule {
    pub module_key: String,
    pub state: WasmState,
}

pub struct OpenRhizaSeed {
    pub identity: SystemIdentity,
    pub log_buffer: Vec<String>,
    pub wasm_modules: Vec<LoadedWasmModule>,
}

impl OpenRhizaSeed {
    pub fn new(identity: SystemIdentity) -> Self {
        Self {
            identity,
            log_buffer: Vec::new(),
            wasm_modules: Vec::new(),
        }
    }

    pub fn execute_wasm_sandbox(&mut self, wasm_bytes: &[u8]) -> ExecutionResult {
        self.execute_named_wasm_sandbox("runtime:default", wasm_bytes)
    }

    pub fn execute_named_wasm_sandbox(&mut self, module_key: &str, wasm_bytes: &[u8]) -> ExecutionResult {
        match self.instantiate_wasm(wasm_bytes) {
            Ok(state) => {
                self.upsert_module_state(module_key, state);
                let msg = format!("Wasm Execution Success! sandbox module initialized. Engine running.");
                self.log_buffer.push(msg.clone());
                ExecutionResult::Success(msg)
            }
            Err(e) => {
                let err_msg = format!("Wasm Sandbox Trap (Panic): {}", e);
                self.log_buffer.push(err_msg.clone());
                ExecutionResult::Panic(err_msg)
            }
        }
    }

    pub fn execute_input_wasm_sandbox(&mut self, kind: HidDeviceKind, wasm_bytes: &[u8]) -> ExecutionResult {
        self.execute_named_wasm_sandbox(input_module_key(kind), wasm_bytes)
    }

    fn instantiate_wasm(&mut self, wasm_bytes: &[u8]) -> Result<WasmState, String> {
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

        linker.func_wrap("env", "os_fetch_hid_packet", |mut caller: Caller<'_, ()>, ptr: u32, max_len: u32| -> u32 {
            let required_len = 4 + crate::input_handoff::MAX_HID_REPORT_BYTES;
            if max_len < required_len as u32 {
                return 0;
            }
            let Some(packet) = crate::input_handoff::fetch_hid_packet() else {
                return 0;
            };

            if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                let mut bytes = [0u8; 4 + crate::input_handoff::MAX_HID_REPORT_BYTES];
                bytes[0] = packet.kind as u8;
                bytes[1] = packet.slot_id;
                bytes[2] = packet.port_id;
                bytes[3] = packet.report_len;
                bytes[4..].copy_from_slice(&packet.report);
                if memory.write(&mut caller, ptr as usize, &bytes).is_ok() {
                    return bytes.len() as u32;
                }
            }
            0
        }).map_err(|_| String::from("Failed to link os_fetch_hid_packet"))?;

        linker.func_wrap("env", "os_fetch_hid_packet_for_kind", |mut caller: Caller<'_, ()>, kind: u32, ptr: u32, max_len: u32| -> u32 {
            let required_len = 4 + crate::input_handoff::MAX_HID_REPORT_BYTES;
            if max_len < required_len as u32 {
                return 0;
            }
            let kind = match kind {
                1 => crate::input_handoff::HidDeviceKind::Keyboard,
                2 => crate::input_handoff::HidDeviceKind::Mouse,
                _ => return 0,
            };
            let Some(packet) = crate::input_handoff::fetch_hid_packet_for_kind(kind) else {
                return 0;
            };

            if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                let mut bytes = [0u8; 4 + crate::input_handoff::MAX_HID_REPORT_BYTES];
                bytes[0] = packet.kind as u8;
                bytes[1] = packet.slot_id;
                bytes[2] = packet.port_id;
                bytes[3] = packet.report_len;
                bytes[4..].copy_from_slice(&packet.report);
                if memory.write(&mut caller, ptr as usize, &bytes).is_ok() {
                    return bytes.len() as u32;
                }
            }
            0
        }).map_err(|_| String::from("Failed to link os_fetch_hid_packet_for_kind"))?;

        linker.func_wrap("env", "os_emit_input_event", |_caller: Caller<'_, ()>, kind: u32, a: i32, b: i32, c: i32| {
            crate::input_handoff::emit_input_event_from_wasm(kind, a, b, c);
        }).map_err(|_| String::from("Failed to link os_emit_input_event"))?;

        linker.func_wrap("env", "os_set_input_driver_mode", |_caller: Caller<'_, ()>, mode: u32| {
            crate::input_handoff::set_routing_mode_from_wasm(mode);
        }).map_err(|_| String::from("Failed to link os_set_input_driver_mode"))?;

        linker.func_wrap("env", "os_set_input_driver_active", |_caller: Caller<'_, ()>, active: u32| {
            crate::input_handoff::set_sandbox_input_active(active != 0);
        }).map_err(|_| String::from("Failed to link os_set_input_driver_active"))?;

        linker.func_wrap("env", "os_set_input_driver_mode_for_kind", |_caller: Caller<'_, ()>, kind: u32, mode: u32| {
            let kind = match kind {
                1 => crate::input_handoff::HidDeviceKind::Keyboard,
                2 => crate::input_handoff::HidDeviceKind::Mouse,
                _ => return,
            };
            let mode = match mode {
                0 => crate::input_handoff::InputRoutingMode::BootstrapOnly,
                1 => crate::input_handoff::InputRoutingMode::HandoffMirror,
                2 => crate::input_handoff::InputRoutingMode::SandboxPreferred,
                3 => crate::input_handoff::InputRoutingMode::SandboxExclusive,
                _ => crate::input_handoff::InputRoutingMode::HandoffMirror,
            };
            crate::input_handoff::set_routing_mode_for_kind(kind, mode);
        }).map_err(|_| String::from("Failed to link os_set_input_driver_mode_for_kind"))?;

        linker.func_wrap("env", "os_set_input_driver_active_for_kind", |_caller: Caller<'_, ()>, kind: u32, active: u32| {
            let kind = match kind {
                1 => crate::input_handoff::HidDeviceKind::Keyboard,
                2 => crate::input_handoff::HidDeviceKind::Mouse,
                _ => return,
            };
            crate::input_handoff::set_sandbox_input_active_for_kind(kind, active != 0);
        }).map_err(|_| String::from("Failed to link os_set_input_driver_active_for_kind"))?;

        linker.func_wrap("env", "os_log", |caller: Caller<'_, ()>, ptr: u32, len: u32| {
            let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
                return;
            };

            let mut bytes = alloc::vec![0u8; len as usize];
            if memory.read(&caller, ptr as usize, &mut bytes).is_err() {
                return;
            }

            if let Ok(text) = core::str::from_utf8(&bytes) {
                crate::result_println!("{}", text.trim_end_matches('\n'));
            }
        }).map_err(|_| String::from("Failed to link os_log"))?;

        linker.func_wrap(
            "env",
            "os_request_display_mode",
            |_caller: Caller<'_, ()>,
             backend: u32,
             text_cols: u32,
             text_rows: u32,
             pixel_width: u32,
             pixel_height: u32| {
                crate::display::request_mode_from_wasm(
                    backend,
                    text_cols,
                    text_rows,
                    pixel_width,
                    pixel_height,
                );
            },
        )
        .map_err(|_| String::from("Failed to link os_request_display_mode"))?;

        linker.func_wrap(
            "env",
            "os_set_gui_session_state",
            |_caller: Caller<'_, ()>, state: u32| {
                crate::display::set_gui_session_state_from_wasm(state);
            },
        )
        .map_err(|_| String::from("Failed to link os_set_gui_session_state"))?;

        linker.func_wrap(
            "env",
            "os_set_display_session_target",
            |_caller: Caller<'_, ()>, target: u32| {
                crate::display::set_display_session_target_from_wasm(target);
            },
        )
        .map_err(|_| String::from("Failed to link os_set_display_session_target"))?;

        linker.func_wrap(
            "env",
            "os_set_display_validation_state",
            |_caller: Caller<'_, ()>, state: u32| {
                crate::display::set_display_validation_state_from_wasm(state);
            },
        )
        .map_err(|_| String::from("Failed to link os_set_display_validation_state"))?;

        linker.func_wrap(
            "env",
            "os_set_display_overlay_line",
            |caller: Caller<'_, ()>, slot: u32, ptr: u32, len: u32| {
                let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
                    return;
                };

                let mut bytes = alloc::vec![0u8; len as usize];
                if memory.read(&caller, ptr as usize, &mut bytes).is_err() {
                    return;
                }

                crate::display::set_overlay_line_from_wasm(slot, &bytes);
            },
        )
        .map_err(|_| String::from("Failed to link os_set_display_overlay_line"))?;

        linker.func_wrap(
            "env",
            "os_gui_select_session",
            |_caller: Caller<'_, ()>, session: u32| {
                crate::display::select_gui_session_from_wasm(session);
            },
        )
        .map_err(|_| String::from("Failed to link os_gui_select_session"))?;

        linker.func_wrap(
            "env",
            "os_gui_focus_object",
            |_caller: Caller<'_, ()>, target: u32| {
                crate::display::focus_gui_object_from_wasm(target);
            },
        )
        .map_err(|_| String::from("Failed to link os_gui_focus_object"))?;

        linker.func_wrap(
            "env",
            "os_gui_set_object_style",
            |_caller: Caller<'_, ()>, handle: u32, style: u32| {
                crate::display::set_gui_style_code_from_wasm(handle, style);
            },
        )
        .map_err(|_| String::from("Failed to link os_gui_set_object_style"))?;

        linker.func_wrap(
            "env",
            "os_gui_set_object_bounds",
            |_caller: Caller<'_, ()>, handle: u32, x: u32, y: u32, width: u32, height: u32| {
                crate::display::set_gui_bounds_from_wasm(handle, x, y, width, height);
            },
        )
        .map_err(|_| String::from("Failed to link os_gui_set_object_bounds"))?;

        linker.func_wrap(
            "env",
            "os_gui_set_object_interaction",
            |_caller: Caller<'_, ()>, handle: u32, interaction: u32| {
                crate::display::set_gui_interaction_code_from_wasm(handle, interaction);
            },
        )
        .map_err(|_| String::from("Failed to link os_gui_set_object_interaction"))?;

        linker.func_wrap(
            "env",
            "os_gui_reset_object_mutations",
            |_caller: Caller<'_, ()>, handle: u32| {
                crate::display::reset_gui_mutations_from_wasm(handle);
            },
        )
        .map_err(|_| String::from("Failed to link os_gui_reset_object_mutations"))?;

        linker.func_wrap(
            "env",
            "os_gui_set_object_label",
            |caller: Caller<'_, ()>, handle: u32, ptr: u32, len: u32| {
                let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
                    return;
                };

                let mut bytes = alloc::vec![0u8; len.min(256) as usize];
                if memory.read(&caller, ptr as usize, &mut bytes).is_err() {
                    return;
                }

                crate::display::set_gui_label_from_wasm(handle, &bytes);
            },
        )
        .map_err(|_| String::from("Failed to link os_gui_set_object_label"))?;

        linker.func_wrap("env", "os_storage_list_images", |_caller: Caller<'_, ()>| -> u32 {
            crate::storage_host::list_images()
        }).map_err(|_| String::from("Failed to link os_storage_list_images"))?;

        linker.func_wrap("env", "os_storage_open_image", |_caller: Caller<'_, ()>, index: u32| -> u32 {
            crate::storage_host::open_image(index)
        }).map_err(|_| String::from("Failed to link os_storage_open_image"))?;

        linker.func_wrap("env", "os_storage_get_block_count", |_caller: Caller<'_, ()>, handle: u32| -> u32 {
            crate::storage_host::block_count(handle)
        }).map_err(|_| String::from("Failed to link os_storage_get_block_count"))?;

        linker.func_wrap("env", "os_storage_get_filesystem_block_count", |_caller: Caller<'_, ()>, handle: u32| -> u32 {
            crate::storage_host::filesystem_block_count(handle)
        }).map_err(|_| String::from("Failed to link os_storage_get_filesystem_block_count"))?;

        linker.func_wrap("env", "os_storage_get_scratch_start_lba", |_caller: Caller<'_, ()>, handle: u32| -> u32 {
            crate::storage_host::scratch_start_lba(handle)
        }).map_err(|_| String::from("Failed to link os_storage_get_scratch_start_lba"))?;

        linker.func_wrap("env", "os_storage_get_scratch_block_count", |_caller: Caller<'_, ()>, handle: u32| -> u32 {
            crate::storage_host::scratch_block_count(handle)
        }).map_err(|_| String::from("Failed to link os_storage_get_scratch_block_count"))?;

        linker.func_wrap("env", "os_storage_is_writable", |_caller: Caller<'_, ()>, handle: u32| -> u32 {
            crate::storage_host::writable(handle) as u32
        }).map_err(|_| String::from("Failed to link os_storage_is_writable"))?;

        linker.func_wrap("env", "os_storage_get_fs_hint", |_caller: Caller<'_, ()>, handle: u32| -> u32 {
            crate::storage_host::fs_hint_code(handle)
        }).map_err(|_| String::from("Failed to link os_storage_get_fs_hint"))?;

        linker.func_wrap(
            "env",
            "os_storage_read_blocks",
            |mut caller: Caller<'_, ()>, handle: u32, lba: u32, count: u32, ptr: u32, max_len: u32| -> u32 {
                let total_bytes = count.saturating_mul(crate::storage_host::STORAGE_BLOCK_SIZE);
                if total_bytes == 0 || max_len < total_bytes {
                    return 0;
                }

                let mut bytes = alloc::vec![0u8; total_bytes as usize];
                if crate::storage_host::read_blocks(handle, lba, count, &mut bytes).is_err() {
                    return 0;
                }

                let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
                    return 0;
                };

                if memory.write(&mut caller, ptr as usize, &bytes).is_ok() {
                    total_bytes
                } else {
                    0
                }
            },
        ).map_err(|_| String::from("Failed to link os_storage_read_blocks"))?;

        linker.func_wrap(
            "env",
            "os_storage_write_blocks",
            |caller: Caller<'_, ()>, handle: u32, lba: u32, count: u32, ptr: u32, len: u32| -> u32 {
                let total_bytes = count.saturating_mul(crate::storage_host::STORAGE_BLOCK_SIZE);
                if total_bytes == 0 || len < total_bytes {
                    return 0;
                }

                let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
                    return 0;
                };

                let mut bytes = alloc::vec![0u8; total_bytes as usize];
                if memory.read(&caller, ptr as usize, &mut bytes).is_err() {
                    return 0;
                }

                if crate::storage_host::write_blocks(handle, lba, count, &bytes).is_ok() {
                    1
                } else {
                    0
                }
            },
        ).map_err(|_| String::from("Failed to link os_storage_write_blocks"))?;

        linker.func_wrap("env", "os_storage_flush_image", |_caller: Caller<'_, ()>, handle: u32| -> u32 {
            if crate::storage_host::flush_image(handle).is_ok() {
                1
            } else {
                0
            }
        }).map_err(|_| String::from("Failed to link os_storage_flush_image"))?;

        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| format!("Instantiate failed: {}", e))?
            .start(&mut store)
            .map_err(|e| format!("Start instance failed: {}", e))?;
        
        let init = instance.get_typed_func::<(), ()>(&store, "init_driver")
            .map_err(|_| String::from("Export 'init_driver' not found"))?;
        init.call(&mut store, ()).map_err(|e| format!("Init trapped: {}", e))?;

        Ok(WasmState { engine, store, instance })
    }

    fn upsert_module_state(&mut self, module_key: &str, state: WasmState) {
        if let Some(existing) = self
            .wasm_modules
            .iter_mut()
            .find(|module| module.module_key == module_key)
        {
            existing.state = state;
            return;
        }

        self.wasm_modules.push(LoadedWasmModule {
            module_key: String::from(module_key),
            state,
        });
    }

    pub fn unload_named_wasm_sandbox(&mut self, module_key: &str) -> bool {
        let Some(index) = self
            .wasm_modules
            .iter()
            .position(|module| module.module_key == module_key)
        else {
            return false;
        };

        self.wasm_modules.remove(index);
        true
    }

    pub fn unload_input_wasm_sandbox(&mut self, kind: HidDeviceKind) -> bool {
        self.unload_named_wasm_sandbox(input_module_key(kind))
    }

    pub fn invoke_named_wasm_entry(
        &mut self,
        module_key: &str,
        export: &str,
    ) -> Result<Option<i32>, String> {
        let Some(module) = self
            .wasm_modules
            .iter_mut()
            .find(|module| module.module_key == module_key)
        else {
            return Err(String::from("sandbox module is not loaded"));
        };

        let state = &mut module.state;

        if let Ok(func) = state.instance.get_typed_func::<(), i32>(&state.store, export) {
            let value = func
                .call(&mut state.store, ())
                .map_err(|e| format!("{} trapped: {}", export, e))?;
            return Ok(Some(value));
        }

        if let Ok(func) = state.instance.get_typed_func::<(), ()>(&state.store, export) {
            func.call(&mut state.store, ())
                .map_err(|e| format!("{} trapped: {}", export, e))?;
            return Ok(None);
        }

        Err(format!("export '{}' not found", export))
    }

    pub fn poll_wasm_network(&mut self) {
        for module in &mut self.wasm_modules {
            let state = &mut module.state;
            if let Ok(poll) = state.instance.get_typed_func::<(), ()>(&state.store, "poll_net") {
                let _ = poll.call(&mut state.store, ());
            }
        }
    }

    pub fn poll_wasm_input(&mut self) {
        for module in &mut self.wasm_modules {
            let state = &mut module.state;
            if let Ok(poll) = state.instance.get_typed_func::<(), ()>(&state.store, "poll_input_driver") {
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

fn input_module_key(kind: HidDeviceKind) -> &'static str {
    match kind {
        HidDeviceKind::Keyboard => "input:keyboard",
        HidDeviceKind::Mouse => "input:mouse",
    }
}
