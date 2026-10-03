# OpenRhiza Build, Run, and Verification Guide

This document describes the current build and test flow for the `main` branch as of May 2, 2026.

It reflects the code that is actually active now:

- recovery console bootstrap
- framebuffer validation handoff
- `1920x1080` bootstrap GUI
- sandbox display skills and GUI mutation path
- native `e1000` and USB input bring-up
- Nexus capability lookup plus signature verification
- in-repo TLS path for OpenRhiza/Gemini API calls
- autonomy mode bootstrap commands
- scheduler dropped-wake recovery and bounded Wasm polling
- ARM64 `aarch64-qemu-virt` serial recovery build and smoke-test path

Authoritative note:

- For document priority, prefer [DOC_INDEX.md](D:/python/github/OpenRhiza/OpenRhiza/DOC_INDEX.md)
- For system rules, prefer [OS.md](D:/python/github/OpenRhiza/OpenRhiza/OS.md)
- For display/core boundaries, prefer [DISPLAY_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/DISPLAY_ABI.md)
- For GUI direction, prefer [GUI_DEVELOPMENT.md](D:/python/github/OpenRhiza/OpenRhiza/GUI_DEVELOPMENT.md)
- For release readiness, prefer [PUBLIC_ALPHA_CHECKLIST.md](D:/python/github/OpenRhiza/OpenRhiza/PUBLIC_ALPHA_CHECKLIST.md)
- For current validation status, prefer [VALIDATION_MATRIX.md](D:/python/github/OpenRhiza/OpenRhiza/VALIDATION_MATRIX.md)
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

Optional filesystem harness build:

```powershell
python .\tools\build_fs_harness.py --source .\.fslab\ext4.img --fs ext4 --output .\fs_harness.img
```

## Current QEMU Layout

The current QEMU flow attaches:

- primary boot image
- secondary FAT driver disk from `rhiza_drivers/`
- serial TCP bridge on `127.0.0.1:4444`
- Intel `e1000`
- xHCI plus USB keyboard and mouse

The FAT driver disk is not just a payload cache now. It is also the seed capability disk used for:

- fixed skill slots:
  `SK000.WAS` display console, `SK001.WAS` GUI session, `SK002.WAS` framebuffer mode,
  `SK003.WAS` GUI compositor seed, `SK004.WAS` registry lookup,
  `SK005.WAS` GUI scene mutator, `SK006.WAS` filesystem image probe,
  `SK007.WAS` modern GUI shell, `SK008.WAS` QEMU driver pack,
  `SK009.WAS` voice bridge, and `SK010.WAS` e1000 sandbox probe
- sandbox input driver artifacts:
  `KEYBDRV.WAS` keyboard HID parser and `MOUSEDRV.WAS` mouse HID parser
- local capability cache text files
- boot autorun input

If `fs_harness.img` exists at the repo root, QEMU also attaches it as an optional secondary-slave raw harness disk for sandbox filesystem testing.

Inside OpenRhiza, the current developer-facing harness commands are:

- `/fs-harness-status`
- `/fs-harness-probe`
- `/fs-bridge-status`

Useful runtime inspection commands:

- `/display-status`
- `/gui-capabilities`
- `/gui-scene`
- `/gui-mutations`
- `/gui-dynamic-demo`
- `/scheduler-status`
- `/smp-status`
- `/platform-status`
- `/wasm-status`
- `/net-driver-status`
- `/input-routing-status`
- `/fs-bridge-status`
- `/semantic-status`
- `/registry-context`
- `/autonomy-status`
- `/voice-status`
- `/voice-route hybrid`
- `/voice-test`
- `/skill-load skill_gui_modern_shell_v1`
- `/skill-run skill_gui_modern_shell_v1`
- `/skill-load skill_e1000_sandbox_probe_v1`
- `/skill-run skill_e1000_sandbox_probe_v1`
- `/skill-load skill_doom_port_v1`
- `/skill-run skill_doom_port_v1`
- `/skill-invoke skill_doom_port_v1 tick_skill`

`skill_doom_port_v1` runs a sandbox-owned Doom runtime loop. After
`/skill-run`, its `poll_net` export advances frames automatically; `tick_skill`
forces another frame for smoke-test evidence.

Launch Doom directly:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1
```

Launch Doom with a legal IWAD or FreeDoom IWAD:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1 -WadPath C:\path\to\FREEDOOM.WAD
```

Download FreeDoom and launch the sandbox runtime:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1 -DownloadFreedoom
```

Download verified Doom shareware data and launch the sandbox runtime:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1 -DownloadDoomShareware
```

Headless Doom runtime validation:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1 -Headless -ExitAfterStart -TimeoutSeconds 120 -TickCount 6
```

Automated GUI dynamic-object smoke:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\x86_gui_dynamic_tcp_smoke.ps1 -KeepLog
```

This boots the x86 image headlessly, injects `/gui-capabilities`, `/gui-dynamic-demo`, `/gui-scene`, the modern shell skill commands, and the Doom sandbox runtime commands through QEMU HMP keyboard events, waits for the Workbench scene, then injects `/gui-reset all` and validates that the sandbox shell policy owner is released back to `none` and the scene returns to neutral fallback object labels.

Plain input without `/` routes to Gemini. Slash-prefixed input is the recovery and inspection surface.

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
- skill loading logs for `SK000.WAS` through `SK010.WAS` and follow-up stages
- `SK010.WAS` logs for the e1000 sandbox host-ABI probe when the development autorun is enabled
- framebuffer / GUI transition logs
- the `1920x1080` bootstrap GUI

## Verification Checklist

For current pass/partial/design status by subsystem, see [VALIDATION_MATRIX.md](D:/python/github/OpenRhiza/OpenRhiza/VALIDATION_MATRIX.md).

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
- `SKFSP.WAS`

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
- plain prompts reach Gemini when a build-time Gemini key is available
- `/autonomy-status` reports default `off` mode
- `/voice on`, `/voice-route hybrid`, and `/voice-test` queue the voice capture/router/audio bridge chain without breaking keyboard or GUI input
- `/scheduler-status` reports wake metrics without increasing wake drops during normal idle use

Optional host-assisted voice import:

```powershell
python .\tools\voice_prompt_bridge.py --audio .\sample.wav --route hybrid --inject
```

### 5. Platform matrix

Run:

```powershell
python .\tools\platform_matrix.py
pwsh.exe -ExecutionPolicy Bypass -File .\build_arm64_recovery.ps1
python .\tools\smoke_arm64_recovery.py
python .\tools\smoke_arm64_recovery.py --cli-baseline
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\arm64_cli_tcp_smoke.ps1
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1
```

Confirm:

- `x86_64-qemu-pc` remains the reference target
- `aarch64-qemu-virt` has a buildable ARM64 serial recovery ELF
- the ARM64 smoke test prints `OpenRhiza ARM64 recovery core` and `input>`
- the ARM64 CLI baseline reports grouped `/help`, registry/skill/driver/display/input status, and the natural-language prompt stub
- ARM64 recovery denies GUI, virtio payload, production routing, and LLM policy commands in core
- the ARM64 runner uses the generated ELF without mutating the current x86 boot path

If `python` resolves to the Microsoft Store shim instead of a real Python interpreter, the ARM64 smoke scripts cannot run. The minimal fallback is:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\build_arm64_recovery.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\arm64_cli_tcp_smoke.ps1 -KeepLog
```

The TCP harness intentionally starts QEMU with `-serial tcp:127.0.0.1:<port>,server` rather than `server,nowait`. In `nowait` mode, QEMU can boot and discard early serial output before the test client attaches, causing false empty logs. The fallback below verifies boot-to-prompt only and should be used only when TCP serial itself is unavailable:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\build_arm64_recovery.ps1
$qemu='C:\Program Files\qemu\qemu-system-aarch64.exe'
$elf='D:\python\github\OpenRhiza\OpenRhiza\target\aarch64-openrhiza-none\debug\openrhiza-arm64.elf'
$log='D:\python\github\OpenRhiza\OpenRhiza\target\arm64-cli-boot.log'
Remove-Item -LiteralPath $log -ErrorAction SilentlyContinue
$p=Start-Process -FilePath $qemu -ArgumentList @('-M','virt','-cpu','cortex-a57','-m','256M','-kernel',$elf,'-display','none','-serial',"file:$log",'-monitor','none') -PassThru
Start-Sleep -Seconds 2
if (!$p.HasExited) { Stop-Process -Id $p.Id -Force }
Get-Content -LiteralPath $log -Raw
```

The serial-file fallback verifies boot-to-prompt only. It is not a replacement for the TCP CLI smoke, which verifies actual command input and output.

ARM64 GUI, virtio payload, and production-routing experiments are no longer part of the default recovery core. Reintroduce them only as sandbox skills or explicit external proof harnesses; the recovery ELF should keep only survival console and bounded identity probes.

## Known Current Gaps

- dedicated Nexus fetch still needs full unification with the generic TLS/API response path
- filesystem family read/write support is still moving behind sandbox filesystem skills
- `skill_gui_compositor_seed_v1` still needs fixed-slot seed-path regression testing
- Right Shift is not yet distinct in the current Windows QEMU USB keyboard path
- long-running GUI conversation history and scroll behavior need more regression testing
- autonomy council needs stale-cycle timeout recovery before it should be considered robust

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
