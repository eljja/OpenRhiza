# Filesystem Skill Objects

This directory contains object-scoped capability descriptions for filesystem support that should remain outside the OpenRhiza core until repeatedly validated.

These are not wired into the current OS runtime.
They are design and implementation objects for the next storage compatibility phase.

Current objects:

- `skill_fs_fat32_bridge_v1.md`
- `skill_fs_exfat_bridge_v1.md`
- `skill_fs_ntfs_bridge_v1.md`
- `skill_fs_ext2_bridge_v1.md`
- `skill_fs_ext3_bridge_v1.md`
- `skill_fs_ext4_bridge_v1.md`
- `workflow_fs_probe_and_validate_v1.md`

They are backed today by the host-assisted validation tool:

- [D:\python\github\OpenRhiza\OpenRhiza\tools\storage_fs_skill_lab.py](D:\python\github\OpenRhiza\OpenRhiza\tools\storage_fs_skill_lab.py)

That tool currently validates:

- format
- mount
- create
- append
- readback
- rename
- delete
- unmount
- remount persistence

without changing the active OpenRhiza GUI/runtime behavior.
