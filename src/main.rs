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

    // 동적 키보드 매핑 테이블 (Normal 128 bytes + Shifted 128 bytes = 256 bytes)
    let mut dynamic_keymap: [u8; 256] = [0x3F; 256];
    let mut keymap_index = 0;
    
    // --- 키보드 상태 (State Machine) ---
    let mut shift_pressed = false;
    let mut _ctrl_pressed = false;
    let mut _alt_pressed = false;
    let mut is_extended = false; // E0 확장 키 플래그

    // --- Wasm 수신 상태 (State Machine) ---
    let mut receiving_wasm = false;
    let mut receiving_wasm_size = false;
    let mut wasm_size_buf = [0u8; 4];
    let mut wasm_size_idx = 0;
    let mut expected_wasm_size = 0;
    let mut wasm_buffer: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    
    let mut uptime_ms: i64 = 0;

    loop {
        // [네트워크 폴링] AI가 생성한 네트워크 드라이버가 살아있다면, DMA 버퍼에서 패킷을 계속 가져옵니다.
        rhiza.poll_wasm_network();
        crate::net::poll(uptime_ms);
        uptime_ms += 1;

        // [데이터 수신] 외부 AI가 시리얼 포트로 매핑 테이블 256바이트를 쏴주면 순서대로 적재합니다!
        if let Some(data) = rhiza.poll_host_data() {
            if data == 0xFD {
                keymap_index = 0; // 수신 버퍼 조용히 초기화 (TCP 쓰레기 데이터 방지)
                dynamic_keymap = [0x3F; 256];
            } else if data == 0xFE && keymap_index < 256 {
                // 실패 시그널 수신 시 캘리브레이션 초기화
                keymap_index = 0;
                crate::println!("[!] Calibration Failed. Try again:");
            } else if keymap_index < 256 {
                dynamic_keymap[keymap_index] = data;
                keymap_index += 1;
                if keymap_index == 256 {
                    crate::println!("[+] Keyboard Driver Loaded.");
                }
            } else if data == 0xFB {
                crate::println!("[*] AI is generating e1000 LAN driver... Please wait.");
            } else if data == 0xFA {
                crate::println!("[!] Failed to generate LAN driver.");
            } else {
                // 키보드 드라이버 로드 이후: Wasm 바이너리 수신 로직
                if !receiving_wasm && !receiving_wasm_size && data == 0xFC {
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
                                crate::arch::x86_64::serial::send_byte(0xF8); // Success protocol token
                            },
                            os_core_seed::ExecutionResult::Panic(err) => {
                                crate::println!("[Sandbox Error] {}", err);
                                crate::arch::x86_64::serial::send_byte(0xF9); // Error protocol token
                                let err_bytes = err.as_bytes();
                                let len = err_bytes.len() as u32;
                                for &b in &len.to_le_bytes() {
                                    crate::arch::x86_64::serial::send_byte(b);
                                }
                                for &b in err_bytes {
                                    crate::arch::x86_64::serial::send_byte(b);
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(scancode) = rhiza.poll_hardware_event() {
            crate::println!("QEMU_LOG: Received scancode -> {:#04X}", scancode);
            
            // [키보드 상태 머신] 확장 키(E0) 처리
            if scancode == 0xE0 {
                is_extended = true;
                continue;
            }

            let is_break = scancode >= 0x80;
            let real_scancode = scancode & 0x7F; // Break 비트(0x80) 제거하여 순수 키 위치 확보

            // Modifier(Shift, Ctrl, Alt) 감지 로직
            match (is_extended, real_scancode) {
                (false, 0x2A) | (false, 0x36) => { shift_pressed = !is_break; is_extended = false; continue; }, // Shift
                (false, 0x1D) | (true, 0x1D) => { _ctrl_pressed = !is_break; is_extended = false; continue; },   // Ctrl
                (false, 0x38) | (true, 0x38) => { _alt_pressed = !is_break; is_extended = false; continue; },    // Alt
                _ => {}
            }
            
            is_extended = false; // 하나의 키 처리가 끝나면 확장 플래그 초기화

            // [동적 드라이버 실행] 키를 누를 때(Make)만 화면에 출력
            if !is_break {
                let map_index = if shift_pressed { real_scancode as usize + 128 } else { real_scancode as usize };
                let char_to_print = dynamic_keymap[map_index];

                if char_to_print != 0x3F { // 매핑되지 않은 키('?')는 무시
                    if char_to_print == 0x0A { // Enter 키 처리
                        crate::println!("");
                    } else if char_to_print == 0x08 { // Backspace 키 처리
                        crate::vga::WRITER.lock().backspace();
                    } else { // 일반 문자 출력
                        crate::print!("{}", (char_to_print as char));
                    }
                }
            }
        }
        x86_64::instructions::hlt(); // CPU를 쉬게 하여 배터리와 자원 낭비 방지
    }
}