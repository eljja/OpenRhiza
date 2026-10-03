# OpenRhiza Module Map

This document describes the current repository layout and the role of each important module.
It distinguishes between:

- active runtime paths
- bootstrap-only paths
- optional tooling
- stale or historical modules

For the current core/sandbox boundary classification, see
[`CORE_BOUNDARY_AUDIT.md`](CORE_BOUNDARY_AUDIT.md). That file is the authoritative tag list for
`Permanent Core`, `Bootstrap Fallback`, and `Must Move To Skill`.

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
├── platform-kernels/
├── platforms/
├── targets/
├── tools/
├── bootloader-patched/
├── openrhiza-nexus/
├── host_brain.py
├── mock_nexus_server.py
└── mock_signer.py
```

## Active Kernel Modules

Boundary tags in this section are summaries only. If there is a conflict, prefer
[`CORE_BOUNDARY_AUDIT.md`](CORE_BOUNDARY_AUDIT.md).

### `src/main.rs`

Boundary tag: **Bootstrap Fallback**.

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
- coordinate boot autorun, autonomy task, skill loads, display transitions, GUI mutations, and display refresh

### `src/core/seed.rs`

Boundary tag: **Permanent Core**.

Current Wasm sandbox engine and host import boundary.

Responsibilities:

- parse and instantiate Wasm modules
- expose host functions for MMIO, DMA, packet flow, display requests, and GUI mutation requests
- call driver and skill entry points
- run bounded round-robin polling across named input and capability modules
- pass sandbox driver claim capability masks into the handle-scoped driver host ABI
- deny sandbox-active NIC cutover through the legacy stage import; active promotion needs a future validated driver promotion ABI

Important limitation:

- capability multiplexing now supports multiple named modules, but per-module quota/accounting/isolation remain bootstrap-grade rather than final

### `src/task/keyboard.rs`

Boundary tag: **Bootstrap Fallback**.

Async keyboard queue and command routing.

Current role:

- accept scancodes from the interrupt/input path
- decode native keyboard input
- feed both the recovery-shell input line and the bootstrap GUI composer
- expose local commands for runtime inspection, GUI mutation, and capability testing

### `src/input_handoff.rs`

Boundary tag: **Permanent Core**.

Raw HID packet and input-event object queue between the hardware transport fallback and sandbox input drivers.

Current role:

- accept bounded keyboard/mouse HID reports from the xHCI fallback transport
- expose kind-filtered packet fetch imports to sandbox input drivers
- accept sandbox-emitted key/mouse events back into the recovery/GUI input path
- keep routing mode, sandbox-active gates, rollback commands, and per-kind validation counters
- expose per-kind raw fetch and emitted event counters so input promotion can be evidence-based instead of self-declared by a Wasm module
- accept sandbox input health reports while keeping activation controlled by core/user validation gates
- boot autorun may load sandbox keyboard/mouse candidates, but promotion remains explicit and evidence-gated

This module must remain a transport and object-capability boundary only. Keymaps, IME, mouse scaling, and HID parser policy belong in `sandbox-input-drivers/`.

### `src/vga.rs`

Boundary tag: **Permanent Core / Bootstrap Fallback**.

Recovery-shell text surface.

Current role:

- maintain the recovery log area
- render the recovery input line
- provide the shared text snapshot used by display handoff and GUI overlay logic

### `src/display.rs`

Boundary tag: **Bootstrap Fallback, Must Shrink**.

Bootstrap high-resolution display and GUI object runtime.

Current responsibilities:

- hold active/requested display mode state
- maintain the framebuffer bootstrap surface
- render the `1920x1080` bootstrap GUI
- track GUI objects, hover, focus, session selection, and object-local redraw
- expose GUI mutation helpers used by sandbox skills and LLM machine actions
- expose bounded GUI object-bounds queries so sandbox skills can compute adaptive layouts without owning framebuffer memory

Important note:

- this is still a bootstrap presenter, not the final long-term GUI engine
- the long-term target remains sandbox-owned GUI behavior through narrow host imports

### `src/gui_contract.rs`

Boundary tag: **Permanent Core Candidate**.

Shared GUI scene and mutation contract used by the bootstrap renderer and the LVGL-style bridge.

### `src/gui_lvgl_bridge.rs`

Boundary tag: **Must Move To Skill**.

LVGL-style scene adapter for the same GUI object scene model.

This is now reference code only. The active core no longer imports it during normal scene reporting.

### `src/gui_font.rs`

Boundary tag: **Bootstrap Fallback**.

Modern GUI font atlas access for the bootstrap GUI.

### `src/net.rs`

Boundary tag: **Bootstrap Fallback**.

Current network-scaffolding layer.

Responsibilities:

- initialize `smoltcp`
- manage RX/TX flow
- provide the networking surface used by higher layers
- track native-fallback, sandbox-candidate, and sandbox-active network driver stages
- route TX/RX to Wasm only after a sandbox driver explicitly promotes `sandbox-active`
- require sandbox NIC health evidence and sustained RX/TX validation before accepting promotion
- expose `/net-driver-status` counters for native and sandbox RX/TX paths

### `src/https.rs`

Boundary tag: **Bootstrap Fallback**.

Current API transport client used by the live runtime.

Responsibilities:

- open a socket through `smoltcp`
- drive OpenRhiza/Gemini requests through the in-repo TLS path
- accumulate response bytes
- parse response headers and expose them to higher layers

Important limitation:

- `ApiClient` and active OpenRhiza/Gemini HTTPS calls now run through `src/tls.rs`
- Nexus fetch still follows a dedicated client path and is not yet fully unified with the generic API transport surface

### `src/autonomy.rs`

Boundary tag: **Must Move To Workflow**.

Bootstrap autonomy runtime and council coordinator.

Current responsibilities:

- persist autonomy mode and interval
- default autonomy to `off`
- expose `assist` and `council` mode state
- build role-separated Gemini prompts for practical, analytical, and bold agents
- summarize council votes without executing machine-action JSON
- keep user control over mode and interval

Important limitation:

- most autonomy planning and evidence gathering should move into sandbox skills/workflows over time
- stale-cycle timeout recovery is still a follow-up hardening item

### `src/security.rs`

Boundary tag: **Permanent Core**.

Nexus trust anchor and signature verification.

### `src/storage.rs`

Boundary tag: **Bootstrap Fallback**.

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

Boundary tag: **Permanent Core**.

Bounded storage host ABI for sandbox filesystem work.

Responsibilities:

- expose the optional `fs_harness.img` device as an object-like block target
- report block count, writability, filesystem hint, and scratch region
- translate sandbox block requests into bounded secondary-slave ATA reads/writes
- expose `flush_image` as a best-effort sync point for the QEMU harness; correctness is validated by readback and restore before promotion
- keep the active recovery FAT16 disk separate from filesystem skill validation
- remain a block-object boundary; controller-specific IDE behavior should move through `driver_host.rs` and sandbox driver artifacts
- track native-fallback, sandbox-candidate, and sandbox-active block-driver stages
- deny sandbox-active storage promotion until one sandbox driver proves controller claim, PCI/PIO access, bounded block read, scratch write/read/restore, sync/readback, no IO errors, and controller-backed block IO ownership

### `src/driver_host.rs`

Boundary tag: **Permanent Core**.

Handle-scoped device host ABI for sandbox drivers.

Responsibilities:

- match sandbox driver claims against enumerated PCI devices
- create opaque driver handles
- release temporary driver handles and their DMA grants after probe-only validation
- enforce per-handle capability bits for PCI config, MMIO, PIO, DMA, and IRQ operations
- bound DMA allocation/copy and MMIO/PIO offset windows
- allow non-disruptive PIIX IDE probing through a sandbox claim for `pci:8086:7010`
- allow non-disruptive xHCI probing through a sandbox claim for `pci:1b36:000d`
- report active grants and claim/release counters through `/driver-host-status`

The core must not contain device policy here. This module is a narrow object-capability gate that lets e1000, xHCI/HID, storage, and future low-level drivers move out of core without giving sandbox code raw machine access.

### `src/e1000.rs`

Boundary tag: **Must Move To Sandbox Driver**.

Substantial native Intel `e1000` NIC driver implementation.

Current status:

- initialized during PCI discovery
- active in the live boot network path
- `qemu_driver_pack_seed` can mark e1000 as a sandbox candidate and now keeps its device claims to baseline PCI identity
- `skill_e1000_sandbox_probe_v1` separately claims the e1000 device through Wasm and probes PCI/MMIO/DMA/IRQ/TX host imports
- probe-only e1000 validation releases its temporary handle after promotion is denied
- it is not yet a complete sandbox NIC driver

### `src/tls.rs`

Boundary tag: **Bootstrap Fallback**.

In-repo software TLS 1.3 client implementation.

Current status:

- active for OpenRhiza API and Gemini HTTPS requests
- still needs final unification with the dedicated Nexus fetch client

### `src/smp.rs`

Boundary tag: **Permanent Core**.

Bootstrap SMP and per-core runtime substrate.

Current responsibilities:

- track discovered logical core count
- track boot-core APIC identity
- record runtime heartbeats
- expose the current SMP bootstrap status while AP bring-up is still a stub

Important limitation:

- OpenRhiza still runs as a single-core runtime in practice

### `src/crypto/*`

Boundary tag: **Permanent Core / Bootstrap Fallback**.

Pure software cryptographic primitives.

## Architecture-Specific Modules

### `src/arch/x86_64/discovery.rs`

Boundary tag: **Permanent Core**.

Active x86_64 hardware discovery module.

### `src/arch/x86_64/interrupts.rs`

Boundary tag: **Permanent Core**.

Current interrupt path.

### `src/arch/x86_64/apic.rs`

Boundary tag: **Permanent Core**.

LAPIC and IOAPIC initialization.

### `src/arch/x86_64/serial.rs`

Boundary tag: **Permanent Core**.

COM1 serial output and polling helpers.

### `src/arch/x86_64/port.rs`

Boundary tag: **Permanent Core**.

Raw I/O port helpers.

### `src/arch/x86_64/usb.rs`

Boundary tag: **Bootstrap Fallback, Must Move Driver Logic**.

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

### `tools/platform_matrix.py`

Validates the machine-readable platform matrix and emits registry match keys.

### `tools/smoke_arm64_recovery.py`

Runs the standalone ARM64 recovery ELF under QEMU and checks for the expected serial prompt.

### `tools/voice_prompt_bridge.py`

Host-assisted voice bridge for early x86_64/QEMU voice testing.

It can write transcript handoff data and inject `/voice-route` plus `/voice-import` into the guest.

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
- e1000 sandbox host-ABI probe
- xHCI sandbox host-ABI probe
- PIIX IDE sandbox host-ABI probe
- storage host ABI scratch validation
- voice capture/router bridge
- virtio-mmio transport probe
- virtio-net, virtio-block, virtio-input, and virtio-gpu sandbox driver candidates

### `sandbox-input-drivers/`

Local Rust/Wasm input driver crates used to produce:

- `KEYBDRV.WAS` keyboard HID parser
- `MOUSEDRV.WAS` mouse HID parser

These are sandbox-owned input drivers. The core should keep only raw HID handoff, fallback routing, and rollback gates.

### `rhiza_drivers/`

Secondary FAT driver disk contents used by QEMU.

Includes:

- fixed skill slots
- seed Wasm artifacts
- cache text files
- boot autorun input

## Platform Bring-Up

### `platform-kernels/aarch64-recovery/`

Standalone ARM64 serial recovery kernel.

Current status:

- builds through `build_arm64_recovery.ps1`
- smoke-boots under QEMU with `tools/smoke_arm64_recovery.py`
- exposes `/abi-status` and read-only `/virtio-mmio-probe` diagnostics
- rejects GUI, virtio payload, production routing, storage policy, and LLM policy commands as non-core behavior
- proves only the survival serial path and bounded virtio-mmio transport identity, not the full OpenRhiza runtime
- keeps historical ARM64 proof code out of the default module tree so future work must reintroduce it as sandbox skills or explicit external harnesses

### `sandbox-skills/virtio_mmio_probe_seed/`

Read-only ARM64 virtio-mmio transport probe.

Purpose:

- claims `virtio-mmio:0x0a000000` through `os_driver_claim_mmio_device`
- reads standard virtio-mmio identity registers
- polls the capability IRQ line
- releases the handle after probing

It must remain a transport/capability proof. Virtio-net, virtio-block, virtio-input, and virtio-gpu behavior belongs in separate sandbox drivers.

### `sandbox-skills/virtio_*_driver_seed/`

Device-specific virtio sandbox driver candidates.

Current seeds:

- `virtio_net_driver_seed` targets standard virtio device ID `1`.
- `virtio_block_driver_seed` targets standard virtio device ID `2`.
- `virtio_gpu_driver_seed` targets standard virtio device ID `16`.
- `virtio_input_driver_seed` targets standard virtio device ID `18`.

Purpose:

- scan bounded QEMU `virt` MMIO transport windows
- claim one opaque MMIO/DMA/IRQ handle at a time
- verify device identity, vendor, features, queue limit, status, and IRQ metadata
- allocate sandbox DMA memory as virtqueue memory
- build and read back device-specific virtqueue payloads
- submit bounded payloads: virtio-net RX/TX buffers, virtio-block sector read plus guarded scratch-sector write, virtio-input event buffer, and virtio-gpu display-info command
- negotiate feature bits, program queue addresses, notify queues, read device-specific response buffers, and record used-ring or IRQ completion evidence
- commit only after the core-tracked live gate has all required evidence, then roll back and reset state
- release handles after candidate probing

They are not full persistent live drivers yet. They now submit bounded test payloads from sandbox DMA, but sustained live ownership remains blocked until repeated completion, rollback, and recovery tests pass per device.

### `sandbox-skills/arm64_virtio_gpu_payload_seed/`

Minimal ARM64 virtio-gpu payload smoke seed for the recovery `wasmi` experiment.

Purpose:

- keep a small Wasm module available while the full generic virtio seeds are tuned for ARM64 runtime execution
- exercise the same host-import surface used by sandbox drivers: MMIO claim/read/write, DMA alloc/read/write, queue notify, and rollback gate evidence
- remain diagnostic-only; it must not become a permanent core driver or boot-default policy

### `platforms/openrhiza-platforms.json`

Machine-readable platform matrix for x86_64, ARM64, Android research, and RISC-V parking-lot targets.

### `targets/*.json`

Rust target specifications used for architecture bring-up scaffolding.

## Legacy or Stale Modules

### `src/arch/discovery.rs`

Older placeholder discovery module.
Not part of the active boot path.

### `src/arch/core_logic/seed.rs`

Older placeholder seed implementation.
Not part of the active boot path.
