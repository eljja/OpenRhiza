# skill_fs_fat32_bridge_v1

## Role

Provide stable FAT32 read/write interoperability for OpenRhiza-managed disk images and removable storage.

## Current backend

- WSL `vfat` mount path
- validation support from `mkfs.vfat`
- future bounded userspace candidate: Rust `fatfs`

## Scope

- directory create/remove
- file create/read/write/append/delete
- rename
- persistence verification after remount

## Why it stays out of the core

The current core still uses FAT16 only as a bootstrap/recovery floor.
FAT32 support is broader filesystem logic and should remain an isolated compatibility object until repeatedly validated.

## Validation status

Validated by host-assisted lab tool:

- format/create/mount/write/read/rename/unmount/remount/delete

## Promotion criteria

- repeated clean lab runs
- explicit rollback strategy
- no regression to active recovery path
