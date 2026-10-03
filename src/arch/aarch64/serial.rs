//! PL011 UART scaffold for ARM64 recovery I/O.
//!
//! Core responsibility is limited to recovery text I/O. Rich input, display,
//! audio, and device-specific behavior must be sandbox capabilities.

pub const QEMU_VIRT_PL011_BASE: u64 = 0x0900_0000;
pub const PL011_MATCH_KEY: &str = "dt:arm,pl011";

#[derive(Clone, Copy, Debug)]
pub struct Pl011Descriptor {
    pub base: u64,
    pub irq: u32,
    pub match_key: &'static str,
    pub purpose: &'static str,
}

pub const QEMU_VIRT_PL011: Pl011Descriptor = Pl011Descriptor {
    base: QEMU_VIRT_PL011_BASE,
    irq: 33,
    match_key: PL011_MATCH_KEY,
    purpose: "serial recovery log/input",
};
