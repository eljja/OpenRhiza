# Platform Kernels

This directory contains minimal survival kernels for non-reference platforms.

These are not alternate full OS forks.
They exist to prove architecture entry, recovery I/O, and the smallest possible sandbox-host substrate before richer capabilities are loaded from OpenRhiza.com.

## Current Kernels

### `aarch64-recovery`

Purpose:

- boot under `qemu-system-aarch64 -machine virt`
- initialize a stack
- write and read PL011 serial
- provide `/status`, `/platform-status`, and `/help`
- keep all virtio, GUI, filesystem, voice, and policy behavior out of core

Build:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\build_arm64_recovery.ps1
```

Smoke test:

```powershell
python .\tools\smoke_arm64_recovery.py
```

Manual run:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1
```
