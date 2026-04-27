# skill_fs_exfat_bridge_v1

## Role

Provide stable exFAT read/write interoperability for OpenRhiza-managed removable media and large cross-platform storage images.

## Current backend

- WSL/Linux exFAT mount path
- `exfatprogs` formatting utilities
- reference implementation: [relan/exfat](https://github.com/relan/exfat)

## Scope

- directory create/remove
- file create/read/write/append/delete
- rename
- persistence verification after remount

## Why it stays out of the core

exFAT is valuable for interoperability but still belongs in a compatibility layer first.
The recovery floor should not grow to absorb broad exFAT logic before repeated validation.

## Validation status

Validated by host-assisted lab tool:

- format/create/mount/write/read/rename/unmount/remount/delete
