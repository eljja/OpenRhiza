#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)] // CPU 예외/인터럽트 처리를 위한 x86-interrupt ABI 활성화

extern crate alloc; // 동적 메모리 할당(Vec, String, Box 등)을 위한 내장 alloc 크레이트 활성화
extern crate smoltcp; // TCP/IP 스택 지원을 명시적으로 링크합니다.

// Rust 내장 core 라이브러리와 이름 충돌을 피하기 위해 경로를 매핑하여 로드합니다.
#[path = "core/seed.rs"]
pub mod os_core_seed;

// 멀티 아키텍처 모듈 트리 선언
pub mod arch {
    pub mod x86_64 {
        pub mod discovery;
        pub mod interrupts; // 블루스크린 방어막 및 우체통 (IDT)
        pub mod port;       // I/O 포트 (키보드/마우스 제어 기초)
        pub mod serial;     // 호스트 PC와 통신할 탯줄 (COM1 UART)
    }
}

// 힙 메모리 할당자 모듈
pub mod allocator;

pub mod net;
pub mod vga;
pub mod storage;
pub mod https;
pub mod task;
pub mod security;

use arch::x86_64::discovery::SystemIdentity;
use os_core_seed::OpenRhizaSeed;
use core::panic::PanicInfo;
use bootloader::bootinfo::BootInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    crate::arch::x86_64::serial::_print(core::format_args!("KERNEL PANIC: {}\n", info));
    loop {}
}

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    // 1. 방어막 분리 (IDT 및 하드웨어 예외 처리) - 가장 먼저 실행!
    arch::x86_64::interrupts::init_idt();
    unsafe {
        arch::x86_64::interrupts::PICS.lock().initialize();
    }
    x86_64::instructions::interrupts::enable(); // CPU가 외부 하드웨어 신호를 듣도록 허용

    let offset = boot_info.physical_memory_offset;
    unsafe {
        crate::arch::x86_64::discovery::PHYS_MEM_OFFSET = offset;
    }

    // 시리얼 포트 통신 시작 알림 (호스트 PC로 전송)
    crate::println!("OpenRhiza Seed (Layer 0) Booting... Serial Connected!");

    // 2. 힙 메모리 할당자(Heap Allocator) 초기화 (Vec 사용을 위해 필수)
    allocator::init_heap();
    crate::println!("Heap Allocator initialized!");

    // 2. 하드웨어 스캔 및 초기화 (CPUID 스캔 및 PCI 버스 Enumeration)
    let identity = SystemIdentity::scan(boot_info);
    crate::println!("Total Usable Memory: {} Bytes", identity.total_memory);
    crate::println!("Hardware Discovery Complete.");
    crate::println!("Found {} PCI devices:", identity.pci_devices.len());
    for dev in &identity.pci_devices {
        crate::println!("  Bus {} Device {}: Vendor {:#06X}, Device {:#06X}, BAR0: {:#010X}", dev.bus, dev.device, dev.vendor_id, dev.device_id, dev.bar0);
    }
    
    crate::println!("Verify Keyboard: Type 'hi!' and Enter");

    // Phase 6: Wasm Cache Bootstrapping Test
    crate::println!("[Storage] Probing Secondary IDE Drive for Wasm Cache...");
    let mut boot_sector = [0u8; 512];
    storage::read_sector_ata_secondary(0, &mut boot_sector);
    if boot_sector[510] == 0x55 && boot_sector[511] == 0xAA {
        crate::println!("[Storage] Native Bootstrap Disk Detected! Boot Signature: 0x55AA");
        
        crate::println!("[Storage] Executing native FAT16 Parse...");
        if let Some(payload) = storage::extract_payload() {
            crate::println!("[Storage] Successfully extracted E1000.BIN payload cluster!");
            if let Ok(preview) = core::str::from_utf8(&payload[0..17]) {
                crate::println!("[Storage] Payload Preview: '{}'", preview);
            }
        } else {
            crate::println!("[Storage] E1000.BIN not found in Root Directory.");
        }
    } else {
        crate::println!("[Storage] No valid Wasm Cache drive found. Wait for external Link.");
    }

    // 3. OpenRhiza Seed (Layer 0) 샌드박스 인스턴스 생성
    let mut rhiza = OpenRhizaSeed::new(identity);
    
    // 4. OS 네트워킹 스택 초기화 (smoltcp IP/TCP)
    crate::net::init_network();

    crate::println!("[OS System] All subsystems initialized. Handing over to Async Executor.");

    let mut executor = task::executor::Executor::new();
    executor.spawn(task::Task::new(task::keyboard::keyboard_task()));
    executor.spawn(task::Task::new(core_os_task(rhiza)));
    executor.run();
}

async fn core_os_task(mut rhiza: OpenRhizaSeed) {
    let mut receiving_wasm = false;
    let mut receiving_wasm_size = false;
    let mut wasm_size_buf = [0u8; 4];
    let mut wasm_size_idx = 0;
    let mut expected_wasm_size = 0;
    let mut wasm_buffer: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    
    let mut uptime_ms: i64 = 0;
    let mut nexus_client: Option<crate::https::NexusClient> = None;
    let mut keymap_index = 0;

    loop {
        // [수면] 타스크를 1 Ticks 동안 대기시킵니다. (CPU 과점유 방지 및 다른 큐 배려)
        crate::task::timer::sleep_ticks(1).await;

        // [네트워크 폴링] AI가 생성한 네트워크 드라이버가 살아있다면, DMA 버퍼에서 패킷을 계속 가져옵니다.
        rhiza.poll_wasm_network();
        crate::net::poll(uptime_ms);
        uptime_ms += 1;

        if uptime_ms == 200 {
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

        while let Some(data) = rhiza.poll_host_data() {
            if data == 0xFD {
                keymap_index = 0; 
                *crate::task::keyboard::DYNAMIC_KEYMAP.lock() = [0x3F; 256];
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
                crate::println!("[!] Calibration Failed. Try again:");
            } else if keymap_index < 256 {
                crate::task::keyboard::DYNAMIC_KEYMAP.lock()[keymap_index] = data;
                keymap_index += 1;
                if keymap_index == 256 {
                    crate::println!("[+] Keyboard Driver Loaded.");
                }
            }
        }
    }
}