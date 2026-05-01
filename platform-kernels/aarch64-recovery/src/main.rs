#![no_std]
#![no_main]

use core::arch::global_asm;
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

global_asm!(
    r#"
    .section .text.boot
    .global _start
_start:
    ldr x0, =__stack_top
    mov sp, x0
    bl rust_main
1:
    wfe
    b 1b
"#
);

const PL011_BASE: usize = 0x0900_0000;
const UART_DR: usize = PL011_BASE + 0x00;
const UART_FR: usize = PL011_BASE + 0x18;
const UART_FR_RXFE: u32 = 1 << 4;
const UART_FR_TXFF: u32 = 1 << 5;
const INPUT_LIMIT: usize = 96;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    writeln("KERNEL PANIC");
    halt()
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    writeln("");
    writeln("OpenRhiza ARM64 recovery core");
    writeln("platform=aarch64-qemu-virt serial=PL011");
    writeln("rule=survival core only; drivers and policy must be sandbox capabilities");
    writeln("commands: /status /platform-status /help");
    prompt();

    let mut input = [0u8; INPUT_LIMIT];
    let mut len = 0usize;

    loop {
        let byte = uart_read();
        match byte {
            b'\r' | b'\n' => {
                writeln("");
                handle_command(&input[..len]);
                len = 0;
                prompt();
            }
            0x08 | 0x7f => {
                if len > 0 {
                    len -= 1;
                    write("\x08 \x08");
                }
            }
            b if (0x20..=0x7e).contains(&b) => {
                if len < input.len() {
                    input[len] = b;
                    len += 1;
                    uart_write(b);
                }
            }
            _ => {}
        }
    }
}

fn handle_command(command: &[u8]) {
    if command.is_empty() {
        return;
    }
    if command == b"/help" {
        writeln("Local commands: /status /platform-status /help");
        return;
    }
    if command == b"/status" {
        writeln("OpenRhiza ARM64 survival core is alive.");
        writeln("next: expose bounded sandbox host ABI, then load virtio skills.");
        return;
    }
    if command == b"/platform-status" {
        writeln("Platform expansion:");
        writeln("- current target: aarch64-qemu-virt");
        writeln("- recovery device: PL011 UART");
        writeln("- interrupt gate: GIC scaffold");
        writeln("- first registry keys: arch:aarch64, machine:qemu-aarch64-virt, virtio:mmio");
        writeln("- forbidden in core: virtio policy, GUI, filesystem, voice, app compatibility");
        return;
    }

    writeln("No LLM or registry path in this survival core yet.");
    writeln("Keep ARM64 boot minimal; load capabilities after sandbox ABI exists.");
}

fn prompt() {
    write("arm64> ");
}

fn uart_read() -> u8 {
    while unsafe { read_volatile(UART_FR as *const u32) } & UART_FR_RXFE != 0 {}
    unsafe { read_volatile(UART_DR as *const u32) as u8 }
}

fn uart_write(byte: u8) {
    while unsafe { read_volatile(UART_FR as *const u32) } & UART_FR_TXFF != 0 {}
    unsafe { write_volatile(UART_DR as *mut u32, byte as u32) };
}

fn write(text: &str) {
    for byte in text.bytes() {
        if byte == b'\n' {
            uart_write(b'\r');
        }
        uart_write(byte);
    }
}

fn writeln(text: &str) {
    write(text);
    write("\n");
}

fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
