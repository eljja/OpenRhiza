# OpenRhiza Build, Run, and Verification Guide

This document describes the current build and test flow for the `main` branch as of late April 2026.

It reflects the code that is actually active now:

- recovery console bootstrap
- framebuffer validation handoff
- `1920x1080` bootstrap GUI
- sandbox display skills and GUI mutation path
- native `e1000` and USB input bring-up
- Nexus capability lookup plus signature verification

Authoritative note:

- For system rules, prefer [OS.md](D:/python/github/OpenRhiza/OpenRhiza/OS.md)
- For display/core boundaries, prefer [DISPLAY_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/DISPLAY_ABI.md)
- For GUI direction, prefer [GUI_DEVELOPMENT.md](D:/python/github/OpenRhiza/OpenRhiza/GUI_DEVELOPMENT.md)
- The older `host_brain.py` flow is now a legacy development path, not the primary runtime path

## Prerequisites

| Tool | Check | Notes |
|------|-------|-------|
| Rust nightly | `rustup show` | The project expects nightly on Windows |
| `x86_64-unknown-none` target | `rustup target list --installed` | Add with `rustup target add x86_64-unknown-none` |
| `wasm32-unknown-unknown` target | `rustup target list --installed` | Needed for sandbox skill and driver Wasm builds |
| `cargo-bootimage` | `cargo bootimage --help` | Install with `cargo install bootimage` |
| QEMU x86_64 | `qemu-system-x86_64 --version` | The path is currently hardcoded in `Cargo.toml` |
| Python 3.10+ | `python --version` | Needed for build helpers, serial log tooling, and optional mock tooling |
| `google-genai` | `pip show google-genai` | Needed only for optional legacy host-side AI flows |

## Environment

The kernel Gemini path uses:

```bash
GEMINI_API_KEY=your_api_key_here
```

The kernel build also loads the key from the repository root `.env` automatically.
It accepts either:

```bash
GEMINI_API_KEY=your_api_key_here
OPENRHIZA_GEMINI_API_KEY=your_api_key_here
```

At build time the value is embedded into the OpenRhiza kernel as `OPENRHIZA_GEMINI_API_KEY`,
so `cargo build` and `cargo bootimage` use the same Gemini credential path without requiring a separate manual export step.

## Build

Fast kernel build:

```bash
cargo build
```

Boot image build:

```bash
cargo bootimage
```

Sandbox skill rebuild:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\build_sandbox_skills.ps1
```

Companion web build:

```bash
cd openrhiza-nexus
npm run build
```

Artifacts:

- Kernel binary: `target/x86_64-unknown-none/debug/OpenRhiza`
- Boot image: `target/x86_64-unknown-none/debug/bootimage-OpenRhiza.bin`
- Seed skill artifacts: `rhiza_drivers/*.WAS`

## Run

Recommended:

```bash
cargo run
```

The most direct Windows GUI test command is:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu.ps1 target\x86_64-unknown-none\debug\bootimage-OpenRhiza.bin
```

## Current QEMU Layout

The current QEMU flow attaches:

- primary boot image
- secondary FAT driver disk from `rhiza_drivers/`
- serial TCP bridge on `127.0.0.1:4444`
- Intel `e1000`
- xHCI plus USB keyboard and mouse

The FAT driver disk is not just a payload cache now. It is also the seed capability disk used for:

- fixed skill slots `SK000.WAS` through `SK005.WAS`
- local capability cache text files
- boot autorun input

## Current Visible Runtime

A healthy QEMU boot typically goes through:

1. recovery console
2. sandbox display skill load
3. framebuffer validation handoff
4. bootstrap GUI

The common end state for desktop testing is the bootstrap GUI rather than the old bottom-row-only CLI.

## Optional Legacy Host AI Loop

If you want the legacy host-assisted flow:

```bash
python host_brain.py
python host_brain.py --model gemini-2.5-flash
python host_brain.py --model gemini-2.5-pro
```

This path is still useful for historical experiments and protocol validation, but it is no longer the primary OpenRhiza runtime path.

## What To Expect At Boot

A healthy boot should show most of the following:

```text
OpenRhiza Seed (Layer 0) Booting...
Heap Allocator initialized!
Total Usable Memory: ...
Hardware Discovery Complete.
Found N PCI devices:
...
[OS System] All subsystems initialized. Handing over to Async Executor.
[Boot Autorun] Running /api-skill
[Skill Runtime] ...
[Display Runtime] ...
```

You may also see:

- FAT driver-disk lookup logs
- skill loading logs for `SK000.WAS`, `SK001.WAS`, and follow-up stages
- framebuffer / GUI transition logs
- the `1920x1080` bootstrap GUI

## Verification Checklist

### 1. Build

Run:

```bash
cargo build
```

### 2. Rebuild seed skills

Run:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\build_sandbox_skills.ps1
```

Confirm that:

- `SKDSP.WAS`
- `SKGUI.WAS`
- `SKFBUF.WAS`
- `SKCOMP.WAS`
- `SKREG.WAS`
- `SKMUT.WAS`

are updated under `rhiza_drivers/`.

### 3. Boot image

Run:

```bash
cargo bootimage
```

Confirm:

- `target/x86_64-unknown-none/debug/bootimage-OpenRhiza.bin` is regenerated

### 4. QEMU

Run:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu.ps1 target\x86_64-unknown-none\debug\bootimage-OpenRhiza.bin
```

Confirm:

- QEMU window appears
- serial log window appears
- recovery console boots
- GUI handoff completes
- GUI input and pointer remain alive

## Known Current Gaps

- `src/https.rs` still uses raw TCP/HTTP rather than the in-repo TLS client
- ATA write support is still missing
- `skill_gui_compositor_seed_v1` is not yet fully stable in the fixed-slot seed path
- Right Shift is not yet distinct in the current Windows QEMU USB keyboard path
- A small amount of residual GUI flicker can still occur during object-boundary pointer movement

## Troubleshooting

### `rust-lld` or toolchain components missing

```bash
rustup component add llvm-tools-preview
```

### QEMU path is wrong

Update `run_qemu.ps1` and the `run-command` entry in `Cargo.toml` to match your local QEMU installation path.

### Boot image build fails with `llvm-objcopy` permission denied

Stop any running QEMU session and rebuild:

```powershell
Get-Process qemu-system-x86_64 -ErrorAction SilentlyContinue | Stop-Process -Force
cargo bootimage
```

### Serial log window does not appear

Check `run_qemu.ps1` Python resolution and the serial log helper script path.

### GUI input dies after handoff

Treat that as a regression.
The recovery path and GUI path must remain separable, and GUI pointer or hit-test changes must not re-enter the VGA writer lock path.
