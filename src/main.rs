#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)] // CPU 예외/인터럽트 처리를 위한 x86-interrupt ABI 활성화

extern crate alloc; // 동적 메모리 할당(Vec, String, Box 등)을 위한 내장 alloc 크레이트 활성화

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

use arch::x86_64::discovery::SystemIdentity;
use os_core_seed::OpenRhizaSeed;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 시리얼 포트 통신 시작 알림 (호스트 PC로 전송)
    crate::serial_println!("OpenRhiza Seed (Layer 0) Booting... Serial Connected!");

    // 1. 힙 메모리 할당자(Heap Allocator) 우선 초기화 (Vec 사용을 위해 필수)
    allocator::init_heap();
    crate::serial_println!("Heap Allocator initialized!");

    // 2. 하드웨어 스캔 및 초기화 (CPUID 스캔 및 PCI 버스 Enumeration)
    let identity = SystemIdentity::scan();
    crate::serial_println!("Hardware Discovery Complete.");
    crate::serial_println!("Found {} PCI devices:", identity.pci_devices.len());
    for dev in &identity.pci_devices {
        crate::serial_println!("  Bus {} Device {}: Vendor {:#06X}, Device {:#06X}, BAR0: {:#010X}", dev.bus, dev.device, dev.vendor_id, dev.device_id, dev.bar0);
    }
    
    // --- VGA 텍스트 출력 도우미 (클로저) ---
    let mut cursor_x: usize = 0; // 가로 위치 (0~79)
    let mut cursor_y: usize = 0; // 세로 위치 (0~24)
    
    let print_vga = |msg: &[u8], color: u8, cx: &mut usize, cy: &mut usize| {
        let vga_buffer = 0xb8000 as *mut u8;
        for &byte in msg {
            if byte == b'\n' {
                *cx = 0; *cy += 1;
            } else {
                unsafe {
                    let offset = (*cy * 80 + *cx) * 2;
                    *vga_buffer.offset(offset as isize) = byte;
                    *vga_buffer.offset((offset + 1) as isize) = color;
                }
                *cx += 1;
                if *cx >= 80 { *cx = 0; *cy += 1; }
            }
            // 스크롤 처리 (빠른 메모리 복사 방식 적용)
            if *cy >= 25 {
                unsafe {
                    core::ptr::copy(vga_buffer.offset(160), vga_buffer, 160 * 24);
                    for x in 0..80 {
                        *vga_buffer.offset((24 * 80 + x) * 2) = b' ';
                        *vga_buffer.offset((24 * 80 + x) * 2 + 1) = 0x0F;
                    }
                }
                *cy = 24;
            }
        }
    };
    
    print_vga(b"Verify Keyboard: Type 'hi!' and Enter\n", 0x0E, &mut cursor_x, &mut cursor_y); // 노란색

    // 3. IDT(인터럽트 설명자 테이블) 및 PIC 초기화 - 방어막/입력 활성화
    arch::x86_64::interrupts::init_idt();
    unsafe {
        arch::x86_64::interrupts::PICS.lock().initialize();
    }
    x86_64::instructions::interrupts::enable(); // CPU가 외부 하드웨어 신호를 듣도록 허용

    // 2. OpenRhiza Seed (Layer 0) 샌드박스 인스턴스 생성
    let mut rhiza = OpenRhizaSeed::new(identity);
    
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

    loop {
        // [데이터 수신] 외부 AI가 시리얼 포트로 매핑 테이블 256바이트를 쏴주면 순서대로 적재합니다!
        if let Some(data) = rhiza.poll_host_data() {
            if data == 0xFD {
                keymap_index = 0; // 수신 버퍼 조용히 초기화 (TCP 쓰레기 데이터 방지)
                dynamic_keymap = [0x3F; 256];
            } else if data == 0xFE && keymap_index < 256 {
                // 실패 시그널 수신 시 캘리브레이션 초기화
                keymap_index = 0;
                print_vga(b"\n[!] Calibration Failed. Try again:\n", 0x0C, &mut cursor_x, &mut cursor_y); // 빨간색
            } else if keymap_index < 256 {
                dynamic_keymap[keymap_index] = data;
                keymap_index += 1;
                if keymap_index == 256 {
                    print_vga(b"\n[+] Keyboard Driver Loaded.\n", 0x0B, &mut cursor_x, &mut cursor_y); // 밝은 청록색(Cyan)
                }
            } else if data == 0xFB {
                print_vga(b"\n[*] AI is generating e1000 LAN driver... Please wait.\n", 0x0E, &mut cursor_x, &mut cursor_y); // 노란색
            } else if data == 0xFA {
                print_vga(b"\n[!] Failed to generate LAN driver.\n", 0x0C, &mut cursor_x, &mut cursor_y); // 빨간색
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
                        crate::serial_println!("[OS] Receiving Wasm binary of size {} bytes...", expected_wasm_size);
                    }
                } else if receiving_wasm {
                    wasm_buffer.push(data);
                    if wasm_buffer.len() == expected_wasm_size {
                        receiving_wasm = false;
                        crate::serial_println!("[OS] Wasm binary received! Executing Sandbox...");
                        match rhiza.execute_wasm_sandbox(&wasm_buffer) {
                            os_core_seed::ExecutionResult::Success(msg) => crate::serial_println!("[Sandbox] {}", msg),
                            os_core_seed::ExecutionResult::Panic(err) => crate::serial_println!("[Sandbox Error] {}", err),
                        }
                    }
                }
            }
        }

        if let Some(scancode) = rhiza.poll_hardware_event() {
            crate::serial_println!("QEMU_LOG: Received scancode -> {:#04X}", scancode);
            
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
                        print_vga(b"\n", 0x0F, &mut cursor_x, &mut cursor_y);
                    } else if char_to_print == 0x08 { // Backspace 키 처리
                        if cursor_x > 0 {
                            cursor_x -= 1;
                            print_vga(b" ", 0x0F, &mut cursor_x, &mut cursor_y);
                            cursor_x -= 1; // 공백 출력 후 커서를 원래 지운 위치로 복구
                        }
                    } else { // 일반 문자 출력
                        print_vga(&[char_to_print], 0x0F, &mut cursor_x, &mut cursor_y);
                    }
                }
            }
        }
        x86_64::instructions::hlt(); // CPU를 쉬게 하여 배터리와 자원 낭비 방지
    }
}