#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

global_asm!(
    r#"
    .section .text.boot
    .global _start
_start:
    ldr x1, =__exception_vector
    msr vbar_el1, x1
    isb
    ldr x0, =__stack_top
    mov sp, x0
    bl rust_main
1:
    wfe
    b 1b

    .align 11
__exception_vector:
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124
    b __exception_entry
    .space 124

__exception_entry:
    mrs x0, esr_el1
    mrs x1, elr_el1
    mrs x2, far_el1
    bl rust_exception
2:
    wfe
    b 2b
"#
);

const PL011_BASE: usize = 0x0900_0000;
const UART_DR: usize = PL011_BASE;
const UART_FR: usize = PL011_BASE + 0x18;
const UART_IBRD: usize = PL011_BASE + 0x24;
const UART_FBRD: usize = PL011_BASE + 0x28;
const UART_LCRH: usize = PL011_BASE + 0x2c;
const UART_CR: usize = PL011_BASE + 0x30;
const UART_IMSC: usize = PL011_BASE + 0x38;
const UART_FR_RXFE: u32 = 1 << 4;
const UART_FR_TXFF: u32 = 1 << 5;
const UART_LCRH_FEN: u32 = 1 << 4;
const UART_LCRH_WLEN_8: u32 = 0b11 << 5;
const UART_CR_UARTEN: u32 = 1;
const UART_CR_TXE: u32 = 1 << 8;
const UART_CR_RXE: u32 = 1 << 9;

const INPUT_LIMIT: usize = 160;
const PROMPT: &str = "input> ";

const VIRTIO_MMIO_BASE: usize = 0x0a00_0000;
const VIRTIO_MMIO_LEN: usize = 0x200;
const VIRTIO_MMIO_STRIDE: usize = 0x200;
const VIRTIO_MMIO_COUNT: usize = 32;
const VIRTIO_MMIO_IRQ_BASE: u32 = 16;
const VIRTIO_MAGIC: u32 = 0x7472_6976;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    writeln("panic");
    halt()
}

#[no_mangle]
pub extern "C" fn rust_exception(esr: u64, elr: u64, far: u64) -> ! {
    writeln("");
    writeln("KERNEL EXCEPTION");
    write("esr=");
    write_hex64(esr);
    write(" elr=");
    write_hex64(elr);
    write(" far=");
    write_hex64(far);
    writeln("");
    halt()
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    uart_init();
    writeln("");
    writeln("OpenRhiza ARM64 recovery core");
    writeln("platform=aarch64-qemu-virt serial=PL011");
    writeln("rule=minimal survival core only");
    writeln("core owns: boot, exception path, recovery console, bounded capability identity probes");
    writeln("core refuses: GUI policy, virtio drivers/payloads, production routing, filesystem policy, LLM policy");
    writeln("type /help for commands or natural-language text for the internal LLM handoff stub");
    prompt();

    let mut input = [0u8; INPUT_LIMIT];
    let mut len = 0usize;
    let mut last_input = [0u8; INPUT_LIMIT];
    let mut last_len = 0usize;
    let mut escape_state = 0u8;

    loop {
        let byte = uart_read();
        if escape_state != 0 {
            match (escape_state, byte) {
                (1, b'[') => escape_state = 2,
                (2, b'A') => {
                    if last_len > 0 {
                        len = last_len;
                        input[..last_len].copy_from_slice(&last_input[..last_len]);
                        redraw_input_line(&input[..len]);
                    }
                    escape_state = 0;
                }
                (2, b'B') => {
                    len = 0;
                    redraw_input_line(&input[..len]);
                    escape_state = 0;
                }
                _ => escape_state = 0,
            }
            continue;
        }

        match byte {
            b'\r' | b'\n' => {
                writeln("");
                let command = trim_ascii(&input[..len]);
                if !command.is_empty() {
                    last_len = command.len();
                    last_input[..last_len].copy_from_slice(command);
                }
                handle_command(command);
                len = 0;
                prompt();
            }
            0x08 | 0x7f => {
                if len > 0 {
                    len -= 1;
                    write("\x08 \x08");
                }
            }
            0x1b => escape_state = 1,
            b if (0x20..=0x7e).contains(&b) && len < input.len() => {
                input[len] = b;
                len += 1;
                uart_write(b);
            }
            _ => {}
        }
    }
}

fn handle_command(command: &[u8]) {
    if command.is_empty() {
        return;
    }
    if command[0] != b'/' {
        handle_natural_prompt(command);
        return;
    }

    if command == b"/help" {
        print_help();
        return;
    }
    if command == b"/clear" {
        write("\x1b[2J\x1b[H");
        writeln("OpenRhiza ARM64 recovery CLI");
        return;
    }
    if command == b"/status" {
        writeln("OpenRhiza ARM64 survival core is alive.");
        writeln("sandbox runtime: not embedded in recovery core");
        writeln("next: load drivers, GUI, production routing, and LLM workflows as skills after host ABI handoff");
        return;
    }
    if command == b"/platform-status" {
        writeln("platform.id=aarch64-qemu-virt");
        writeln("arch=aarch64");
        writeln("machine=qemu virt");
        writeln("console=PL011 serial recovery");
        writeln("sandbox.path=required for non-survival capabilities");
        return;
    }
    if command == b"/abi-status" || command == b"/driver-host-status" {
        print_abi_status();
        return;
    }
    if command == b"/virtio-mmio-probe" {
        virtio_mmio_probe();
        return;
    }
    if command == b"/registry-context" || command == b"/nexus-fetch" {
        print_registry_context();
        return;
    }
    if command == b"/skill-cache" {
        print_skill_cache();
        return;
    }
    if command == b"/display-status" || command == b"/gui-status" {
        writeln("[Display] recovery surface=PL011 serial");
        writeln("[Display] GUI/framebuffer policy is not compiled into ARM64 recovery core.");
        writeln("[Display] Load a sandbox display/GUI skill after the display ABI exists.");
        return;
    }
    if command == b"/input-routing-status" {
        writeln("[Input] recovery input=PL011 serial");
        writeln("[Input] virtio-input/HID parsing must be sandbox-owned and evidence-gated.");
        return;
    }
    if handle_skill_command(command) {
        return;
    }
    if is_removed_core_policy_command(command) {
        writeln("[Boundary] denied: this behavior is not ARM64 recovery core.");
        writeln("[Boundary] implement or run it as a sandbox skill/workflow through the internal console.");
        return;
    }
    if command == b"/poweroff" || command == b"/exit" {
        writeln("[Power] ARM64 PSCI SYSTEM_OFF requested.");
        system_off();
    }

    writeln("Unknown command. Use /help.");
}

fn print_help() {
    writeln("Local survival commands:");
    writeln("- /status");
    writeln("- /platform-status");
    writeln("- /abi-status");
    writeln("- /virtio-mmio-probe");
    writeln("- /registry-context");
    writeln("- /skill-cache");
    writeln("- /display-status");
    writeln("- /input-routing-status");
    writeln("- /skill-load <skill_id>");
    writeln("- /skill-run <skill_id>");
    writeln("- /clear");
    writeln("- /poweroff");
    writeln("Boundary rule: GUI, drivers, production routing, storage policy, and LLM policy are skill/workflow work.");
}

fn print_abi_status() {
    writeln("[ARM64 ABI] bounded platform identity probes only");
    write("- virtio-mmio range base=");
    write_hex64(VIRTIO_MMIO_BASE as u64);
    write(" window=");
    write_hex32(VIRTIO_MMIO_LEN as u32);
    write(" stride=");
    write_hex32(VIRTIO_MMIO_STRIDE as u32);
    write(" count=");
    write_u32(VIRTIO_MMIO_COUNT as u32);
    writeln("");
    writeln("- allowed: read-only transport identity probe");
    writeln("- forbidden in core: feature negotiation, queue programming, payload construction, live routing policy");
}

fn print_registry_context() {
    writeln("[Registry] internal console handoff is present, but remote registry/LLM execution is not core.");
    writeln("[Registry] required next skills: sandbox net transport, TLS/API workflow, registry lookup workflow.");
}

fn print_skill_cache() {
    writeln("[Skill Cache] ARM64 recovery knows seed skill names only; it does not embed their behavior.");
    writeln("- skill_virtio_mmio_probe_v1");
    writeln("- skill_virtio_net_driver_v1");
    writeln("- skill_virtio_block_driver_v1");
    writeln("- skill_virtio_input_driver_v1");
    writeln("- skill_virtio_gpu_driver_v1");
    writeln("- skill_display_framebuffer_mode_v1");
    writeln("- skill_gui_session_bootstrap_v1");
    writeln("- skill_gui_modern_shell_v1");
}

fn handle_skill_command(command: &[u8]) -> bool {
    let Some(text) = core::str::from_utf8(command).ok() else {
        return false;
    };
    if let Some(skill_id) = text.strip_prefix("/skill-load ") {
        write("[Skill] lookup request: ");
        write(skill_id.trim());
        writeln("");
        if known_arm64_skill(skill_id.trim()) {
            writeln("[Skill] known seed identity. Loading/execution must happen through sandbox artifact runtime, not core dispatch.");
        } else {
            writeln("[Skill] not in the local seed identity list. Registry download requires sandbox networking.");
        }
        return true;
    }
    if let Some(skill_id) = text.strip_prefix("/skill-run ") {
        write("[Skill] run request: ");
        write(skill_id.trim());
        writeln("");
        if skill_id.trim() == "skill_virtio_mmio_probe_v1" {
            writeln("[Skill] temporary compatibility: using the core read-only ABI probe.");
            virtio_mmio_probe();
        } else {
            writeln("[Skill] denied in core: use sandbox runtime export invocation after artifact load.");
        }
        return true;
    }
    false
}

fn known_arm64_skill(skill_id: &str) -> bool {
    matches!(
        skill_id,
        "skill_virtio_mmio_probe_v1"
            | "skill_virtio_net_driver_v1"
            | "skill_virtio_block_driver_v1"
            | "skill_virtio_input_driver_v1"
            | "skill_virtio_gpu_driver_v1"
            | "skill_display_framebuffer_mode_v1"
            | "skill_gui_session_bootstrap_v1"
            | "skill_gui_modern_shell_v1"
    )
}

fn handle_natural_prompt(prompt_bytes: &[u8]) {
    write("[Prompt] received: ");
    write_bytes(prompt_bytes);
    writeln("");
    writeln("[Prompt] internal console handoff is required, but ARM64 recovery core has no embedded LLM policy.");
    writeln("[Prompt] route this through sandbox net/TLS/registry/LLM workflow skills after they are loaded.");
}

fn is_removed_core_policy_command(command: &[u8]) -> bool {
    starts_with(command, b"/vos")
        || starts_with(command, b"/virtio-wasm")
        || starts_with(command, b"/vospolicy")
        || starts_with(command, b"/vospromote")
        || starts_with(command, b"/gui-start")
        || starts_with(command, b"/gui-redraw")
        || starts_with(command, b"/api-")
        || command == b"/gemini-test"
        || command == b"/virtio-payload-selftest"
}

fn virtio_mmio_probe() {
    writeln("[ARM64 ABI] read-only virtio-mmio identity probe begins.");
    let mut found = 0u32;
    let mut index = 0usize;
    while index < VIRTIO_MMIO_COUNT {
        let base = VIRTIO_MMIO_BASE + index * VIRTIO_MMIO_STRIDE;
        let magic = mmio_read32_at_base(base, 0x000).unwrap_or(0);
        let version = mmio_read32_at_base(base, 0x004).unwrap_or(0);
        let device_id = mmio_read32_at_base(base, 0x008).unwrap_or(0);
        let vendor_id = mmio_read32_at_base(base, 0x00c).unwrap_or(0);
        if magic == VIRTIO_MAGIC && version >= 1 && device_id != 0 {
            found = found.saturating_add(1);
            write("- base=");
            write_hex64(base as u64);
            write(" device_id=");
            write_hex32(device_id);
            write(" vendor=");
            write_hex32(vendor_id);
            write(" irq=");
            write_u32(VIRTIO_MMIO_IRQ_BASE + index as u32);
            writeln("");
        }
        index += 1;
    }
    if found == 0 {
        writeln("[ARM64 ABI] no active virtio-mmio device identity detected.");
    }
    writeln("[ARM64 ABI] probe complete; device drivers and payloads must remain sandbox skills.");
}

fn mmio_read32_at_base(base: usize, offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    if end > VIRTIO_MMIO_LEN || (offset & 0x3) != 0 {
        return None;
    }
    Some(unsafe { read_volatile((base + offset) as *const u32) })
}

fn uart_init() {
    unsafe {
        write_volatile(UART_CR as *mut u32, 0);
        write_volatile(UART_IMSC as *mut u32, 0);
        write_volatile(UART_IBRD as *mut u32, 1);
        write_volatile(UART_FBRD as *mut u32, 40);
        write_volatile(UART_LCRH as *mut u32, UART_LCRH_FEN | UART_LCRH_WLEN_8);
        write_volatile(
            UART_CR as *mut u32,
            UART_CR_UARTEN | UART_CR_TXE | UART_CR_RXE,
        );
    }
}

fn prompt() {
    write(PROMPT);
}

fn redraw_input_line(input: &[u8]) {
    write("\r");
    prompt();
    write_bytes(input);
    write("\x1b[K");
}

fn trim_ascii(mut input: &[u8]) -> &[u8] {
    while let Some((&first, rest)) = input.split_first() {
        if first == b' ' || first == b'\t' {
            input = rest;
        } else {
            break;
        }
    }
    while let Some((&last, rest)) = input.split_last() {
        if last == b' ' || last == b'\t' {
            input = rest;
        } else {
            break;
        }
    }
    input
}

fn starts_with(input: &[u8], prefix: &[u8]) -> bool {
    input.len() >= prefix.len() && &input[..prefix.len()] == prefix
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

fn write_bytes(bytes: &[u8]) {
    for &byte in bytes {
        uart_write(byte);
    }
}

fn writeln(text: &str) {
    write(text);
    write("\n");
}

fn write_u32(mut value: u32) {
    let mut buf = [0u8; 10];
    let mut pos = buf.len();
    if value == 0 {
        uart_write(b'0');
        return;
    }
    while value > 0 {
        pos -= 1;
        buf[pos] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    write_bytes(&buf[pos..]);
}

fn write_hex32(value: u32) {
    write("0x");
    for shift in (0..32).step_by(4).rev() {
        let nibble = ((value >> shift) & 0x0f) as u8;
        uart_write(hex_digit(nibble));
    }
}

fn write_hex64(value: u64) {
    write("0x");
    for shift in (0..64).step_by(4).rev() {
        let nibble = ((value >> shift) & 0x0f) as u8;
        uart_write(hex_digit(nibble));
    }
}

fn hex_digit(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        _ => b'a' + (nibble - 10),
    }
}

fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

fn system_off() -> ! {
    unsafe {
        asm!("hvc #0", in("x0") 0x8400_0008u64, options(nomem, nostack));
    }
    halt()
}
