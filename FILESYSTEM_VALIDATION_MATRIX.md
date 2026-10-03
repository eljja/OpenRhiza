# OpenRhiza Filesystem Validation Matrix

This matrix records the current family-level read/write validation result from
`tools/storage_fs_skill_lab.py`. It is host-assisted WSL validation for the
filesystem bridge path, not yet an in-OS promoted `skill_fs_bridge_v1` result.

| Filesystem | Backend | Probe | Read | Write | Restore | Remount | Overall |
| --- | --- | --- | --- | --- | --- | --- | --- |
| FAT32 | WSL vfat | pass | pass | pass | pass | pass | passed |
| exFAT | WSL exfat / exfat-fuse | pass | pass | pass | pass | pass | passed |
| NTFS | WSL ntfs3 / ntfs-3g | pass | pass | pass | pass | pass | passed |
| ext2 | WSL ext2 | pass | pass | pass | pass | pass | passed |
| ext3 | WSL ext3 | pass | pass | pass | pass | pass | passed |
| ext4 | WSL ext4 | pass | pass | pass | pass | pass | passed |

Validation meaning:

- `probe`: mounted filesystem exposes the expected OpenRhiza validation path.
- `read`: original payload can be read back after creation.
- `write`: mutated payload can be written and verified.
- `restore`: original payload can be restored after mutation.
- `remount`: unmount/remount preserves the restored payload plus appended phase marker.

Latest generated reports:

- `logs/filesystem_skill_report.json`
- `logs/filesystem_validation_matrix.md`

Remaining promotion gap: repeat the same matrix through in-OS filesystem bridge
skills over `storage_host` block objects, then route only the bounded filesystem
operation through the active sandbox owner.
