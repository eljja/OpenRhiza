# USB HID Handoff ABI

OpenRhiza should not keep full keyboard and mouse logic inside the kernel.

The kernel should only provide the minimum survival path:

- xHCI or USB transport
- device enumeration
- interrupt transfer completion
- raw HID packet capture
- a safe handoff boundary to the sandbox
- a canonical input event sink for the running OS

Everything above that boundary should eventually move toward sandbox drivers.

## Current Transitional Model

Today OpenRhiza uses a transitional model:

1. USB core captures raw HID reports.
2. The reports are placed into a raw handoff queue.
3. If no sandbox input driver is active, a bootstrap parser still runs in core.
4. The bootstrap parser no longer writes directly to keyboard or VGA state.
5. Instead, it emits canonical input events into the runtime input event queue.
6. The runtime input sink applies those canonical events to the current system.

This keeps the current system usable while removing direct coupling between USB parsing and final UI behavior.

## Raw HID Packet Format

The raw handoff queue uses this fixed packet shape:

- `kind`
  - `1` = keyboard
  - `2` = mouse
- `slot_id`
- `port_id`
- `report_len`
- `report[8]`

The current Wasm host import is:

- `os_fetch_hid_packet(ptr, max_len) -> u32`

The guest receives 12 bytes:

1. `kind`
2. `slot_id`
3. `port_id`
4. `report_len`
5. `report[0..8]`

## Canonical Input Event ABI

Sandbox input drivers should not write VGA state or keyboard queues directly.

They should emit canonical events:

- `KeyScancode`
- `MousePacket`

The current Wasm host import is:

- `os_emit_input_event(kind, a, b, c)`

Current meanings:

- `kind = 1`
  - key scancode event
  - `a = scancode`
  - `b = extended flag`
  - `c = pressed flag`
- `kind = 2`
  - mouse packet event
  - `a = dx`
  - `b = dy`
  - `c = buttons`

## Routing Modes

The runtime supports four routing modes:

- `BootstrapOnly`
- `HandoffMirror`
- `SandboxPreferred`
- `SandboxExclusive`

Current policy:

- `HandoffMirror` is the safest default during development.
- Raw packets are always queued.
- Bootstrap parsing continues unless a sandbox input driver is active and routing mode prefers sandbox ownership.

The current Wasm host imports are:

- `os_set_input_driver_mode(mode)`
- `os_set_input_driver_active(active)`

## Why This Boundary Matters

This boundary allows OpenRhiza to keep the kernel minimal:

- USB core only knows transport and survival fallback
- keyboard and mouse parsing can become sandbox modules
- activation can happen without reboot
- rollback can happen without reboot

That matches the OpenRhiza principle:

- core is minimal
- non-core logic is hot-swappable
- sandbox is the default entry path

## Immediate Next Refactor

The next refactor should remove the remaining bootstrap parsers for:

- keyboard HID Boot report decoding
- mouse HID Boot report decoding

Those should become sandbox input drivers that read from `os_fetch_hid_packet` and emit canonical events through `os_emit_input_event`.

The repository now includes first-stage sandbox bootstrap drivers for:

- mouse
- keyboard

They still rely on core transport and fallback routing, but they prove the ABI works end to end.

The runtime now also supports:

- testing load of sandbox input drivers
- promotion to persisted active input bindings
- rollback to bootstrap fallback without reboot
- automatic restore of persisted `input:keyboard` and `input:mouse` bindings on boot
- dynamic keyed Wasm module registration instead of a fixed keyboard/mouse-only sandbox slot layout

## Long-Term Goal

The long-term target is:

- kernel transport only
- sandbox input parser drivers
- runtime activation and rollback
- persisted preferred input driver binding for later boots

The current implementation is intentionally transitional so the system remains usable while the boundary is being established.
