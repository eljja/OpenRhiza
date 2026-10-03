# `skill_fs_image_probe_v1`

## Purpose

This skill is the first in-OS filesystem bridge bootstrap for OpenRhiza.

It does not mount or mutate the active recovery storage floor.
Instead, it targets the optional image-backed filesystem harness exposed through the storage host ABI.

## Responsibilities

- open the optional image-backed harness object
- probe the filesystem family from raw disk signatures
- report FAT32, exFAT, NTFS, ext2, ext3, or ext4 when possible
- validate bounded block write/read/restore against the harness scratch region

## Why It Exists

This skill proves that OpenRhiza can execute filesystem logic inside the OS through a sandbox boundary without absorbing FAT32, NTFS, or ext parsing into the core.

## Current Limits

- no full directory traversal yet
- no file read/write surface yet
- no mount abstraction yet

It is intentionally a narrow first bridge skill, not the final filesystem runtime.
