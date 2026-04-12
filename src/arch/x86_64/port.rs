// src/arch/x86_64/port.rs
use x86_64::instructions::port::Port;

/// Read an 8-bit I/O port value for hardware status access.
pub fn read_port_u8(port_addr: u16) -> u8 {
    let mut port: Port<u8> = Port::new(port_addr);
    unsafe { port.read() }
}

/// Write an 8-bit value to an I/O port.
pub fn write_port_u8(port_addr: u16, value: u8) {
    let mut port: Port<u8> = Port::new(port_addr);
    unsafe { port.write(value) }
}

/// Read a 16-bit I/O port value, used for ATA PIO.
pub fn read_port_u16(port_addr: u16) -> u16 {
    let mut port: Port<u16> = Port::new(port_addr);
    unsafe { port.read() }
}

/// Write a 16-bit value to an I/O port, used for ATA PIO.
pub fn write_port_u16(port_addr: u16, value: u16) {
    let mut port: Port<u16> = Port::new(port_addr);
    unsafe { port.write(value) }
}
