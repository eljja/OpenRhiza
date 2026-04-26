# OpenRhiza Development Harness

This is the primary handoff document for anyone continuing work on OpenRhiza.
It is written for human contributors and for model-to-model handoff.

## 0. Project Identity

OpenRhiza is an attempt to build an operating system where AI is not an application on top of the OS,
but part of the OS itself.

The current codebase focuses on the bootstrap layers:

- Bare-metal `no_std` Rust kernel on x86_64
- Layer 0 sandboxing through an embedded Wasm runtime
- Recovery I/O through serial, VGA text, APIC timer, and native input support
- Early networking scaffolding through `smoltcp`
- Signed Nexus payload verification through Ed25519
- Bootstrap framebuffer and `1920x1080` GUI handoff
- Object-scoped GUI mutation path

The long-term direction remains:

- Layer 0: minimal trusted seed
- Layer 1: first sensory/control drivers
- Layer 2+: advanced drivers, storage, accelerators, local inference
- Layer 4: generative applications and networked AI exchange

## 1. Read These First

| File | Purpose |
|------|---------|
| `ARCHITECTURE.md` | High-level layered architecture and evolution model |
| `OS.md` | The baseline operating rules and non-negotiable architectural constraints |
| `DISPLAY_ABI.md` | Core/sandbox boundary for display and GUI handoff |
| `GUI_DEVELOPMENT.md` | Current GUI direction and dual-track native/LVGL strategy |
| `LEVEL1_BLUEPRINT.md` | Design intent for early bootstrap and safety boundaries |
| `MODULE_MAP.md` | Current module inventory and active vs stale paths |
| `BUILD_AND_TEST.md` | Current build/run/verification workflow |
| `PROTOCOL.md` | Development serial protocol used by the host tooling |
| `KNOWN_ISSUES.md` | Current unresolved problems and stale assumptions |
| `Gemini.md` | Historical short log only; not authoritative for current architecture |

## 2. Current Code Reality

This section is intentionally about the code that exists today, not the idealized roadmap.

### Implemented and active in the current boot path

- x86_64 boot through the patched `bootloader`
- IDT plus exception handling
- LAPIC/IOAPIC initialization
- Async task executor with a timer-driven sleep primitive
- Serial logging over COM1
- VGA text output
- PCI enumeration
- DMA region discovery from usable physical memory
- Wasm sandbox with host functions for MMIO, DMA allocation, RX injection, and TX fetch
- Queue-backed `smoltcp` interface
- Native xHCI initialization and USB keyboard polling
- Recovery-shell CLI with native keyboard input
- Native `e1000` traffic routed into the live network path
- Nexus payload download attempt plus Ed25519 signature verification before Wasm execution
- Boot autorun through the seed driver disk
- Bootstrap framebuffer and GUI session handoff
- Object-based sidebar, conversation, composer, and footer rendering

### Implemented but not fully integrated into the live boot path

- Full software TLS 1.3 stack in `src/tls.rs`
- Software crypto primitives for SHA-256, AES-GCM, HKDF, and P-256 ECDH

### Still legacy, partial, or stale

- Serial-injected dynamic keymap path is still supported, but native keyboard handling also exists
- `src/arch/core_logic/seed.rs` and `src/arch/discovery.rs` are legacy placeholders
- Documentation written on April 2, 2026 no longer matched the code until this refresh
- Right Shift is still unreliable in the current Windows plus QEMU USB keyboard path

## 3. Cross-Model Workflow

Recommended handoff sequence:

1. Read this file
2. Read `MODULE_MAP.md`
3. Read `KNOWN_ISSUES.md`
4. Read the modules you are about to change
5. Run `cargo build`
6. Make the change
7. Run `cargo build` again
8. Update docs if behavior or architecture changed

## 4. Engineering Rules

### Architecture rules

1. Keep Layer 0 small and auditable.
2. Treat sandboxing as mandatory for AI-generated code paths.
3. Do not pretend a module is "done" if it is not connected to the active runtime path.
4. Prefer documenting active, partial, and stale paths separately.

### Code rules

1. Stay in `#![no_std]`.
2. Keep `unsafe` localized and justified.
3. Avoid panics in runtime paths unless they protect invariant violations during bring-up.
4. When user-visible logging changes, update the docs that describe it.

### Process rules

1. Build after each meaningful change.
2. Update `MODULE_MAP.md` and `KNOWN_ISSUES.md` when active behavior changes.
3. Do not silently keep stale docs around once the code moves on.

## 5. Current Residual Risks

- Only one Wasm driver instance is stored in `OpenRhizaSeed`
- `https.rs` is not actually performing a TLS-backed HTTPS transaction yet
- ATA is still read-only
- Right Shift does not yet behave distinctly in the current Windows QEMU USB keyboard path
- Several modules still rely on low-level global state and raw pointers, even though the warning cleanup work is complete

## 6. Suggested Near-Term Priorities

### Priority 1

- Wire the in-repo TLS client into the Nexus fetch path, or explicitly keep the current path as plain HTTP during development
- Finish the QEMU right-Shift investigation with raw HID report comparison on Windows

### Priority 2

- Support multiple Wasm driver instances or a proper driver registry
- Add ATA write support for persistent cache storage

### Priority 3

- DHCP and DNS
- Better documentation around native vs host-assisted driver loading
- Cleanup of stale legacy modules
- Continue shrinking redraw scope and remaining GUI flicker
- Stabilize the compositor-seed follow-up stage

## 7. Code Review Checklist

- Does the change stay inside `no_std`?
- Does it introduce new low-level global state exposure?
- Does it change the active boot path or only add dormant code?
- Is the logging accurate enough for early boot debugging?
- Does `cargo build` still pass?
