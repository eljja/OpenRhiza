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

## 5. Safety Rules

- writes must be explicitly bounded
- the core must reject invalid ranges
- image-backed harness devices should be isolated from the active recovery storage floor
- the recovery bootstrap disk must not be silently repurposed as a general test target

## 6. Intended Initial Use

The first internal use is an image-backed harness disk attached only for validation.

That harness allows sandbox skills to:

- probe filesystem signatures
- verify block IO behavior
- later mount and mutate filesystem structures through bridge logic

The first harness implementation does not expose the active recovery FAT16 disk.
It exposes only an optional separate harness image attached as a secondary slave device.

## 7. Current Implementation Notes

- The active bootstrap storage floor remains `secondary master`.
- The optional harness lives on `secondary slave`.
- The core virtualizes the harness so filesystem skills see:
  - filesystem blocks at `lba 0..fs_block_count-1`
  - scratch blocks at `lba fs_block_count..`
- The scratch region is appended outside the source filesystem image boundary.

## 8. Long-Term Role

This ABI is the foundation for:

- filesystem bridge skills
- program compatibility images
- semantic graph refresh against mounted or image-backed filesystems
- bounded import/export workflows
