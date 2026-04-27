from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
import textwrap
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Iterable


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_LAB_ROOT = REPO_ROOT / ".fslab"
DEFAULT_REPORT = REPO_ROOT / "logs" / "filesystem_skill_report.json"


@dataclass
class ValidationResult:
    filesystem: str
    image_path: str
    mount_path: str
    status: str
    backend: str
    notes: list[str] = field(default_factory=list)
    persisted_readback: str | None = None


class FsSkillLabError(RuntimeError):
    pass


def run(
    args: list[str],
    *,
    cwd: Path | None = None,
    capture: bool = True,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=str(cwd) if cwd else None,
        capture_output=capture,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )


def run_wsl(script: str) -> subprocess.CompletedProcess[str]:
    DEFAULT_LAB_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        newline="\n",
        suffix=".sh",
        delete=False,
        dir=DEFAULT_LAB_ROOT,
    ) as handle:
        handle.write(script)
        script_path = Path(handle.name)
    try:
        wsl_script = to_wsl_path(script_path)
        return run(["wsl", "-u", "root", "--", "bash", wsl_script])
    finally:
        try:
            script_path.unlink(missing_ok=True)
        except OSError:
            pass


def quote(text: Path | str) -> str:
    value = str(text)
    return "'" + value.replace("'", "'\"'\"'") + "'"


def to_wsl_path(path: Path) -> str:
    resolved = path.resolve()
    drive = resolved.drive.rstrip(":").lower()
    tail = resolved.as_posix().split(":/", 1)[1]
    return f"/mnt/{drive}/{tail}"


def ensure_tools() -> None:
    required = {
        "mkfs.vfat": "dosfstools",
        "mkfs.exfat": "exfatprogs",
        "mkntfs": "ntfs-3g",
        "ntfs-3g": "ntfs-3g",
        "mkfs.ext2": "e2fsprogs",
        "mkfs.ext3": "e2fsprogs",
        "mkfs.ext4": "e2fsprogs",
        "mount": "util-linux",
        "umount": "util-linux",
    }
    missing: list[str] = []
    for tool, pkg in required.items():
        result = run_wsl(f"command -v {tool}")
        if result.returncode != 0:
            missing.append(f"{tool} (package: {pkg})")
    if missing:
        raise FsSkillLabError(
            "Missing required WSL tools: " + ", ".join(missing)
        )


def build_fs_script(fs_kind: str, image_path: Path, mount_path: Path) -> tuple[str, str]:
    image = quote(to_wsl_path(image_path))
    mount = quote(to_wsl_path(mount_path))
    created = "/openrhiza/phase1.txt"
    renamed = "/openrhiza/renamed.txt"
    payload = "OpenRhiza filesystem skill lab"
    common_prefix = textwrap.dedent(
        f"""
        set -euo pipefail
        img={image}
        mnt={mount}
        mkdir -p "$mnt"
        loopdev=""
        cleanup() {{
          sync || true
          if mountpoint -q "$mnt"; then
            umount "$mnt" || umount -l "$mnt" || true
          fi
          if [ -n "$loopdev" ]; then
            losetup -d "$loopdev" || true
          fi
        }}
        trap cleanup EXIT
        """
    ).strip()
    if fs_kind == "fat32":
        mkfs = 'truncate -s 64M "$img"\nmkfs.vfat -F 32 "$img" >/dev/null'
        mount_script = 'mount -t vfat -o loop,rw,uid=0,gid=0 "$img" "$mnt"'
        backend = "WSL vfat"
    elif fs_kind == "exfat":
        mkfs = 'truncate -s 128M "$img"\nmkfs.exfat -f "$img" >/dev/null'
        mount_script = textwrap.dedent(
            """
            if mount -t exfat -o loop,rw "$img" "$mnt"; then
              backend_used="WSL exfat"
            elif command -v mount.exfat-fuse >/dev/null 2>&1; then
              loopdev=$(losetup --find --show "$img")
              mount.exfat-fuse "$loopdev" "$mnt"
              backend_used="mount.exfat-fuse"
            else
              backend_used="exfat-unavailable"
              exit 92
            fi
            """
        ).strip()
        backend = "WSL exfat / exfat-fuse"
    elif fs_kind == "ntfs":
        mkfs = 'truncate -s 128M "$img"\nmkntfs -F -q "$img" >/dev/null'
        mount_script = textwrap.dedent(
            """
            if mount -t ntfs3 -o loop,rw "$img" "$mnt"; then
              backend_used="WSL ntfs3"
            else
              ntfs-3g "$img" "$mnt" -o rw,force
              backend_used="ntfs-3g"
            fi
            """
        ).strip()
        backend = "WSL ntfs3 / ntfs-3g"
    elif fs_kind == "ext2":
        mkfs = 'truncate -s 128M "$img"\nmkfs.ext2 -F "$img" >/dev/null 2>&1'
        mount_script = 'mount -t ext2 -o loop,rw "$img" "$mnt"\nbackend_used="WSL ext2"'
        backend = "WSL ext2"
    elif fs_kind == "ext3":
        mkfs = 'truncate -s 128M "$img"\nmkfs.ext3 -F "$img" >/dev/null 2>&1'
        mount_script = 'mount -t ext3 -o loop,rw "$img" "$mnt"\nbackend_used="WSL ext3"'
        backend = "WSL ext3"
    elif fs_kind == "ext4":
        mkfs = 'truncate -s 128M "$img"\nmkfs.ext4 -F "$img" >/dev/null 2>&1'
        mount_script = 'mount -t ext4 -o loop,rw "$img" "$mnt"\nbackend_used="WSL ext4"'
        backend = "WSL ext4"
    else:
        raise ValueError(fs_kind)

    script = "\n".join(
        [
            common_prefix,
            'rm -f "$img"',
            mkfs,
            mount_script,
            'mkdir -p "$mnt/openrhiza"',
            f'printf "%s\\n" "{payload}" > "$mnt{created}"',
            f'printf "%s\\n" "phase2" >> "$mnt{created}"',
            f'test -f "$mnt{created}"',
            f'mv "$mnt{created}" "$mnt{renamed}"',
            f'test -f "$mnt{renamed}"',
            'sync',
            'umount "$mnt"',
            'mountpoint -q "$mnt" && exit 91 || true',
            mount_script,
            f'cat "$mnt{renamed}"',
            f'grep -q "{payload}" "$mnt{renamed}"',
            f'rm "$mnt{renamed}"',
            'sync',
        ]
    )
    return script, backend


def validate_one(fs_kind: str, lab_root: Path) -> ValidationResult:
    lab_root.mkdir(parents=True, exist_ok=True)
    image_path = lab_root / f"{fs_kind}.img"
    mount_path = lab_root / f"{fs_kind}_mnt"
    if mount_path.exists():
        shutil.rmtree(mount_path, ignore_errors=True)
    mount_path.mkdir(parents=True, exist_ok=True)

    script, backend = build_fs_script(fs_kind, image_path, mount_path)
    result = run_wsl(script)
    if result.returncode != 0:
        notes = []
        if (result.stdout or "").strip():
            notes.append("stdout:\n" + result.stdout.strip())
        if (result.stderr or "").strip():
            notes.append("stderr:\n" + result.stderr.strip())
        return ValidationResult(
            filesystem=fs_kind,
            image_path=str(image_path),
            mount_path=str(mount_path),
            status="failed",
            backend=backend,
            notes=notes,
        )

    persisted = None
    output_lines = [line.strip() for line in result.stdout.splitlines() if line.strip()]
    if output_lines:
        persisted = "\n".join(output_lines[-2:]) if len(output_lines) >= 2 else output_lines[-1]
    return ValidationResult(
        filesystem=fs_kind,
        image_path=str(image_path),
        mount_path=str(mount_path),
        status="passed",
        backend=backend,
        persisted_readback=persisted,
        notes=[
            "format/create/mount/write/read/rename/unmount/remount/delete succeeded"
        ],
    )


def write_report(results: Iterable[ValidationResult], report_path: Path) -> None:
    report_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "tool": "storage_fs_skill_lab.py",
        "results": [asdict(result) for result in results],
    }
    report_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="OpenRhiza host-assisted filesystem skill lab"
    )
    parser.add_argument(
        "--filesystems",
        nargs="+",
        default=["fat32", "exfat", "ntfs", "ext2", "ext3", "ext4"],
        choices=["fat32", "exfat", "ntfs", "ext2", "ext3", "ext4"],
        help="filesystems to validate",
    )
    parser.add_argument(
        "--lab-root",
        type=Path,
        default=DEFAULT_LAB_ROOT,
        help="workspace-local root for generated disk images and mounts",
    )
    parser.add_argument(
        "--report",
        type=Path,
        default=DEFAULT_REPORT,
        help="JSON report output path",
    )
    args = parser.parse_args()

    ensure_tools()
    results = [validate_one(fs_kind, args.lab_root) for fs_kind in args.filesystems]
    write_report(results, args.report)

    failed = [result for result in results if result.status != "passed"]
    for result in results:
        print(f"[{result.filesystem}] {result.status} via {result.backend}")
        if result.persisted_readback:
            print(f"  readback: {result.persisted_readback}")
        for note in result.notes:
            print("  " + note.replace("\n", "\n  "))
    if failed:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
