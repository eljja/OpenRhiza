# Storage Host ABI

This document defines the bounded storage ABI between the OpenRhiza core and sandboxed filesystem skills.

## 1. Purpose

OpenRhiza should support richer filesystems without growing all filesystem logic into the core.

The core should only provide:

- raw block device discovery
- bounded block reads
- bounded block writes
- flush or sync primitives
- optional metadata such as size and mutability

Everything above that should be implemented in filesystem bridge skills and workflows.

## 2. Core Rule

The host ABI is not a mount implementation.
It is a narrow transport and control surface.

Do not move FAT32, exFAT, NTFS, or ext parsing logic into the core just because the core can read sectors.

## 3. Minimal Object Model

### Block device object

Each exposed image or disk target should behave like an object with:

- stable handle
- read/write policy
- reported block size
- reported block count
- bounded request surface

## 4. Required Operations

Initial ABI operations:

- `list_images() -> count`
- `open_image(index) -> handle`
- `describe_image(handle) -> block_count, writable, fs_hint`
- `read_blocks(handle, lba, count, out)`
- `write_blocks(handle, lba, count, data)`
- `flush_image(handle)`

Current initial in-OS mapping:

- `os_storage_list_images`
- `os_storage_open_image`
- `os_storage_get_block_count`
- `os_storage_get_filesystem_block_count`
- `os_storage_get_scratch_start_lba`
- `os_storage_get_scratch_block_count`
- `os_storage_is_writable`
- `os_storage_get_fs_hint`
- `os_storage_read_blocks`
- `os_storage_write_blocks`
- `os_storage_flush_image`
- `os_storage_set_sandbox_block_stage`
- `os_storage_get_sandbox_block_stage`
- `os_storage_report_sandbox_block_health`
- `os_storage_request_sandbox_block_promotion`

## 5. Safety Rules

- writes must be explicitly bounded
- the core must reject invalid ranges
- image-backed harness devices should be isolated from the active recovery storage floor
- the recovery bootstrap disk must not be silently repurposed as a general test target
- `flush_image` is a best-effort synchronization point in the current QEMU harness; skills must verify writes by readback before promotion and must not assume ATA cache-flush semantics are authoritative until a sandbox-owned block driver replaces the bootstrap PIO path

## 6. Sandbox Block Driver Promotion Gate

Storage is high-risk. A sandbox skill may mark itself as `sandbox-candidate`, but it cannot become the active block driver through the legacy stage setter.

Active promotion requires explicit health evidence through `os_storage_report_sandbox_block_health` followed by `os_storage_request_sandbox_block_promotion`.

Required evidence:

- controller/device was claimed through `DRIVER_HOST_ABI.md`
- PCI config access validated
- PIO or equivalent block transport access validated
- block object opened
- bounded block read validated
- scratch write/read/restore validated
- flush/readback or equivalent sync validation completed
- no IO error state observed
- the sandbox driver owns the controller-backed block IO path, not only the image-backed harness object

Current seed skills deliberately do not satisfy every requirement:

- `skill_piix_ide_sandbox_probe_v1` proves controller PCI/PIO access, secondary-slave scratch read/write/restore, and an OS-to-sandbox exported block-operation selftest.
- `skill_storage_host_probe_v1` proves bounded image block read/write/restore only.

Both are expected to remain `sandbox-candidate` and have active promotion denied until a real sandbox storage driver combines controller ownership, block-object validation, and live request routing.

## 6.1 Sandbox Block Driver Exports

The first controller-backed sandbox block driver exports:

- `storage_block_bytes() -> u32`
- `storage_scratch_ptr() -> wasm_ptr`
- `storage_read_block(lba, wasm_ptr) -> bytes_or_error`
- `storage_write_block(lba, wasm_ptr) -> bytes_or_error`
- `storage_flush() -> ok_or_error`

These exports are intentionally driver-local. The core runtime uses them only for bounded selftests until the storage owner is promoted. A passing selftest proves that the OS can dispatch a block operation into a sandbox driver and restore data safely. It does not by itself mean the driver owns live storage.

## 6.2 Runtime Dispatch Broker

The core now has an `OpenRhizaSeed` storage dispatch broker for loaded sandbox storage drivers:

- `invoke_storage_driver_read_block_into`
- `invoke_storage_driver_write_block_from`
- `invoke_storage_driver_flush`
- `invoke_storage_driver_scratch_selftest`

The scratch selftest uses this broker instead of calling storage exports inline. This is the correct migration shape: every block request that targets a sandbox storage owner should pass through one narrow broker path with explicit module identity, bounded 512-byte buffers, error reporting, and health snapshot updates.

Current limitation:

- Normal `storage_host::read_blocks` and `storage_host::write_blocks` still route to native ATA fallback unless an explicit sandbox-active route is added.
- This is intentional. Those static host ABI calls cannot safely re-enter arbitrary loaded Wasm storage owners until ownership, rollback, and synchronous request routing are proven.

Next required step:

- Add a live storage-owner route that forwards normal block requests through the broker only after `sandbox-active` promotion, lease heartbeat, and rollback recovery are available.

## 7. Intended Initial Use

The first internal use is an image-backed harness disk attached only for validation.

That harness allows sandbox skills to:

- probe filesystem signatures
- verify block IO behavior
- later mount and mutate filesystem structures through bridge logic

The first harness implementation does not expose the active recovery FAT16 disk.
It exposes only an optional separate harness image attached as a secondary slave device.

Controller-level storage migration is handled separately through `DRIVER_HOST_ABI.md`.
`skill_piix_ide_sandbox_probe_v1` currently proves that a sandbox artifact can claim the QEMU PIIX IDE controller and inspect bounded PCI/PIO state while the native bootstrap storage path remains the fallback.

## 8. Current Implementation Notes

- The active bootstrap storage floor remains `secondary master`.
- The optional harness lives on `secondary slave`.
- The core virtualizes the harness so filesystem skills see:
  - filesystem blocks at `lba 0..fs_block_count-1`
  - scratch blocks at `lba fs_block_count..`
- The scratch region is appended outside the source filesystem image boundary.

## 9. Long-Term Role

This ABI is the foundation for:

- filesystem bridge skills
- program compatibility images
- semantic graph refresh against mounted or image-backed filesystems
- bounded import/export workflows
