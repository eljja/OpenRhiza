use x2apic::lapic::{LocalApic, LocalApicBuilder};
use x2apic::ioapic::{IoApic, IrqFlags, IrqMode, RedirectionTableEntry};
use spin::Mutex;

pub struct SafeLocalApic(pub LocalApic);
unsafe impl Send for SafeLocalApic {}
unsafe impl Sync for SafeLocalApic {}

pub struct SafeIoApic(pub IoApic);
unsafe impl Send for SafeIoApic {}
unsafe impl Sync for SafeIoApic {}

pub static LAPIC: Mutex<Option<SafeLocalApic>> = Mutex::new(None);
pub static IOAPIC: Mutex<Option<SafeIoApic>> = Mutex::new(None);

pub fn init_apic(phys_mem_offset: u64) {
    let lapic_physical = 0xFEE00000;
    let lapic_virtual = phys_mem_offset + lapic_physical;
    
    let mut lapic = LocalApicBuilder::new()
        .timer_vector(32)
        .error_vector(51)
        .spurious_vector(255)
        .set_xapic_base(lapic_virtual)
        .build()
        .unwrap_or_else(|err| panic!("Failed to build LAPIC: {:?}", err));

    unsafe { lapic.enable(); }
    crate::serial_println!("[APIC] LAPIC Mapped and Enabled at Virtual {:#X}", lapic_virtual);
    *LAPIC.lock() = Some(SafeLocalApic(lapic));

    let ioapic_physical = 0xFEC00000;
    let ioapic_virtual = phys_mem_offset + ioapic_physical;
    
    let mut ioapic = unsafe { IoApic::new(ioapic_virtual) };
    unsafe { ioapic.init(0); } // Assuming offset starts at 0

    // GSI 1 (Keyboard) -> Vector 33
    let mut keyboard_entry = RedirectionTableEntry::default();
    keyboard_entry.set_vector(33);
    keyboard_entry.set_dest(0);
    keyboard_entry.set_flags(IrqFlags::empty());
    keyboard_entry.set_mode(IrqMode::Fixed);

    unsafe { 
        ioapic.set_table_entry(1, keyboard_entry); 
        ioapic.enable_irq(1); 
    }

    // GSI 2 (Timer) -> Vector 32 (Standard ACPI routing overrides PIC IRQ0)
    let mut timer_entry = RedirectionTableEntry::default();
    timer_entry.set_vector(32);
    timer_entry.set_dest(0);
    timer_entry.set_flags(IrqFlags::empty());
    timer_entry.set_mode(IrqMode::Fixed);

    unsafe { 
        ioapic.set_table_entry(2, timer_entry); 
        ioapic.enable_irq(2); 
    }

    crate::serial_println!("[APIC] IOAPIC Routing Configured. Timer/Keyboard Active.");
    *IOAPIC.lock() = Some(SafeIoApic(ioapic));
}

pub fn end_of_interrupt() {
    if let Some(wrapper) = LAPIC.lock().as_mut() {
        unsafe { wrapper.0.end_of_interrupt(); }
    }
}
