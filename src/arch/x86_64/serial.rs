// src/arch/x86_64/serial.rs
use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    // Initialize COM1 (0x3F8) and wrap it in a global lock for safe access.
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
}

/// Serial output macro used to send logs to the host.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::arch::x86_64::serial::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(concat!($fmt, "\n"), $($arg)*));
}

/// Send a raw byte stream to the host-side AI process.
pub fn send_byte(data: u8) {
    SERIAL1.lock().send(data);
}

/// Poll a byte from the host connection without blocking.
pub fn poll_receive() -> Option<u8> {
    let mut line_status = x86_64::instructions::port::Port::<u8>::new(0x3FD); // COM1 line status register
    let status = unsafe { line_status.read() };
    if (status & 1) != 0 { // Data Ready
        let mut data_port = x86_64::instructions::port::Port::<u8>::new(0x3F8); // COM1 data register
        Some(unsafe { data_port.read() })
    } else {
        None
    }
}
