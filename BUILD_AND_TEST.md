# OpenRhiza Build, Run, and Verification Guide

This document describes the current build and test flow for the `main` branch as of April 2026.
It reflects the code that is actually present in the repository, including the native xHCI path,
the async executor, the VGA CLI, the native `e1000` path, and the Nexus signature-verification flow.

## Prerequisites

| Tool | Check | Notes |
|------|-------|-------|
| Rust nightly | `rustup show` | The project expects nightly on Windows |
| `x86_64-unknown-none` target | `rustup target list --installed` | Add with `rustup target add x86_64-unknown-none` |
| `wasm32-unknown-unknown` target | `rustup target list --installed` | Needed by `host_brain.py` when compiling Wasm drivers |
| `cargo-bootimage` | `cargo bootimage --help` | Install with `cargo install bootimage` |
| QEMU x86_64 | `qemu-system-x86_64 --version` | The path is currently hardcoded in `Cargo.toml` |
| Python 3.10+ | `python --version` | Needed for `host_brain.py`, `mock_nexus_server.py`, and `mock_signer.py` |
| `google-genai` | `pip show google-genai` | Needed only for the host-side AI workflow |

## Environment

The host AI path and the kernel Gemini path use:

```bash
GEMINI_API_KEY=your_api_key_here
```

The kernel build now also loads the key from the repository root `.env` automatically.
It accepts either:

```bash
GEMINI_API_KEY=your_api_key_here
OPENRHIZA_GEMINI_API_KEY=your_api_key_here
```

At build time the value is embedded into the OpenRhiza kernel as `OPENRHIZA_GEMINI_API_KEY`,
so `cargo build` and `cargo bootimage` use the same Gemini credential path without requiring a
separate manual export step.

## Build

Fast kernel build:

```bash
cargo build
```

Boot image build:

```bash
cargo bootimage
```

Release build:

```bash
cargo build --release
cargo bootimage --release
```

Artifacts:

- Kernel binary: `target/x86_64-unknown-none/debug/OpenRhiza`
- Boot image: `target/x86_64-unknown-none/debug/bootimage-OpenRhiza.bin`

## Run

Recommended:

```bash
cargo run
```

`cargo run` uses the `bootimage` runner configured in `.cargo/config.toml` and the QEMU command
declared in `Cargo.toml`.

On Windows, `cargo run` currently launches QEMU through `run_qemu.ps1` so the GUI window stays attached
to the desktop session more reliably than the older direct runner setup.

### Current QEMU device layout

The current repository is configured to boot with:

```text
qemu-system-x86_64.exe
  -drive format=raw,file={bootimage}
  -drive file=fat:rw:rhiza_drivers,format=raw,index=2
  -serial tcp:127.0.0.1:4444,server,nowait
  -netdev user,id=n1
  -device e1000,netdev=n1
  -device qemu-xhci,id=xhci
  -device usb-kbd,bus=xhci.0
```

Meaning:

- Primary boot image: OpenRhiza bootable disk image
- Secondary FAT disk: `rhiza_drivers/` mounted as a writable test disk
- Serial bridge: host tooling can connect to `127.0.0.1:4444`
- NIC: Intel `e1000`
- USB controller: QEMU xHCI
- USB keyboard: attached directly to the xHCI controller

### Current interactive UI behavior

After boot completes:

- the log area remains in the upper VGA rows
- the bottom row acts as a simple CLI input line
- `help`, `status`, and `clear` are currently implemented

Important current caveat:

- left Shift is working in the current QEMU path
- right Shift is still inconsistent under the present Windows plus QEMU USB keyboard setup and remains under investigation

## Optional Host AI Loop

If you want the legacy host-assisted flow:

```bash
python host_brain.py
python host_brain.py --model gemini-2.5-flash
python host_brain.py --model gemini-2.5-pro
```

This path is still useful for Wasm driver generation and protocol validation, but it is not the
only runtime path anymore.

## What To Expect At Boot

A healthy boot should show most of the following:

```text
OpenRhiza Seed (Layer 0) Booting... Serial Connected!
Heap Allocator initialized!
Total Usable Memory: ...
Hardware Discovery Complete.
Found N PCI devices:
...
[USB] Initializing xHCI Host Controller at BAR0: ...
[xHCI] Controller Running! Scanning ports...
[OS System] All subsystems initialized. Handing over to Async Executor.
[OS Core] Autonomous Nexus Fetch Success
```

You may also see:

- Storage probing logs for the FAT bootstrap disk
- xHCI port and HID keyboard logs
- Nexus fetch logs after the executor has been running for a short period
- `Engine running` on the VGA side once the executor is active
- a bottom-row `cli>` prompt

## Verification Checklist

### 1. Build

Run:

```bash
cargo build
```

Current status:

- The build succeeds on the current tree
- The current tree builds cleanly without Rust warnings

### 2. Boot

Run:

```bash
cargo run
```

Confirm:

- Serial output appears
- Heap initialization completes
- PCI enumeration lists devices
- xHCI initialization starts when the controller is present
- The executor starts
- The VGA bottom row shows the CLI prompt
- Enter submits a CLI command and redraws the prompt

### 3. Optional serial protocol validation

If you launch `host_brain.py`, confirm that it:

- Connects to `127.0.0.1:4444`
- Injects the default QWERTY keymap when prompted
- Optionally sends Wasm drivers over the serial protocol

### 4. Nexus verification path

The current core loop attempts a Nexus fetch after startup and verifies the returned Wasm payload
with the embedded Ed25519 public key before execution.

Important note:

- `src/net.rs` now routes traffic through the native `e1000` path when available
- `src/https.rs` currently implements the active TCP/HTTP fetch path
- `src/tls.rs` contains an in-repo TLS 1.3 client implementation, but it is not yet wired into the live Nexus path
- the repository has already been tested end-to-end against the local mock Nexus flow through payload extraction and Ed25519 verification

## Known Current Gaps

- `src/https.rs` still uses raw TCP/HTTP rather than the in-repo TLS client
- DHCP and DNS are still missing
- ATA write support is still missing
- Only one Wasm driver instance can be active at a time
- right Shift is not yet distinct in the current Windows QEMU USB keyboard path

## Troubleshooting

### `rust-lld` not found

```bash
rustup component add llvm-tools-preview
```

### QEMU path is wrong

Update `run_qemu.ps1` and the `run-command` entry in `Cargo.toml` to match your local QEMU installation path.

### Serial host cannot connect

Make sure QEMU is already running and listening on `127.0.0.1:4444`.

### Infinite reboot or triple fault

Use QEMU debug flags such as:

```text
-d int -no-reboot
```

and focus on serial-only logging while diagnosing early boot faults.

### Out of memory

Increase `HEAP_SIZE` in `src/allocator.rs` if a new feature genuinely requires more memory.
