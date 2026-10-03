# skill_fs_ext4_bridge_v1

## Role

Provide stable Linux-native filesystem read/write interoperability for OpenRhiza, starting with ext4.

## Current backend

- WSL / Linux kernel ext4 mount path

## Future backend candidates

- `lwext4`
- SharpExt4-style wrapper approaches
- other bounded userspace ext bridge layers

## Scope

- directory create/remove
- file create/read/write/append/delete
- rename
- persistence verification after remount

## Why it stays out of the core

Linux filesystem support is broader than the current bootstrap floor and should remain a separate capability object while compatibility is proven.

## Validation status

Validated by host-assisted lab tool:

- format/create/mount/write/read/rename/unmount/remount/delete

## Promotion criteria

- preserve recovery path isolation
- no effect on GUI/input/runtime stability
- repeatable persistence validation
