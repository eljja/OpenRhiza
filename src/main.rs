#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)] // Enable the x86-interrupt ABI for CPU exceptions and interrupts

extern crate alloc; // Enable the built-in alloc crate for Vec, String, Box, and other heap types
extern crate smoltcp; // Explicitly link the TCP/IP stack crate

// Load the seed module under a custom path to avoid naming conflicts with Rust's core crate.
#[path = "core/seed.rs"]
pub mod os_core_seed;

// Multi-architecture module tree
pub mod arch {
    pub mod x86_64 {
        pub mod discovery;
        pub mod interrupts; // IDT and hardware interrupt handling
        pub mod port;       // Raw I/O port access primitives
        pub mod serial;     // COM1 serial link to the host
        pub mod apic;       // LAPIC/IOAPIC initialization
        pub mod usb;        // Native USB/xHCI support
    }
}

// Heap allocator
pub mod allocator;

pub mod net;
pub mod vga;
pub mod storage;
pub mod https;
pub mod dns;
pub mod display;
pub mod task;
pub mod security;
pub mod e1000;
pub mod keyboard;
pub mod crypto;
pub mod tls;
pub mod identity;
pub mod api_v1;
pub mod boot_automation;
pub mod capability_cache;
pub mod driver_cache;
pub mod driver_runtime;
pub mod runtime_bindings;
pub mod component_runtime;
pub mod sandbox_lifecycle;
pub mod input_handoff;
pub mod input_runtime;
pub mod skill_cache;
pub mod skill_runtime;
pub mod prompt_orchestrator;
pub mod firmware;
pub mod wifi_mac;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use arch::x86_64::discovery::SystemIdentity;
use os_core_seed::OpenRhizaSeed;
use core::panic::PanicInfo;
use bootloader::bootinfo::BootInfo;
use smoltcp::wire::Ipv4Address;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ServiceApiPhase {
    Idle,
    Register,
    HealthHttps,
    RootHttps,
    HardwareReport,
    DriverQuery,
    SoftwareQuery,
    SkillQuery,
    WorkflowQuery,
    PolicyQuery,
    EvaluationQuery,
    EvaluationUpload,
    SkillDownload,
    DriverUpload,
    DriverDownload,
    DriverComment,
    DriverVote,
    Done,
}

#[derive(Clone)]
struct ServiceRequestSpec {
    phase: ServiceApiPhase,
    path: String,
    body: String,
}

enum PendingDnsAction {
    Service {
        phase: ServiceApiPhase,
        chain_active: bool,
    },
    CustomService(ServiceRequestSpec),
    PlainHttp(PlainHttpAction),
    Gemini(GeminiRequest),
}

#[derive(Clone, Copy)]
enum PlainHttpAction {
    Register,
    Health,
}

struct GeminiRequest {
    prompt: String,
    model_index: usize,
}

const ENABLE_SERVICE_API_BOOTSTRAP: bool = false;
const ENABLE_NEXUS_BOOTSTRAP: bool = false;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    crate::arch::x86_64::serial::_print(core::format_args!("KERNEL PANIC: {}\n", info));
    loop {}
}

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    // 1. Install the exception/interrupt guardrail first.
    arch::x86_64::interrupts::init_idt();

    let offset = boot_info.physical_memory_offset;
    unsafe {
        crate::arch::x86_64::discovery::PHYS_MEM_OFFSET = offset;
    }

    // 2. Disable the legacy PIC and take control through APIC.
    unsafe {
        arch::x86_64::interrupts::PICS.lock().disable();
    }
    arch::x86_64::apic::init_apic(offset);

    x86_64::instructions::interrupts::enable(); // Allow the CPU to receive external interrupts

    // Notify the host that serial communications are available.
    crate::println!("OpenRhiza Seed (Layer 0) Booting... Serial Connected!");

    // 3. Initialize the heap allocator so Vec and other alloc types can be used.
    allocator::init_heap();
    crate::println!("Heap Allocator initialized!");

    // 4. Scan the hardware and enumerate PCI devices.
    let identity = SystemIdentity::scan(boot_info);
    crate::println!("Total Usable Memory: {} Bytes", identity.total_memory);
    crate::println!(
        "Storage Interface Detected: {}",
        if identity.storage_detected { "yes" } else { "no" }
    );
    crate::println!("Hardware Discovery Complete.");
    crate::println!("Found {} PCI devices:", identity.pci_devices.len());
    for dev in &identity.pci_devices {
        crate::println!(
            "  Bus {} Device {} Func {}: Vendor {:#06X}, Device {:#06X}, BAR0: {:#010X}",
            dev.bus,
            dev.device,
            dev.func,
            dev.vendor_id,
            dev.device_id,
            dev.bar0
        );
        
        // Bootstrap native xHCI from Layer 0 when a controller is discovered.
        if dev.class_code == 0x0C && dev.subclass == 0x03 && dev.bar0 != 0 {
            crate::println!(
                "[USB] bootstrap controller at bus {} dev {} func {} prog_if={:#04X}",
                dev.bus,
                dev.device,
                dev.func,
                dev.prog_if
            );
            crate::arch::x86_64::usb::init_xhci(dev.bar0, offset, dev.bus, dev.device);
        }

        if dev.vendor_id == 0x8086 && dev.device_id == 0x100E && dev.bar0 != 0 {
            crate::e1000::enable_pci_bus_mastering(dev.bus, dev.device);
            if let Some(nic) = crate::e1000::E1000::init(dev.bar0, offset) {
                crate::net::attach_native_e1000(nic);
            }
        }
    }
    
    // Probe the bootstrap storage image and look for cached payloads.
    crate::println!("[Storage] Probing Secondary IDE Drive for Wasm Cache...");
    let mut local_driver_bindings = None;
    if storage::probe_secondary_bootstrap_disk() {
        crate::println!("[Storage] Native Bootstrap Disk Detected! Boot Signature: 0x55AA");
        
        crate::println!("[Storage] Executing native FAT16 Parse...");
        if let Some(payload) = storage::extract_payload() {
            crate::println!("[Storage] Successfully extracted E1000.BIN payload ({} bytes).", payload.len());
            let preview_len = payload.len().min(17);
            if let Ok(preview) = core::str::from_utf8(&payload[..preview_len]) {
                crate::println!("[Storage] Payload Preview: '{}'", preview);
            }
        } else {
            crate::println!("[Storage] E1000.BIN not found in Root Directory.");
        }

        local_driver_bindings = crate::driver_cache::load_active_driver_map();
        match &local_driver_bindings {
            Some(bindings) => {
                crate::println!(
                    "[Driver Cache] Loaded {} active driver bindings from local storage.",
                    bindings.len()
                );
                let installed = crate::driver_runtime::install_local_bindings(bindings);
                crate::println!(
                    "[Driver Runtime] Installed {} live driver bindings from local cache.",
                    installed
                );
            }
            None => crate::println!(
                "[Driver Cache] No local active driver map found. Using registry-first fallback."
            ),
        }

        let cached_skills = crate::skill_cache::load_cached_skills();
        if cached_skills.is_empty() {
            crate::println!(
                "[Skill Cache] No local skill cache found. Using registry-first fallback."
            );
        } else {
            crate::println!(
                "[Skill Cache] Loaded {} cached skills from local storage.",
                cached_skills.len()
            );
            let installed = crate::skill_runtime::install_cached_skills(&cached_skills, "local-cache");
            crate::println!(
                "[Skill Runtime] Registered {} cached skills for runtime use.",
                installed
            );
            let scheduled = crate::skill_runtime::schedule_persisted_skill_restores();
            if scheduled > 0 {
                crate::println!(
                    "[Skill Runtime] Scheduled {} persisted skill restores.",
                    scheduled
                );
            }
        }
    } else {
        crate::println!("[Storage] No valid Wasm Cache drive found. Wait for external Link.");
    }

    // 5. Create the Layer 0 seed / sandbox engine.
    let node_profile = crate::identity::NodeProfile::collect(&identity).install();
    node_profile.log_summary();
    crate::api_v1::log_request_previews(&node_profile);
    if local_driver_bindings.is_some() {
        let mut matched_local = 0usize;
        for device in &node_profile.machine_profile.pci_devices {
            let match_key = crate::identity::stable_device_match_key(device);
            if let Some(driver_id) = crate::runtime_bindings::current_driver(&match_key) {
                crate::println!(
                    "[Driver Runtime] Live driver for {} -> {}",
                    match_key,
                    driver_id
                );
                matched_local += 1;
            }
        }

        if matched_local == 0 {
            crate::println!("[Driver Runtime] No local live driver matched current PCI devices.");
        }
    }

    crate::input_runtime::schedule_persisted_restores();

    let rhiza = OpenRhizaSeed::new(identity);
    
    // 6. Initialize the networking stack.
    crate::net::init_network();

    crate::println!("[OS System] All subsystems initialized. Handing over to Async Executor.");

    let mut executor = task::executor::Executor::new();
    executor.spawn(task::Task::new(task::keyboard::keyboard_task()));
    executor.spawn(task::Task::new(usb_input_task()));
    executor.spawn(task::Task::new(runtime_status_task()));
    executor.spawn(task::Task::new(crate::boot_automation::boot_autorun_task()));
    executor.spawn(task::Task::new(core_os_task(rhiza)));
    executor.run();
}

async fn usb_input_task() {
    crate::println!("[Task] usb_input_task started");
    let mut status_logged = false;
    loop {
        crate::task::timer::sleep_ticks(1).await;
        crate::arch::x86_64::usb::poll_usb_keyboard();
        crate::arch::x86_64::usb::tick_usb_keyboard();
        crate::arch::x86_64::usb::maintain_hid_hotplug();
        crate::input_handoff::apply_runtime_input_events();
        if !status_logged && crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) >= 1000 {
            crate::arch::x86_64::usb::log_hid_status();
            status_logged = true;
        }
    }
}

async fn runtime_status_task() {
    loop {
        let ticks = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
        let seconds = ticks / crate::task::timer::TICKS_PER_SECOND;
        crate::vga::render_runtime(seconds);
        crate::task::timer::sleep_ticks(crate::task::timer::TICKS_PER_SECOND).await;
    }
}

async fn core_os_task(mut rhiza: OpenRhizaSeed) {
    crate::println!("[Task] core_os_task started");
    let mut receiving_wasm = false;
    let mut receiving_wasm_size = false;
    let mut wasm_size_buf = [0u8; 4];
    let mut wasm_size_idx = 0;
    let mut expected_wasm_size = 0;
    let mut wasm_buffer: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    
    let mut uptime_ms: i64 = 0;
    let mut nexus_client: Option<crate::https::NexusClient> = None;
    let mut service_api_phase = ServiceApiPhase::Idle;
    let mut service_api_client: Option<crate::https::ApiClient> = None;
    let mut plain_http_client: Option<crate::https::PlainHttpClient> = None;
    let mut service_api_chain_active = false;
    let mut gemini_client: Option<crate::https::ApiClient> = None;
    let mut gemini_request: Option<GeminiRequest> = None;
    let mut pending_orchestration_prompt: Option<String> = None;
    let mut prompt_orchestration_plan: Option<crate::prompt_orchestrator::PromptOrchestrationPlan> = None;
    let mut prompt_auto_apply_drivers = false;
    let mut prompt_waiting_for_driver_registry = false;
    let mut gemini_auto_upload_match_key: Option<String> = None;
    let mut openrhiza_ip: Option<Ipv4Address> = None;
    let mut gemini_ip: Option<Ipv4Address> = None;
    let mut dns_client: Option<crate::dns::DnsClient> = None;
    let mut pending_dns_action: Option<PendingDnsAction> = None;
    let mut keymap_index = 0;

    loop {
        // Sleep for one tick to avoid monopolizing the CPU.
        crate::task::timer::sleep_ticks(1).await;

        // Poll the Wasm-side NIC path if an AI-generated driver is active.
        rhiza.poll_wasm_network();
        rhiza.poll_wasm_input();
        crate::net::poll(uptime_ms);

        uptime_ms += 1;

        if ENABLE_NEXUS_BOOTSTRAP && uptime_ms == 200 {
            crate::println!("[OS] Triggering Native Nexus Fetch for xHCI (0x0C_0x03) over TCP/IP...");
            let socket_handle = crate::net::create_tcp_socket();
            nexus_client = Some(crate::https::NexusClient::new(
                socket_handle,
                smoltcp::wire::IpAddress::Ipv4(smoltcp::wire::Ipv4Address::new(10, 0, 2, 2)),
                4443,
                "0x0C_0x03"
            ));
        }

        if let Some(client) = &mut nexus_client {
            client.poll();
            if let Some((wasm_payload, signature)) = client.take_payload() {
                crate::net::destroy_socket(client.handle());
                crate::println!("[OS] Downloaded Wasm size: {} bytes. Verifying Signature...", wasm_payload.len());
                if crate::security::verify_nexus_signature(&wasm_payload, &signature) {
                    crate::println!("[SECURITY] Nexus Ed25519 Signature Verified! Trusting Wasm Payload.");
                    match rhiza.execute_wasm_sandbox(&wasm_payload) {
                        os_core_seed::ExecutionResult::Success(msg) => crate::println!("[Sandbox] Autonomous Nexus Fetch Success: {}", msg),
                        os_core_seed::ExecutionResult::Panic(err) => crate::println!("[Sandbox Error] {}", err),
                    }
                } else {
                    crate::println!("[SECURITY_ALERT] Invalid Ed25519 Signature! Malicious Payload Dropped.");
                }
                nexus_client = None;
            }
        }

        if ENABLE_SERVICE_API_BOOTSTRAP
            && uptime_ms >= 600
            && nexus_client.is_none()
            && service_api_phase == ServiceApiPhase::Idle
            && service_api_client.is_none()
            && plain_http_client.is_none()
            && gemini_client.is_none()
            && dns_client.is_none()
        {
            if let Some(ip) = openrhiza_ip {
                service_api_client = spawn_service_api_client(ServiceApiPhase::Register, ip);
                if service_api_client.is_some() {
                    service_api_phase = ServiceApiPhase::Register;
                }
            } else {
                dns_client = Some(spawn_dns_client(crate::api_v1::openrhiza_host()));
                pending_dns_action = Some(PendingDnsAction::Service {
                    phase: ServiceApiPhase::Register,
                    chain_active: false,
                });
            }
        }

        if nexus_client.is_none()
            && service_api_client.is_none()
            && plain_http_client.is_none()
            && gemini_client.is_none()
            && dns_client.is_none()
            && service_api_phase == ServiceApiPhase::Idle
        {
            if let Some(command) = crate::input_handoff::SANDBOX_INPUT_COMMAND_QUEUE.pop() {
                match command {
                    crate::input_handoff::SandboxInputCommand::LoadMouseDriver => {
                        let kind = crate::input_handoff::HidDeviceKind::Mouse;
                        crate::result_println!("[Sandbox Input] Loading MOUSEDRV.WAS from local driver disk...");
                        match crate::storage::read_named_file_from_secondary_fat16(
                            crate::input_runtime::local_driver_files_for_kind(kind),
                        ) {
                            Some(wasm) => match rhiza.execute_input_wasm_sandbox(kind, &wasm) {
                                crate::os_core_seed::ExecutionResult::Success(msg) => {
                                    crate::result_println!("[Sandbox Input] {}", msg);
                                    if let Some(state) = crate::input_runtime::finish_load_success(kind) {
                                        crate::result_println!(
                                            "[Sandbox Input] {} stage={:?} driver={}",
                                            crate::input_runtime::kind_label(kind),
                                            state.component.stage,
                                            state.component.current_artifact_id.as_deref().unwrap_or("none")
                                        );
                                    }
                                }
                                crate::os_core_seed::ExecutionResult::Panic(err) => {
                                    crate::result_println!("[Sandbox Input] {}", err);
                                    crate::input_runtime::finish_load_failure(kind, err.as_str());
                                }
                            },
                            None => crate::result_println!(
                                "[Sandbox Input] MOUSEDRV.WAS not found on the secondary driver disk."
                            )
                        }
                    }
                    crate::input_handoff::SandboxInputCommand::UnloadMouseDriver => {
                        if rhiza.unload_input_wasm_sandbox(crate::input_handoff::HidDeviceKind::Mouse) {
                            crate::result_println!("[Sandbox Input] Unloaded mouse sandbox driver.");
                        }
                    }
                    crate::input_handoff::SandboxInputCommand::LoadKeyboardDriver => {
                        let kind = crate::input_handoff::HidDeviceKind::Keyboard;
                        crate::result_println!("[Sandbox Input] Loading KEYBDRV.WAS from local driver disk...");
                        match crate::storage::read_named_file_from_secondary_fat16(
                            crate::input_runtime::local_driver_files_for_kind(kind),
                        ) {
                            Some(wasm) => match rhiza.execute_input_wasm_sandbox(kind, &wasm) {
                                crate::os_core_seed::ExecutionResult::Success(msg) => {
                                    crate::result_println!("[Sandbox Input] {}", msg);
                                    if let Some(state) = crate::input_runtime::finish_load_success(kind) {
                                        crate::result_println!(
                                            "[Sandbox Input] {} stage={:?} driver={}",
                                            crate::input_runtime::kind_label(kind),
                                            state.component.stage,
                                            state.component.current_artifact_id.as_deref().unwrap_or("none")
                                        );
                                    }
                                }
                                crate::os_core_seed::ExecutionResult::Panic(err) => {
                                    crate::result_println!("[Sandbox Input] {}", err);
                                    crate::input_runtime::finish_load_failure(kind, err.as_str());
                                }
                            },
                            None => crate::result_println!(
                                "[Sandbox Input] KEYBDRV.WAS not found on the secondary driver disk."
                            )
                        }
                    }
                    crate::input_handoff::SandboxInputCommand::UnloadKeyboardDriver => {
                        if rhiza.unload_input_wasm_sandbox(crate::input_handoff::HidDeviceKind::Keyboard) {
                            crate::result_println!("[Sandbox Input] Unloaded keyboard sandbox driver.");
                        }
                    }
                }
            }
        }

        if nexus_client.is_none()
            && service_api_client.is_none()
            && plain_http_client.is_none()
            && gemini_client.is_none()
            && dns_client.is_none()
            && service_api_phase == ServiceApiPhase::Idle
        {
            if let Some(command) = crate::skill_runtime::SKILL_RUNTIME_COMMAND_QUEUE.pop() {
                match command {
                    crate::skill_runtime::SkillRuntimeCommand::Load { skill_id } => {
                        let Some(cached) = crate::skill_cache::find_cached_skill(skill_id.as_str()) else {
                            crate::result_println!(
                                "[Skill Runtime] Cached skill artifact not found for {}.",
                                skill_id
                            );
                            let _ = crate::skill_runtime::finish_load_failure(
                                skill_id.as_str(),
                                "cached local skill artifact not found",
                            );
                            continue;
                        };

                        let Some(fat_name) =
                            crate::skill_cache::fat_name_bytes_from_text(cached.fat_name_text.as_str())
                        else {
                            crate::result_println!(
                                "[Skill Runtime] Invalid FAT file name for cached skill {} -> {}",
                                skill_id,
                                cached.fat_name_text
                            );
                            let _ = crate::skill_runtime::finish_load_failure(
                                skill_id.as_str(),
                                "invalid local FAT skill artifact name",
                            );
                            continue;
                        };

                        crate::result_println!(
                            "[Skill Runtime] Loading {} from local skill cache...",
                            cached.fat_name_text
                        );
                        match crate::storage::read_named_file_from_secondary_fat16(&[fat_name]) {
                            Some(wasm) => {
                                let module_key = crate::skill_runtime::module_key_for_skill(skill_id.as_str());
                                match rhiza.execute_named_wasm_sandbox(module_key.as_str(), &wasm) {
                                    crate::os_core_seed::ExecutionResult::Success(msg) => {
                                        crate::result_println!("[Skill Runtime] {}", msg);
                                        if let Some(state) =
                                            crate::skill_runtime::finish_load_success(skill_id.as_str())
                                        {
                                            crate::result_println!(
                                                "[Skill Runtime] {} stage={:?} current={}",
                                                state.skill_id,
                                                state.stage,
                                                state.current_artifact_id.as_deref().unwrap_or("none")
                                            );
                                            if crate::skill_runtime::take_auto_run(skill_id.as_str()) {
                                                match crate::skill_runtime::queue_run(skill_id.as_str()) {
                                                    Ok(()) => crate::result_println!(
                                                        "[Skill Runtime] Queued autorun for {}.",
                                                        skill_id
                                                    ),
                                                    Err(error) => crate::result_println!(
                                                        "[Skill Runtime] Autorun for {} failed: {}",
                                                        skill_id,
                                                        error
                                                    ),
                                                }
                                            }
                                        }
                                    }
                                    crate::os_core_seed::ExecutionResult::Panic(err) => {
                                        crate::result_println!("[Skill Runtime] {}", err);
                                        let _ = crate::skill_runtime::finish_load_failure(
                                            skill_id.as_str(),
                                            err.as_str(),
                                        );
                                    }
                                }
                            }
                            None => {
                                crate::result_println!(
                                    "[Skill Runtime] {} not found on the secondary skill cache disk.",
                                    cached.fat_name_text
                                );
                                let _ = crate::skill_runtime::finish_load_failure(
                                    skill_id.as_str(),
                                    "local skill wasm not found",
                                );
                            }
                        }
                    }
                    crate::skill_runtime::SkillRuntimeCommand::Unload { skill_id } => {
                        let module_key = crate::skill_runtime::module_key_for_skill(skill_id.as_str());
                        if rhiza.unload_named_wasm_sandbox(module_key.as_str()) {
                            crate::result_println!("[Skill Runtime] Unloaded {}.", skill_id);
                        }
                    }
                    crate::skill_runtime::SkillRuntimeCommand::Run { skill_id } => {
                        let module_key = crate::skill_runtime::module_key_for_skill(skill_id.as_str());
                        match rhiza.invoke_named_wasm_entry(module_key.as_str(), "run_skill") {
                            Ok(Some(code)) => {
                                crate::result_println!(
                                    "[Skill Runtime] {} returned {}.",
                                    skill_id,
                                    code
                                );
                                maybe_queue_followup_workflow_for_skill(skill_id.as_str());
                            }
                            Ok(None) => crate::result_println!(
                                "[Skill Runtime] {} executed.",
                                skill_id
                            ),
                            Err(error) => crate::result_println!(
                                "[Skill Runtime] {} failed: {}",
                                skill_id,
                                error
                            ),
                        }
                    }
                }
            }
        }

        if nexus_client.is_none()
            && service_api_client.is_none()
            && plain_http_client.is_none()
            && gemini_client.is_none()
            && dns_client.is_none()
            && service_api_phase == ServiceApiPhase::Idle
        {
            if let Some(command) = crate::api_v1::SERVICE_API_QUEUE.pop() {
                match command {
                    crate::api_v1::ServiceApiCommand::NexusFetch => {
                        if nexus_client.is_none() {
                            crate::println!(
                                "[OS] Triggering Native Nexus Fetch for xHCI (0x0C_0x03) over TCP/IP..."
                            );
                            let socket_handle = crate::net::create_tcp_socket();
                            nexus_client = Some(crate::https::NexusClient::new(
                                socket_handle,
                                smoltcp::wire::IpAddress::Ipv4(
                                    smoltcp::wire::Ipv4Address::new(10, 0, 2, 2),
                                ),
                                4443,
                                "0x0C_0x03",
                            ));
                        }
                    }
                    crate::api_v1::ServiceApiCommand::Register => {
                        start_service_or_resolve(
                            ServiceApiPhase::Register,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::HealthHttps => {
                        start_service_or_resolve(
                            ServiceApiPhase::HealthHttps,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::RootHttps => {
                        start_service_or_resolve(
                            ServiceApiPhase::RootHttps,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::RegisterHttp => {
                        if let Some(ip) = openrhiza_ip {
                            plain_http_client = spawn_plain_http_register_client(ip);
                        } else {
                            dns_client = Some(spawn_dns_client(crate::api_v1::openrhiza_host()));
                            pending_dns_action = Some(PendingDnsAction::PlainHttp(
                                PlainHttpAction::Register,
                            ));
                        }
                    }
                    crate::api_v1::ServiceApiCommand::HealthHttp => {
                        if let Some(ip) = openrhiza_ip {
                            plain_http_client = spawn_plain_http_health_client(ip);
                        } else {
                            dns_client = Some(spawn_dns_client(crate::api_v1::openrhiza_host()));
                            pending_dns_action =
                                Some(PendingDnsAction::PlainHttp(PlainHttpAction::Health));
                        }
                    }
                    crate::api_v1::ServiceApiCommand::HardwareReport => {
                        start_service_or_resolve(
                            ServiceApiPhase::HardwareReport,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::DriverQuery => {
                        start_service_or_resolve(
                            ServiceApiPhase::DriverQuery,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::SoftwareQuery => {
                        start_service_or_resolve(
                            ServiceApiPhase::SoftwareQuery,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::SkillQuery => {
                        start_service_or_resolve(
                            ServiceApiPhase::SkillQuery,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::WorkflowQuery => {
                        start_service_or_resolve(
                            ServiceApiPhase::WorkflowQuery,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::PolicyQuery => {
                        start_service_or_resolve(
                            ServiceApiPhase::PolicyQuery,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::EvaluationQuery => {
                        start_service_or_resolve(
                            ServiceApiPhase::EvaluationQuery,
                            false,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                    crate::api_v1::ServiceApiCommand::All => {
                        start_service_or_resolve(
                            ServiceApiPhase::Register,
                            true,
                            openrhiza_ip,
                            &mut dns_client,
                            &mut pending_dns_action,
                            &mut service_api_client,
                            &mut service_api_phase,
                            &mut service_api_chain_active,
                        );
                    }
                }
            }
        }

        if nexus_client.is_none()
            && service_api_client.is_none()
            && plain_http_client.is_none()
            && gemini_client.is_none()
            && dns_client.is_none()
            && service_api_phase == ServiceApiPhase::Idle
        {
            if let Some(command) = crate::api_v1::DRIVER_REGISTRY_QUEUE.pop() {
                if let Some(spec) = build_custom_service_request_from_driver_command(&command) {
                    if let Some(ip) = openrhiza_ip {
                        service_api_client = spawn_service_api_client_from_spec(&spec, ip);
                        if service_api_client.is_some() {
                            service_api_phase = spec.phase;
                        }
                    } else {
                        dns_client = Some(spawn_dns_client(crate::api_v1::openrhiza_host()));
                        pending_dns_action = Some(PendingDnsAction::CustomService(spec));
                    }
                }
            }
        }

        if nexus_client.is_none()
            && service_api_client.is_none()
            && plain_http_client.is_none()
            && gemini_client.is_none()
            && dns_client.is_none()
            && service_api_phase == ServiceApiPhase::Idle
        {
            if let Some(command) = crate::api_v1::SKILL_REGISTRY_QUEUE.pop() {
                if let Some(spec) = build_custom_service_request_from_skill_command(&command) {
                    if let Some(ip) = openrhiza_ip {
                        service_api_client = spawn_service_api_client_from_spec(&spec, ip);
                        if service_api_client.is_some() {
                            service_api_phase = spec.phase;
                        }
                    } else {
                        dns_client = Some(spawn_dns_client(crate::api_v1::openrhiza_host()));
                        pending_dns_action = Some(PendingDnsAction::CustomService(spec));
                    }
                }
            }
        }

        if nexus_client.is_none()
            && service_api_client.is_none()
            && gemini_client.is_none()
            && dns_client.is_none()
            && service_api_phase == ServiceApiPhase::Idle
        {
            if let Some(queued_prompt) = crate::api_v1::GEMINI_PROMPT_QUEUE.pop() {
                gemini_auto_upload_match_key = queued_prompt.auto_upload_match_key.clone();
                if queued_prompt.orchestrate {
                    crate::api_v1::clear_registry_context();
                    pending_orchestration_prompt = Some(queued_prompt.prompt);
                    prompt_orchestration_plan = pending_orchestration_prompt
                        .as_ref()
                        .map(|value| crate::prompt_orchestrator::build_plan(value.as_str()));
                    if let Some(plan) = prompt_orchestration_plan.as_ref() {
                        prompt_auto_apply_drivers = plan.auto_apply_drivers;
                        prompt_waiting_for_driver_registry = false;
                        crate::result_println!("[Orchestrator] {}", plan.summary);
                        crate::result_println!("[Orchestrator] registry steps: {}", plan.phases.len());
                        if plan.auto_apply_drivers {
                            crate::result_println!(
                                "[Orchestrator] automatic driver apply is enabled for this prompt."
                            );
                        }
                    }
                    start_next_prompt_orchestration_step(
                        &mut prompt_orchestration_plan,
                        &mut prompt_waiting_for_driver_registry,
                        openrhiza_ip,
                        gemini_ip,
                        &mut dns_client,
                        &mut pending_dns_action,
                        &mut service_api_client,
                        &mut service_api_phase,
                        &mut service_api_chain_active,
                        &mut gemini_client,
                        &mut gemini_request,
                        pending_orchestration_prompt.as_ref(),
                    );
                } else {
                    pending_orchestration_prompt = None;
                    prompt_orchestration_plan = None;
                    prompt_auto_apply_drivers = false;
                    prompt_waiting_for_driver_registry = false;
                    let request = GeminiRequest {
                        prompt: queued_prompt.prompt,
                        model_index: 0,
                    };
                    if let Some(ip) = gemini_ip {
                        gemini_client = spawn_gemini_client(ip, &request);
                        gemini_request = Some(request);
                    } else {
                        dns_client = Some(spawn_dns_client(crate::api_v1::gemini_host()));
                        pending_dns_action = Some(PendingDnsAction::Gemini(request));
                    }
                }
            }
        }

        if nexus_client.is_none()
            && service_api_client.is_none()
            && plain_http_client.is_none()
            && gemini_client.is_none()
            && dns_client.is_none()
            && service_api_phase == ServiceApiPhase::Idle
            && prompt_waiting_for_driver_registry
            && pending_orchestration_prompt.is_some()
            && prompt_orchestration_plan.is_some()
            && crate::api_v1::DRIVER_REGISTRY_QUEUE.is_empty()
        {
            start_next_prompt_orchestration_step(
                &mut prompt_orchestration_plan,
                &mut prompt_waiting_for_driver_registry,
                openrhiza_ip,
                gemini_ip,
                &mut dns_client,
                &mut pending_dns_action,
                &mut service_api_client,
                &mut service_api_phase,
                &mut service_api_chain_active,
                &mut gemini_client,
                &mut gemini_request,
                pending_orchestration_prompt.as_ref(),
            );
        }

        if let Some(client) = &mut dns_client {
            client.poll();

            if let Some(resolved_ip) = client.take_resolved_ip() {
                crate::net::destroy_socket(client.handle());
                let action = pending_dns_action.take();
                dns_client = None;

                match action {
                    Some(PendingDnsAction::Service {
                        phase,
                        chain_active,
                    }) => {
                        openrhiza_ip = Some(resolved_ip);
                        service_api_chain_active = chain_active;
                        service_api_client = spawn_service_api_client(phase, resolved_ip);
                        if service_api_client.is_some() {
                            service_api_phase = phase;
                        } else {
                            service_api_chain_active = false;
                            service_api_phase = ServiceApiPhase::Idle;
                        }
                    }
                    Some(PendingDnsAction::CustomService(spec)) => {
                        openrhiza_ip = Some(resolved_ip);
                        service_api_client = spawn_service_api_client_from_spec(&spec, resolved_ip);
                        if service_api_client.is_some() {
                            service_api_phase = spec.phase;
                        } else {
                            service_api_phase = ServiceApiPhase::Idle;
                        }
                    }
                    Some(PendingDnsAction::PlainHttp(action)) => {
                        openrhiza_ip = Some(resolved_ip);
                        plain_http_client = match action {
                            PlainHttpAction::Register => spawn_plain_http_register_client(resolved_ip),
                            PlainHttpAction::Health => spawn_plain_http_health_client(resolved_ip),
                        };
                    }
                    Some(PendingDnsAction::Gemini(request)) => {
                        gemini_ip = Some(resolved_ip);
                        gemini_client = spawn_gemini_client(resolved_ip, &request);
                        gemini_request = Some(request);
                    }
                    None => {}
                }
            } else if let Some(error) = client.error_message() {
                crate::net::destroy_socket(client.handle());
                crate::println!("[DNS] resolution failed: {}", error);
                dns_client = None;
                pending_dns_action = None;
                service_api_chain_active = false;
                service_api_phase = ServiceApiPhase::Idle;
            }
        }

        if let Some(client) = &mut service_api_client {
            client.poll();

            if let Some(response) = client.take_response() {
                crate::net::destroy_socket(client.handle());
                log_service_api_response(service_api_phase, &response);
                if response.status_code >= 200 && response.status_code < 300 {
                    handle_successful_service_api_side_effects(
                        service_api_phase,
                        &response,
                        prompt_auto_apply_drivers,
                    );
                } else if service_api_phase == ServiceApiPhase::DriverUpload {
                    let _ = crate::api_v1::take_pending_driver_upload_match_key();
                } else if service_api_phase == ServiceApiPhase::DriverDownload {
                    if let Some(pending) = crate::api_v1::take_pending_driver_download() {
                        handle_failed_driver_download(response.status_code, &pending);
                    }
                } else if service_api_phase == ServiceApiPhase::SkillDownload {
                    let _ = crate::api_v1::take_pending_skill_download();
                }

                if prompt_orchestration_plan.is_some() && is_prompt_orchestration_phase(service_api_phase)
                {
                    service_api_client = None;
                    service_api_phase = ServiceApiPhase::Idle;
                    service_api_chain_active = false;
                    start_next_prompt_orchestration_step(
                        &mut prompt_orchestration_plan,
                        &mut prompt_waiting_for_driver_registry,
                        openrhiza_ip,
                        gemini_ip,
                        &mut dns_client,
                        &mut pending_dns_action,
                        &mut service_api_client,
                        &mut service_api_phase,
                        &mut service_api_chain_active,
                        &mut gemini_client,
                        &mut gemini_request,
                        pending_orchestration_prompt.as_ref(),
                    );
                    continue;
                }

                service_api_phase = match service_api_phase {
                    ServiceApiPhase::Register => {
                        if service_api_chain_active {
                            service_api_client = openrhiza_ip
                                .and_then(|ip| spawn_service_api_client(ServiceApiPhase::HardwareReport, ip));
                            if service_api_client.is_some() {
                                ServiceApiPhase::HardwareReport
                            } else {
                                service_api_chain_active = false;
                                ServiceApiPhase::Done
                            }
                        } else {
                            ServiceApiPhase::Done
                        }
                    }
                    ServiceApiPhase::HardwareReport => {
                        if service_api_chain_active {
                            service_api_client = openrhiza_ip
                                .and_then(|ip| spawn_service_api_client(ServiceApiPhase::DriverQuery, ip));
                            if service_api_client.is_some() {
                                ServiceApiPhase::DriverQuery
                            } else {
                                service_api_chain_active = false;
                                ServiceApiPhase::Done
                            }
                        } else {
                            ServiceApiPhase::Done
                        }
                    }
                    ServiceApiPhase::DriverQuery => {
                        service_api_chain_active = false;
                        service_api_client = None;
                        ServiceApiPhase::Done
                    }
                    _ => ServiceApiPhase::Done,
                };

                if service_api_phase == ServiceApiPhase::Done {
                    service_api_chain_active = false;
                    service_api_client = None;
                    service_api_phase = ServiceApiPhase::Idle;
                }
            } else if let Some(error) = client.error_message() {
                crate::net::destroy_socket(client.handle());
                crate::println!("[API v1] {:?} failed: {}", service_api_phase_name(service_api_phase), error);
                if service_api_phase == ServiceApiPhase::DriverUpload {
                    let _ = crate::api_v1::take_pending_driver_upload_match_key();
                } else if service_api_phase == ServiceApiPhase::DriverDownload {
                    let _ = crate::api_v1::take_pending_driver_download();
                } else if service_api_phase == ServiceApiPhase::SkillDownload {
                    let _ = crate::api_v1::take_pending_skill_download();
                }

                if prompt_orchestration_plan.is_some() && is_prompt_orchestration_phase(service_api_phase)
                {
                    service_api_chain_active = false;
                    service_api_client = None;
                    service_api_phase = ServiceApiPhase::Idle;
                    start_next_prompt_orchestration_step(
                        &mut prompt_orchestration_plan,
                        &mut prompt_waiting_for_driver_registry,
                        openrhiza_ip,
                        gemini_ip,
                        &mut dns_client,
                        &mut pending_dns_action,
                        &mut service_api_client,
                        &mut service_api_phase,
                        &mut service_api_chain_active,
                        &mut gemini_client,
                        &mut gemini_request,
                        pending_orchestration_prompt.as_ref(),
                    );
                    continue;
                }
                service_api_chain_active = false;
                service_api_client = None;
                service_api_phase = ServiceApiPhase::Idle;
            }
        }

        if let Some(client) = &mut plain_http_client {
            client.poll();

            if let Some(response) = client.take_response() {
                crate::net::destroy_socket(client.handle());
                log_plain_http_response(&response);
                plain_http_client = None;
            } else if let Some(error) = client.error_message() {
                crate::net::destroy_socket(client.handle());
                crate::println!("[HTTP] request failed: {}", error);
                plain_http_client = None;
            }
        }

        if let Some(client) = &mut gemini_client {
            client.poll();

            if let Some(response) = client.take_response() {
                crate::net::destroy_socket(client.handle());
                if (200..300).contains(&response.status_code) {
                    if let Some(text) = extract_first_text_field(&response.body) {
                        if let Some(request) = &gemini_request {
                            crate::api_v1::record_last_gemini_response(
                                gemini_model_name(request.model_index),
                                text.as_str(),
                            );
                        }
                        if let Some(match_key) = gemini_auto_upload_match_key.take() {
                            match crate::api_v1::queue_driver_registry_command(
                                crate::api_v1::DriverRegistryCommand::UploadGenerated {
                                    match_key: match_key.clone(),
                                },
                            ) {
                                Ok(()) => crate::result_println!(
                                    "[Driver Runtime] Queued generated driver upload for {}",
                                    match_key
                                ),
                                Err(_) => crate::result_println!(
                                    "[Driver Runtime] Driver registry queue full; generated upload skipped for {}",
                                    match_key
                                ),
                            }
                        }
                    }
                    log_gemini_response(&response);
                    gemini_client = None;
                    gemini_request = None;
                    pending_orchestration_prompt = None;
                    prompt_orchestration_plan = None;
                    prompt_auto_apply_drivers = false;
                    prompt_waiting_for_driver_registry = false;
                } else if let Some(next_request) = advance_gemini_request(gemini_request.take()) {
                    let previous_model = gemini_model_name(next_request.model_index - 1);
                    let next_model = gemini_model_name(next_request.model_index);
                    crate::println!(
                        "[Gemini] {} returned status {}. Falling back to {}",
                        previous_model,
                        response.status_code,
                        next_model
                    );
                    gemini_client = gemini_ip.and_then(|ip| spawn_gemini_client(ip, &next_request));
                    gemini_request = Some(next_request);
                } else {
                    log_gemini_response(&response);
                    gemini_client = None;
                    gemini_request = None;
                    pending_orchestration_prompt = None;
                    prompt_orchestration_plan = None;
                    prompt_auto_apply_drivers = false;
                    prompt_waiting_for_driver_registry = false;
                    gemini_auto_upload_match_key = None;
                }
            } else if let Some(error) = client.error_message() {
                crate::net::destroy_socket(client.handle());
                if let Some(next_request) = advance_gemini_request(gemini_request.take()) {
                    let previous_model = gemini_model_name(next_request.model_index - 1);
                    let next_model = gemini_model_name(next_request.model_index);
                    crate::println!(
                        "[Gemini] {} failed: {}. Falling back to {}",
                        previous_model,
                        error,
                        next_model
                    );
                    gemini_client = gemini_ip.and_then(|ip| spawn_gemini_client(ip, &next_request));
                    gemini_request = Some(next_request);
                } else {
                    crate::println!("[Gemini] request failed: {}", error);
                    gemini_client = None;
                    gemini_request = None;
                    pending_orchestration_prompt = None;
                    prompt_orchestration_plan = None;
                    prompt_auto_apply_drivers = false;
                    prompt_waiting_for_driver_registry = false;
                    gemini_auto_upload_match_key = None;
                }
            }
        }

        while let Some(data) = rhiza.poll_host_data() {
            if data == 0xFD {
                keymap_index = 0; 
                *crate::task::keyboard::DYNAMIC_KEYMAP.lock() = [0x3F; 256];
                crate::task::keyboard::KEYMAP_OVERRIDE_ACTIVE.store(false, core::sync::atomic::Ordering::Relaxed);
            } else if data == 0xFB {
                crate::println!("[*] AI is generating e1000 LAN driver... Please wait.");
            } else if data == 0xFA {
                crate::println!("[!] Failed to generate LAN driver.");
            } else if !receiving_wasm && !receiving_wasm_size && data == 0xFC {
                receiving_wasm_size = true;
                wasm_size_idx = 0;
                wasm_buffer.clear();
            } else if receiving_wasm_size {
                wasm_size_buf[wasm_size_idx] = data;
                wasm_size_idx += 1;
                if wasm_size_idx == 4 {
                    expected_wasm_size = u32::from_le_bytes(wasm_size_buf) as usize;
                    receiving_wasm_size = false;
                    receiving_wasm = true;
                    crate::println!("[OS] Receiving Wasm binary of size {} bytes...", expected_wasm_size);
                }
            } else if receiving_wasm {
                wasm_buffer.push(data);
                if wasm_buffer.len() == expected_wasm_size {
                    receiving_wasm = false;
                    crate::println!("[OS] Wasm binary received! Executing Sandbox...");
                    match rhiza.execute_wasm_sandbox(&wasm_buffer) {
                        os_core_seed::ExecutionResult::Success(msg) => {
                            crate::println!("[Sandbox] {}", msg);
                            crate::arch::x86_64::serial::send_byte(0xF8); 
                        },
                        os_core_seed::ExecutionResult::Panic(err) => {
                            crate::println!("[Sandbox] {}", err);
                            crate::arch::x86_64::serial::send_byte(0xF9); 
                        }
                    }
                }
            } else if data == 0xFE && keymap_index < 256 {
                keymap_index = 0;
                crate::task::keyboard::KEYMAP_OVERRIDE_ACTIVE.store(false, core::sync::atomic::Ordering::Relaxed);
                crate::println!("[!] Calibration Failed. Try again:");
            } else if keymap_index < 256 {
                crate::task::keyboard::DYNAMIC_KEYMAP.lock()[keymap_index] = data;
                keymap_index += 1;
                if keymap_index == 256 {
                    crate::task::keyboard::KEYMAP_OVERRIDE_ACTIVE.store(true, core::sync::atomic::Ordering::Relaxed);
                    crate::println!("[+] Keyboard Driver Loaded.");
                }
            }
        }
    }
}

fn start_service_or_resolve(
    phase: ServiceApiPhase,
    chain_active: bool,
    openrhiza_ip: Option<Ipv4Address>,
    dns_client: &mut Option<crate::dns::DnsClient>,
    pending_dns_action: &mut Option<PendingDnsAction>,
    service_api_client: &mut Option<crate::https::ApiClient>,
    service_api_phase: &mut ServiceApiPhase,
    service_api_chain_active: &mut bool,
) {
    *service_api_chain_active = chain_active;
    if let Some(ip) = openrhiza_ip {
        *service_api_client = spawn_service_api_client(phase, ip);
        if service_api_client.is_some() {
            *service_api_phase = phase;
        } else {
            *service_api_chain_active = false;
            *service_api_phase = ServiceApiPhase::Idle;
        }
    } else {
        *dns_client = Some(spawn_dns_client(crate::api_v1::openrhiza_host()));
        *pending_dns_action = Some(PendingDnsAction::Service { phase, chain_active });
    }
}

fn is_prompt_orchestration_phase(phase: ServiceApiPhase) -> bool {
    matches!(
        phase,
        ServiceApiPhase::DriverQuery
            | ServiceApiPhase::SoftwareQuery
            | ServiceApiPhase::SkillQuery
            | ServiceApiPhase::WorkflowQuery
            | ServiceApiPhase::PolicyQuery
            | ServiceApiPhase::EvaluationQuery
    )
}

fn start_next_prompt_orchestration_step(
    plan: &mut Option<crate::prompt_orchestrator::PromptOrchestrationPlan>,
    wait_for_driver_registry: &mut bool,
    openrhiza_ip: Option<Ipv4Address>,
    gemini_ip: Option<Ipv4Address>,
    dns_client: &mut Option<crate::dns::DnsClient>,
    pending_dns_action: &mut Option<PendingDnsAction>,
    service_api_client: &mut Option<crate::https::ApiClient>,
    service_api_phase: &mut ServiceApiPhase,
    service_api_chain_active: &mut bool,
    gemini_client: &mut Option<crate::https::ApiClient>,
    gemini_request: &mut Option<GeminiRequest>,
    prompt: Option<&String>,
) {
    if let Some(plan_ref) = plan.as_mut() {
        if let Some(phase) = plan_ref.phases.get(plan_ref.next_index).copied() {
            plan_ref.next_index += 1;
            start_service_or_resolve(
                map_registry_query_phase_to_service_phase(phase),
                false,
                openrhiza_ip,
                dns_client,
                pending_dns_action,
                service_api_client,
                service_api_phase,
                service_api_chain_active,
            );
            return;
        }

        if plan_ref.auto_apply_drivers && !crate::api_v1::DRIVER_REGISTRY_QUEUE.is_empty() {
            *wait_for_driver_registry = true;
            return;
        }
    }

    *wait_for_driver_registry = false;
    let local_context_block = plan
        .as_ref()
        .map(|plan_ref| plan_ref.local_context_block.clone());
    let orchestration_summary = plan.as_ref().map(|plan_ref| plan_ref.summary.clone());
    *plan = None;
    let Some(prompt) = prompt.cloned() else {
        return;
    };
    let llm_prompt = if let Some(local_context_block) = local_context_block {
        let mut composed = String::new();
        composed.push_str(local_context_block.as_str());
        composed.push('\n');
        composed.push_str("Orchestration summary: ");
        composed.push_str(
            orchestration_summary
                .as_deref()
                .unwrap_or("generic prompt"),
        );
        composed.push_str("\n\nUser task:\n");
        composed.push_str(prompt.as_str());
        composed
    } else {
        prompt.clone()
    };
    let request = GeminiRequest {
        prompt: llm_prompt,
        model_index: 0,
    };
    if let Some(ip) = gemini_ip {
        *gemini_client = spawn_gemini_client(ip, &request);
        *gemini_request = Some(request);
    } else {
        *dns_client = Some(spawn_dns_client(crate::api_v1::gemini_host()));
        *pending_dns_action = Some(PendingDnsAction::Gemini(request));
    }
}

fn maybe_queue_followup_workflow_for_skill(skill_id: &str) {
    match skill_id {
        "skill_display_console_mode_v1" => {
            match crate::api_v1::queue_custom_workflow_query(
                "display console expansion framebuffer transition gui bootstrap",
                &["display_console", "gui_session", "registry_lookup", "sandbox", "gemini"],
            ) {
                Ok(()) => crate::result_println!(
                    "[Display Workflow] Queued workflow query for console expansion."
                ),
                Err(_) => crate::result_println!(
                    "[Display Workflow] Workflow query queue full."
                ),
            }
        }
        "skill_gui_session_bootstrap_v1" => {
            match crate::api_v1::queue_custom_workflow_query(
                "gui session bootstrap compositor input orchestration",
                &["gui_session", "display_console", "registry_lookup", "sandbox", "gemini"],
            ) {
                Ok(()) => crate::result_println!(
                    "[Display Workflow] Queued workflow query for GUI bootstrap."
                ),
                Err(_) => crate::result_println!(
                    "[Display Workflow] Workflow query queue full."
                ),
            }
        }
        "skill_display_framebuffer_mode_v1" => {
            match crate::api_v1::queue_custom_workflow_query(
                "gui session bootstrap compositor input orchestration",
                &["display_framebuffer", "gui_session", "registry_lookup", "sandbox", "gemini"],
            ) {
                Ok(()) => crate::result_println!(
                    "[Display Workflow] Queued GUI bootstrap workflow after framebuffer mode."
                ),
                Err(_) => crate::result_println!(
                    "[Display Workflow] Workflow query queue full."
                ),
            }
        }
        _ => {}
    }
}

fn map_registry_query_phase_to_service_phase(
    phase: crate::prompt_orchestrator::RegistryQueryPhase,
) -> ServiceApiPhase {
    match phase {
        crate::prompt_orchestrator::RegistryQueryPhase::Driver => ServiceApiPhase::DriverQuery,
        crate::prompt_orchestrator::RegistryQueryPhase::Software => ServiceApiPhase::SoftwareQuery,
        crate::prompt_orchestrator::RegistryQueryPhase::Skill => ServiceApiPhase::SkillQuery,
        crate::prompt_orchestrator::RegistryQueryPhase::Workflow => ServiceApiPhase::WorkflowQuery,
        crate::prompt_orchestrator::RegistryQueryPhase::Policy => ServiceApiPhase::PolicyQuery,
        crate::prompt_orchestrator::RegistryQueryPhase::Evaluation => ServiceApiPhase::EvaluationQuery,
    }
}

fn spawn_dns_client(hostname: &'static str) -> crate::dns::DnsClient {
    let socket_handle = crate::net::create_udp_socket(512, 512);
    crate::dns::DnsClient::new(socket_handle, crate::dns::DEFAULT_DNS_SERVER, hostname)
}

fn spawn_service_api_client_from_spec(
    spec: &ServiceRequestSpec,
    target_ip: Ipv4Address,
) -> Option<crate::https::ApiClient> {
    let socket_handle = crate::net::create_tcp_socket();
    crate::println!(
        "[API v1] Starting {} -> {}",
        service_api_phase_name(spec.phase),
        spec.path
    );

    Some(crate::https::ApiClient::new(
        socket_handle,
        smoltcp::wire::IpAddress::Ipv4(target_ip),
        443,
        crate::api_v1::openrhiza_host(),
        crate::https::ApiMethod::Post,
        spec.path.as_str(),
        spec.body.as_bytes().to_vec(),
    ))
}

fn spawn_service_api_client(
    phase: ServiceApiPhase,
    target_ip: Ipv4Address,
) -> Option<crate::https::ApiClient> {
    let profile = crate::identity::current_profile()?;
    let socket_handle = crate::net::create_tcp_socket();
    let (method, path, body) = match phase {
        ServiceApiPhase::Register => (
            crate::https::ApiMethod::Post,
            "/api/v1/node/register",
            crate::api_v1::build_node_register_request(&profile).into_bytes(),
        ),
        ServiceApiPhase::HealthHttps => (
            crate::https::ApiMethod::Get,
            "/api/health",
            alloc::vec::Vec::new(),
        ),
        ServiceApiPhase::RootHttps => (
            crate::https::ApiMethod::Get,
            "/",
            alloc::vec::Vec::new(),
        ),
        ServiceApiPhase::HardwareReport => (
            crate::https::ApiMethod::Post,
            "/api/v1/hardware/report",
            crate::api_v1::build_hardware_report_request(&profile).into_bytes(),
        ),
        ServiceApiPhase::DriverQuery => (
            crate::https::ApiMethod::Post,
            "/api/v1/driver/query",
            crate::api_v1::build_driver_query_request(&profile).into_bytes(),
        ),
        ServiceApiPhase::SoftwareQuery => (
            crate::https::ApiMethod::Post,
            "/api/v1/software/query",
            crate::api_v1::build_software_query_request(&profile).into_bytes(),
        ),
        ServiceApiPhase::SkillQuery => (
            crate::https::ApiMethod::Post,
            "/api/v1/skill/query",
            crate::api_v1::build_skill_query_request(&profile).into_bytes(),
        ),
        ServiceApiPhase::WorkflowQuery => (
            crate::https::ApiMethod::Post,
            "/api/v1/workflow/query",
            crate::api_v1::build_workflow_query_request(&profile).into_bytes(),
        ),
        ServiceApiPhase::PolicyQuery => (
            crate::https::ApiMethod::Post,
            "/api/v1/policy/query",
            crate::api_v1::build_policy_query_request(&profile).into_bytes(),
        ),
        ServiceApiPhase::EvaluationQuery => (
            crate::https::ApiMethod::Post,
            "/api/v1/evaluation/query",
            crate::api_v1::build_evaluation_query_request(&profile).into_bytes(),
        ),
        _ => return None,
    };

    crate::println!(
        "[API v1] Starting {} -> {}",
        service_api_phase_name(phase),
        path
    );

    Some(crate::https::ApiClient::new(
        socket_handle,
        smoltcp::wire::IpAddress::Ipv4(target_ip),
        443,
        crate::api_v1::openrhiza_host(),
        method,
        path,
        body,
    ))
}

fn build_custom_service_request_from_driver_command(
    command: &crate::api_v1::DriverRegistryCommand,
) -> Option<ServiceRequestSpec> {
    let profile = crate::identity::current_profile()?;
    match command {
        crate::api_v1::DriverRegistryCommand::UploadGenerated { match_key } => {
            let payload = match crate::api_v1::last_gemini_text() {
                Some(payload) if !payload.is_empty() => payload,
                _ => {
                    crate::result_println!("[Driver] No generated Gemini text is available to upload.");
                    return None;
                }
            };
            crate::api_v1::record_pending_driver_upload(match_key);
            Some(ServiceRequestSpec {
                phase: ServiceApiPhase::DriverUpload,
                path: String::from("/api/v1/driver/upload"),
                body: crate::api_v1::build_driver_upload_request(&profile, match_key, payload.as_str()),
            })
        }
        crate::api_v1::DriverRegistryCommand::DownloadCandidate { driver_id, match_key } => {
            let scheduled_activation = crate::api_v1::take_scheduled_driver_download_activation(
                driver_id,
                match_key,
            );
            let activation_source = scheduled_activation
                .as_ref()
                .map(|entry| entry.source.as_str())
                .unwrap_or("registry-download");
            crate::api_v1::record_pending_driver_download(
                driver_id,
                match_key,
                scheduled_activation.is_some(),
                activation_source,
            );
            Some(ServiceRequestSpec {
                phase: ServiceApiPhase::DriverDownload,
                path: String::from("/api/v1/driver/download"),
                body: crate::api_v1::build_driver_download_request(&profile, driver_id, match_key),
            })
        }
        crate::api_v1::DriverRegistryCommand::Comment { driver_id, comment } => Some(ServiceRequestSpec {
            phase: ServiceApiPhase::DriverComment,
            path: String::from("/api/v1/driver/comment"),
            body: crate::api_v1::build_driver_comment_request(&profile, driver_id, comment),
        }),
        crate::api_v1::DriverRegistryCommand::Vote { driver_id, vote } => Some(ServiceRequestSpec {
            phase: ServiceApiPhase::DriverVote,
            path: String::from("/api/v1/driver/vote"),
            body: crate::api_v1::build_driver_vote_request(&profile, driver_id, *vote),
        }),
        crate::api_v1::DriverRegistryCommand::UploadEvaluation {
            driver_id,
            match_key,
            note,
        } => Some(ServiceRequestSpec {
            phase: ServiceApiPhase::EvaluationUpload,
            path: String::from("/api/v1/evaluation/upload"),
            body: crate::api_v1::build_driver_evaluation_upload_request(
                &profile,
                driver_id,
                match_key,
                note,
            ),
        }),
    }
}

fn build_custom_service_request_from_skill_command(
    command: &crate::api_v1::SkillRegistryCommand,
) -> Option<ServiceRequestSpec> {
    let profile = crate::identity::current_profile()?;
    match command {
        crate::api_v1::SkillRegistryCommand::DownloadCandidate {
            skill_id,
            auto_load,
            auto_run,
        } => {
            crate::api_v1::record_pending_skill_download(skill_id.as_str(), *auto_load, *auto_run);
            Some(ServiceRequestSpec {
                phase: ServiceApiPhase::SkillDownload,
                path: String::from("/api/v1/skill/download"),
                body: crate::api_v1::build_skill_download_request(&profile, skill_id),
            })
        }
    }
}

fn spawn_gemini_client(
    target_ip: Ipv4Address,
    request: &GeminiRequest,
) -> Option<crate::https::ApiClient> {
    let api_key = match crate::api_v1::gemini_api_key() {
        Some(key) => key,
        None => {
            crate::println!(
                "[Gemini] OPENRHIZA_GEMINI_API_KEY is not set at build time. Rebuild with the key."
            );
            return None;
        }
    };

    let socket_handle = crate::net::create_tcp_socket();
    let model = gemini_model_name(request.model_index);
    let path = crate::api_v1::build_gemini_generate_path(model);
    let body = crate::api_v1::build_gemini_generate_request(&request.prompt).into_bytes();
    let headers = vec![(
        String::from("x-goog-api-key"),
        String::from(api_key),
    )];

    crate::println!(
        "[Gemini] Starting direct request with {} -> {}{}",
        model,
        crate::api_v1::gemini_host(),
        path
    );

    Some(crate::https::ApiClient::new_with_headers(
        socket_handle,
        smoltcp::wire::IpAddress::Ipv4(target_ip),
        443,
        crate::api_v1::gemini_host(),
        crate::https::ApiMethod::Post,
        path.as_str(),
        body,
        headers,
    ))
}

fn spawn_plain_http_register_client(
    target_ip: Ipv4Address,
) -> Option<crate::https::PlainHttpClient> {
    let profile = crate::identity::current_profile()?;
    let body = crate::api_v1::build_node_register_request(&profile);
    let request = format!(
        "POST /api/v1/node/register HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        crate::api_v1::openrhiza_host(),
        body.len(),
        body
    );
    let socket_handle = crate::net::create_tcp_socket();
    crate::println!("[HTTP] Starting plain register -> /api/v1/node/register");
    Some(crate::https::PlainHttpClient::new(
        socket_handle,
        smoltcp::wire::IpAddress::Ipv4(target_ip),
        80,
        request.into_bytes(),
    ))
}

fn spawn_plain_http_health_client(
    target_ip: Ipv4Address,
) -> Option<crate::https::PlainHttpClient> {
    let request = format!(
        "GET /api/health HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        crate::api_v1::openrhiza_host()
    );
    let socket_handle = crate::net::create_tcp_socket();
    crate::println!("[HTTP] Starting plain health -> /api/health");
    Some(crate::https::PlainHttpClient::new(
        socket_handle,
        smoltcp::wire::IpAddress::Ipv4(target_ip),
        80,
        request.into_bytes(),
    ))
}

fn advance_gemini_request(request: Option<GeminiRequest>) -> Option<GeminiRequest> {
    let mut request = request?;
    if request.model_index + 1 >= crate::api_v1::gemini_models().len() {
        return None;
    }
    request.model_index += 1;
    Some(request)
}

fn gemini_model_name(index: usize) -> &'static str {
    crate::api_v1::gemini_models()
        .get(index)
        .copied()
        .unwrap_or("unknown-model")
}

fn log_service_api_response(phase: ServiceApiPhase, response: &crate::https::ApiResponse) {
    crate::result_println!(
        "[API v1] {} response status: {}",
        service_api_phase_name(phase),
        response.status_code
    );

    log_service_api_summary(phase, &response.body);

    match core::str::from_utf8(&response.body) {
        Ok(body) => crate::println!("[API v1] {} response body: {}", service_api_phase_name(phase), body),
        Err(_) => crate::println!(
            "[API v1] {} response body is not valid UTF-8 ({} bytes)",
            service_api_phase_name(phase),
            response.body.len()
        ),
    }
}

fn log_service_api_summary(phase: ServiceApiPhase, body: &[u8]) {
    let Some(body_text) = core::str::from_utf8(body).ok() else {
        return;
    };

    match phase {
        ServiceApiPhase::Register => {
            if let Some(node_id) = extract_json_string(body_text, "node_id") {
                crate::result_println!("[API v1] register node_id: {}", node_id);
            }
            if let Some(trust_tier) = extract_json_string(body_text, "trust_tier") {
                crate::result_println!("[API v1] register trust_tier: {}", trust_tier);
            }
        }
        ServiceApiPhase::HardwareReport => {
            if let Some(profile_id) = extract_json_string(body_text, "profile_id") {
                crate::result_println!("[API v1] hardware profile_id: {}", profile_id);
            }
            if let Some(recognized) = extract_json_number(body_text, "recognized_devices") {
                crate::result_println!("[API v1] hardware recognized_devices: {}", recognized);
            }
            if let Some(unknown) = extract_json_number(body_text, "unknown_devices") {
                crate::result_println!("[API v1] hardware unknown_devices: {}", unknown);
            }
        }
        ServiceApiPhase::DriverQuery => {
            if let Some(requested) = extract_json_number(body_text, "requested_devices") {
                crate::result_println!("[API v1] driver requested_devices: {}", requested);
            }
            if let Some(matched) = extract_json_number(body_text, "matched_devices") {
                crate::result_println!("[API v1] driver matched_devices: {}", matched);
            }
            if let Some(unmatched) = extract_json_number(body_text, "unmatched_devices") {
                crate::result_println!("[API v1] driver unmatched_devices: {}", unmatched);
            }

            let recommendation_count = body_text.matches("\"driver_id\":\"").count();
            crate::result_println!(
                "[API v1] driver recommendations: {}",
                recommendation_count
            );

            let match_keys = extract_json_string_list(body_text, "match_key");
            let driver_ids = extract_json_string_list(body_text, "driver_id");
            let mut candidates = alloc::vec::Vec::new();
            for (driver_id, match_key) in driver_ids.iter().zip(match_keys.iter()) {
                candidates.push(crate::api_v1::DriverRegistryCandidate {
                    driver_id: driver_id.clone(),
                    match_key: match_key.clone(),
                });
            }
            crate::api_v1::record_driver_registry_candidates(&candidates);
            for driver_id in extract_json_string_list(body_text, "driver_id").iter().take(3) {
                crate::result_println!("[API v1] driver candidate: {}", driver_id);
            }
            if !driver_ids.is_empty() {
                crate::api_v1::record_driver_registry_summary(
                    summarize_registry_ids(&driver_ids, 4).as_str(),
                );
            }
            log_unmatched_local_devices(&match_keys);
        }
        ServiceApiPhase::SoftwareQuery => {
            let package_count = body_text.matches("\"package_id\":\"").count();
            crate::result_println!("[API v1] software packages returned: {}", package_count);
            let package_ids = extract_json_string_list(body_text, "package_id");
            for package_id in package_ids.iter().take(4) {
                crate::result_println!("[API v1] software candidate: {}", package_id);
            }
            if !package_ids.is_empty() {
                let summary = summarize_registry_ids(&package_ids, 4);
                crate::api_v1::record_software_registry_summary(summary.as_str());
                let _ = crate::capability_cache::persist_registry_summary(
                    crate::capability_cache::RegistryDomain::Software,
                    summary.as_str(),
                );
            }
        }
        ServiceApiPhase::SkillQuery => {
            let skill_count = body_text.matches("\"skill_id\":\"").count();
            crate::result_println!("[API v1] skills returned: {}", skill_count);
            let skill_ids = extract_json_string_list(body_text, "skill_id");
            for skill_id in skill_ids.iter().take(4) {
                crate::result_println!("[API v1] skill candidate: {}", skill_id);
            }
            if !skill_ids.is_empty() {
                let summary = summarize_registry_ids(&skill_ids, 4);
                crate::api_v1::record_skill_registry_summary(summary.as_str());
                let _ = crate::capability_cache::persist_registry_summary(
                    crate::capability_cache::RegistryDomain::Skill,
                    summary.as_str(),
                );
            }
        }
        ServiceApiPhase::WorkflowQuery => {
            let workflow_count = body_text.matches("\"workflow_id\":\"").count();
            crate::result_println!("[API v1] workflows returned: {}", workflow_count);
            let workflow_ids = extract_json_string_list(body_text, "workflow_id");
            for workflow_id in workflow_ids.iter().take(4) {
                crate::result_println!("[API v1] workflow candidate: {}", workflow_id);
            }
            if !workflow_ids.is_empty() {
                let summary = summarize_registry_ids(&workflow_ids, 4);
                crate::api_v1::record_workflow_registry_summary(summary.as_str());
                let _ = crate::capability_cache::persist_registry_summary(
                    crate::capability_cache::RegistryDomain::Workflow,
                    summary.as_str(),
                );
                if workflow_ids
                    .iter()
                    .any(|workflow_id| workflow_id == "workflow_display_expand_v1")
                    && crate::skill_cache::find_cached_skill("skill_display_framebuffer_mode_v1").is_none()
                {
                    match crate::api_v1::queue_skill_registry_command(
                        crate::api_v1::SkillRegistryCommand::DownloadCandidate {
                            skill_id: String::from("skill_display_framebuffer_mode_v1"),
                            auto_load: true,
                            auto_run: true,
                        },
                    ) {
                        Ok(()) => crate::result_println!(
                            "[Display Workflow] Queued framebuffer mode skill download."
                        ),
                        Err(_) => crate::result_println!(
                            "[Display Workflow] Skill registry queue full."
                        ),
                    }
                }
                if workflow_ids
                    .iter()
                    .any(|workflow_id| workflow_id == "workflow_display_expand_v1")
                    && crate::skill_cache::find_cached_skill("skill_gui_session_bootstrap_v1").is_none()
                {
                    match crate::api_v1::queue_skill_registry_command(
                        crate::api_v1::SkillRegistryCommand::DownloadCandidate {
                            skill_id: String::from("skill_gui_session_bootstrap_v1"),
                            auto_load: true,
                            auto_run: true,
                        },
                    ) {
                        Ok(()) => crate::result_println!(
                            "[Display Workflow] Queued GUI bootstrap skill download."
                        ),
                        Err(_) => crate::result_println!(
                            "[Display Workflow] Skill registry queue full."
                        ),
                    }
                }
                if workflow_ids
                    .iter()
                    .any(|workflow_id| workflow_id == "workflow_gui_bootstrap_v1")
                    && crate::skill_cache::find_cached_skill("skill_gui_compositor_seed_v1").is_none()
                {
                    match crate::api_v1::queue_skill_registry_command(
                        crate::api_v1::SkillRegistryCommand::DownloadCandidate {
                            skill_id: String::from("skill_gui_compositor_seed_v1"),
                            auto_load: true,
                            auto_run: true,
                        },
                    ) {
                        Ok(()) => crate::result_println!(
                            "[Display Workflow] Queued compositor seed skill download."
                        ),
                        Err(_) => crate::result_println!(
                            "[Display Workflow] Skill registry queue full."
                        ),
                    }
                }
            }
        }
        ServiceApiPhase::PolicyQuery => {
            let policy_count = body_text.matches("\"policy_id\":\"").count();
            crate::result_println!("[API v1] policies returned: {}", policy_count);
            let policy_ids = extract_json_string_list(body_text, "policy_id");
            for policy_id in policy_ids.iter().take(4) {
                crate::result_println!("[API v1] policy candidate: {}", policy_id);
            }
            if !policy_ids.is_empty() {
                let summary = summarize_registry_ids(&policy_ids, 4);
                crate::api_v1::record_policy_registry_summary(summary.as_str());
                let _ = crate::capability_cache::persist_registry_summary(
                    crate::capability_cache::RegistryDomain::Policy,
                    summary.as_str(),
                );
            }
        }
        ServiceApiPhase::EvaluationQuery => {
            let evaluation_count = body_text.matches("\"evaluation_id\":\"").count();
            crate::result_println!("[API v1] evaluations returned: {}", evaluation_count);
            let evaluation_ids = extract_json_string_list(body_text, "evaluation_id");
            for evaluation_id in evaluation_ids.iter().take(4) {
                crate::result_println!("[API v1] evaluation id: {}", evaluation_id);
            }
            if !evaluation_ids.is_empty() {
                let summary = summarize_registry_ids(&evaluation_ids, 4);
                crate::api_v1::record_evaluation_registry_summary(summary.as_str());
                let _ = crate::capability_cache::persist_registry_summary(
                    crate::capability_cache::RegistryDomain::Evaluation,
                    summary.as_str(),
                );
            }
        }
        ServiceApiPhase::EvaluationUpload => {
            if let Some(evaluation_id) = extract_json_string(body_text, "evaluation_id") {
                crate::result_println!("[API v1] uploaded evaluation_id: {}", evaluation_id);
            }
            if let Some(subject_type) = extract_json_string(body_text, "subject_type") {
                crate::result_println!("[API v1] evaluation subject_type: {}", subject_type);
            }
            if let Some(subject_id) = extract_json_string(body_text, "subject_id") {
                crate::result_println!("[API v1] evaluation subject_id: {}", subject_id);
            }
        }
        ServiceApiPhase::SkillDownload => {
            if let Some(skill_id) = extract_json_string(body_text, "skill_id") {
                crate::result_println!("[API v1] downloaded skill_id: {}", skill_id);
            }
            if let Some(artifact_id) = extract_json_string(body_text, "artifact_id") {
                crate::result_println!("[API v1] downloaded skill artifact_id: {}", artifact_id);
            }
            if let Some(payload_hex) = extract_json_string(body_text, "payload_hex") {
                crate::result_println!("[API v1] downloaded skill payload hex bytes: {}", payload_hex.len() / 2);
            }
        }
        ServiceApiPhase::DriverDownload => {
            if let Some(driver_id) = extract_json_string(body_text, "driver_id") {
                crate::result_println!("[API v1] downloaded driver_id: {}", driver_id);
            }
            if let Some(match_key) = extract_json_string(body_text, "match_key") {
                crate::result_println!("[API v1] download match_key: {}", match_key);
            }
            if let Some(artifact_id) = extract_json_string(body_text, "artifact_id") {
                crate::result_println!("[API v1] downloaded artifact_id: {}", artifact_id);
            }
            if let Some(payload_kind) = extract_json_string(body_text, "payload_kind") {
                crate::result_println!("[API v1] downloaded payload_kind: {}", payload_kind);
            }
            if let Some(payload_text) = extract_json_string_decoded(body_text, "payload_text") {
                crate::result_println!("[API v1] downloaded payload bytes: {}", payload_text.len());
            }
        }
        ServiceApiPhase::DriverUpload => {
            if let Some(driver_id) = extract_json_string(body_text, "driver_id") {
                crate::result_println!("[API v1] uploaded driver_id: {}", driver_id);
            }
            if let Some(artifact_id) = extract_json_string(body_text, "artifact_id") {
                crate::result_println!("[API v1] uploaded artifact_id: {}", artifact_id);
            }
        }
        ServiceApiPhase::DriverComment => {
            if let Some(comment_id) = extract_json_string(body_text, "comment_id") {
                crate::result_println!("[API v1] driver comment_id: {}", comment_id);
            }
        }
        ServiceApiPhase::DriverVote => {
            if let Some(upvotes) = extract_json_number(body_text, "upvotes") {
                crate::result_println!("[API v1] driver upvotes: {}", upvotes);
            }
            if let Some(downvotes) = extract_json_number(body_text, "downvotes") {
                crate::result_println!("[API v1] driver downvotes: {}", downvotes);
            }
            if let Some(score) = extract_json_signed_number(body_text, "score") {
                crate::result_println!("[API v1] driver vote score: {}", score);
            }
        }
        _ => {}
    }
}

fn handle_successful_service_api_side_effects(
    phase: ServiceApiPhase,
    response: &crate::https::ApiResponse,
    auto_apply_drivers: bool,
) {
    let Some(body_text) = core::str::from_utf8(&response.body).ok() else {
        return;
    };

    match phase {
        ServiceApiPhase::DriverQuery => {
            maybe_queue_driver_candidate_download(body_text, auto_apply_drivers);
        }
        ServiceApiPhase::SkillDownload => {
            let pending = crate::api_v1::take_pending_skill_download();
            let skill_id = extract_json_string(body_text, "skill_id")
                .map(String::from)
                .or_else(|| pending.as_ref().map(|value| value.skill_id.clone()));
            let payload_hex = extract_json_string(body_text, "payload_hex").unwrap_or_default();
            let artifact_id = extract_json_string(body_text, "artifact_id");

            let Some(skill_id) = skill_id else {
                return;
            };

            if payload_hex.is_empty() {
                crate::result_println!(
                    "[Skill Runtime] Download response for {} did not include a payload.",
                    skill_id
                );
                return;
            }

            let Some(payload) = decode_hex_string(payload_hex) else {
                crate::result_println!(
                    "[Skill Runtime] Downloaded payload for {} is not valid hex.",
                    skill_id
                );
                return;
            };

            match crate::skill_cache::persist_downloaded_skill(skill_id.as_str(), payload.as_slice()) {
                Ok(fat_name) => {
                    crate::result_println!(
                        "[Skill Cache] Stored downloaded skill {} in {}",
                        skill_id,
                        fat_name
                    );
                    let _ = crate::capability_cache::persist_registry_summary(
                        crate::capability_cache::RegistryDomain::Skill,
                        crate::skill_runtime::local_skill_ids_summary().as_str(),
                    );

                    if pending.as_ref().map(|value| value.auto_run).unwrap_or(false) {
                        crate::skill_runtime::schedule_auto_run(skill_id.as_str());
                    }

                    if pending.as_ref().map(|value| value.auto_load).unwrap_or(false) {
                        match crate::skill_runtime::queue_load(skill_id.as_str()) {
                            Ok(slot) => crate::result_println!(
                                "[Skill Runtime] Queued {} from {}",
                                skill_id,
                                slot
                            ),
                            Err(error) => crate::result_println!(
                                "[Skill Runtime] {} load deferred: {}",
                                skill_id,
                                error
                            ),
                        }
                    }

                    if let Some(artifact_id) = artifact_id {
                        crate::result_println!(
                            "[Skill Cache] Cached artifact {} for {}",
                            artifact_id,
                            skill_id
                        );
                    }
                }
                Err(error) => crate::result_println!(
                    "[Skill Cache] Failed to store {}: {}",
                    skill_id,
                    error
                ),
            }
        }
        ServiceApiPhase::DriverDownload => {
            let pending = crate::api_v1::take_pending_driver_download();
            let driver_id = extract_json_string(body_text, "driver_id")
                .map(String::from)
                .or_else(|| pending.as_ref().map(|value| value.driver_id.clone()));
            let match_key = extract_json_string(body_text, "match_key")
                .map(String::from)
                .or_else(|| pending.as_ref().map(|value| value.match_key.clone()));
            let artifact_id = extract_json_string(body_text, "artifact_id").map(String::from);
            let payload_kind = extract_json_string(body_text, "payload_kind")
                .map(String::from)
                .unwrap_or_else(|| String::from("unknown"));
            let payload_text = extract_json_string_decoded(body_text, "payload_text").unwrap_or_default();

            let (Some(driver_id), Some(match_key)) = (driver_id, match_key) else {
                return;
            };

            crate::driver_runtime::cache_binding_candidate(
                match_key.as_str(),
                driver_id.as_str(),
                "registry-download",
            );
            crate::result_println!(
                "[Driver Runtime] Cached registry candidate {} -> {}",
                match_key,
                driver_id
            );

            let activation_source = pending
                .as_ref()
                .map(|value| value.source.as_str())
                .unwrap_or("registry-download");
            let activate_after_download = auto_apply_drivers
                || pending
                    .as_ref()
                    .map(|value| value.activate_after_download)
                    .unwrap_or(false);

            if activate_after_download {
                let activation = crate::driver_runtime::activate_binding(
                    match_key.as_str(),
                    driver_id.as_str(),
                    activation_source,
                );
                if activation.changed {
                    if let Some(previous) = activation.previous_driver_id.as_deref() {
                        crate::result_println!(
                            "[Driver Runtime] Activated {} -> {} (previously {})",
                            match_key,
                            driver_id,
                            previous
                        );
                    } else {
                        crate::result_println!(
                            "[Driver Runtime] Activated {} -> {}",
                            match_key,
                            driver_id
                        );
                    }
                } else {
                    crate::result_println!(
                        "[Driver Runtime] {} already active for {}",
                        driver_id,
                        match_key
                    );
                }

                match crate::driver_runtime::promote_binding(match_key.as_str()) {
                    Ok(_) => crate::result_println!(
                        "[Driver Cache] Persisted preferred binding {} -> {}",
                        match_key,
                        driver_id
                    ),
                    Err(error) => crate::result_println!(
                        "[Driver Cache] Active binding not persisted: {}",
                        error
                    ),
                }
            }

            if !payload_text.is_empty() {
                match crate::driver_cache::persist_last_generated_driver_note(
                    match_key.as_str(),
                    driver_id.as_str(),
                    payload_text.as_str(),
                ) {
                    Ok(()) => crate::result_println!(
                        "[Driver Cache] Persisted last downloaded driver payload for {}",
                        match_key
                    ),
                    Err(error) => crate::result_println!(
                        "[Driver Cache] Downloaded driver payload not persisted: {}",
                        error
                    ),
                }
            }

            if let Some(artifact_id) = artifact_id {
                crate::result_println!(
                    "[Driver Runtime] Cached artifact {} kind={} for {}",
                    artifact_id,
                    payload_kind,
                    match_key
                );
            }

            if payload_kind == "source_text" {
                crate::result_println!(
                    "[Driver Runtime] Registry payload is source text only; execution remains deferred until a runnable sandbox artifact exists."
                );
            }
        }
        ServiceApiPhase::DriverUpload => {
            let Some(driver_id) = extract_json_string(body_text, "driver_id") else {
                return;
            };
            let Some(match_key) = crate::api_v1::take_pending_driver_upload_match_key() else {
                return;
            };

            let activation = crate::driver_runtime::activate_binding(
                match_key.as_str(),
                driver_id,
                "uploaded-driver",
            );
            if activation.changed {
                if let Some(previous) = activation.previous_driver_id.as_deref() {
                    crate::result_println!(
                        "[Driver Runtime] Activated {} -> {} (previously {})",
                        match_key,
                        driver_id,
                        previous
                    );
                } else {
                    crate::result_println!(
                        "[Driver Runtime] Activated {} -> {}",
                        match_key,
                        driver_id
                    );
                }
            } else {
                crate::result_println!(
                    "[Driver Runtime] {} is already active for {}",
                    driver_id,
                    match_key
                );
            }

            match crate::driver_runtime::promote_binding(match_key.as_str()) {
                Ok(_) => crate::result_println!(
                    "[Driver Cache] Persisted preferred binding {} -> {}",
                    match_key,
                    driver_id
                ),
                Err(error) => crate::result_println!(
                    "[Driver Cache] Active binding not persisted: {}",
                    error
                ),
            }

            if let Some(text) = crate::api_v1::last_gemini_text() {
                match crate::driver_cache::persist_last_generated_driver_note(
                    match_key.as_str(),
                    driver_id,
                    text.as_str(),
                ) {
                    Ok(()) => crate::result_println!(
                        "[Driver Cache] Persisted last generated driver note for {}",
                        match_key
                    ),
                    Err(error) => crate::result_println!(
                        "[Driver Cache] Generated driver note not persisted: {}",
                        error
                    ),
                }
            }

            let generator = crate::api_v1::last_gemini_model().unwrap_or_else(|| String::from("unknown"));
            let evaluation_note = format!(
                "Generated candidate uploaded and activated live for {} using {}.",
                match_key,
                generator
            );
            match crate::api_v1::queue_driver_registry_command(
                crate::api_v1::DriverRegistryCommand::UploadEvaluation {
                    driver_id: String::from(driver_id),
                    match_key: match_key.clone(),
                    note: evaluation_note,
                },
            ) {
                Ok(()) => crate::result_println!(
                    "[Evaluation] Queued initial driver evaluation for {}",
                    driver_id
                ),
                Err(_) => crate::result_println!(
                    "[Evaluation] Evaluation queue full; initial report not queued."
                ),
            }
        }
        _ => {}
    }
}

fn maybe_queue_driver_candidate_download(body_text: &str, auto_apply_drivers: bool) {
    if !auto_apply_drivers {
        return;
    }

    let driver_ids = extract_json_string_list(body_text, "driver_id");
    let match_keys = extract_json_string_list(body_text, "match_key");
    let candidate_count = core::cmp::min(driver_ids.len(), match_keys.len());
    let mut queued_match_keys = alloc::vec::Vec::new();

    for index in 0..candidate_count {
        let driver_id = driver_ids[index].as_str();
        let match_key = match_keys[index].as_str();
        if driver_id.is_empty() || match_key.is_empty() {
            continue;
        }
        if crate::runtime_bindings::current_driver(match_key).is_some() {
            continue;
        }
        if queued_match_keys.iter().any(|existing| existing == match_key) {
            continue;
        }
        queued_match_keys.push(String::from(match_key));
        crate::api_v1::schedule_driver_download_activation(
            driver_id,
            match_key,
            "registry-download",
        );

        match crate::api_v1::queue_driver_registry_command(
            crate::api_v1::DriverRegistryCommand::DownloadCandidate {
                driver_id: String::from(driver_id),
                match_key: String::from(match_key),
            },
        ) {
            Ok(()) => crate::result_println!(
                "[Driver Runtime] Queued registry download for {} ({})",
                driver_id,
                match_key
            ),
            Err(_) => crate::result_println!(
                "[Driver Runtime] Registry download queue full; skipped {}",
                driver_id
            ),
        }
    }
}

fn handle_failed_driver_download(status_code: u16, pending: &crate::api_v1::PendingDriverDownload) {
    if status_code != 404 || !pending.activate_after_download {
        return;
    }

    crate::result_println!(
        "[Driver Runtime] Registry payload missing for {} ({}). Falling back to Gemini generation.",
        pending.driver_id,
        pending.match_key
    );

    let Some(prompt) = crate::api_v1::build_driver_generation_prompt(pending.match_key.as_str()) else {
        crate::result_println!(
            "[Driver Runtime] Node profile unavailable; could not build generation prompt for {}",
            pending.match_key
        );
        return;
    };

    crate::api_v1::record_last_gemini_prompt(prompt.as_str());
    match crate::api_v1::queue_generated_driver_gemini_prompt(
        pending.match_key.as_str(),
        prompt,
    ) {
        Ok(()) => crate::result_println!(
            "[Driver Runtime] Queued Gemini generation for {}",
            pending.match_key
        ),
        Err(_) => crate::result_println!(
            "[Driver Runtime] Gemini prompt queue full; generation skipped for {}",
            pending.match_key
        ),
    }
}

fn log_unmatched_local_devices(match_keys: &[String]) {
    let Some(profile) = crate::identity::current_profile() else {
        return;
    };

    let mut unmatched_count = 0;
    for device in &profile.machine_profile.pci_devices {
        let exact_key = crate::identity::stable_device_match_key(device);
        let class_key = class_device_match_key(device);
        let matched = match_keys.iter().any(|key| key == &exact_key || key == &class_key);
        if !matched {
            unmatched_count += 1;
            crate::result_println!(
                "[API v1] unmatched local device: {} (class {:02x}:{:02x})",
                exact_key,
                device.class_code,
                device.subclass
            );
        }
    }

    if unmatched_count == 0 {
        crate::result_println!("[API v1] all local PCI devices matched a driver baseline");
    }
}

fn class_device_match_key(device: &crate::identity::HardwareDeviceSummary) -> String {
    format!(
        "{}:class:{:02x}{:02x}",
        device.bus_type, device.class_code, device.subclass
    )
}

fn extract_json_string<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let pattern = alloc::format!("\"{}\":\"", key);
    let start = body.find(pattern.as_str())? + pattern.len();
    let rest = &body[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn extract_json_string_decoded(body: &str, key: &str) -> Option<String> {
    let pattern = alloc::format!("\"{}\":\"", key);
    let start = body.find(pattern.as_str())? + pattern.len();
    let bytes = body.as_bytes();
    let mut index = start;
    let mut out = String::new();

    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'"' {
            return Some(out);
        }

        if byte == b'\\' {
            index += 1;
            if index >= bytes.len() {
                return None;
            }
            match bytes[index] {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                other => out.push(other as char),
            }
            index += 1;
            continue;
        }

        out.push(byte as char);
        index += 1;
    }

    None
}

fn extract_json_number(body: &str, key: &str) -> Option<u64> {
    let pattern = alloc::format!("\"{}\":", key);
    let start = body.find(pattern.as_str())? + pattern.len();
    let rest = &body[start..];
    let digits_len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits_len == 0 {
        return None;
    }
    rest[..digits_len].parse::<u64>().ok()
}

fn extract_json_signed_number(body: &str, key: &str) -> Option<i64> {
    let pattern = alloc::format!("\"{}\":", key);
    let start = body.find(pattern.as_str())? + pattern.len();
    let rest = &body[start..];
    let digits_len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b'-')
        .count();
    if digits_len == 0 {
        return None;
    }
    rest[..digits_len].parse::<i64>().ok()
}

fn extract_json_string_list(body: &str, key: &str) -> alloc::vec::Vec<String> {
    let pattern = alloc::format!("\"{}\":\"", key);
    let mut values = alloc::vec::Vec::new();
    let mut search_start = 0;

    while let Some(found) = body[search_start..].find(pattern.as_str()) {
        let value_start = search_start + found + pattern.len();
        let rest = &body[value_start..];
        let Some(end) = rest.find('"') else {
            break;
        };
        values.push(String::from(&rest[..end]));
        search_start = value_start + end + 1;
    }

    values
}

fn summarize_registry_ids(ids: &[String], limit: usize) -> String {
    let mut summary = String::new();
    for (index, id) in ids.iter().take(limit).enumerate() {
        if index > 0 {
            summary.push_str(", ");
        }
        summary.push_str(id.as_str());
    }

    if ids.len() > limit {
        summary.push_str(", ...");
    }

    summary
}

fn log_gemini_response(response: &crate::https::ApiResponse) {
    crate::result_println!("[Gemini] response status: {}", response.status_code);

    if let Some(text) = extract_first_text_field(&response.body) {
        crate::result_println!("[Gemini] {}", text);
        execute_gemini_machine_actions(text.as_str());
        return;
    }

    match core::str::from_utf8(&response.body) {
        Ok(body) => crate::println!("[Gemini] raw body: {}", body),
        Err(_) => crate::println!(
            "[Gemini] response body is not valid UTF-8 ({} bytes)",
            response.body.len()
        ),
    }
}

fn execute_gemini_machine_actions(text: &str) {
    let actions = extract_gemini_driver_actions(text);
    if actions.is_empty() {
        return;
    }

    crate::result_println!(
        "[Gemini] machine actions detected: {}",
        actions.len()
    );

    for driver_id in actions {
        let Some(match_key) = crate::api_v1::find_driver_registry_match_key(driver_id.as_str()) else {
            crate::result_println!(
                "[Gemini] no registry match_key is known for {}; skipped execution.",
                driver_id
            );
            continue;
        };

        crate::api_v1::schedule_driver_download_activation(
            driver_id.as_str(),
            match_key.as_str(),
            "gemini-action",
        );

        match crate::api_v1::queue_driver_registry_command(
            crate::api_v1::DriverRegistryCommand::DownloadCandidate {
                driver_id: driver_id.clone(),
                match_key: match_key.clone(),
            },
        ) {
            Ok(()) => crate::result_println!(
                "[Driver Runtime] Queued Gemini driver apply for {} ({})",
                driver_id,
                match_key
            ),
            Err(_) => crate::result_println!(
                "[Driver Runtime] Registry download queue full; Gemini action skipped for {}",
                driver_id
            ),
        }
    }
}

fn extract_gemini_driver_actions(text: &str) -> alloc::vec::Vec<String> {
    let mut actions = alloc::vec::Vec::new();
    let mut search_start = 0usize;

    while let Some(action_index) = text[search_start..].find("\"action\"") {
        let object_start = search_start + action_index;
        let remainder = &text[object_start..];

        let Some(driver_name_index) = remainder.find("\"driver_name\"") else {
            search_start = object_start + 8;
            continue;
        };

        let Some(action_value) = extract_json_like_string(remainder, "action") else {
            search_start = object_start + 8;
            continue;
        };

        if action_value != "load_driver" {
            search_start = object_start + driver_name_index + 13;
            continue;
        }

        if let Some(driver_id) = extract_json_like_string(remainder, "driver_name") {
            if !actions.iter().any(|existing| existing == &driver_id) {
                actions.push(driver_id);
            }
        }

        search_start = object_start + driver_name_index + 13;
    }

    actions
}

fn extract_json_like_string(body: &str, key: &str) -> Option<String> {
    let pattern = alloc::format!("\"{}\"", key);
    let start = body.find(pattern.as_str())? + pattern.len();
    let rest = &body[start..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let quote = rest.find('"')?;
    let bytes = rest.as_bytes();
    let mut index = quote + 1;
    let mut out = String::new();
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            match byte {
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                _ => out.push(byte as char),
            }
            escaped = false;
            index += 1;
            continue;
        }

        match byte {
            b'\\' => escaped = true,
            b'"' => return Some(out),
            _ => out.push(byte as char),
        }
        index += 1;
    }

    None
}

fn log_plain_http_response(response: &crate::https::ApiResponse) {
    crate::println!("[HTTP] response status: {}", response.status_code);
    match core::str::from_utf8(&response.body) {
        Ok(body) => crate::println!("[HTTP] response body: {}", body),
        Err(_) => crate::println!(
            "[HTTP] response body is not valid UTF-8 ({} bytes)",
            response.body.len()
        ),
    }
}

fn extract_first_text_field(body: &[u8]) -> Option<String> {
    let key_start = find_subsequence(body, b"\"text\"")?;
    let mut cursor = key_start + 6;

    while let Some(byte) = body.get(cursor) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        cursor += 1;
    }

    if body.get(cursor)? != &b':' {
        return None;
    }
    cursor += 1;

    while let Some(byte) = body.get(cursor) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        cursor += 1;
    }

    if body.get(cursor)? != &b'"' {
        return None;
    }
    cursor += 1;

    let mut out = String::new();
    let mut escaped = false;

    for &byte in &body[cursor..] {
        if escaped {
            match byte {
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                _ => out.push(byte as char),
            }
            escaped = false;
            continue;
        }

        match byte {
            b'\\' => escaped = true,
            b'"' => return Some(out),
            _ => out.push(byte as char),
        }
    }

    None
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn service_api_phase_name(phase: ServiceApiPhase) -> &'static str {
    match phase {
        ServiceApiPhase::Idle => "idle",
        ServiceApiPhase::Register => "register",
        ServiceApiPhase::HealthHttps => "health_https",
        ServiceApiPhase::RootHttps => "root_https",
        ServiceApiPhase::HardwareReport => "hardware_report",
        ServiceApiPhase::DriverQuery => "driver_query",
        ServiceApiPhase::SoftwareQuery => "software_query",
        ServiceApiPhase::SkillQuery => "skill_query",
        ServiceApiPhase::WorkflowQuery => "workflow_query",
        ServiceApiPhase::PolicyQuery => "policy_query",
        ServiceApiPhase::EvaluationQuery => "evaluation_query",
        ServiceApiPhase::EvaluationUpload => "evaluation_upload",
        ServiceApiPhase::SkillDownload => "skill_download",
        ServiceApiPhase::DriverUpload => "driver_upload",
        ServiceApiPhase::DriverDownload => "driver_download",
        ServiceApiPhase::DriverComment => "driver_comment",
        ServiceApiPhase::DriverVote => "driver_vote",
        ServiceApiPhase::Done => "done",
    }
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_hex_string(text: &str) -> Option<alloc::vec::Vec<u8>> {
    let bytes = text.as_bytes();
    if bytes.len() % 2 != 0 {
        return None;
    }

    let mut out = alloc::vec::Vec::with_capacity(bytes.len() / 2);
    let mut index = 0usize;
    while index < bytes.len() {
        let high = decode_hex_nibble(bytes[index])?;
        let low = decode_hex_nibble(bytes[index + 1])?;
        out.push((high << 4) | low);
        index += 2;
    }

    Some(out)
}






