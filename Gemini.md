# OpenRhiza Project Log

This file is the short-form running log for the repository.
Use `Gemini_stepbystep#1.md` for a more detailed narrative.

## Project Identity

- OpenRhiza is an AI-first operating system experiment
- The current repository focuses on Layer 0 and early Layer 1 bootstrap work
- Safety is centered on an in-kernel Wasm sandbox
- The long-term direction still includes a Nexus-style signed knowledge and driver exchange

## Current Repository State

### Active in the boot path

- Bare-metal x86_64 boot
- IDT and exception handling
- LAPIC/IOAPIC initialization
- Async executor with timer wakeups
- Serial logging
- VGA text output
- Bottom-row VGA CLI input line
- PCI enumeration and DMA region selection
- Wasm sandbox with MMIO and DMA host functions
- Native xHCI initialization
- USB HID keyboard polling
- Native `e1000` routing into the live `smoltcp` path
- Nexus payload fetch attempt plus Ed25519 signature verification

### Present in source but not yet the live path

- Full software TLS 1.3 stack
- Software crypto stack for SHA-256, AES-GCM, HKDF, and P-256 ECDH

## Key Historical Milestones

- Basic QEMU boot and VGA text output
- IDT and interrupt safety net
- Serial host link to `host_brain.py`
- Wasm sandbox integration
- Bootloader memory-map and early paging bring-up fixes
- Host-assisted Wasm driver compilation/injection loop
- Async executor and timer-driven task model
- Native xHCI USB controller bring-up with HID keyboard polling
- Bottom-row CLI rendering and native keyboard input loop
- Native `e1000` activation in the live network path
- Warning cleanup so `cargo build` is clean again
- Signed Nexus payload verification before Wasm execution

## Immediate Gaps

- Only one Wasm instance can be active
- ATA write path is missing
- `tls.rs` is not yet connected to the live Nexus client
- Right Shift is still inconsistent in the current Windows QEMU USB keyboard path

## Working Environment

- Target platform: x86_64 bare metal via QEMU during bring-up
- Development host: Windows
- Build flow: `cargo build`, `cargo bootimage`, `cargo run`
- Runtime launcher: `run_qemu.ps1` for the current Windows QEMU GUI flow
