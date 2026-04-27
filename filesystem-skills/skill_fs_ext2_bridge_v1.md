# skill_fs_ext2_bridge_v1

## Role

Provide ext2 read/write interoperability as part of the Linux-native filesystem family support path.

## Current backend

- WSL / Linux kernel ext2 mount path

## Scope

- directory create/remove
- file create/read/write/append/delete
- rename
- persistence verification after remount

## Validation status

Validated by host-assisted lab tool:

- format/create/mount/write/read/rename/unmount/remount/delete
