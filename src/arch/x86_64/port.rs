// src/arch/x86_64/port.rs
use x86_64::instructions::port::Port;

/// AI 샌드박스가 하드웨어 상태(키보드 입력 등)를 읽기 위한 I/O 포트 읽기 원시 함수
pub fn read_port_u8(port_addr: u16) -> u8 {
    let mut port = Port::new(port_addr);
    unsafe { port.read() }
}

/// AI 샌드박스가 하드웨어(예: 마우스/키보드 컨트롤러)에 명령을 내리기 위한 I/O 포트 쓰기 원시 함수
pub fn write_port_u8(port_addr: u16, value: u8) {
    let mut port = Port::new(port_addr);
    unsafe { port.write(value) }
}