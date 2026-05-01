# Cross-Platform Bring-Up

OpenRhiza must support multiple processors without turning the core into a large traditional OS.

The invariant is:

Core contains only survival boot, recovery I/O, memory/interrupt setup, and sandbox host ABI.
Everything else is a driver, skill, workflow, policy, or evaluation record that can be fetched from OpenRhiza.com or generated and validated in a sandbox.

## Current Platform Matrix

The machine-readable matrix lives at:

```text
platforms/openrhiza-platforms.json
```

Inspect it with:

```powershell
python .\tools\platform_matrix.py
python .\tools\platform_matrix.py --registry-keys
```

Current targets:

- `x86_64-qemu-pc`: active reference target.
- `aarch64-qemu-virt`: first serial recovery ELF builds and smoke-boots under QEMU.
- `android-unlocked-aarch64`: research target after ARM64 QEMU works.
- `riscv64-qemu-virt`: parking-lot target after ARM64 path is stable.

## ARM64 First Milestone

Target:

```text
aarch64-qemu-virt
```

The first ARM64 boot must be deliberately small:

1. CPU entry.
2. Minimal page tables/MMU.
3. Exception vectors.
4. GIC interrupt gate.
5. PL011 UART recovery log/input.
6. Sandbox host ABI stubs.

The first milestone now exists as a standalone survival kernel:

```text
platform-kernels/aarch64-recovery
```

Build it with:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\build_arm64_recovery.ps1
```

Smoke-test it with:

```powershell
python .\tools\smoke_arm64_recovery.py
```

Expected serial output:

```text
OpenRhiza ARM64 recovery core
platform=aarch64-qemu-virt serial=PL011
arm64>
```

Do not add virtio-net, virtio-block, virtio-gpu, GUI, filesystem, or voice logic to the ARM core.
Those must be sandbox capabilities once the survival path exists.

## ARM64 QEMU Runner

The scaffolded runner is:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1
```

Without an ARM64 kernel image it only validates QEMU availability and prints the next command shape.
After `build_arm64_recovery.ps1` runs, it defaults to:

```text
target/aarch64-openrhiza-none/debug/openrhiza-arm64.elf
```

Manual run:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1 .\target\aarch64-openrhiza-none\debug\openrhiza-arm64.elf
```

## Android Direction

Android is not the first ARM target.

Use Android later for:

- an unlocked-device recovery core
- Android app/user-space compatibility experiments
- audio/touch/display bridge skills
- device-specific registry metadata

The phone goal is not "replace every Android device."
The first phone goal is one known unlocked device with a minimal recovery core and sandbox-loaded capabilities.

## Registry Keys

Every platform capability should use explicit match keys:

- `arch:x86_64`
- `arch:aarch64`
- `arch:riscv64`
- `machine:qemu-pc`
- `machine:qemu-aarch64-virt`
- `machine:qemu-riscv64-virt`
- `android-device:<vendor>:<model>:<codename>`
- `dt:<compatible-string>`
- `virtio:mmio`
- `virtio:<device-class>`

Drivers and skills must declare:

- supported architecture
- supported machine or board
- required host ABI version
- whether it is boot-critical, recovery-critical, or optional
- validation status per platform

## Required OS Commands

OpenRhiza exposes:

```text
/platform-status
```

This command reports the active platform plan inside the OS so the LLM and operator can keep architecture expansion aligned with the core-minimal rule.

## Work Queue

1. Keep x86_64 QEMU stable.
2. Split x86-specific boot code out of `src/main.rs` into an x86 platform entry module.
3. Keep `platform-kernels/aarch64-recovery` as the serial survival baseline.
4. Split x86-specific boot code out of the main kernel path.
5. Add ARM64 sandbox host ABI stubs.
6. Add virtio-mmio host ABI handles.
7. Load virtio driver skills from OpenRhiza.com.
8. Add platform evaluation uploads per boot target.
