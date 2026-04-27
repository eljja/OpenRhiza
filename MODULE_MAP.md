# OpenRhiza Module Map

This document describes the current repository layout and the role of each important module.
It distinguishes between:

- active runtime paths
- bootstrap-only paths
- optional tooling
- stale or historical modules

## Top-Level Structure

```text
OpenRhiza/
├── .cargo/config.toml
├── Cargo.toml
├── run_qemu.ps1
├── src/
├── assets/
├── sandbox-skills/
├── rhiza_drivers/
├── bootloader-patched/
├── openrhiza-nexus/
├── host_brain.py
├── mock_nexus_server.py
└── mock_signer.py
```

## Active Kernel Modules

### `src/main.rs`

Kernel entry point and current boot orchestration.

Active responsibilities:

- initialize IDT
- initialize heap
- scan PCI and hardware identity
- initialize native `e1000` if discovered
- initialize USB input if discovered
- probe bootstrap storage
- create the Wasm seed runtime
- initialize the network stack
- start the async executor
- coordinate boot autorun, skill loads, display transitions, GUI mutations, and display refresh

### `src/core/seed.rs`

Current Wasm sandbox engine and host import boundary.

Responsibilities:

- parse and instantiate Wasm modules
- expose host functions for MMIO, DMA, packet flow, display requests, and GUI mutation requests
- call driver and skill entry points

Important limitation:

- capability multiplexing is still incomplete in some runtime paths, and older single-instance assumptions are not fully gone yet

### `src/task/keyboard.rs`

Async keyboard queue and command routing.

Current role:

- accept scancodes from the interrupt/input path
- decode native keyboard input
- feed both the recovery-shell input line and the bootstrap GUI composer
- expose local commands for runtime inspection, GUI mutation, and capability testing

### `src/vga.rs`

Recovery-shell text surface.

Current role:

- maintain the recovery log area
- render the recovery input line
- provide the shared text snapshot used by display handoff and GUI overlay logic

### `src/display.rs`

Bootstrap high-resolution display and GUI object runtime.

Current responsibilities:

- hold active/requested display mode state
- maintain the framebuffer bootstrap surface
- render the `1920x1080` bootstrap GUI
- track GUI objects, hover, focus, session selection, and object-local redraw
- expose GUI mutation helpers used by sandbox skills and LLM machine actions

Important note:

- this is still a bootstrap presenter, not the final long-term GUI engine
- the long-term target remains sandbox-owned GUI behavior through narrow host imports

### `src/gui_contract.rs`

Shared GUI scene and mutation contract used by the bootstrap renderer and the LVGL-style bridge.

### `src/gui_lvgl_bridge.rs`

LVGL-style scene adapter for the same GUI object scene model.

This is a reference bridge, not a core dependency.

### `src/gui_font.rs`

Modern GUI font atlas access for the bootstrap GUI.

### `src/net.rs`

Current network-scaffolding layer.

Responsibilities:

- initialize `smoltcp`
- manage RX/TX flow
- provide the networking surface used by higher layers

### `src/https.rs`

Current Nexus transport client used by the live runtime.

Responsibilities:

- open a TCP socket through `smoltcp`
- send an HTTP request
- accumulate response bytes
- parse response headers
- extract the signature header

Important limitation:

- despite the file name, it is not yet wired through `src/tls.rs`

### `src/security.rs`

Nexus trust anchor and signature verification.

### `src/storage.rs`

ATA PIO bootstrap storage path.

Responsibilities:

- read sectors from the secondary ATA bus
- identify secondary master and secondary slave ATA devices
- distinguish MBR from FAT boot sectors
- locate cached capability artifacts and fixed skill slots
- extract payloads into a `Vec<u8>`
- trim padded fixed-slot Wasm files back to their actual module length
- provide bounded write support for the active bootstrap FAT floor and optional harness use

### `src/storage_host.rs`

Bounded storage host ABI for sandbox filesystem work.

Responsibilities:

- expose the optional `fs_harness.img` device as an object-like block target
- report block count, writability, filesystem hint, and scratch region
- translate sandbox block requests into bounded secondary-slave ATA reads/writes
- keep the active recovery FAT16 disk separate from filesystem skill validation

### `src/e1000.rs`

Substantial native Intel `e1000` NIC driver implementation.

Current status:

- initialized during PCI discovery
- active in the live boot network path

### `src/tls.rs`

In-repo software TLS 1.3 client implementation.

Current status:

- present in source
- not yet connected to the active `https` flow

### `src/crypto/*`

Pure software cryptographic primitives.

## Architecture-Specific Modules

### `src/arch/x86_64/discovery.rs`

Active x86_64 hardware discovery module.

### `src/arch/x86_64/interrupts.rs`

Current interrupt path.

### `src/arch/x86_64/apic.rs`

LAPIC and IOAPIC initialization.

### `src/arch/x86_64/serial.rs`

COM1 serial output and polling helpers.

### `src/arch/x86_64/port.rs`

Raw I/O port helpers.

### `src/arch/x86_64/usb.rs`

Current native USB/xHCI implementation and HID polling path.

Current caveat:

- Right Shift is still not reliably distinct in the present Windows plus QEMU USB keyboard path

## Tooling and Companion Scripts

### `run_qemu.ps1`

Windows QEMU launcher used by the current `cargo run` flow.

Responsibilities:

- launch QEMU in the desktop session
- attach the boot image, FAT driver disk, optional raw filesystem harness disk, serial TCP bridge, `e1000`, and USB input devices
- seed fixed skill slots from the local driver artifacts
- launch the serial log viewer

### `tools/serial_log_server.ps1`

Serial log viewer launcher/helper.

### `tools/generate_gui_font.py`

Font atlas generation helper for the bootstrap GUI.

### `tools/strip_wasm_custom_sections.py`

Utility that strips Wasm custom sections before artifacts are copied into the driver disk.

### `tools/build_fs_harness.py`

Builds an optional raw `fs_harness.img` from a validated FAT32, exFAT, NTFS, or ext2/ext3/ext4 source image.

The output appends a scratch region and metadata footer so sandbox filesystem skills can validate bounded write/read/restore in OpenRhiza without mutating the active recovery storage floor.

### `host_brain.py`

Legacy host-side AI orchestration script.

Status note:

- preserved for historical and optional experimental workflows
- not the primary OpenRhiza runtime path anymore

### `mock_nexus_server.py`

Companion server for local Nexus-style payload flow testing.

### `mock_signer.py`

Helper for signing payloads compatible with the embedded trust root.

## Companion Project

### `openrhiza-nexus/`

Next.js-based companion registry and web surface.

## Assets and Seed Skills

### `assets/fonts/`

Bundled GUI font assets used by the bootstrap GUI path.

### `sandbox-skills/`

Local Rust/Wasm capability crates used to produce seed skill artifacts for:

- display console bootstrap
- framebuffer mode handoff
- GUI session bootstrap
- GUI compositor seed
- GUI scene mutator
- registry lookup

### `rhiza_drivers/`

Secondary FAT driver disk contents used by QEMU.

Includes:

- fixed skill slots
- seed Wasm artifacts
- cache text files
- boot autorun input

## Legacy or Stale Modules

### `src/arch/discovery.rs`

Older placeholder discovery module.
Not part of the active boot path.

### `src/arch/core_logic/seed.rs`

Older placeholder seed implementation.
Not part of the active boot path.
