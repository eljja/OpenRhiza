# Sandbox Input Driver Model

OpenRhiza should treat input parsing as a sandbox capability, not as kernel policy.

The kernel should survive without a sandbox input driver, but the preferred architecture is:

- USB or transport in core
- parser and behavior in sandbox
- runtime activation without reboot

## Driver Responsibilities

A sandbox input driver should:

1. fetch raw HID packets from the handoff queue
2. parse only the device formats it owns
3. emit canonical input events
4. avoid direct writes into VGA, keyboard, or unrelated kernel state
5. declare whether it is active and what routing mode it expects

## Minimum Wasm ABI

Current host imports:

- `os_fetch_hid_packet(ptr, max_len) -> u32`
- `os_emit_input_event(kind, a, b, c)`
- `os_set_input_driver_mode(mode)`
- `os_set_input_driver_active(active)`

Optional export:

- `poll_input_driver()`

The OS calls `poll_input_driver()` if the sandbox module provides it.

## Transitional Fallback

OpenRhiza currently still includes bootstrap keyboard and mouse parsers in core.

That is not the final architecture.

Those parsers exist only to preserve usability while the handoff ABI and event sink are stabilized.

The intended order is:

1. raw HID capture in core
2. canonical input event sink in core
3. sandbox parser driver
4. bootstrap parser removal

The repository now contains:

- a sandbox bootstrap mouse driver
- a sandbox bootstrap keyboard driver

These are not yet the final production input drivers, but they provide a working migration path away from hard-coded kernel parsing.

Current operator commands:

- `/sandbox-mouse-load`
- `/sandbox-keyboard-load`
- `/input-routing-status`
- `/input-activate <keyboard|mouse>`
- `/input-rollback <keyboard|mouse>`

## Activation Model

Input drivers should follow the same hot-swap model as other non-core components:

1. `discovered`
2. `staged`
3. `testing`
4. `active`
5. `rollback`

Input activation should prefer:

- sandbox-first validation
- runtime activation
- immediate rollback if keyboard or pointer behavior degrades
- persisted active binding only after successful testing

## Safety Rules

Input is a survival path.

That means sandbox input activation must be conservative:

- never remove the last known-good keyboard path without a rollback target
- keep a bootstrap fallback until sandbox input is stable
- prefer mirrored handoff while validating new parser drivers
- treat keyboard regression as higher risk than mouse regression

## Preferred Runtime Flow

When the user asks to enable or improve input support:

1. inspect current hardware
2. query OpenRhiza.com for existing input drivers or skills
3. reuse known-good parser drivers first
4. if none exist, generate a sandbox input driver
5. test in mirrored handoff mode
6. if successful, promote to sandbox-preferred mode
7. upload artifact, evaluation, comments, and votes

The current implementation now supports:

- testing load without reboot
- promotion to persisted active binding
- rollback to bootstrap fallback
- automatic restore of persisted input bindings on the next boot
- dynamic keyed Wasm module loading, so input drivers are no longer limited to a fixed two-slot sandbox layout

## Long-Term Result

The end state should be:

- mouse logic outside the kernel
- keyboard logic outside the kernel
- the kernel only owns transport, handoff, and survival fallback
- sandbox input drivers become plug-and-play runtime modules
