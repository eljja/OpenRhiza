# OpenRhiza Image-Backed Storage Harness

This document describes the first internal validation path for sandboxed filesystem skills inside OpenRhiza.

## Purpose

OpenRhiza should validate filesystem bridge skills against a bounded block target without expanding filesystem-specific parsing into the core.

The first practical target is an optional raw disk image attached as a separate QEMU disk.

## Layout

The harness disk is built from:

1. a validated source filesystem image
2. an appended scratch region
3. a final metadata sector

The source filesystem stays byte-compatible with the original image.
The scratch region sits outside the filesystem image boundary so block write validation does not corrupt the source filesystem structure.

## Footer Metadata

The final sector stores:

- magic: `ORFSHAR1`
- version
- filesystem family hint
- filesystem block count
- scratch block count

The OpenRhiza core reads only this bounded metadata and exposes a block device object through the storage host ABI.

## Runtime Role

Inside OpenRhiza:

- the core exposes the harness as a raw block object
- a sandbox filesystem probe skill detects the filesystem family
- the same skill validates bounded write/read/restore against the scratch region

This allows real in-OS validation without moving FAT32, exFAT, NTFS, or ext parsing into the core.

## Builder

Use:

```powershell
python .\tools\build_fs_harness.py --source .\.fslab\fat32.img --fs fat32 --output .\fs_harness.img
```

If `fs_harness.img` exists at the repo root, `run_qemu.ps1` attaches it as a separate optional disk.
