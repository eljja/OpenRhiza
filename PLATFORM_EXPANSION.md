# Platform Expansion Plan

OpenRhiza should eventually run on more than x86_64 QEMU, but platform expansion must not violate the core rule:

Keep only the minimum survival path in the core. Move platform-specific drivers and higher-level behavior into sandboxed capabilities whenever possible.

## Current Baseline

The current working target is:

- architecture: `x86_64`
- first-class test machine: QEMU PC
- boot path: current bootimage flow
- early devices: VGA/framebuffer, keyboard/mouse bootstrap, e1000, FAT-style driver disk
- capability model: local seed skills plus OpenRhiza.com registry lookup

This is the reference target used to stabilize the core/sandbox boundary before wider hardware expansion.

## Expansion Order

### 1. x86_64 QEMU

This remains the primary development target.

Required before broad public release:

- stable GUI and recovery shell
- stable registry download and local cache
- sandbox driver host ABI for e1000, xHCI/HID, storage, and display-facing devices
- filesystem bridge smoke tests
- voice input path as a sandbox capability

### 2. aarch64 QEMU `virt`

This is the first ARM target.

Use it before real phones or boards because it is deterministic, documented, and easier to automate.

Expected platform core work:

- ARM64 exception vectors
- minimal page tables/MMU bring-up
- GIC interrupt controller support
- PL011 UART recovery log/input path
- virtio-mmio discovery
- virtio-net, virtio-block, virtio-input, and virtio-gpu host ABI exposure

Expected sandbox work:

- ARM-targeted driver artifacts
- virtio driver skills
- ARM platform capability query keys
- ARM-specific validation harness

The core should expose bounded platform handles, not absorb virtio driver policy.

### 3. Raspberry Pi Or Similar Open ARM Board

Use this after `qemu-system-aarch64 -machine virt`.

Reason:

- public documentation is available
- boot chain is more controllable than phones
- hardware is stable enough for repeatable tests

Expected differences:

- board-specific boot files
- device tree parsing
- MMIO peripheral map
- USB/storage/display differences

### 4. Android Phone

Android phones are a later target.

They are possible in principle, but difficult because:

- bootloader unlock may be blocked
- secure boot and verified boot are device-specific
- display, touch, modem, storage, power, and sensors are heavily vendor-specific
- Android boot image and device tree handling are required

The first phone goal should not be "replace Android on every device."
It should be "boot a minimal OpenRhiza recovery core on one known-unlocked device, then load device capabilities through sandbox drivers."

## Test Environments

The canonical machine-readable target matrix is now:

```text
platforms/openrhiza-platforms.json
```

Inspect it with:

```powershell
python .\tools\platform_matrix.py
python .\tools\platform_matrix.py --registry-keys
```

OpenRhiza also exposes `/platform-status` inside the OS so the local LLM prompt path can reason about platform expansion without reading host files.

### x86_64

- `qemu-system-x86_64`
- VMware later, using a minimal packaged boot disk plus seed/cache disk

### ARM64

Recommended first command shape:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1
```

Without an ARM64 kernel image, this validates that the host has `qemu-system-aarch64` available and prints the next command shape.
After an ARM64 serial recovery ELF exists, pass it as the first argument.

Target specs have been added under:

```text
targets/aarch64-openrhiza-none.json
targets/riscv64-openrhiza-none.json
```

These are scaffolds for bring-up work, not proof that the full current kernel compiles for those architectures.
The current kernel still contains x86_64-specific boot and device code that must be split behind platform entry modules.

### Android

Android Emulator is QEMU-derived, but it is mainly useful for Android app/system-image testing.
It is not the best first target for booting OpenRhiza itself.

Use Android Emulator later for compatibility-layer and user-space experiments.
Use QEMU `virt` first for OS-core ARM bring-up.

## Capability Registry Requirements

Platform expansion needs registry metadata that separates architecture, platform, and device.

Recommended match keys:

- `arch:x86_64`
- `arch:aarch64`
- `machine:qemu-pc`
- `machine:qemu-aarch64-virt`
- `board:raspberry-pi-4`
- `android-device:<vendor>:<model>:<codename>`
- `pci:<vendor>:<device>`
- `usb:<vid>:<pid>`
- `dt:<compatible-string>`
- `virtio:<device-class>`

Every driver/skill should declare:

- supported architecture
- supported machine or board
- required host ABI version
- whether it is boot-critical, recovery-critical, or optional
- validation status per platform

## Core Boundary Rule For New Platforms

Adding a platform may require new core code for:

- CPU entry
- exception handling
- minimal memory management
- interrupt controller bootstrap
- recovery UART/display/input
- sandbox host ABI surface
- rollback gates

Adding a platform must not become an excuse to put full device drivers, GUI logic, filesystem logic, speech recognition, or policy engines into the core.

Those belong in sandbox capabilities whenever the survival path permits it.

## Milestones

1. Keep x86_64 QEMU stable.
2. Add ARM64 build target skeleton.
3. Boot to serial text on `qemu-system-aarch64 -machine virt`.
4. Add ARM64 sandbox host ABI compatibility.
5. Load a registry-fetched ARM64/virtio skill.
6. Bring up display/input through sandbox skills.
7. Move to one real ARM board.
8. Only then evaluate Android phone targets.
