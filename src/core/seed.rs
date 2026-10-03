// src/core/seed.rs
use crate::arch::x86_64::discovery::SystemIdentity;
use crate::input_handoff::HidDeviceKind;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

const INPUT_POLL_BUDGET_PER_TICK: usize = 4;
const CAPABILITY_POLL_BUDGET_PER_TICK: usize = 4;
const FRAME_PUMP_DEFAULT_HZ: u32 = 60;
const FRAME_PUMP_MAX_HZ: u32 = 1_000;
const FRAME_PUMP_BUDGET_PER_TICK: usize = 8;
const WASM_QUARANTINE_CONSECUTIVE_TRAPS: u32 = 3;
const WASM_QUARANTINE_SELFTEST_MODULE: &str = "selftest:wasm_quarantine";
const WASM_QUARANTINE_TRAP_FIXTURE: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03,
    0x03, 0x02, 0x00, 0x00, 0x07, 0x1a, 0x02, 0x0b, b'i', b'n', b'i', b't', b'_', b'd',
    b'r', b'i', b'v', b'e', b'r', 0x00, 0x00, 0x08, b'p', b'o', b'l', b'l', b'_', b'n',
    b'e', b't', 0x00, 0x01, 0x0a, 0x08, 0x02, 0x02, 0x00, 0x0b, 0x03, 0x00, 0x00, 0x0b,
];

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
    pub poll_count: u64,
    pub frame_pump_count: u64,
    pub frame_pump_hz: u32,
    pub frame_pump_interval_ticks: u64,
    pub next_frame_pump_tick: u64,
    pub trap_count: u64,
    pub consecutive_traps: u32,
    pub quarantined: bool,
    pub last_active_tick: u64,
    pub last_error: Option<String>,
    pub byte_len: usize,
}

#[derive(Clone, Debug)]
pub struct WasmModuleHealth {
    pub module_key: String,
    pub poll_count: u64,
    pub frame_pump_count: u64,
    pub frame_pump_hz: u32,
    pub trap_count: u64,
    pub consecutive_traps: u32,
    pub quarantined: bool,
    pub last_active_tick: u64,
    pub byte_len: usize,
    pub loaded: bool,
    pub last_error: Option<String>,
}

lazy_static! {
    static ref WASM_HEALTH_SNAPSHOT: Mutex<Vec<WasmModuleHealth>> = Mutex::new(Vec::new());
}

fn note_wasm_module_success(module: &mut LoadedWasmModule, tick: u64) {
    module.consecutive_traps = 0;
    module.last_active_tick = tick;
    module.last_error = None;
}

fn note_wasm_module_trap(module: &mut LoadedWasmModule, tick: u64, error: String) {
    module.trap_count = module.trap_count.saturating_add(1);
    module.consecutive_traps = module.consecutive_traps.saturating_add(1);
    module.last_active_tick = tick;
    if module.consecutive_traps >= WASM_QUARANTINE_CONSECUTIVE_TRAPS {
        module.quarantined = true;
        module.last_error = Some(format!(
            "quarantined after {} consecutive traps: {}",
            module.consecutive_traps, error
        ));
    } else {
        module.last_error = Some(error);
    }
}

pub struct OpenRhizaSeed {
    pub identity: SystemIdentity,
    pub log_buffer: Vec<String>,
    pub wasm_modules: Vec<LoadedWasmModule>,
    next_network_poll_index: usize,
    next_input_poll_index: usize,
    last_health_refresh_tick: u64,
}

impl OpenRhizaSeed {
    pub fn new(identity: SystemIdentity) -> Self {
        Self {
            identity,
            log_buffer: Vec::new(),
            wasm_modules: Vec::new(),
            next_network_poll_index: 0,
            next_input_poll_index: 0,
            last_health_refresh_tick: 0,
        }
    }

    pub fn execute_wasm_sandbox(&mut self, wasm_bytes: &[u8]) -> ExecutionResult {
        self.execute_named_wasm_sandbox("runtime:default", wasm_bytes)
    }

    pub fn execute_named_wasm_sandbox(
        &mut self,
        module_key: &str,
        wasm_bytes: &[u8],
    ) -> ExecutionResult {
        match self.instantiate_wasm(wasm_bytes) {
            Ok(state) => {
                self.upsert_module_state(module_key, state, wasm_bytes.len());
                self.refresh_health_snapshot();
                let msg =
                    format!("Wasm Execution Success! sandbox module initialized. Engine running.");
                self.log_buffer.push(msg.clone());
                ExecutionResult::Success(msg)
            }
            Err(e) => {
                let err_msg = format!("Wasm Sandbox Trap (Panic): {}", e);
                self.log_buffer.push(err_msg.clone());
                self.record_module_trap(module_key, err_msg.clone());
                ExecutionResult::Panic(err_msg)
            }
        }
    }

    pub fn execute_input_wasm_sandbox(
        &mut self,
        kind: HidDeviceKind,
        wasm_bytes: &[u8],
    ) -> ExecutionResult {
        self.execute_named_wasm_sandbox(input_module_key(kind), wasm_bytes)
    }

    fn instantiate_wasm(&mut self, wasm_bytes: &[u8]) -> Result<WasmState, String> {
        crate::driver_host::install_identity(&self.identity);

        let engine = Engine::default();
        let module =
            Module::new(&engine, wasm_bytes).map_err(|e| format!("Failed to parse Wasm: {}", e))?;
        let mut store = Store::new(&engine, ());
        let mut linker = <Linker<()>>::new(&engine);

        linker
            .func_wrap(
                "env",
                "read_mmio",
                |_caller: Caller<'_, ()>, addr: u32| -> u32 {
                    let _ = addr;
                    0
                },
            )
            .map_err(|_| String::from("Failed to link read_mmio"))?;

        linker
            .func_wrap(
                "env",
                "write_mmio",
                |_caller: Caller<'_, ()>, addr: u32, val: u32| {
                    let _ = (addr, val);
                },
            )
            .map_err(|_| String::from("Failed to link write_mmio"))?;

        linker
            .func_wrap("env", "alloc_dma_page", |_caller: Caller<'_, ()>| -> u32 {
                0
            })
            .map_err(|_| String::from("Failed to link alloc_dma_page"))?;

        linker
            .func_wrap(
                "env",
                "os_rx_packet",
                |caller: Caller<'_, ()>, ptr: u32, len: u32| {
                    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    {
                        let mut packet = alloc::vec![0u8; len as usize];
                        if memory.read(&caller, ptr as usize, &mut packet).is_ok() {
                            crate::net::RX_QUEUE.lock().push(packet);
                        }
                    }
                },
            )
            .map_err(|_| String::from("Failed to link os_rx_packet"))?;

        linker
            .func_wrap(
                "env",
                "os_fetch_tx_packet",
                |mut caller: Caller<'_, ()>, ptr: u32, max_len: u32| -> u32 {
                    let mut tx_queue = crate::net::TX_QUEUE.lock();
                    if !tx_queue.is_empty() {
                        let packet = tx_queue.remove(0);
                        if (packet.len() as u32) <= max_len {
                            if let Some(memory) =
                                caller.get_export("memory").and_then(|e| e.into_memory())
                            {
                                if memory.write(&mut caller, ptr as usize, &packet).is_ok() {
                                    return packet.len() as u32;
                                }
                            }
                        }
                    }
                    0
                },
            )
            .map_err(|_| String::from("Failed to link os_fetch_tx_packet"))?;

        linker.func_wrap("env", "os_net_set_sandbox_ethernet_stage", |_caller: Caller<'_, ()>, stage: u32| -> u32 {
            if stage > 1 {
                crate::println!(
                    "[Net Driver Runtime] sandbox-active promotion denied: legacy stage import only allows fallback/candidate."
                );
                return 0;
            }
            crate::net::set_driver_stage(crate::net::NetDriverStage::from(stage));
            1
        }).map_err(|_| String::from("Failed to link os_net_set_sandbox_ethernet_stage"))?;

        linker
            .func_wrap(
                "env",
                "os_net_get_sandbox_ethernet_stage",
                |_caller: Caller<'_, ()>| -> u32 {
                    match crate::net::driver_stage() {
                        crate::net::NetDriverStage::NativeFallback => 0,
                        crate::net::NetDriverStage::SandboxCandidate => 1,
                        crate::net::NetDriverStage::SandboxActive => 2,
                    }
                },
            )
            .map_err(|_| String::from("Failed to link os_net_get_sandbox_ethernet_stage"))?;

        linker
            .func_wrap(
                "env",
                "os_net_report_sandbox_ethernet_health",
                |caller: Caller<'_, ()>, id_ptr: u32, id_len: u32, flags: u32| -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if id_len == 0 || id_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; id_len as usize];
                    if memory.read(&caller, id_ptr as usize, &mut bytes).is_err() {
                        return 0;
                    }
                    let Ok(driver_id) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::net::report_sandbox_ethernet_health(driver_id, flags)
                },
            )
            .map_err(|_| String::from("Failed to link os_net_report_sandbox_ethernet_health"))?;

        linker
            .func_wrap(
                "env",
                "os_net_request_sandbox_ethernet_promotion",
                |caller: Caller<'_, ()>, id_ptr: u32, id_len: u32| -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if id_len == 0 || id_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; id_len as usize];
                    if memory.read(&caller, id_ptr as usize, &mut bytes).is_err() {
                        return 0;
                    }
                    let Ok(driver_id) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::net::request_sandbox_ethernet_promotion(driver_id)
                },
            )
            .map_err(|_| {
                String::from("Failed to link os_net_request_sandbox_ethernet_promotion")
            })?;

        linker
            .func_wrap(
                "env",
                "os_net_sandbox_ethernet_heartbeat",
                |caller: Caller<'_, ()>, id_ptr: u32, id_len: u32, flags: u32| -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if id_len == 0 || id_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; id_len as usize];
                    if memory.read(&caller, id_ptr as usize, &mut bytes).is_err() {
                        return 0;
                    }
                    let Ok(driver_id) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::net::sandbox_ethernet_heartbeat(driver_id, flags)
                },
            )
            .map_err(|_| String::from("Failed to link os_net_sandbox_ethernet_heartbeat"))?;

        linker
            .func_wrap(
                "env",
                "os_fetch_hid_packet",
                |mut caller: Caller<'_, ()>, ptr: u32, max_len: u32| -> u32 {
                    let required_len = 4 + crate::input_handoff::MAX_HID_REPORT_BYTES;
                    if max_len < required_len as u32 {
                        return 0;
                    }
                    let Some(packet) = crate::input_handoff::fetch_hid_packet() else {
                        return 0;
                    };

                    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    {
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
                },
            )
            .map_err(|_| String::from("Failed to link os_fetch_hid_packet"))?;

        linker
            .func_wrap(
                "env",
                "os_fetch_hid_packet_for_kind",
                |mut caller: Caller<'_, ()>, kind: u32, ptr: u32, max_len: u32| -> u32 {
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

                    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    {
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
                },
            )
            .map_err(|_| String::from("Failed to link os_fetch_hid_packet_for_kind"))?;

        linker
            .func_wrap(
                "env",
                "os_emit_input_event",
                |_caller: Caller<'_, ()>, kind: u32, a: i32, b: i32, c: i32| {
                    crate::input_handoff::emit_input_event_from_wasm(kind, a, b, c);
                },
            )
            .map_err(|_| String::from("Failed to link os_emit_input_event"))?;

        linker
            .func_wrap(
                "env",
                "os_set_input_driver_mode",
                |_caller: Caller<'_, ()>, mode: u32| {
                    crate::input_handoff::set_routing_mode_from_wasm(mode);
                },
            )
            .map_err(|_| String::from("Failed to link os_set_input_driver_mode"))?;

        linker
            .func_wrap(
                "env",
                "os_set_input_driver_active",
                |_caller: Caller<'_, ()>, active: u32| {
                    let active = active != 0;
                    for kind in [
                        crate::input_handoff::HidDeviceKind::Keyboard,
                        crate::input_handoff::HidDeviceKind::Mouse,
                    ] {
                        if !crate::input_runtime::allow_wasm_active_request(kind, active) {
                            crate::println!(
                                "[Input Runtime] sandbox {} active request denied until validation/promote.",
                                crate::input_runtime::kind_label(kind)
                            );
                        }
                    }
                },
            )
            .map_err(|_| String::from("Failed to link os_set_input_driver_active"))?;

        linker
            .func_wrap(
                "env",
                "os_set_input_driver_mode_for_kind",
                |_caller: Caller<'_, ()>, kind: u32, mode: u32| {
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
                },
            )
            .map_err(|_| String::from("Failed to link os_set_input_driver_mode_for_kind"))?;

        linker
            .func_wrap(
                "env",
                "os_set_input_driver_active_for_kind",
                |_caller: Caller<'_, ()>, kind: u32, active: u32| {
                    let kind = match kind {
                        1 => crate::input_handoff::HidDeviceKind::Keyboard,
                        2 => crate::input_handoff::HidDeviceKind::Mouse,
                        _ => return,
                    };
                    if !crate::input_runtime::allow_wasm_active_request(kind, active != 0) {
                        crate::println!(
                            "[Input Runtime] sandbox {} active request denied until validation/promote.",
                            crate::input_runtime::kind_label(kind)
                        );
                    }
                },
            )
            .map_err(|_| String::from("Failed to link os_set_input_driver_active_for_kind"))?;

        linker
            .func_wrap(
                "env",
                "os_input_report_driver_health",
                |caller: Caller<'_, ()>, kind: u32, id_ptr: u32, id_len: u32, flags: u32| -> u32 {
                    let kind = match kind {
                        1 => crate::input_handoff::HidDeviceKind::Keyboard,
                        2 => crate::input_handoff::HidDeviceKind::Mouse,
                        _ => return 0,
                    };
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if id_len == 0 || id_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; id_len as usize];
                    if memory.read(&caller, id_ptr as usize, &mut bytes).is_err() {
                        return 0;
                    }
                    let Ok(driver_id) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::input_runtime::report_sandbox_input_health(kind, driver_id, flags)
                },
            )
            .map_err(|_| String::from("Failed to link os_input_report_driver_health"))?;

        linker
            .func_wrap(
                "env",
                "os_input_request_driver_promotion",
                |caller: Caller<'_, ()>, kind: u32, id_ptr: u32, id_len: u32| -> u32 {
                    let kind = match kind {
                        1 => crate::input_handoff::HidDeviceKind::Keyboard,
                        2 => crate::input_handoff::HidDeviceKind::Mouse,
                        _ => return 0,
                    };
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if id_len == 0 || id_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; id_len as usize];
                    if memory.read(&caller, id_ptr as usize, &mut bytes).is_err() {
                        return 0;
                    }
                    let Ok(driver_id) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::input_runtime::request_sandbox_input_promotion(kind, driver_id)
                },
            )
            .map_err(|_| String::from("Failed to link os_input_request_driver_promotion"))?;

        linker
            .func_wrap(
                "env",
                "os_log",
                |caller: Caller<'_, ()>, ptr: u32, len: u32| {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return;
                    };

                    let mut bytes = alloc::vec![0u8; len as usize];
                    if memory.read(&caller, ptr as usize, &mut bytes).is_err() {
                        return;
                    }

                    if let Ok(text) = core::str::from_utf8(&bytes) {
                        crate::result_println!("{}", text.trim_end_matches('\n'));
                    }
                },
            )
            .map_err(|_| String::from("Failed to link os_log"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_activate_binding",
                |caller: Caller<'_, ()>,
                 key_ptr: u32,
                 key_len: u32,
                 id_ptr: u32,
                 id_len: u32|
                 -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };

                    let mut key_bytes = alloc::vec![0u8; key_len.min(96) as usize];
                    let mut id_bytes = alloc::vec![0u8; id_len.min(96) as usize];
                    if memory
                        .read(&caller, key_ptr as usize, &mut key_bytes)
                        .is_err()
                    {
                        return 0;
                    }
                    if memory
                        .read(&caller, id_ptr as usize, &mut id_bytes)
                        .is_err()
                    {
                        return 0;
                    }

                    let Ok(match_key) = core::str::from_utf8(&key_bytes) else {
                        return 0;
                    };
                    let Ok(driver_id) = core::str::from_utf8(&id_bytes) else {
                        return 0;
                    };
                    if match_key.trim().is_empty() || driver_id.trim().is_empty() {
                        return 0;
                    }

                    crate::driver_runtime::activate_binding(
                        match_key.trim(),
                        driver_id.trim(),
                        "sandbox-driver-skill",
                    );
                    1
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_activate_binding"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_claim_device",
                |caller: Caller<'_, ()>,
                 key_ptr: u32,
                 key_len: u32,
                 id_ptr: u32,
                 id_len: u32,
                 caps: u32|
                 -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };

                    let mut key_bytes = alloc::vec![0u8; key_len.min(96) as usize];
                    let mut id_bytes = alloc::vec![0u8; id_len.min(96) as usize];
                    if memory
                        .read(&caller, key_ptr as usize, &mut key_bytes)
                        .is_err()
                    {
                        return 0;
                    }
                    if memory
                        .read(&caller, id_ptr as usize, &mut id_bytes)
                        .is_err()
                    {
                        return 0;
                    }

                    let Ok(match_key) = core::str::from_utf8(&key_bytes) else {
                        return 0;
                    };
                    let Ok(driver_id) = core::str::from_utf8(&id_bytes) else {
                        return 0;
                    };

                    crate::driver_host::claim_device(match_key.trim(), driver_id.trim(), caps)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_claim_device"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_claim_mmio_device",
                |caller: Caller<'_, ()>,
                 key_ptr: u32,
                 key_len: u32,
                 id_ptr: u32,
                 id_len: u32,
                 caps: u32|
                 -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };

                    let mut key_bytes = alloc::vec![0u8; key_len.min(96) as usize];
                    let mut id_bytes = alloc::vec![0u8; id_len.min(96) as usize];
                    if memory
                        .read(&caller, key_ptr as usize, &mut key_bytes)
                        .is_err()
                    {
                        return 0;
                    }
                    if memory
                        .read(&caller, id_ptr as usize, &mut id_bytes)
                        .is_err()
                    {
                        return 0;
                    }

                    let Ok(match_key) = core::str::from_utf8(&key_bytes) else {
                        return 0;
                    };
                    let Ok(driver_id) = core::str::from_utf8(&id_bytes) else {
                        return 0;
                    };

                    crate::driver_host::claim_mmio_device(match_key.trim(), driver_id.trim(), caps)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_claim_mmio_device"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_release_device",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    crate::driver_host::release_device(handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_release_device"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_handle_heartbeat",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    crate::driver_host::handle_heartbeat(handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_handle_heartbeat"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_mmio_read32",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32| -> u32 {
                    crate::driver_host::mmio_read32(handle, offset)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_mmio_read32"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_mmio_write32",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32, value: u32| -> u32 {
                    crate::driver_host::mmio_write32(handle, offset, value)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_mmio_write32"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_pio_read8",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32| -> u32 {
                    crate::driver_host::pio_read8(handle, offset)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_pio_read8"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_pio_read16",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32| -> u32 {
                    crate::driver_host::pio_read16(handle, offset)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_pio_read16"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_pio_read32",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32| -> u32 {
                    crate::driver_host::pio_read32(handle, offset)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_pio_read32"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_pio_write8",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32, value: u32| -> u32 {
                    crate::driver_host::pio_write8(handle, offset, value)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_pio_write8"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_pio_write16",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32, value: u32| -> u32 {
                    crate::driver_host::pio_write16(handle, offset, value)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_pio_write16"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_pio_write32",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32, value: u32| -> u32 {
                    crate::driver_host::pio_write32(handle, offset, value)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_pio_write32"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_pci_config_read32",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32| -> u32 {
                    crate::driver_host::pci_config_read32(handle, offset)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_pci_config_read32"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_pci_config_write32",
                |_caller: Caller<'_, ()>, handle: u32, offset: u32, value: u32| -> u32 {
                    crate::driver_host::pci_config_write32(handle, offset, value)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_pci_config_write32"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_dma_alloc",
                |_caller: Caller<'_, ()>, handle: u32, byte_len: u32, align: u32| -> u32 {
                    crate::driver_host::dma_alloc(handle, byte_len, align)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_dma_alloc"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_dma_phys",
                |_caller: Caller<'_, ()>, handle: u32, dma_handle: u32| -> u32 {
                    crate::driver_host::dma_phys(handle, dma_handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_dma_phys"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_dma_len",
                |_caller: Caller<'_, ()>, handle: u32, dma_handle: u32| -> u32 {
                    crate::driver_host::dma_len(handle, dma_handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_dma_len"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_dma_write",
                |caller: Caller<'_, ()>,
                 handle: u32,
                 dma_handle: u32,
                 dma_offset: u32,
                 ptr: u32,
                 len: u32|
                 -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    let mut bytes = alloc::vec![0u8; len.min(65536) as usize];
                    if memory.read(&caller, ptr as usize, &mut bytes).is_err() {
                        return 0;
                    }
                    crate::driver_host::dma_write(handle, dma_handle, dma_offset, &bytes)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_dma_write"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_dma_read",
                |mut caller: Caller<'_, ()>,
                 handle: u32,
                 dma_handle: u32,
                 dma_offset: u32,
                 ptr: u32,
                 len: u32|
                 -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    let mut bytes = alloc::vec![0u8; len.min(65536) as usize];
                    let copied =
                        crate::driver_host::dma_read(handle, dma_handle, dma_offset, &mut bytes);
                    if copied == 0 {
                        return 0;
                    }
                    if memory
                        .write(&mut caller, ptr as usize, &bytes[..copied as usize])
                        .is_err()
                    {
                        return 0;
                    }
                    copied
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_dma_read"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_irq_poll",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    crate::driver_host::irq_poll(handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_irq_poll"))?;

        linker
            .func_wrap(
                "env",
                "os_driver_irq_ack",
                |_caller: Caller<'_, ()>, handle: u32, mask: u32| -> u32 {
                    crate::driver_host::irq_ack(handle, mask)
                },
            )
            .map_err(|_| String::from("Failed to link os_driver_irq_ack"))?;

        linker
            .func_wrap(
                "env",
                "os_virtio_prepare_live_gate",
                |_caller: Caller<'_, ()>,
                 handle: u32,
                 expected_device_id: u32,
                 queue_index: u32,
                 dma_handle: u32,
                 queue_size: u32|
                 -> u32 {
                    crate::driver_host::virtio_prepare_live_gate(
                        handle,
                        expected_device_id,
                        queue_index,
                        dma_handle,
                        queue_size,
                    )
                },
            )
            .map_err(|_| String::from("Failed to link os_virtio_prepare_live_gate"))?;

        linker
            .func_wrap(
                "env",
                "os_virtio_record_feature_negotiation",
                |_caller: Caller<'_, ()>,
                 gate: u32,
                 device_features: u32,
                 driver_features: u32,
                 status: u32|
                 -> u32 {
                    crate::driver_host::virtio_record_feature_negotiation(
                        gate,
                        device_features,
                        driver_features,
                        status,
                    )
                },
            )
            .map_err(|_| String::from("Failed to link os_virtio_record_feature_negotiation"))?;

        linker
            .func_wrap(
                "env",
                "os_virtio_record_queue_programming",
                |_caller: Caller<'_, ()>,
                 gate: u32,
                 queue_index: u32,
                 queue_size: u32,
                 desc_phys: u32,
                 driver_phys: u32,
                 device_phys: u32|
                 -> u32 {
                    crate::driver_host::virtio_record_queue_programming(
                        gate,
                        queue_index,
                        queue_size,
                        desc_phys,
                        driver_phys,
                        device_phys,
                    )
                },
            )
            .map_err(|_| String::from("Failed to link os_virtio_record_queue_programming"))?;

        linker
            .func_wrap(
                "env",
                "os_virtio_record_queue_notify",
                |_caller: Caller<'_, ()>, gate: u32, queue_index: u32| -> u32 {
                    crate::driver_host::virtio_record_queue_notify(gate, queue_index)
                },
            )
            .map_err(|_| String::from("Failed to link os_virtio_record_queue_notify"))?;

        linker
            .func_wrap(
                "env",
                "os_virtio_record_irq_completion",
                |_caller: Caller<'_, ()>, gate: u32, irq_status: u32, ack_mask: u32| -> u32 {
                    crate::driver_host::virtio_record_irq_completion(gate, irq_status, ack_mask)
                },
            )
            .map_err(|_| String::from("Failed to link os_virtio_record_irq_completion"))?;

        linker
            .func_wrap(
                "env",
                "os_virtio_commit_live_gate",
                |_caller: Caller<'_, ()>, gate: u32| -> u32 {
                    crate::driver_host::virtio_commit_live_gate(gate)
                },
            )
            .map_err(|_| String::from("Failed to link os_virtio_commit_live_gate"))?;

        linker
            .func_wrap(
                "env",
                "os_virtio_rollback_live_gate",
                |_caller: Caller<'_, ()>, gate: u32| -> u32 {
                    crate::driver_host::virtio_rollback_live_gate(gate)
                },
            )
            .map_err(|_| String::from("Failed to link os_virtio_rollback_live_gate"))?;

        linker
            .func_wrap(
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

        linker
            .func_wrap(
                "env",
                "os_set_gui_session_state",
                |_caller: Caller<'_, ()>, state: u32| {
                    crate::display::set_gui_session_state_from_wasm(state);
                },
            )
            .map_err(|_| String::from("Failed to link os_set_gui_session_state"))?;

        linker
            .func_wrap(
                "env",
                "os_set_display_session_target",
                |_caller: Caller<'_, ()>, target: u32| {
                    crate::display::set_display_session_target_from_wasm(target);
                },
            )
            .map_err(|_| String::from("Failed to link os_set_display_session_target"))?;

        linker
            .func_wrap(
                "env",
                "os_set_display_validation_state",
                |_caller: Caller<'_, ()>, state: u32| {
                    crate::display::set_display_validation_state_from_wasm(state);
                },
            )
            .map_err(|_| String::from("Failed to link os_set_display_validation_state"))?;

        linker
            .func_wrap(
                "env",
                "os_set_display_overlay_line",
                |caller: Caller<'_, ()>, slot: u32, ptr: u32, len: u32| {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
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

        linker
            .func_wrap(
                "env",
                "os_gui_select_session",
                |_caller: Caller<'_, ()>, session: u32| {
                    crate::display::select_gui_session_from_wasm(session);
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_select_session"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_focus_object",
                |_caller: Caller<'_, ()>, target: u32| {
                    crate::display::focus_gui_object_from_wasm(target);
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_focus_object"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_set_object_style",
                |_caller: Caller<'_, ()>, handle: u32, style: u32| {
                    crate::display::set_gui_style_code_from_wasm(handle, style);
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_set_object_style"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_set_object_bounds",
                |_caller: Caller<'_, ()>, handle: u32, x: u32, y: u32, width: u32, height: u32| {
                    crate::display::set_gui_bounds_from_wasm(handle, x, y, width, height);
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_set_object_bounds"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_set_object_interaction",
                |_caller: Caller<'_, ()>, handle: u32, interaction: u32| {
                    crate::display::set_gui_interaction_code_from_wasm(handle, interaction);
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_set_object_interaction"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_get_object_bounds",
                |_caller: Caller<'_, ()>, handle: u32, component: u32| -> u32 {
                    crate::display::gui_object_bounds_component_from_wasm(handle, component)
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_get_object_bounds"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_reset_object_mutations",
                |_caller: Caller<'_, ()>, handle: u32| {
                    crate::display::reset_gui_mutations_from_wasm(handle);
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_reset_object_mutations"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_claim_shell_policy",
                |caller: Caller<'_, ()>, owner_ptr: u32, owner_len: u32| -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if owner_len == 0 || owner_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; owner_len as usize];
                    if memory
                        .read(&caller, owner_ptr as usize, &mut bytes)
                        .is_err()
                    {
                        return 0;
                    }
                    let Ok(owner) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::display::claim_gui_shell_policy(owner)
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_claim_shell_policy"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_shell_policy_heartbeat",
                |caller: Caller<'_, ()>, owner_ptr: u32, owner_len: u32| -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if owner_len == 0 || owner_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; owner_len as usize];
                    if memory
                        .read(&caller, owner_ptr as usize, &mut bytes)
                        .is_err()
                    {
                        return 0;
                    }
                    let Ok(owner) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::display::gui_shell_policy_heartbeat(owner)
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_shell_policy_heartbeat"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_set_object_label",
                |caller: Caller<'_, ()>, handle: u32, ptr: u32, len: u32| {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
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

        linker
            .func_wrap("env", "os_now_ms", |_caller: Caller<'_, ()>| -> u32 {
                let ticks = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
                let ms = ticks.saturating_mul(1000) / crate::task::timer::TICKS_PER_SECOND.max(1);
                ms.min(u32::MAX as u64) as u32
            })
            .map_err(|_| String::from("Failed to link os_now_ms"))?;

        linker
            .func_wrap("env", "os_gui_release_frame", |_caller: Caller<'_, ()>| {
                crate::display::release_sandbox_frame();
            })
            .map_err(|_| String::from("Failed to link os_gui_release_frame"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_frame_close_requested",
                |_caller: Caller<'_, ()>| -> u32 {
                    crate::display::take_sandbox_frame_close_request_from_wasm()
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_frame_close_requested"))?;

        linker
            .func_wrap(
                "env",
                "os_frame_resource_create",
                |_caller: Caller<'_, ()>, width: u32, height: u32, format: u32, target_hz: u32| {
                    crate::display::create_frame_resource(width, height, format, target_hz)
                },
            )
            .map_err(|_| String::from("Failed to link os_frame_resource_create"))?;

        linker
            .func_wrap(
                "env",
                "os_frame_resource_metric",
                |_caller: Caller<'_, ()>, handle: u32, metric: u32| -> u32 {
                    crate::display::frame_resource_metric(handle, metric)
                },
            )
            .map_err(|_| String::from("Failed to link os_frame_resource_metric"))?;

        linker
            .func_wrap(
                "env",
                "os_frame_resource_submit_rgba",
                |caller: Caller<'_, ()>,
                 handle: u32,
                 ptr: u32,
                 width: u32,
                 height: u32,
                 pitch_bytes: u32|
                 -> u32 {
                    if handle == 0 || width == 0 || height == 0 || width > 640 || height > 480 {
                        return 0;
                    }
                    let expected_pitch = width.saturating_mul(4);
                    if pitch_bytes != expected_pitch {
                        return 0;
                    }
                    let total_bytes = pitch_bytes.saturating_mul(height);
                    if total_bytes == 0 || total_bytes > 640 * 480 * 4 {
                        return 0;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    let data = memory.data(&caller);
                    let start = ptr as usize;
                    let end = start.saturating_add(total_bytes as usize);
                    if end > data.len() {
                        return 0;
                    }
                    crate::display::submit_frame_resource_rgba(
                        handle,
                        width,
                        height,
                        &data[start..end],
                    )
                },
            )
            .map_err(|_| String::from("Failed to link os_frame_resource_submit_rgba"))?;

        linker
            .func_wrap(
                "env",
                "os_frame_resource_submit_rgba_seq",
                |caller: Caller<'_, ()>,
                 handle: u32,
                 ptr: u32,
                 width: u32,
                 height: u32,
                 pitch_bytes: u32,
                 frame_sequence: u32|
                 -> u32 {
                    if handle == 0 || width == 0 || height == 0 || width > 640 || height > 480 {
                        return 0;
                    }
                    let expected_pitch = width.saturating_mul(4);
                    if pitch_bytes != expected_pitch {
                        return 0;
                    }
                    let total_bytes = pitch_bytes.saturating_mul(height);
                    if total_bytes == 0 || total_bytes > 640 * 480 * 4 {
                        return 0;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    let data = memory.data(&caller);
                    let start = ptr as usize;
                    let end = start.saturating_add(total_bytes as usize);
                    if end > data.len() {
                        return 0;
                    }
                    crate::display::submit_frame_resource_rgba_with_sequence(
                        handle,
                        width,
                        height,
                        &data[start..end],
                        frame_sequence,
                    )
                },
            )
            .map_err(|_| String::from("Failed to link os_frame_resource_submit_rgba_seq"))?;

        linker
            .func_wrap(
                "env",
                "os_frame_resource_submit_indexed8_seq",
                |caller: Caller<'_, ()>,
                 handle: u32,
                 pixels_ptr: u32,
                 width: u32,
                 height: u32,
                 pitch_bytes: u32,
                 palette_ptr: u32,
                 palette_entries: u32,
                 frame_sequence: u32|
                 -> u32 {
                    if handle == 0
                        || width == 0
                        || height == 0
                        || width > 640
                        || height > 480
                        || pitch_bytes < width
                        || palette_entries < 256
                    {
                        return 0;
                    }
                    let total_bytes = pitch_bytes.saturating_mul(height);
                    if total_bytes == 0 || total_bytes > 640 * 480 {
                        return 0;
                    }
                    let palette_bytes = palette_entries.saturating_mul(4);
                    if palette_bytes < 1024 || palette_bytes > 4096 {
                        return 0;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    let data = memory.data(&caller);
                    let pixels_start = pixels_ptr as usize;
                    let pixels_end = pixels_start.saturating_add(total_bytes as usize);
                    let palette_start = palette_ptr as usize;
                    let palette_end = palette_start.saturating_add(1024);
                    if pixels_end > data.len() || palette_end > data.len() {
                        return 0;
                    }
                    crate::display::submit_frame_resource_indexed8_with_sequence(
                        handle,
                        width,
                        height,
                        &data[pixels_start..pixels_end],
                        pitch_bytes,
                        &data[palette_start..palette_end],
                        frame_sequence,
                    )
                },
            )
            .map_err(|_| String::from("Failed to link os_frame_resource_submit_indexed8_seq"))?;

        linker
            .func_wrap(
                "env",
                "os_gui_blit_rgba_frame",
                |caller: Caller<'_, ()>, ptr: u32, width: u32, height: u32| -> u32 {
                    if width == 0 || height == 0 || width > 640 || height > 480 {
                        return 0;
                    }
                    let total_bytes = width.saturating_mul(height).saturating_mul(4);
                    if total_bytes == 0 || total_bytes > 640 * 480 * 4 {
                        return 0;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    let data = memory.data(&caller);
                    let start = ptr as usize;
                    let end = start.saturating_add(total_bytes as usize);
                    if end > data.len() {
                        return 0;
                    }
                    crate::display::blit_rgba_frame_from_wasm("", width, height, &data[start..end])
                },
            )
            .map_err(|_| String::from("Failed to link os_gui_blit_rgba_frame"))?;

        linker
            .func_wrap(
                "env",
                "os_file_size_root_8_3",
                |caller: Caller<'_, ()>, name_ptr: u32| -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    let mut name = [0u8; 11];
                    if memory.read(&caller, name_ptr as usize, &mut name).is_err() {
                        return 0;
                    }
                    crate::storage::file_size_from_secondary_fat16(&[name])
                        .map(|len| len.min(u32::MAX as usize) as u32)
                        .unwrap_or(0)
                },
            )
            .map_err(|_| String::from("Failed to link os_file_size_root_8_3"))?;

        linker
            .func_wrap(
                "env",
                "os_file_read_root_8_3",
                |mut caller: Caller<'_, ()>,
                 name_ptr: u32,
                 offset: u32,
                 ptr: u32,
                 max_len: u32|
                 -> u32 {
                    if max_len == 0 || max_len > 4096 {
                        return 0;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    let mut name = [0u8; 11];
                    if memory.read(&caller, name_ptr as usize, &mut name).is_err() {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; max_len as usize];
                    let Some(read) = crate::storage::read_named_file_range_from_secondary_fat16(
                        &[name],
                        offset as usize,
                        &mut bytes,
                    ) else {
                        return 0;
                    };
                    if read == 0 {
                        return 0;
                    }
                    if memory
                        .write(&mut caller, ptr as usize, &bytes[..read])
                        .is_ok()
                    {
                        read as u32
                    } else {
                        0
                    }
                },
            )
            .map_err(|_| String::from("Failed to link os_file_read_root_8_3"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_list_images",
                |_caller: Caller<'_, ()>| -> u32 { crate::storage_host::list_images() },
            )
            .map_err(|_| String::from("Failed to link os_storage_list_images"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_set_sandbox_block_stage",
                |_caller: Caller<'_, ()>, stage: u32| -> u32 {
                    if stage > 1 {
                        crate::println!(
                            "[Storage Runtime] sandbox-active promotion denied: legacy stage import only allows fallback/candidate."
                        );
                        return 0;
                    }
                    crate::storage_host::set_driver_stage(
                        crate::storage_host::StorageDriverStage::from(stage),
                    );
                    1
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_set_sandbox_block_stage"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_get_sandbox_block_stage",
                |_caller: Caller<'_, ()>| -> u32 {
                    match crate::storage_host::driver_stage() {
                        crate::storage_host::StorageDriverStage::NativeFallback => 0,
                        crate::storage_host::StorageDriverStage::SandboxCandidate => 1,
                        crate::storage_host::StorageDriverStage::SandboxActive => 2,
                    }
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_get_sandbox_block_stage"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_report_sandbox_block_health",
                |caller: Caller<'_, ()>, id_ptr: u32, id_len: u32, flags: u32| -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if id_len == 0 || id_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; id_len as usize];
                    if memory.read(&caller, id_ptr as usize, &mut bytes).is_err() {
                        return 0;
                    }
                    let Ok(driver_id) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::storage_host::report_sandbox_block_health(driver_id, flags)
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_report_sandbox_block_health"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_request_sandbox_block_promotion",
                |caller: Caller<'_, ()>, id_ptr: u32, id_len: u32| -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if id_len == 0 || id_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; id_len as usize];
                    if memory.read(&caller, id_ptr as usize, &mut bytes).is_err() {
                        return 0;
                    }
                    let Ok(driver_id) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::storage_host::request_sandbox_block_promotion(driver_id)
                },
            )
            .map_err(|_| {
                String::from("Failed to link os_storage_request_sandbox_block_promotion")
            })?;

        linker
            .func_wrap(
                "env",
                "os_storage_sandbox_block_heartbeat",
                |caller: Caller<'_, ()>, id_ptr: u32, id_len: u32, flags: u32| -> u32 {
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };
                    if id_len == 0 || id_len > 128 {
                        return 0;
                    }
                    let mut bytes = alloc::vec![0u8; id_len as usize];
                    if memory.read(&caller, id_ptr as usize, &mut bytes).is_err() {
                        return 0;
                    }
                    let Ok(driver_id) = core::str::from_utf8(bytes.as_slice()) else {
                        return 0;
                    };
                    crate::storage_host::sandbox_block_heartbeat(driver_id, flags)
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_sandbox_block_heartbeat"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_open_image",
                |_caller: Caller<'_, ()>, index: u32| -> u32 {
                    crate::storage_host::open_image(index)
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_open_image"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_get_block_count",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    crate::storage_host::block_count(handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_get_block_count"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_get_filesystem_block_count",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    crate::storage_host::filesystem_block_count(handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_get_filesystem_block_count"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_get_scratch_start_lba",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    crate::storage_host::scratch_start_lba(handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_get_scratch_start_lba"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_get_scratch_block_count",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    crate::storage_host::scratch_block_count(handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_get_scratch_block_count"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_is_writable",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    crate::storage_host::writable(handle) as u32
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_is_writable"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_get_fs_hint",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    crate::storage_host::fs_hint_code(handle)
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_get_fs_hint"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_read_blocks",
                |mut caller: Caller<'_, ()>,
                 handle: u32,
                 lba: u32,
                 count: u32,
                 ptr: u32,
                 max_len: u32|
                 -> u32 {
                    let total_bytes = count.saturating_mul(crate::storage_host::STORAGE_BLOCK_SIZE);
                    if total_bytes == 0 || max_len < total_bytes {
                        return 0;
                    }

                    let mut bytes = alloc::vec![0u8; total_bytes as usize];
                    if crate::storage_host::read_blocks(handle, lba, count, &mut bytes).is_err() {
                        return 0;
                    }

                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return 0;
                    };

                    if memory.write(&mut caller, ptr as usize, &bytes).is_ok() {
                        total_bytes
                    } else {
                        0
                    }
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_read_blocks"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_write_blocks",
                |caller: Caller<'_, ()>,
                 handle: u32,
                 lba: u32,
                 count: u32,
                 ptr: u32,
                 len: u32|
                 -> u32 {
                    let total_bytes = count.saturating_mul(crate::storage_host::STORAGE_BLOCK_SIZE);
                    if total_bytes == 0 || len < total_bytes {
                        return 0;
                    }

                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
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
            )
            .map_err(|_| String::from("Failed to link os_storage_write_blocks"))?;

        linker
            .func_wrap(
                "env",
                "os_storage_flush_image",
                |_caller: Caller<'_, ()>, handle: u32| -> u32 {
                    if crate::storage_host::flush_image(handle).is_ok() {
                        1
                    } else {
                        0
                    }
                },
            )
            .map_err(|_| String::from("Failed to link os_storage_flush_image"))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| format!("Instantiate failed: {}", e))?
            .start(&mut store)
            .map_err(|e| format!("Start instance failed: {}", e))?;

        let init = instance
            .get_typed_func::<(), ()>(&store, "init_driver")
            .map_err(|_| String::from("Export 'init_driver' not found"))?;
        init.call(&mut store, ())
            .map_err(|e| format!("Init trapped: {}", e))?;

        Ok(WasmState {
            engine,
            store,
            instance,
        })
    }

    fn upsert_module_state(&mut self, module_key: &str, mut state: WasmState, byte_len: usize) {
        let tick = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
        let frame_pump_hz = detect_frame_pump_hz(&mut state);
        let frame_pump_interval_ticks = frame_pump_interval_ticks(frame_pump_hz);
        if frame_pump_hz > 0 {
            crate::serial_println!(
                "[Skill Runtime] {} frame_pump_hz={} interval_ticks={}",
                module_key,
                frame_pump_hz,
                frame_pump_interval_ticks
            );
        }
        if let Some(existing) = self
            .wasm_modules
            .iter_mut()
            .find(|module| module.module_key == module_key)
        {
            existing.state = state;
            existing.byte_len = byte_len;
            existing.frame_pump_count = 0;
            existing.frame_pump_hz = frame_pump_hz;
            existing.frame_pump_interval_ticks = frame_pump_interval_ticks;
            existing.next_frame_pump_tick = tick;
            existing.last_active_tick = tick;
            existing.consecutive_traps = 0;
            existing.quarantined = false;
            existing.last_error = None;
            return;
        }

        self.wasm_modules.push(LoadedWasmModule {
            module_key: String::from(module_key),
            state,
            poll_count: 0,
            frame_pump_count: 0,
            frame_pump_hz,
            frame_pump_interval_ticks,
            next_frame_pump_tick: tick,
            trap_count: 0,
            consecutive_traps: 0,
            quarantined: false,
            last_active_tick: tick,
            last_error: None,
            byte_len,
        });
    }

    fn record_module_trap(&mut self, module_key: &str, error: String) {
        let tick = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
        if let Some(existing) = self
            .wasm_modules
            .iter_mut()
            .find(|module| module.module_key == module_key)
        {
            note_wasm_module_trap(existing, tick, error);
        } else {
            *WASM_HEALTH_SNAPSHOT.lock() = vec![WasmModuleHealth {
                module_key: String::from(module_key),
                poll_count: 0,
                frame_pump_count: 0,
                frame_pump_hz: 0,
                trap_count: 1,
                consecutive_traps: 1,
                quarantined: false,
                last_active_tick: tick,
                byte_len: 0,
                loaded: false,
                last_error: Some(error),
            }];
            return;
        }
        self.refresh_health_snapshot();
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
        if !self.wasm_modules.is_empty() {
            self.next_network_poll_index %= self.wasm_modules.len();
            self.next_input_poll_index %= self.wasm_modules.len();
        } else {
            self.next_network_poll_index = 0;
            self.next_input_poll_index = 0;
        }
        self.refresh_health_snapshot();
        true
    }

    pub fn unload_input_wasm_sandbox(&mut self, kind: HidDeviceKind) -> bool {
        self.unload_named_wasm_sandbox(input_module_key(kind))
    }

    pub fn run_wasm_quarantine_selftest(&mut self) -> Result<String, String> {
        let _ = self.unload_named_wasm_sandbox(WASM_QUARANTINE_SELFTEST_MODULE);
        match self.execute_named_wasm_sandbox(
            WASM_QUARANTINE_SELFTEST_MODULE,
            WASM_QUARANTINE_TRAP_FIXTURE,
        ) {
            ExecutionResult::Success(_) => {}
            ExecutionResult::Panic(error) => return Err(error),
        }

        let mut trap_errors = 0u32;
        for _ in 0..WASM_QUARANTINE_CONSECUTIVE_TRAPS {
            if self
                .invoke_named_wasm_entry(WASM_QUARANTINE_SELFTEST_MODULE, "poll_net")
                .is_err()
            {
                trap_errors = trap_errors.saturating_add(1);
            }
        }

        if trap_errors != WASM_QUARANTINE_CONSECUTIVE_TRAPS {
            return Err(format!(
                "expected {} trap errors, observed {}",
                WASM_QUARANTINE_CONSECUTIVE_TRAPS, trap_errors
            ));
        }

        let Some(health) = wasm_health_snapshot()
            .into_iter()
            .find(|module| module.module_key == WASM_QUARANTINE_SELFTEST_MODULE)
        else {
            return Err(String::from("quarantine selftest module missing from health snapshot"));
        };

        if !health.quarantined
            || health.consecutive_traps < WASM_QUARANTINE_CONSECUTIVE_TRAPS
            || health.trap_count < WASM_QUARANTINE_CONSECUTIVE_TRAPS as u64
        {
            return Err(format!(
                "quarantine not reached: quarantined={} traps={} consecutive={}",
                health.quarantined, health.trap_count, health.consecutive_traps
            ));
        }

        match self.invoke_named_wasm_entry(WASM_QUARANTINE_SELFTEST_MODULE, "poll_net") {
            Ok(_) => Err(String::from(
                "quarantined module unexpectedly accepted another invocation",
            )),
            Err(error) if error.contains("quarantined") => Ok(format!(
                "quarantine selftest passed module={} traps={} consecutive={}",
                health.module_key, health.trap_count, health.consecutive_traps
            )),
            Err(error) => Err(format!(
                "quarantined module rejected with unexpected error: {}",
                error
            )),
        }
    }

    pub fn unquarantine_named_wasm_sandbox(&mut self, module_key: &str) -> Result<String, String> {
        let tick = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
        let Some(module) = self
            .wasm_modules
            .iter_mut()
            .find(|module| module.module_key == module_key)
        else {
            return Err(format!("sandbox module is not loaded: {}", module_key));
        };

        let retained_traps = module.trap_count;
        module.quarantined = false;
        module.consecutive_traps = 0;
        module.last_active_tick = tick;
        module.last_error = None;
        self.refresh_health_snapshot();
        Ok(format!(
            "unquarantined module={} retained_traps={}",
            module_key, retained_traps
        ))
    }

    pub fn run_wasm_quarantine_recovery_selftest(&mut self) -> Result<String, String> {
        let _ = self.run_wasm_quarantine_selftest()?;
        let _ = self.unquarantine_named_wasm_sandbox(WASM_QUARANTINE_SELFTEST_MODULE)?;

        let Some(health) = wasm_health_snapshot()
            .into_iter()
            .find(|module| module.module_key == WASM_QUARANTINE_SELFTEST_MODULE)
        else {
            return Err(String::from("recovery selftest module missing from health snapshot"));
        };

        if health.quarantined || health.consecutive_traps != 0 {
            return Err(format!(
                "recovery did not clear quarantine: quarantined={} consecutive={}",
                health.quarantined, health.consecutive_traps
            ));
        }

        Ok(format!(
            "quarantine recovery selftest passed module={} retained_traps={}",
            health.module_key, health.trap_count
        ))
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

        if module.quarantined {
            return Err(format!("sandbox module is quarantined: {}", module_key));
        }

        let state = &mut module.state;

        if let Ok(func) = state
            .instance
            .get_typed_func::<(), i32>(&state.store, export)
        {
            let value = match func.call(&mut state.store, ()) {
                Ok(value) => value,
                Err(e) => {
                    let error = format!("{} trapped: {}", export, e);
                    let tick =
                        crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed)
                            as u64;
                    note_wasm_module_trap(module, tick, error.clone());
                    self.refresh_health_snapshot();
                    return Err(error);
                }
            };
            module.poll_count = module.poll_count.saturating_add(1);
            let tick =
                crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
            note_wasm_module_success(module, tick);
            self.refresh_health_snapshot();
            return Ok(Some(value));
        }

        if let Ok(func) = state
            .instance
            .get_typed_func::<(), ()>(&state.store, export)
        {
            if let Err(e) = func.call(&mut state.store, ()) {
                let error = format!("{} trapped: {}", export, e);
                let tick =
                    crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
                note_wasm_module_trap(module, tick, error.clone());
                self.refresh_health_snapshot();
                return Err(error);
            }
            module.poll_count = module.poll_count.saturating_add(1);
            let tick =
                crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
            note_wasm_module_success(module, tick);
            self.refresh_health_snapshot();
            return Ok(None);
        }

        Err(format!("export '{}' not found", export))
    }

    pub fn invoke_storage_driver_read_block_into(
        &mut self,
        module_key: &str,
        lba: u32,
        out: &mut [u8; crate::storage_host::STORAGE_BLOCK_SIZE as usize],
    ) -> Result<(), String> {
        let result = {
            let Some(module) = self
                .wasm_modules
                .iter_mut()
                .find(|module| module.module_key == module_key)
            else {
                return Err(String::from("sandbox module is not loaded"));
            };

            let state = &mut module.state;
            let memory = state
                .instance
                .get_export(&state.store, "memory")
                .and_then(|export| export.into_memory())
                .ok_or_else(|| String::from("sandbox storage driver has no memory export"))?;
            let scratch_ptr = state
                .instance
                .get_typed_func::<(), u32>(&state.store, "storage_scratch_ptr")
                .map_err(|_| String::from("storage_scratch_ptr export not found"))?;
            let read_block = state
                .instance
                .get_typed_func::<(u32, u32), i32>(&state.store, "storage_read_block")
                .map_err(|_| String::from("storage_read_block export not found"))?;
            let tick = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;

            let ptr = scratch_ptr.call(&mut state.store, ()).map_err(|error| {
                module.trap_count = module.trap_count.saturating_add(1);
                module.last_active_tick = tick;
                module.last_error = Some(format!("storage_scratch_ptr trapped: {}", error));
                module.last_error.clone().unwrap_or_default()
            })?;
            if ptr == 0 {
                return Err(String::from("storage_scratch_ptr returned null"));
            }

            let read_len = read_block
                .call(&mut state.store, (lba, ptr))
                .map_err(|error| {
                    module.trap_count = module.trap_count.saturating_add(1);
                    module.last_active_tick = tick;
                    module.last_error = Some(format!("storage_read_block trapped: {}", error));
                    module.last_error.clone().unwrap_or_default()
                })?;
            if read_len != crate::storage_host::STORAGE_BLOCK_SIZE as i32 {
                return Err(format!("storage_read_block returned {}", read_len));
            }

            memory.read(&state.store, ptr as usize, out).map_err(|_| {
                String::from("failed to read sandbox storage block from wasm memory")
            })?;
            module.poll_count = module.poll_count.saturating_add(1);
            module.last_active_tick = tick;
            module.last_error = None;
            Ok(())
        };

        self.refresh_health_snapshot();
        result
    }

    pub fn invoke_storage_driver_write_block_from(
        &mut self,
        module_key: &str,
        lba: u32,
        data: &[u8; crate::storage_host::STORAGE_BLOCK_SIZE as usize],
    ) -> Result<(), String> {
        let result = {
            let Some(module) = self
                .wasm_modules
                .iter_mut()
                .find(|module| module.module_key == module_key)
            else {
                return Err(String::from("sandbox module is not loaded"));
            };

            let state = &mut module.state;
            let memory = state
                .instance
                .get_export(&state.store, "memory")
                .and_then(|export| export.into_memory())
                .ok_or_else(|| String::from("sandbox storage driver has no memory export"))?;
            let scratch_ptr = state
                .instance
                .get_typed_func::<(), u32>(&state.store, "storage_scratch_ptr")
                .map_err(|_| String::from("storage_scratch_ptr export not found"))?;
            let write_block = state
                .instance
                .get_typed_func::<(u32, u32), i32>(&state.store, "storage_write_block")
                .map_err(|_| String::from("storage_write_block export not found"))?;
            let tick = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;

            let ptr = scratch_ptr.call(&mut state.store, ()).map_err(|error| {
                module.trap_count = module.trap_count.saturating_add(1);
                module.last_active_tick = tick;
                module.last_error = Some(format!("storage_scratch_ptr trapped: {}", error));
                module.last_error.clone().unwrap_or_default()
            })?;
            if ptr == 0 {
                return Err(String::from("storage_scratch_ptr returned null"));
            }

            memory
                .write(&mut state.store, ptr as usize, data)
                .map_err(|_| {
                    String::from("failed to write sandbox storage block into wasm memory")
                })?;
            let write_len = write_block
                .call(&mut state.store, (lba, ptr))
                .map_err(|error| {
                    module.trap_count = module.trap_count.saturating_add(1);
                    module.last_active_tick = tick;
                    module.last_error = Some(format!("storage_write_block trapped: {}", error));
                    module.last_error.clone().unwrap_or_default()
                })?;
            if write_len != crate::storage_host::STORAGE_BLOCK_SIZE as i32 {
                return Err(format!("storage_write_block returned {}", write_len));
            }

            module.poll_count = module.poll_count.saturating_add(1);
            module.last_active_tick = tick;
            module.last_error = None;
            Ok(())
        };

        self.refresh_health_snapshot();
        result
    }

    pub fn invoke_storage_driver_flush(&mut self, module_key: &str) -> Result<(), String> {
        let result = {
            let Some(module) = self
                .wasm_modules
                .iter_mut()
                .find(|module| module.module_key == module_key)
            else {
                return Err(String::from("sandbox module is not loaded"));
            };

            let state = &mut module.state;
            let flush = state
                .instance
                .get_typed_func::<(), i32>(&state.store, "storage_flush")
                .map_err(|_| String::from("storage_flush export not found"))?;
            let tick = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;

            let flush_result = flush.call(&mut state.store, ()).map_err(|error| {
                module.trap_count = module.trap_count.saturating_add(1);
                module.last_active_tick = tick;
                module.last_error = Some(format!("storage_flush trapped: {}", error));
                module.last_error.clone().unwrap_or_default()
            })?;
            if flush_result <= 0 {
                return Err(format!("storage_flush returned {}", flush_result));
            }

            module.poll_count = module.poll_count.saturating_add(1);
            module.last_active_tick = tick;
            module.last_error = None;
            Ok(())
        };

        self.refresh_health_snapshot();
        result
    }

    pub fn invoke_storage_driver_scratch_selftest(
        &mut self,
        module_key: &str,
        scratch_lba: u32,
    ) -> Result<u32, String> {
        let mut original = [0u8; crate::storage_host::STORAGE_BLOCK_SIZE as usize];
        self.invoke_storage_driver_read_block_into(module_key, scratch_lba, &mut original)?;

        let mut pattern = original;
        pattern[0..8].copy_from_slice(b"ORSDISP1");
        for index in 8..pattern.len() {
            pattern[index] = original[index]
                .wrapping_add((index as u8).wrapping_mul(23))
                .wrapping_add(0x45);
        }

        self.invoke_storage_driver_write_block_from(module_key, scratch_lba, &pattern)?;

        let mut verify = [0u8; crate::storage_host::STORAGE_BLOCK_SIZE as usize];
        self.invoke_storage_driver_read_block_into(module_key, scratch_lba, &mut verify)?;
        if verify != pattern {
            let _ = self.invoke_storage_driver_write_block_from(module_key, scratch_lba, &original);
            return Err(String::from(
                "sandbox storage readback did not match written pattern",
            ));
        }

        self.invoke_storage_driver_write_block_from(module_key, scratch_lba, &original)?;
        self.invoke_storage_driver_flush(module_key)?;
        self.invoke_storage_driver_read_block_into(module_key, scratch_lba, &mut verify)?;
        if verify != original {
            return Err(String::from("sandbox storage restore verification failed"));
        }

        Ok(storage_selftest_checksum(&pattern) ^ storage_selftest_checksum(&original))
    }

    pub fn poll_wasm_network(&mut self) {
        if crate::display::sandbox_frame_lease_active() {
            return;
        }
        self.poll_round_robin("poll_net", false, CAPABILITY_POLL_BUDGET_PER_TICK);
    }

    pub fn poll_wasm_frame_pumps(&mut self) {
        if self.wasm_modules.is_empty() {
            return;
        }

        let now = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
        let mut pumped = 0usize;
        let mut changed = false;

        for module in self.wasm_modules.iter_mut() {
            if pumped >= FRAME_PUMP_BUDGET_PER_TICK {
                break;
            }
            if module.quarantined {
                continue;
            }
            if module.frame_pump_hz == 0 || now < module.next_frame_pump_tick {
                continue;
            }

            let state = &mut module.state;
            let mut result: Result<(), String> = Err(String::from("frame_pump export not found"));

            if let Ok(frame_pump) = state
                .instance
                .get_typed_func::<(), ()>(&state.store, "frame_pump")
            {
                result = frame_pump
                    .call(&mut state.store, ())
                    .map_err(|error| format!("frame_pump trapped: {}", error));
            } else if let Ok(frame_pump) = state
                .instance
                .get_typed_func::<(), i32>(&state.store, "frame_pump")
            {
                result = frame_pump
                    .call(&mut state.store, ())
                    .map(|_| ())
                    .map_err(|error| format!("frame_pump trapped: {}", error));
            }

            match result {
                Ok(()) => {
                    module.frame_pump_count = module.frame_pump_count.saturating_add(1);
                    module.poll_count = module.poll_count.saturating_add(1);
                    note_wasm_module_success(module, now);
                }
                Err(error) => {
                    note_wasm_module_trap(module, now, error);
                }
            }

            let interval = module.frame_pump_interval_ticks.max(1);
            module.next_frame_pump_tick = if module.next_frame_pump_tick.saturating_add(interval) > now {
                module.next_frame_pump_tick.saturating_add(interval)
            } else {
                now.saturating_add(interval)
            };
            pumped += 1;
            changed = true;
        }

        if changed && now.saturating_sub(self.last_health_refresh_tick) >= 250 {
            self.last_health_refresh_tick = now;
            self.refresh_health_snapshot();
        }
    }

    pub fn poll_wasm_input(&mut self) {
        let budget = if crate::display::sandbox_frame_lease_active() {
            1
        } else {
            INPUT_POLL_BUDGET_PER_TICK
        };
        self.poll_round_robin("poll_input_driver", true, budget);
    }

    pub fn poll_hardware_event(&self) -> Option<u8> {
        crate::arch::x86_64::interrupts::KEYBOARD_QUEUE.lock().pop()
    }

    pub fn poll_host_data(&self) -> Option<u8> {
        crate::arch::x86_64::serial::poll_receive()
    }

    pub fn loaded_module_keys(&self) -> Vec<String> {
        let mut keys = Vec::with_capacity(self.wasm_modules.len());
        for module in &self.wasm_modules {
            keys.push(module.module_key.clone());
        }
        keys
    }

    pub fn module_count(&self) -> usize {
        self.wasm_modules.len()
    }

    fn poll_round_robin(&mut self, export: &str, input_only: bool, budget: usize) {
        if self.wasm_modules.is_empty() {
            return;
        }

        let len = self.wasm_modules.len();
        let start = if input_only {
            self.next_input_poll_index % len
        } else {
            self.next_network_poll_index % len
        };

        let mut polled = 0usize;
        let mut scanned = 0usize;
        for offset in 0..len {
            let index = (start + offset) % len;
            scanned += 1;
            let is_input_module = self.wasm_modules[index].module_key.starts_with("input:");
            if is_input_module != input_only {
                continue;
            }
            if self.wasm_modules[index].quarantined {
                continue;
            }

            let state = &mut self.wasm_modules[index].state;
            if let Ok(poll) = state
                .instance
                .get_typed_func::<(), ()>(&state.store, export)
            {
                let tick =
                    crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as u64;
                let module = &mut self.wasm_modules[index];
                match poll.call(&mut module.state.store, ()) {
                    Ok(()) => {
                        module.poll_count = module.poll_count.saturating_add(1);
                        note_wasm_module_success(module, tick);
                    }
                    Err(error) => {
                        note_wasm_module_trap(
                            module,
                            tick,
                            format!("{} trapped: {}", export, error),
                        );
                    }
                }
                polled += 1;
                if input_only {
                    self.next_input_poll_index = (index + 1) % len;
                } else {
                    self.next_network_poll_index = (index + 1) % len;
                }
                self.refresh_health_snapshot();
                if polled >= budget {
                    return;
                }
            }
        }

        if polled > 0 {
            return;
        }

        if input_only {
            self.next_input_poll_index = (start + scanned.max(1)) % len;
        } else {
            self.next_network_poll_index = (start + scanned.max(1)) % len;
        }
    }

    fn refresh_health_snapshot(&self) {
        let mut snapshot = WASM_HEALTH_SNAPSHOT.lock();
        snapshot.clear();
        for module in &self.wasm_modules {
            snapshot.push(WasmModuleHealth {
                module_key: module.module_key.clone(),
                poll_count: module.poll_count,
                frame_pump_count: module.frame_pump_count,
                frame_pump_hz: module.frame_pump_hz,
                trap_count: module.trap_count,
                consecutive_traps: module.consecutive_traps,
                quarantined: module.quarantined,
                last_active_tick: module.last_active_tick,
                byte_len: module.byte_len,
                loaded: true,
                last_error: module.last_error.clone(),
            });
        }
    }
}

fn input_module_key(kind: HidDeviceKind) -> &'static str {
    match kind {
        HidDeviceKind::Keyboard => "input:keyboard",
        HidDeviceKind::Mouse => "input:mouse",
    }
}

fn storage_selftest_checksum(bytes: &[u8; 512]) -> u32 {
    let mut sum = 2166136261u32;
    for byte in bytes {
        sum = sum.wrapping_mul(16777619) ^ *byte as u32;
    }
    sum
}

fn detect_frame_pump_hz(state: &mut WasmState) -> u32 {
    if state
        .instance
        .get_typed_func::<(), ()>(&state.store, "frame_pump")
        .is_err()
        && state
            .instance
            .get_typed_func::<(), i32>(&state.store, "frame_pump")
            .is_err()
    {
        return 0;
    }

    let mut hz = FRAME_PUMP_DEFAULT_HZ;
    if let Ok(target_hz) = state
        .instance
        .get_typed_func::<(), i32>(&state.store, "frame_pump_target_hz")
    {
        if let Ok(value) = target_hz.call(&mut state.store, ()) {
            if value > 0 {
                hz = value as u32;
            }
        }
    }

    hz.clamp(1, FRAME_PUMP_MAX_HZ)
}

fn frame_pump_interval_ticks(hz: u32) -> u64 {
    if hz == 0 {
        return 0;
    }
    let ticks = (crate::task::timer::TICKS_PER_SECOND + hz as u64 - 1) / hz as u64;
    ticks.max(1)
}

pub fn wasm_health_snapshot() -> Vec<WasmModuleHealth> {
    WASM_HEALTH_SNAPSHOT.lock().clone()
}

pub fn wasm_health_report() -> String {
    let snapshot = wasm_health_snapshot();
    if snapshot.is_empty() {
        return String::from("[Wasm Runtime] no sandbox modules are loaded.");
    }

    let mut out = format!("[Wasm Runtime] loaded_modules={}\n", snapshot.len());
    for module in snapshot.iter().take(16) {
        let status = if module.quarantined {
            "quarantined"
        } else if module.last_error.is_some() {
            "degraded"
        } else {
            "healthy"
        };
        out.push_str(
            format!(
                "- {} status={} loaded={} bytes={} polls={} frame_pump_hz={} frames={} traps={} consecutive_traps={} last_tick={}",
                module.module_key,
                status,
                module.loaded as u8,
                module.byte_len,
                module.poll_count,
                module.frame_pump_hz,
                module.frame_pump_count,
                module.trap_count,
                module.consecutive_traps,
                module.last_active_tick
            )
            .as_str(),
        );
        if let Some(error) = module.last_error.as_deref() {
            out.push_str(" error=");
            out.push_str(error);
        }
        out.push('\n');
    }
    out
}
