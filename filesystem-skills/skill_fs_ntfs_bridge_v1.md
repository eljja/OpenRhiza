# skill_fs_ntfs_bridge_v1

## Role

Provide stable NTFS read/write interoperability for OpenRhiza-managed disk images and shared Windows-facing media.

## Current backend

- `ntfs-3g` fallback
- `ntfs3` mount path when available

## Scope

- directory create/remove
- file create/read/write/append/delete
- rename
- persistence verification after remount

## Why it stays out of the core

NTFS is a complex compatibility target.
Stable write support should come from proven compatibility layers first, not from rushed kernel growth.

## Validation status

Validated by host-assisted lab tool:

- format/create/mount/write/read/rename/unmount/remount/delete

## Notes

The lab prefers `ntfs3` when available and falls back to `ntfs-3g`.
This keeps the object honest about backend reality while preserving the OpenRhiza skill boundary.
