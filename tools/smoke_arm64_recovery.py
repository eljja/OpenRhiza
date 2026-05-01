from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ELF = REPO_ROOT / "target" / "aarch64-openrhiza-none" / "debug" / "openrhiza-arm64.elf"
QEMU_CANDIDATES = [
    Path("C:/Program Files/qemu/qemu-system-aarch64.exe"),
    Path("qemu-system-aarch64.exe"),
]


def resolve_qemu() -> str:
    for candidate in QEMU_CANDIDATES:
        if candidate.exists():
            return str(candidate)
    return "qemu-system-aarch64"


def main() -> int:
    parser = argparse.ArgumentParser(description="Smoke-test the OpenRhiza ARM64 serial recovery ELF in QEMU.")
    parser.add_argument("--elf", type=Path, default=DEFAULT_ELF)
    parser.add_argument("--timeout", type=float, default=4.0)
    args = parser.parse_args()

    elf = args.elf.resolve()
    if not elf.exists():
        raise SystemExit(f"ARM64 recovery ELF not found: {elf}")

    command = [
        resolve_qemu(),
        "-machine",
        "virt",
        "-cpu",
        "cortex-a72",
        "-m",
        "1024",
        "-nographic",
        "-no-reboot",
        "-kernel",
        str(elf),
    ]

    try:
        completed = subprocess.run(
            command,
            cwd=REPO_ROOT,
            input="/status\n",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            errors="replace",
            timeout=args.timeout,
            check=False,
        )
        output = completed.stdout or ""
    except subprocess.TimeoutExpired as exc:
        output = exc.output or ""

    print(output)
    if "OpenRhiza ARM64 recovery core" not in output or "arm64>" not in output:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
