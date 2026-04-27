# Filesystem Skills

This document defines the OpenRhiza direction for filesystem support beyond the current FAT16 bootstrap path.

It is intentionally separated from the active core runtime.
The current OS runtime should remain stable while these filesystem capabilities are developed, validated, and promoted independently.

## 1. Scope

OpenRhiza should eventually support at least these filesystems as reusable capabilities:

- FAT32
- exFAT
- NTFS
- Linux-native filesystems (starting with ext2/ext3/ext4)

The purpose is:

- read and write external storage safely
- import/export drivers, skills, programs, and logs
- preserve stable interoperability across Windows, Linux, and embedded-style media

## 2. Non-Negotiable Rule

Do not expand the core filesystem logic until the capability has already been validated as an isolated skill or host-assisted runner.

That means:

- the current FAT16 bootstrap path remains the recovery/storage floor
- FAT32, NTFS, and Linux filesystem support should be developed as compatibility skills, bridges, or runners first
- promotion into tighter runtime integration should happen only after repeatable validation

## 3. Current Practical Strategy

For now, OpenRhiza uses a **host-assisted filesystem skill lab** approach.

Why:

- FAT32 read/write is straightforward with mature tooling and libraries
- exFAT is a practical cross-platform removable media target
- NTFS stable read/write is best served today by established implementations such as `ntfs-3g`
- Linux filesystem stable read/write is best served today by established ext family tooling or kernel support

This is still consistent with OpenRhiza's model:

- the core does not absorb all filesystem logic
- a skill/runner/tooling layer mediates the capability
- the capability is validated before it becomes trusted

## 4. Capability Objects

Filesystem support should be modeled as isolated capability objects.

Recommended initial objects:

- `skill_fs_fat32_bridge_v1`
- `skill_fs_exfat_bridge_v1`
- `skill_fs_ntfs_bridge_v1`
- `skill_fs_ext2_bridge_v1`
- `skill_fs_ext3_bridge_v1`
- `skill_fs_ext4_bridge_v1`
- `workflow_fs_probe_and_validate_v1`
- `workflow_fs_import_export_v1`

Each should declare:

- target filesystem family
- read/write scope
- host requirements
- validation steps
- rollback/failure handling

## 5. Implementation Layers

### Layer A: Validation Lab

Used now.

- host-assisted
- image-based
- repeatable smoke tests
- no changes to active OS runtime

### Layer B: Runtime Bridge

Later.

- bounded import/export operations
- explicit request surface
- object-scoped mount or image access
- still outside the minimal core

### Layer C: Optional Promotion

Only after repeated validation.

- tighter integration for narrow high-value paths
- still keep broad filesystem logic out of the core when possible

## 6. Known External Code / Tooling Sources

These are the current preferred foundations:

### FAT32

- Rust `fatfs` crate for userspace read/write logic
- Linux / WSL `mkfs.vfat` and `vfat` mount path for validation

### exFAT

- `exfatprogs` for formatting and repair tooling
- Linux exFAT kernel support or FUSE-style mount path for validation
- historical and reference implementation: [relan/exfat](https://github.com/relan/exfat)

### NTFS

- `ntfs-3g` for stable read/write interoperability
- avoid pretending NTFS write is "solved" by minimal experimental code

### Linux ext family

- Linux kernel ext2/ext3/ext4 support for validation
- `lwext4` and related projects as future bounded bridge candidates

## 7. Validation Requirements

A filesystem bridge should not be considered stable until it can pass repeated image-based tests that include:

- format
- mount or open
- directory create
- file create
- file append
- file readback verification
- rename
- delete
- unmount
- remount and persistence verification

## 8. Current Deliverable

The current deliverable is a host-assisted lab tool that:

- creates FAT32, exFAT, NTFS, and ext2/ext3/ext4 images
- formats them
- mounts them
- performs stable read/write smoke tests
- emits structured validation reports

This allows OpenRhiza to progress toward multi-filesystem support without destabilizing the active GUI, input, or recovery runtime.
