# Persistent Driver Cache Design

This document defines the recommended local persistence model for OpenRhiza after the May 2026 registry, sandbox, and local-cache milestones.

The goal is simple:

- first boot may discover, generate, and validate drivers
- later boots should prefer already validated local drivers
- remote lookup and regeneration should be fallback paths, not the default every time
- runtime activation should happen without reboot whenever the changed component is outside the core

## Current Repository Reality

At the time of writing, the repository has:

- native PCI discovery
- native xHCI keyboard input
- native `e1000` networking
- OpenRhiza.com registry access
- Gemini direct access
- ATA PIO sector read and write support in `src/storage.rs`
- write verification and ATA cache flush after local updates
- FAT16 payload extraction for bootstrap artifacts
- fixed-size FAT16 file overwrite support for local skill/driver cache slots
- runtime driver map inspection and promotion commands

The repository does **not** yet have:

- a general writable filesystem layer
- general file creation/allocation in FAT16
- FAT32/exFAT/NTFS/ext read-write implementations inside OpenRhiza
- a full manifest directory tree with dynamic allocation
- a production boot-time manifest resolver

This means the design below is the target structure.
The active path is still a constrained bootstrap cache: preallocated FAT16 files can be updated safely, but OpenRhiza does not yet own a general-purpose filesystem stack.

## Design Goal

OpenRhiza should use this boot order for driver acquisition:

1. Load validated local driver cache.
2. If missing or unsuitable, query OpenRhiza.com.
3. If still missing or unsuitable, generate a new driver with the LLM.
4. Validate it.
5. Activate it live when safe.
6. Persist it locally.
7. Upload metadata and evaluation back to OpenRhiza.com.

## Stability Principles

- Prefer local validated reuse over regeneration.
- Prefer read-only support before write support.
- Prefer a simple recoverable storage layout over a feature-rich one.
- Prefer immutable driver artifacts plus small metadata files.
- Prefer rollback over in-place mutation of the active driver.
- Prefer runtime hot-swap over reboot for non-core changes.

## Recommended Disk Layout

For early OpenRhiza persistence, use two partitions:

1. `BOOT`
2. `RHIZA_DATA`

Recommended shape:

- `BOOT`
  - small FAT32 partition
  - stores bootloader/kernel assets and recovery tools
- `RHIZA_DATA`
  - FAT32 at first
  - stores driver cache, manifests, evaluations, and generated artifacts

Why FAT32 first:

- simple
- widely inspectable
- easy to recover from outside OpenRhiza
- easier to validate than a more complex filesystem in this phase

Longer term, OpenRhiza may move to:

- a custom append-friendly metadata store
- or a safer native filesystem once the storage stack is proven stable

But the first persistent cache should optimize for recoverability, not sophistication.

## Directory Layout

Recommended initial layout inside `RHIZA_DATA`:

```text
/rhiza/
  /drivers/
    /pci:8086:100e/
      /drv_e1000_native_v1/
        artifact.wasm
        manifest.json
        evaluation.log
    /pci:class:0600/
      /drv_pci_hostbridge_qemu_v1/
        artifact.wasm
        manifest.json
        evaluation.log
  /active/
    drivers.json
  /staging/
  /logs/
  /recovery/
```

## Manifest Model

Each driver should have a small manifest.

Recommended fields:

- `driver_id`
- `match_key`
- `version`
- `artifact_type`
- `artifact_hash`
- `source_type`
- `model`
- `status`
- `validated`
- `last_result`
- `created_at`
- `updated_at`
- `rollback_target`

Suggested meanings:

- `status`
  - `generated`
  - `testing`
  - `validated`
  - `broken`
  - `deprecated`
- `last_result`
  - `pass`
  - `fail`
  - `partial`

## Active Driver Map

The file `/rhiza/active/drivers.json` should map hardware match keys to the currently preferred local driver.

Example:

```json
{
  "pci:8086:100e": "drv_e1000_native_v1",
  "pci:class:0600": "drv_pci_hostbridge_qemu_v1"
}
```

This file should only reference drivers whose manifests say:

- `validated: true`
- `status: "validated"`

OpenRhiza should conceptually keep two maps:

- live active runtime binding
- persisted preferred binding for later boots

They may be the same after a stable promotion, but they should not be treated as the same decision.

The live runtime binding should support:

- inspection at runtime
- manual activation without reboot
- rollback to the previous candidate without reboot

## Boot-Time Driver Selection

At boot:

1. Discover hardware.
2. Compute stable local match keys.
3. Check `/rhiza/active/drivers.json`.
4. If a validated local driver exists, load it first.
5. If not, query OpenRhiza.com.
6. If a verified remote driver exists, fetch and stage it.
7. If not, generate a new candidate.

Do not immediately overwrite the active mapping during staging.
Only promote after validation succeeds.

At runtime, OpenRhiza should also be able to switch the active binding without reboot when the changed component is outside the core.

## Validation Policy

Before a new driver becomes active:

1. Sandbox smoke test.
2. Minimal hardware init test.
3. No keyboard/display/network/storage regression.
4. No panic or hang during the short-run validation window.
5. Basic functional score recorded.

Only after that should the driver:

- become a valid live runtime candidate
- become locally trusted
- update `/rhiza/active/drivers.json`
- upload its metadata and evaluation to OpenRhiza.com

## Storage Driver Risk Policy

Storage drivers are higher risk than many other drivers.

Therefore:

- storage driver development must begin with read-only support
- write support must be staged later
- write support must not be trusted until timeout handling, retry behavior, and rollback are proven
- generated storage drivers should receive stricter validation than ordinary drivers

OpenRhiza should prefer this order:

1. read-only storage access
2. directory listing
3. artifact load
4. manifest read
5. staging write
6. active-map update

That keeps persistence work incremental and reduces corruption risk.

## Write Strategy

Avoid in-place mutation of validated artifacts.

Preferred early strategy:

1. Write new artifact to `/rhiza/staging/...`
2. Validate it
3. Write manifest
4. Update active map only after success
5. Keep previous validated artifact available as rollback target

This is safer than overwriting an active driver directly.

For runtime behavior, prefer:

1. stage candidate
2. validate in sandbox
3. switch live binding
4. monitor
5. persist preferred binding
6. fall back to rollback target if needed

## Recommended Implementation Order

### Phase 1

- keep current ATA PIO read/write path small and auditable
- preserve sector verification and cache flush after writes
- keep using fixed-size cache files for early skill/driver payloads
- expose active driver map inspection from the guest

### Phase 2

- add a local cache manifest format
- add boot-time loader for local manifests
- create staging file support without dynamic allocation first
- keep rollback copies

### Phase 3

- update active driver map after validation
- persist evaluation summaries locally
- sync with OpenRhiza.com when network is available

### Phase 4

- add software package cache using the same pattern
- unify driver and program persistence policy

## Practical Next Step For This Repository

The next concrete step should be:

1. keep storage capability reporting honest: fixed-slot FAT16 writes are available, general filesystem writes are not
2. define the local cache manifest format in code
3. add a loader for local driver and skill manifests
4. add image-backed filesystem tests that run through the OpenRhiza storage host ABI, not only host-side scripts
5. keep generated drivers staged until validation and rollback metadata are written

That path matches the repository's current reality and avoids pretending a complete persistence stack exists before the sandbox filesystem bridge is proven.
