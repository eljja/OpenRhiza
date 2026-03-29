// src/arch/x86_64/interrupts.rs
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;

// 하드웨어 인터럽트 번호가 CPU 에러(0~31)와 겹치지 않도록 32번부터 시작하도록 밀어냅니다.
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // PIC_1_OFFSET + 1 (33번)
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

// --- 하드웨어 이벤트를 담아둘 Ring Buffer (AI가 읽어갈 큐) ---
pub struct ScancodeQueue {
    buffer: [u8; 256],
    head: usize,
    tail: usize,
}

impl ScancodeQueue {
    const fn new() -> Self { Self { buffer: [0; 256], head: 0, tail: 0 } }
    pub fn push(&mut self, val: u8) {
        let next = (self.head + 1) % 256;
        if next != self.tail { self.buffer[self.head] = val; self.head = next; }
    }
    pub fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail { None }
        else { let val = self.buffer[self.tail]; self.tail = (self.tail + 1) % 256; Some(val) }
    }
}

// 글로벌 스캔코드 큐 (인터럽트와 AI 샌드박스가 공유)
pub static KEYBOARD_QUEUE: spin::Mutex<ScancodeQueue> = spin::Mutex::new(ScancodeQueue::new());
// -------------------------------------------------------------

lazy_static! {
    // IDT를 한 번만 초기화하고 전역적으로 안전하게 사용하기 위한 정적 변수
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        
        // 에러 방어막 1: Breakpoint (디버깅용 중단점) 예외 핸들러 등록
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        
        // 에러 방어막 2: Page Fault (잘못된 메모리 접근) 예외 핸들러 등록
        idt.page_fault.set_handler_fn(page_fault_handler);
        
        // 하드웨어 인터럽트: 타이머 (IRQ 0)
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        
        // 하드웨어 인터럽트: 키보드 (IRQ 1)
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);
        
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    crate::println!("[B] Breakpoint Hit!");
}

extern "x86-interrupt" fn page_fault_handler(_stack_frame: InterruptStackFrame, error_code: PageFaultErrorCode) {
    crate::arch::x86_64::serial::_print(core::format_args!("CRITICAL: PAGE FAULT! Code: {:?}\n", error_code));
    loop {} 
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // 타이머 인터럽트는 지금 당장 처리할 일이 없으므로, PIC에게 "잘 받았다"고 신호(EOI)만 보냅니다.
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // 1. 키보드 포트(0x60)에서 스캔코드를 읽어옵니다. (이전에 만든 port.rs 활용)
    let scancode = crate::arch::x86_64::port::read_port_u8(0x60);
    
    // 2. 화면에 바로 찍지 않고, AI가 읽어갈 수 있도록 큐에 밀어 넣습니다.
    KEYBOARD_QUEUE.lock().push(scancode);

    // 3. PIC에게 다음 키보드 입력을 받아도 된다고 신호(EOI)를 보냅니다.
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}