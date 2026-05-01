from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MATRIX = REPO_ROOT / "platforms" / "openrhiza-platforms.json"


def run_capture(command: list[str]) -> tuple[int, str]:
    try:
        completed = subprocess.run(
            command,
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=30,
            check=False,
        )
        return completed.returncode, completed.stdout.strip()
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 1, str(exc)


def rust_target_installed(target: str) -> bool:
    if target.endswith(".json"):
        return (REPO_ROOT / target).exists()
    code, output = run_capture(["rustup", "target", "list", "--installed"])
    if code != 0:
        return False
    return target in output.splitlines()


def qemu_available(binary: str) -> bool:
    if binary == "not-direct":
        return False
    if shutil.which(binary) is not None:
        return True
    if sys.platform.startswith("win"):
        return (Path("C:/Program Files/qemu") / f"{binary}.exe").exists()
    return False


def load_matrix(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def print_table(matrix: dict) -> int:
    targets = matrix.get("targets", [])
    print("OpenRhiza platform matrix")
    print("Rule:", matrix.get("core_rule", ""))
    print()
    print(f"{'target':28} {'status':12} {'rust':8} {'qemu':8} boot_goal")
    print("-" * 100)
    missing = 0
    for target in targets:
        rust_ok = rust_target_installed(target["rust_target"])
        qemu_ok = qemu_available(target["qemu_binary"])
        if target["status"] in {"reference", "scaffold"} and not rust_ok:
            missing += 1
        print(
            f"{target['id']:28} {target['status']:12} "
            f"{'ok' if rust_ok else 'missing':8} "
            f"{'ok' if qemu_ok else 'missing':8} "
            f"{target['boot_goal']}"
        )
    return missing


def emit_registry_keys(matrix: dict) -> None:
    for target in matrix.get("targets", []):
        print(f"[{target['id']}]")
        for key in target.get("registry_keys", []):
            print(key)
        print()


def main() -> int:
    parser = argparse.ArgumentParser(description="Inspect OpenRhiza's cross-platform bring-up matrix.")
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--registry-keys", action="store_true", help="Print registry match keys per platform.")
    parser.add_argument("--strict", action="store_true", help="Return non-zero when a reference/scaffold Rust target is missing.")
    args = parser.parse_args()

    matrix = load_matrix(args.matrix)
    if args.registry_keys:
        emit_registry_keys(matrix)
        return 0

    missing = print_table(matrix)
    if args.strict and missing:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
