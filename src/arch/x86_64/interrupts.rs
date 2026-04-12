// src/arch/x86_64/interrupts.rs
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;

// Shift hardware interrupts so they do not overlap CPU exceptions (0..31).
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // PIC_1_OFFSET + 1 (vector 33)
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

// --- Ring buffer for hardware events consumed by the AI/runtime ---
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

// Global scancode queue shared by interrupt handlers and the runtime.
pub static KEYBOARD_QUEUE: spin::Mutex<ScancodeQueue> = spin::Mutex::new(ScancodeQueue::new());
// -------------------------------------------------------------

lazy_static! {
    // Static IDT initialized once and reused globally.
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        
        // Guardrail 1: breakpoint exception handler.
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        
        // Guardrail 2: page fault handler for invalid memory access.
        idt.page_fault.set_handler_fn(page_fault_handler);
        
        // Hardware interrupt: timer (IRQ 0)
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        
        // Hardware interrupt: keyboard (IRQ 1)
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
    crate::task::timer::timer_tick();
    crate::arch::x86_64::apic::end_of_interrupt();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // 1. Read the scancode from keyboard port 0x60.
    let scancode = crate::arch::x86_64::port::read_port_u8(0x60);
    
    // 2. Push it into the async queue instead of printing directly.
    crate::task::keyboard::add_scancode(scancode);

    // 3. Notify the APIC that the interrupt has been handled (EOI).
    crate::arch::x86_64::apic::end_of_interrupt();
}
