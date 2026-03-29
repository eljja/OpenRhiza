// src/arch/x86_64/port.rs
use x86_64::instructions::port::Port;

/// AI 샌드박스가 하드웨어 상태를 읽기 위한 I/O 포트 읽기 (8-bit)
pub fn read_port_u8(port_addr: u16) -> u8 {
    let mut port: Port<u8> = Port::new(port_addr);
    unsafe { port.read() }
}

/// AI 샌드박스가 하드웨어에 명령을 내리기 위한 I/O 포트 쓰기 (8-bit)
pub fn write_port_u8(port_addr: u16, value: u8) {
    let mut port: Port<u8> = Port::new(port_addr);
    unsafe { port.write(value) }
}

/// 디스크(ATA PIO) 상태를 읽기 위한 I/O 포트 읽기 (16-bit)
pub fn read_port_u16(port_addr: u16) -> u16 {
    let mut port: Port<u16> = Port::new(port_addr);
    unsafe { port.read() }
}

/// 디스크(ATA PIO) 명령을 내리기 위한 I/O 포트 쓰기 (16-bit)
pub fn write_port_u16(port_addr: u16, value: u16) {
    let mut port: Port<u16> = Port::new(port_addr);
    unsafe { port.write(value) }
}