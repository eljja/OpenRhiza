// src/arch/x86_64/serial.rs
use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    // COM1 포트(0x3F8)를 초기화하고 전역적으로 안전하게 사용할 수 있도록 Lock을 겁니다.
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

/// 호스트 PC로 로그를 전송하기 위한 시리얼 출력 매크로
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

/// AI 샌드박스가 외부(호스트)로부터 데이터를 비동기적으로 읽어오기 위한 원시 함수
pub fn poll_receive() -> Option<u8> {
    let mut line_status = x86_64::instructions::port::Port::<u8>::new(0x3FD); // COM1 상태 레지스터
    let status = unsafe { line_status.read() };
    if (status & 1) != 0 { // 데이터가 들어왔다는 신호(Data Ready) 확인
        let mut data_port = x86_64::instructions::port::Port::<u8>::new(0x3F8); // COM1 데이터 레지스터
        Some(unsafe { data_port.read() })
    } else {
        None
    }
}