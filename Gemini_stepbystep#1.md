# OpenRhiza Detailed Progress Log - Step 1

> Historical note:
> This file captures an older step-by-step checkpoint.
> It is useful for historical context only and should not override the current baseline docs.

This file is a longer-form narrative companion to `Gemini.md`.
It has been refreshed so that the description matches the current repository state rather than the older April 2 snapshot.

## 0. Long-Term Direction

OpenRhiza is trying to bootstrap an operating system where AI participates in the OS itself.
The architectural idea is still layered:

- Layer 0: minimal trusted seed, exception handling, sandboxing, early device access
- Layer 1: first sensory/control drivers such as keyboard, USB, and networking
- Layer 2+: advanced drivers, storage, accelerators, and local inference
- Layer 4: generated interfaces and a networked exchange of verified capabilities

## 1. What Exists Today

The current repository is no longer only a "boot and print text" prototype.
The active code path now includes:

- x86_64 bring-up using the patched bootloader
- IDT and exception handling
- LAPIC/IOAPIC initialization
- timer-driven async task execution
- serial and VGA output
- bottom-row VGA CLI rendering
- PCI enumeration plus DMA region discovery
- an embedded Wasm runtime with host functions for MMIO and packet exchange
- native xHCI controller initialization
- HID keyboard polling through the xHCI event path
- native `e1000` routing into the live `smoltcp` path
- Nexus payload fetch plus Ed25519 signature validation

## 2. Important Distinction: Present In Source vs Active In Boot Flow

Some modules are real and substantial, but not yet the active path at boot:

- `src/tls.rs` contains a software TLS 1.3 client
- `src/crypto/*` contains the crypto needed to support it

However:

- the main runtime still uses the queue-backed `WasmEthernetDevice` as the `smoltcp` integration surface
- the active Nexus fetch path is still implemented in `src/https.rs`
- the repository therefore contains more capability than the live boot path currently exercises

## 3. Milestone Narrative

### Early bootstrap phase

- bootable `no_std` Rust image
- VGA text output
- serial bridge to the host
- exception handling and interrupt setup

### Sandbox phase

- embedded Wasm runtime in the kernel
- host functions for MMIO and DMA
- runtime execution of injected Wasm payloads

### Host-assisted experimentation phase

- `host_brain.py` compiles AI-generated Rust into Wasm
- Wasm drivers are transferred over the serial protocol
- successful artifacts are cached under `nexus_cache/`

### Native systems phase

- APIC-based interrupt path
- async executor and timer futures
- native xHCI initialization and HID keyboard polling
- bottom-row CLI prompt and command handling
- native `e1000` activation in the live network path
- Ed25519 verification of Nexus-provided payloads
- Windows QEMU launcher stabilization through `run_qemu.ps1`

## 4. Current Open Questions

1. How should `https.rs` be replaced or wrapped by the software TLS client in `tls.rs`?
2. How should multiple Wasm drivers be tracked once USB, NIC, and storage paths all coexist?
3. Should the serial-injected dynamic keymap path remain, or be clearly downgraded to legacy-only status?
4. Why is right Shift still not arriving distinctly in the current Windows QEMU USB keyboard path?

## 5. Recommended Next Moves

- Either integrate `tls.rs` or clearly declare the current transport as development-only
- Add persistent ATA write support
- Replace single-instance Wasm storage with a driver registry
- Finish the right-Shift investigation with raw HID capture under the current Windows QEMU path
