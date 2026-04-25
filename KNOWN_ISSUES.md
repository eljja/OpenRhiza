# OpenRhiza Known Issues and Constraints

This file tracks the current state of the repository after the April 2026 documentation refresh.

## High Priority

### KI-001: Only one Wasm driver instance can be active

- Location: `src/core/seed.rs`
- Current state: `OpenRhizaSeed` stores `wasm_state: Option<WasmState>`
- Impact: loading a new Wasm driver replaces the previous one
- Result: no concurrent Wasm-based NIC + USB + storage driver set
- Suggested fix: move to a driver registry keyed by hardware or capability

### KI-002: ATA write support is missing

- Location: `src/storage.rs`
- Impact: successful drivers and payloads cannot be persisted back to disk from the kernel
- Current status: read-only bootstrap and payload extraction only

### KI-003: The live network path still carries legacy queue-wrapper structure

- Location: `src/net.rs`, `src/main.rs`, `src/e1000.rs`
- Current state:
  - `src/e1000.rs` is now initialized during PCI discovery
  - `src/net.rs` routes TX/RX traffic through the native `e1000` driver
  - the `WasmEthernetDevice` wrapper still exists as the `smoltcp` integration surface
- Impact: the NIC path is now live, but the abstraction layer still carries legacy queue-oriented naming and fallback behavior

### KI-004: The Nexus fetch path is not yet wired to the in-repo TLS client

- Location: `src/https.rs`, `src/tls.rs`
- Current state:
  - `src/https.rs` opens a TCP socket, sends an HTTP request, parses headers, extracts a payload, and verifies the Ed25519 signature
  - `src/tls.rs` contains a software TLS 1.3 implementation
  - the two are not connected in the active flow
- Impact: the transport path is not yet aligned with the repository's longer-term security goals

### KI-005: Right Shift is not yet distinct in the current Windows QEMU USB keyboard path

- Location: `src/arch/x86_64/usb.rs`, `run_qemu.ps1`
- Current state:
  - left Shift is observed and decoded correctly
  - right Shift is not consistently delivered to the guest as a distinct HID modifier in the current Windows plus QEMU setup
  - recent serial captures showed plain `a` input without the expected right-Shift modifier byte
- Impact: uppercase and symbol entry through the right Shift key is unreliable during interactive VGA CLI testing
- Suggested fix: continue investigating the QEMU input backend and compare raw HID reports across Windows host paths

### KI-006: USB numpad translation is incomplete in the current Windows QEMU USB keyboard path

- Location: `src/arch/x86_64/usb.rs`, `run_qemu.ps1`
- Current state:
  - PS/2 keyboard transport delivers numpad keys to the guest
  - USB keyboard transport under the current Windows plus QEMU path still does not reliably deliver numpad presses into the guest HID report stream
  - recent VGA and serial testing shows that the guest-side keypad mapping is present, but the expected HID usages are often missing before OpenRhiza decodes them
- Impact: keypad-driven numeric entry and keypad operators are unreliable in the preferred USB keyboard runtime path
- Suggested fix: inspect raw USB HID reports during keypad input and compare Windows host plus QEMU frontend behavior against the PS/2 fallback path

## Medium Priority

### KI-007: Global hardware state still relies on low-level mutable statics and raw pointers

- Location: multiple modules, especially `src/arch/x86_64/discovery.rs`, `src/arch/x86_64/usb.rs`, and `src/allocator.rs`
- Impact: the current code builds cleanly, but still depends on carefully constrained low-level global state
- Suggested fix: migrate global mutable state toward atomics, wrappers, or safer ownership patterns

### KI-008: Legacy placeholder modules still exist

- Location:
  - `src/arch/core_logic/seed.rs`
  - `src/arch/discovery.rs`
- Impact: these files can mislead future contributors because they no longer describe the active runtime path
- Suggested fix: either remove them or clearly isolate them as legacy references

## Resolved or Partially Resolved Historical Items

### KI-R01: Native keyboard support exists

- Status: implemented
- Notes: a full native QWERTY decoder exists in `src/keyboard.rs`
- Caveat: the older serial-injected keymap flow is still present in the core loop, and the current Windows QEMU path still has a right-Shift-specific mismatch under investigation

### KI-R02: Native xHCI initialization exists

- Status: implemented
- Notes: `src/arch/x86_64/usb.rs` now contains a substantial native xHCI path including command/event rings and HID keyboard polling

### KI-R03: Signed Nexus payload verification exists

- Status: implemented
- Notes: `src/security.rs` validates payloads against a built-in Ed25519 public key before Wasm execution

### KI-R04: TLS and crypto primitives exist in-tree

- Status: implemented but not fully integrated
- Notes: software SHA-256, AES-GCM, HKDF, P-256 ECDH, and a TLS 1.3 client are present

## Practical Interpretation

The repository is beyond the original "just boot and print text" stage.
However, several modules are ahead of the active runtime path, so "implemented in source" and
"active in boot flow" must be treated as different statuses.
