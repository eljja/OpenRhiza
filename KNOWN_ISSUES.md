# OpenRhiza Known Issues and Constraints

This file tracks the current state of the repository after the May 1, 2026 runtime and documentation refresh.

## High Priority

### KI-001: Wasm capability multiplexing is present, but bounded scheduling and isolation are still incomplete

- Location: `src/core/seed.rs`
- Current state:
  - `OpenRhizaSeed` now stores `wasm_modules: Vec<LoadedWasmModule>`
  - named modules can coexist
  - polling now uses bounded round-robin capability multiplexing
- Remaining gap:
  - there is a bootstrap poll budget, but no per-module quota, eviction policy, or memory accounting
  - the runtime is still bootstrap-grade rather than a fully isolated multi-instance substrate
- Impact: concurrent capabilities are possible, but long-term fairness and fault containment still need work

### KI-002: ATA write support exists, but the persistent write floor is still bootstrap-grade

- Location: `src/storage.rs`
- Current state:
  - bounded FAT16 writes exist
  - sector write-back verification and cache flushes are now part of the write floor
- Remaining gap:
  - no journal
  - no power-failure recovery protocol
  - still limited to preallocated bootstrap files rather than a general filesystem layer
- Impact: persistence works for cache/config/bootstrap records, but it is not yet a general durable storage subsystem

### KI-003: The live network path still carries legacy queue-wrapper structure

- Location: `src/net.rs`, `src/main.rs`, `src/e1000.rs`
- Current state:
  - `src/e1000.rs` is now initialized during PCI discovery
  - `src/net.rs` routes TX/RX traffic through the native `e1000` driver
  - the `WasmEthernetDevice` wrapper still exists as the `smoltcp` integration surface
- Impact: the NIC path is now live, but the abstraction layer still carries legacy queue-oriented naming and fallback behavior

### KI-004: The service API path uses the in-repo TLS client, but Nexus fetch still follows a dedicated transport path

- Location: `src/https.rs`, `src/tls.rs`
- Current state:
  - `ApiClient` and Gemini/OpenRhiza HTTPS service calls run through the in-repo TLS client in `src/tls.rs`
  - `ApiResponse` now preserves HTTP headers for higher-level transport consumers
  - `NexusClient` still exists as a dedicated payload-and-signature fetch path
- Impact: most active HTTPS traffic now follows the in-tree TLS stack, but the Nexus fetch flow is still a separate client path instead of a single unified transport surface

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

### KI-009: `skill_gui_compositor_seed_v1` still needs fixed-slot seed-path regression testing

- Location: local seed skill path, currently surfaced during autorun and manual seed-load attempts
- Current state:
  - `skill_display_console_mode_v1`, `skill_display_framebuffer_mode_v1`, and `skill_gui_session_bootstrap_v1` load from fixed slots successfully in the known-good path
  - historical runs showed `skill_gui_compositor_seed_v1` bad-magic failures from `SK003.WAS`
  - the latest runtime changes have not yet been QEMU-regression-tested end-to-end
- Impact: bootstrap GUI can come up, but the compositor-seed follow-up stage should not be treated as fully hardened
- Suggested fix: regression test `SK003.WAS` generation, stripping, padding, seed-disk copy, and guest load flow end-to-end

### KI-010: GUI and conversation persistence need long-session regression testing

- Location: `src/display.rs`
- Current state:
  - major pointer-motion flicker sources were reduced
  - Korean and UTF-8 text handling were improved
  - long Gemini conversations, scroll persistence, and message retention still need repeat testing
- Impact: the GUI is usable in bootstrap form, but not yet a polished long-session desktop
- Suggested fix: add repeatable QEMU scenarios for long prompt/response cycles, Korean input, page scrolling, and GUI mode transitions

### KI-011: SMP substrate exists only as bootstrap state and heartbeat reporting

- Location: `src/smp.rs`, `src/arch/x86_64/apic.rs`
- Current state:
  - logical core count is discovered
  - boot-core APIC state is tracked
  - runtime heartbeats can report core activity
- Remaining gap:
  - no AP startup
  - no SIPI flow
  - no per-core executor
  - no interrupt affinity beyond bootstrap routing to core 0
- Impact: OpenRhiza is still effectively single-core even on multi-core hardware

### KI-012: Scheduler fairness improved, but execution remains cooperative and single-runner

- Location: `src/task/executor.rs`
- Current state:
  - queue capacity increased
  - wake-drop metrics exist, and dropped wakes request a full task rescan instead of becoming silent lost wakeups
  - run-loop batching prevents one flood of wakeups from monopolizing the executor as easily
- Remaining gap:
  - no preemption
  - no per-core queues
  - no work stealing
  - no task priority or CPU affinity model
- Impact: the system is more observable and more bounded, but not yet a high-performance SMP scheduler

### KI-013: Autonomy council is functional but still bootstrap-grade

- Location: `src/autonomy.rs`, `src/api_v1.rs`, `src/main.rs`
- Current state:
  - autonomy defaults to off
  - assist/council modes and interval are user-controlled
  - council requests use Gemini with practical/analytical/bold roles
  - autonomy-origin responses are separated from interactive prompts and do not execute machine actions
- Remaining gap:
  - no stale-cycle timeout recovery yet
  - no sandbox-owned autonomy agents yet
  - evidence gathering is still mostly prompt/context based rather than skill-orchestrated
- Impact: autonomy is a useful substrate, but not yet the final autonomous OS brain

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
