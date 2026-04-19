# Persistent Driver Cache Design

This document defines the recommended local persistence model for OpenRhiza after the April 2026 networking and registry milestone.

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
- a read-only ATA PIO path in `src/storage.rs`
- FAT16 payload extraction for bootstrap artifacts

The repository does **not** yet have:

- ATA write support
- a general writable filesystem layer
- a persistent local driver cache
- a boot-time driver manifest loader

This means the design below is the target structure, not the fully active path yet.

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

- keep current read-only ATA PIO path
- add local cache manifest format
- add boot-time loader for local manifests
- no writes yet

### Phase 2

- add minimal storage write path
- create staging directory support
- write generated artifacts and manifests
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

1. mark storage capability honestly in discovery/runtime
2. define the local cache manifest format in code
3. add a read-only loader for local driver manifests
4. keep generated drivers session-local until write support exists

That path matches the repository's current reality and avoids pretending persistence already exists when it does not.
