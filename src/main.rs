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
pub mod task;
pub mod security;
pub mod e1000;
pub mod keyboard;
pub mod crypto;
pub mod tls;
pub mod identity;
pub mod api_v1;

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
    Done,
}

enum PendingDnsAction {
    Service {
        phase: ServiceApiPhase,
        chain_active: bool,
    },
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
    crate::println!("Hardware Discovery Complete.");
    crate::println!("Found {} PCI devices:", identity.pci_devices.len());
    for dev in &identity.pci_devices {
        crate::println!("  Bus {} Device {}: Vendor {:#06X}, Device {:#06X}, BAR0: {:#010X}", dev.bus, dev.device, dev.vendor_id, dev.device_id, dev.bar0);
        
        // Bootstrap native xHCI from Layer 0 when a controller is discovered.
        if dev.class_code == 0x0C && dev.subclass == 0x03 && dev.prog_if == 0x30 && dev.bar0 != 0 {
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
    let mut boot_sector = [0u8; 512];
    storage::read_sector_ata_secondary(0, &mut boot_sector);
    if boot_sector[510] == 0x55 && boot_sector[511] == 0xAA {
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
    } else {
        crate::println!("[Storage] No valid Wasm Cache drive found. Wait for external Link.");
    }

    // 5. Create the Layer 0 seed / sandbox engine.
    let node_profile = crate::identity::NodeProfile::collect(&identity).install();
    node_profile.log_summary();
    crate::api_v1::log_request_previews(&node_profile);

    let rhiza = OpenRhizaSeed::new(identity);
    
    // 6. Initialize the networking stack.
    crate::net::init_network();

    crate::println!("[OS System] All subsystems initialized. Handing over to Async Executor.");

    let mut executor = task::executor::Executor::new();
    executor.spawn(task::Task::new(task::keyboard::keyboard_task()));
    executor.spawn(task::Task::new(usb_input_task()));
    executor.spawn(task::Task::new(core_os_task(rhiza)));
    executor.spawn(task::Task::new(background_llm_worker()));
    executor.run();
}

async fn background_llm_worker() {
    crate::println!("[Task] background_llm_worker started");
    loop {
        if let Some(_prompt) = crate::task::keyboard::PROMPT_QUEUE.pop() {
            crate::task::timer::sleep_ticks(10).await;
        } else {
            crate::task::timer::sleep_ticks(100).await;
        }
    }
}

async fn usb_input_task() {
    crate::println!("[Task] usb_input_task started");
    loop {
        crate::task::timer::sleep_ticks(1).await;
        crate::arch::x86_64::usb::poll_usb_keyboard();
        crate::arch::x86_64::usb::tick_usb_keyboard();
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
            && gemini_client.is_none()
            && dns_client.is_none()
            && service_api_phase == ServiceApiPhase::Idle
        {
            if let Some(prompt) = crate::api_v1::GEMINI_PROMPT_QUEUE.pop() {
                let request = GeminiRequest {
                    prompt,
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
                    log_gemini_response(&response);
                    gemini_client = None;
                    gemini_request = None;
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

fn spawn_dns_client(hostname: &'static str) -> crate::dns::DnsClient {
    let socket_handle = crate::net::create_udp_socket(512, 512);
    crate::dns::DnsClient::new(socket_handle, crate::dns::DEFAULT_DNS_SERVER, hostname)
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
    crate::println!(
        "[API v1] {} response status: {}",
        service_api_phase_name(phase),
        response.status_code
    );

    match core::str::from_utf8(&response.body) {
        Ok(body) => crate::println!("[API v1] {} response body: {}", service_api_phase_name(phase), body),
        Err(_) => crate::println!(
            "[API v1] {} response body is not valid UTF-8 ({} bytes)",
            service_api_phase_name(phase),
            response.body.len()
        ),
    }
}

fn log_gemini_response(response: &crate::https::ApiResponse) {
    crate::println!("[Gemini] response status: {}", response.status_code);

    if let Some(text) = extract_first_text_field(&response.body) {
        crate::println!("[Gemini] text: {}", text);
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
    let start = find_subsequence(body, b"\"text\":\"")? + 8;
    let mut out = String::new();
    let mut escaped = false;

    for &byte in &body[start..] {
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
        ServiceApiPhase::Done => "done",
    }
}
