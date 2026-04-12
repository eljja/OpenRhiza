# OpenRhiza Module Map

This document describes the current repository layout and the role of each important module.
It intentionally distinguishes between active runtime paths, optional tooling, and stale or legacy modules.

## Top-Level Structure

```text
OpenRhiza/
├── .cargo/config.toml
├── Cargo.toml
├── host_brain.py
├── mock_nexus_server.py
├── mock_signer.py
├── run_qemu.ps1
├── src/
├── bootloader-patched/
├── rhiza_drivers/
├── nexus_cache/
└── openrhiza-nexus/
```

## Active Kernel Modules

### `src/main.rs`

Kernel entry point and current boot orchestration.

Active responsibilities:

- initialize IDT
- set physical memory offset
- disable PIC and initialize APIC
- initialize heap
- scan PCI and hardware identity
- initialize native `e1000` if discovered
- initialize native xHCI if discovered
- probe bootstrap storage
- create `OpenRhizaSeed`
- initialize the network stack
- start the async executor

The `core_os_task` inside `main.rs` also:

- polls Wasm networking
- polls USB keyboard input through xHCI
- optionally processes host-side serial protocol bytes
- kicks off Nexus payload download and signature verification

### `src/core/seed.rs`

Current Wasm sandbox engine.

Responsibilities:

- parse and instantiate Wasm modules
- expose host functions:
  - `read_mmio`
  - `write_mmio`
  - `alloc_dma_page`
  - `os_rx_packet`
  - `os_fetch_tx_packet`
- call `init_driver`
- optionally call `poll_net`

Important limitation:

- only one active Wasm instance is stored at a time

### `src/task/mod.rs`

Task abstraction for the async executor.

### `src/task/executor.rs`

Single-core async executor built on:

- `TaskId`
- `ArrayQueue`
- cached wakers
- `enable_and_hlt()` when idle

### `src/task/timer.rs`

Tick-based timer future support.

- `timer_tick()` advances global ticks
- `sleep_ticks()` allows cooperative delays in async tasks

### `src/task/keyboard.rs`

Async keyboard scancode queue.

Current role:

- accepts scancodes from the interrupt path
- decodes native keyboard input through `src/keyboard.rs`
- supports the legacy dynamic keymap injection path as an optional compatibility path
- runs an async keyboard task for the bottom-row CLI input line
- handles `help`, `status`, and `clear`

### `src/vga.rs`

VGA text writer and printing macros.

Current role:

- maintain the upper log area
- render the bottom-row CLI prompt
- keep a software cursor for the active input line

### `src/net.rs`

Current network-scaffolding layer.

Important note:

- this is the active network path in the boot flow
- it still exposes a `WasmEthernetDevice` wrapper to `smoltcp`
- the live TX/RX path now routes through the native `e1000` driver when present
- the queue layer remains as a fallback and as the Wasm host-function bridge

Responsibilities:

- initialize `smoltcp`
- manage global RX/TX queues
- provide the `smoltcp` `Device` wrapper
- create TCP sockets for higher layers

### `src/https.rs`

Current Nexus transport client used by the live runtime.

Responsibilities:

- open a TCP socket through `smoltcp`
- send an HTTP request
- accumulate response bytes
- parse response headers
- extract the `X-Nexus-Signature` header
- return `(payload, signature)` to the caller

Important limitation:

- despite the file name, it is not yet wired through `src/tls.rs`

### `src/security.rs`

Nexus trust anchor and signature verification.

- stores a built-in Ed25519 public key
- validates downloaded Wasm payloads before sandbox execution

### `src/storage.rs`

ATA PIO read-only bootstrap storage path.

Responsibilities:

- read sectors from the secondary ATA bus
- distinguish MBR from FAT boot sectors
- locate `E1000.BIN` or `E1000.WAS`
- extract the full payload length into a `Vec<u8>`

Limitation:

- no write path yet

### `src/keyboard.rs`

Native PS/2 scancode decoder and full QWERTY mapping logic.

This module is present and useful, but note that the active keyboard flow in `main.rs` currently emphasizes
USB keyboard polling plus the async keyboard task.

### `src/e1000.rs`

Substantial native Intel `e1000` NIC driver implementation.

Capabilities in source:

- MMIO register access
- EEPROM MAC read
- RX/TX descriptor setup
- DMA-backed buffers

Current status:

- initialized from `main.rs` during PCI discovery
- active in the live boot network path
- still shares some higher-level integration through `src/net.rs`

### `src/tls.rs`

In-repo software TLS 1.3 client implementation.

Dependencies:

- `src/crypto/sha256.rs`
- `src/crypto/aes.rs`
- `src/crypto/p256.rs`
- `src/crypto/random.rs`

Current status:

- present in source
- not yet connected to `src/https.rs`

### `src/crypto/*`

Pure software cryptographic primitives:

- `aes.rs`: AES-128 and AES-GCM
- `bignum.rs`: 256-bit arithmetic
- `p256.rs`: P-256 ECDH
- `random.rs`: entropy helper using RDRAND with fallback
- `sha256.rs`: SHA-256, HMAC, HKDF
- `mod.rs`: exports the crypto modules

## Architecture-Specific Modules

### `src/arch/x86_64/discovery.rs`

Active x86_64 hardware discovery module.

- CPUID-based core count
- usable-memory scan
- DMA base selection
- PCI enumeration

### `src/arch/x86_64/interrupts.rs`

Current interrupt path.

- IDT setup
- page-fault handler
- timer interrupt handler
- keyboard interrupt handler

### `src/arch/x86_64/apic.rs`

LAPIC and IOAPIC initialization.

### `src/arch/x86_64/serial.rs`

COM1 serial output and polling helpers.

### `src/arch/x86_64/port.rs`

Raw I/O port helpers for 8-bit and 16-bit access.

### `src/arch/x86_64/usb.rs`

Current native xHCI implementation.

Implemented pieces include:

- controller halt/reset/start
- command ring
- event ring
- DCBAA
- scratchpad handling
- slot enable
- address device
- endpoint configuration
- HID keyboard polling

This is one of the most advanced active modules in the repository.

Current caveat:

- right Shift is still not reliably distinct in the present Windows plus QEMU USB keyboard path, even though left Shift and ordinary keys are functioning

### `src/arch/x86_64/linker.ld`

Kernel linker script.

## Legacy or Stale Modules

### `src/arch/discovery.rs`

Older placeholder discovery module.
Not part of the active boot path.

### `src/arch/core_logic/seed.rs`

Older placeholder seed implementation.
Not part of the active boot path.

## Tooling and Companion Scripts

### `host_brain.py`

Host-side AI orchestration script.

Responsibilities:

- connect to the serial bridge
- generate Rust code via Gemini
- compile generated code into Wasm
- inject Wasm payloads into the running kernel
- cache validated payloads under `nexus_cache/`

### `run_qemu.ps1`

Windows QEMU launcher used by the current `cargo run` flow.

Responsibilities:

- launch QEMU in the desktop session through PowerShell
- attach the boot image, FAT driver disk, serial TCP bridge, `e1000`, and xHCI keyboard
- keep the window alive with `-no-reboot` and `-no-shutdown`

### `mock_nexus_server.py`

Companion server for exercising the Nexus-style payload flow locally.

### `mock_signer.py`

Helper for signing payloads compatible with the embedded Ed25519 trust root.

## Companion Project

### `openrhiza-nexus/`

Separate Next.js-based companion application included in the repository.
It is not part of the bare-metal kernel build.

## Bootloader

### `bootloader-patched/`

Local patched copy of the `bootloader` crate used by this repository.
It is part of the actual build and should be treated as a first-class dependency, not a vendor dump.
