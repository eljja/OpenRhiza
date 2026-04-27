# skill_fs_bridge Interface

This document defines the common interface for sandboxed filesystem bridge skills.

## 1. Role

A filesystem bridge skill owns filesystem-specific logic while the core owns only raw block transport and recovery boundaries.

Examples:

- `skill_fs_fat32_bridge_v1`
- `skill_fs_exfat_bridge_v1`
- `skill_fs_ntfs_bridge_v1`
- `skill_fs_ext2_bridge_v1`
- `skill_fs_ext3_bridge_v1`
- `skill_fs_ext4_bridge_v1`

## 2. Common Responsibilities

Each filesystem bridge skill should:

- detect whether the image matches its filesystem family
- validate minimal consistency before mutation
- provide bounded read/write operations
- report capability and failure status clearly
- avoid touching unrelated devices or filesystems

## 3. Common Interface

Initial conceptual operations:

- `probe(handle) -> supported / unsupported / uncertain`
- `describe(handle) -> label, fs_family, writable, health_hint`
- `list_dir(handle, path)`
- `read_file(handle, path)`
- `write_file(handle, path, bytes)`
- `rename(handle, from, to)`
- `delete(handle, path)`
- `sync(handle)`
- `validate(handle)`

The first implemented in-OS bridge path is narrower and block-oriented:

- `probe(handle)`
- `describe(handle)`
- `read_blocks(handle, lba, count)`
- `write_blocks(handle, lba, count)` only against validated bounded regions
- `validate_scratch(handle)`

## 4. Initial Practical Scope

The first in-OS implementation does not need the full surface immediately.

The first useful bridge can start with:

- `probe`
- `describe`
- bounded scratch write verification
- structured log/report output

and then grow toward real file operations.

## 4.1 First Concrete Skill

The first internal sandbox skill is:

- `skill_fs_image_probe_v1`

Its purpose is to:

- open the optional image-backed harness
- detect FAT32, exFAT, NTFS, ext2, ext3, or ext4 from raw on-disk signatures
- validate bounded block write/read/restore against the harness scratch region
- produce structured log output without touching the active recovery storage floor

## 5. Promotion Rules

A bridge should only be trusted for broader read/write use after:

- repeated image-backed validation
- persistence verification
- rollback safety checks
- no regression to active GUI/input/recovery runtime

## 6. Relationship To Semantic Graph

The semantic graph layer should not replace filesystem bridge skills.

Instead:

- the filesystem bridge exposes structured file access
- the semantic graph skill indexes meaning on top of that access

## 7. Promotion Path

Bridge progression should be:

1. image-backed probe skill
2. bounded scratch write validation
3. read-only filesystem traversal
4. controlled file mutation
5. promotion to real import/export workflows
