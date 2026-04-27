# workflow_fs_probe_and_validate_v1

## Purpose

Detect a filesystem target, select the correct bridge skill, validate bounded read/write behavior, and emit a structured report before promotion.

## Candidate skill order

1. `skill_fs_fat32_bridge_v1`
2. `skill_fs_ntfs_bridge_v1`
3. `skill_fs_ext4_bridge_v1`

## Validation steps

1. open or create an image target
2. format using the chosen backend
3. mount or open the filesystem surface
4. create a directory
5. create and append a file
6. read back contents
7. rename the file
8. unmount
9. remount
10. verify persistence
11. delete test artifact
12. emit structured success/failure report

## Required behavior

- no mutation of unrelated runtime state
- no dependence on current GUI state
- no dependence on active OpenRhiza session contents
- rollback/cleanup on failure

## Current implementation

Host-assisted lab tool:

- [D:\python\github\OpenRhiza\OpenRhiza\tools\storage_fs_skill_lab.py](D:\python\github\OpenRhiza\OpenRhiza\tools\storage_fs_skill_lab.py)
