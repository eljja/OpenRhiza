#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)] // CPU 예외/인터럽트 처리를 위한 x86-interrupt ABI 활성화

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

use arch::x86_64::discovery::SystemIdentity;
use os_core_seed::OpenRhizaSeed;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 1. 하드웨어 스캔 및 초기화 (CPUID를 통한 코어 수 스캔)
    let identity = SystemIdentity::scan();
    
    // --- 화면 출력 (VGA Text Buffer 0xb8000 직접 제어) ---
    let vga_buffer = 0xb8000 as *mut u8;
    let greeting = b"Hello OpenRhiza!";
    for (i, &byte) in greeting.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;       // 아스키 코드 (글자)
            *vga_buffer.offset(i as isize * 2 + 1) = 0x0A;   // 색상 (Light Green, 연두색)
        }
    }
    // ------------------------------------------------------

    // 시리얼 포트 통신 시작 알림 (호스트 PC로 전송)
    crate::serial_println!("OpenRhiza Seed (Layer 0) Booting... Serial Connected!");

    // 1.5 IDT(인터럽트 설명자 테이블) 초기화 - 방어막 활성화
    arch::x86_64::interrupts::init_idt();

    // 방어막(IDT) 작동 테스트: 강제로 Breakpoint(중단점) 예외 발생
    // 원래라면 시스템이 패닉(블루스크린)으로 멈추겠지만, 우리가 만든 IDT가 이를 가로챕니다!
    x86_64::instructions::interrupts::int3();

    // 1.6 PIC(하드웨어 인터럽트 컨트롤러) 초기화 및 활성화
    unsafe {
        arch::x86_64::interrupts::PICS.lock().initialize();
    }
    x86_64::instructions::interrupts::enable(); // CPU가 외부 하드웨어 신호를 듣도록 허용

    // 2. OpenRhiza Seed (Layer 0) 샌드박스 인스턴스 생성
    let mut rhiza = OpenRhizaSeed::new(identity);
    
    // 3. 샌드박스 실행 루프 (모의 코드 실행)
    let mut cursor_x: usize = 0; // 가로 위치 (0~79)
    let mut cursor_y: usize = 2; // 세로 위치 (0~24), Hello OpenRhiza 밑에서 시작
    
    // 동적 키보드 매핑 테이블 (처음엔 모두 '?'로 초기화)
    let mut dynamic_keymap: [u8; 128] = [b'?'; 128];
    let mut keymap_index = 0;

    loop {
        // [데이터 수신] 외부 AI가 시리얼 포트로 매핑 테이블 128바이트를 쏴주면 메모리에 순서대로 적재합니다!
        if let Some(data) = rhiza.poll_host_data() {
            if keymap_index < 128 {
                dynamic_keymap[keymap_index] = data;
                keymap_index += 1;
            }
        }

        // AI가 주기적으로 하드웨어 큐를 확인합니다.
        if let Some(scancode) = rhiza.poll_hardware_event() {
            
            // [탯줄 통신] 호스트 PC(외부 AI)로 수신된 하드웨어 이벤트 로그를 쏩니다!
            crate::serial_println!("QEMU_LOG: Received scancode -> {:#04X}", scancode);
            
            // [동적 드라이버 실행] AI가 주입한 매핑 테이블을 사용하여 화면에 출력합니다.
            if scancode < 0x80 {
                let char_to_print = dynamic_keymap[scancode as usize];
                if char_to_print != 0x3F { // 매핑되지 않은 키('?')는 무시
                    let vga_buffer = 0xb8000 as *mut u8;
                    
                    if char_to_print == 0x0A { // Enter 키 처리
                        cursor_x = 0;
                        cursor_y += 1;
                    } else if char_to_print == 0x08 { // Backspace 키 처리
                        if cursor_x > 0 {
                            cursor_x -= 1;
                            unsafe {
                                let offset = (cursor_y * 80 + cursor_x) * 2;
                                *vga_buffer.offset(offset as isize) = b' '; // 글자 지우기
                            }
                        }
                    } else { // 일반 문자 출력
                        unsafe {
                            let offset = (cursor_y * 80 + cursor_x) * 2;
                            *vga_buffer.offset(offset as isize) = char_to_print;
                            *vga_buffer.offset((offset + 1) as isize) = 0x0F; // 흰색
                        }
                        cursor_x += 1;
                        if cursor_x >= 80 { cursor_x = 0; cursor_y += 1; }
                    }

                    // 스크롤 (Scroll) 처리: 화면 맨 밑을 넘어가면 한 줄씩 위로 올립니다.
                    if cursor_y >= 25 {
                        unsafe {
                            for y in 1..25 {
                                for x in 0..80 {
                                    let src = (y * 80 + x) * 2;
                                    let dst = ((y - 1) * 80 + x) * 2;
                                    *vga_buffer.offset(dst as isize) = *vga_buffer.offset(src as isize);
                                    *vga_buffer.offset((dst + 1) as isize) = *vga_buffer.offset((src + 1) as isize);
                                }
                            }
                            // 마지막 줄 초기화 (공백으로 덮기)
                            for x in 0..80 {
                                let offset = (24 * 80 + x) * 2;
                                *vga_buffer.offset(offset as isize) = b' ';
                            }
                        }
                        cursor_y = 24;
                    }
                }
            }
        }
        x86_64::instructions::hlt(); // CPU를 쉬게 하여 배터리와 자원 낭비 방지
    }
}