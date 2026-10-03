from __future__ import annotations

import argparse
import json
from dataclasses import asdict, dataclass, field
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_LOG = REPO_ROOT / "logs" / "serial.log"
DEFAULT_SCREENSHOT_DIR = REPO_ROOT / "logs"
DEFAULT_REPORT = REPO_ROOT / "logs" / "gui_public_alpha_regression.json"


@dataclass
class Check:
    name: str
    status: str
    evidence: list[str] = field(default_factory=list)
    missing: list[str] = field(default_factory=list)


def contains(path: Path, needles: list[str]) -> Check:
    text = path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""
    missing = [needle for needle in needles if needle not in text]
    return Check(
        name=str(path.relative_to(REPO_ROOT)),
        status="passed" if not missing else "failed",
        evidence=[needle for needle in needles if needle not in missing],
        missing=missing,
    )


def forbidden_contains(path: Path, needles: list[str], name: str) -> Check:
    text = path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""
    present = [needle for needle in needles if needle in text]
    return Check(
        name=name,
        status="passed" if not present else "failed",
        evidence=[f"absent:{needle}" for needle in needles if needle not in present],
        missing=[f"forbidden:{needle}" for needle in present],
    )


def glob_check(root: Path, name: str, patterns: list[str]) -> Check:
    evidence: list[str] = []
    missing: list[str] = []
    for pattern in patterns:
        matches = sorted(root.glob(pattern), key=lambda path: path.stat().st_mtime, reverse=True)
        if matches:
            evidence.append(str(matches[0].relative_to(REPO_ROOT)))
        else:
            missing.append(pattern)
    return Check(name=name, status="passed" if not missing else "failed", evidence=evidence, missing=missing)


def transcript_check(log_path: Path) -> Check:
    if not log_path.exists():
        return Check(
            name="runtime transcript",
            status="failed",
            missing=[str(log_path.relative_to(REPO_ROOT))],
        )
    text = log_path.read_text(encoding="utf-8", errors="replace")
    probes = {
        "long_session": ["Gemini", "response"],
        "scroll": ["/gui-scroll", "scroll"],
        "korean": ["한", "hangul", "Korean"],
        "persistence": ["conversation", "history"],
        "object_local_redraw": ["object-local", "redraw"],
        "dynamic_gui_capabilities": ["/gui-capabilities", "dynamic_handle_range"],
        "dynamic_gui_demo": ["/gui-dynamic-demo", "120..159"],
        "modern_shell_skill": ["skill_gui_modern_shell_v1", "OpenRhiza Workbench"],
        "shell_rollback": ["/gui-reset all", "released shell policy owner"],
    }
    evidence: list[str] = []
    missing: list[str] = []
    for name, needles in probes.items():
        if any(needle in text for needle in needles):
            evidence.append(name)
        else:
            missing.append(name)
    return Check(
        name="runtime transcript",
        status="passed" if not missing else "failed",
        evidence=evidence,
        missing=missing,
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate OpenRhiza GUI public-alpha regression evidence"
    )
    parser.add_argument("--log", type=Path, default=DEFAULT_LOG)
    parser.add_argument("--screenshot-dir", type=Path, default=DEFAULT_SCREENSHOT_DIR)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument(
        "--allow-missing-runtime",
        action="store_true",
        help="write the report but do not fail when runtime transcript markers are missing",
    )
    args = parser.parse_args()

    checks = [
        contains(
            REPO_ROOT / "src" / "display.rs",
            [
                "conversation_scroll_items",
                "scroll_gui_conversation",
                "redraw_gui_interaction_delta",
                "draw_gui_conversation_scrollbar",
                "is_dynamic_gui_handle",
                "append_dynamic_gui_nodes",
                "draw_gui_skill_overlays",
                "gui_capability_report",
            ],
        ),
        forbidden_contains(
            REPO_ROOT / "src" / "display.rs",
            [
                "OpenRhiza Workbench",
                "Skills online",
                "Sandbox first",
                "Evidence trail",
                "Ask OpenRhiza",
                "Flow: prompt",
            ],
            "core shell policy strings",
        ),
        contains(
            REPO_ROOT / "src" / "task" / "keyboard.rs",
            [
                "gui-capabilities",
                "gui-dynamic-demo",
                "set_gui_dynamic_demo",
            ],
        ),
        contains(
            REPO_ROOT / "src" / "prompt_orchestrator.rs",
            [
                "gui_capability_report",
                "handles 120..159",
                '\\"handle\\":120',
            ],
        ),
        contains(
            REPO_ROOT / "sandbox-skills" / "gui_modern_shell_seed" / "src" / "lib.rs",
            [
                "CHIP_SKILLS",
                "SIDEBAR_STATUS_A",
                "os_gui_set_object_bounds(120",
            ],
        ),
        contains(
            REPO_ROOT / "src" / "gui_contract.rs",
            ["object_local_redraw"],
        ),
        contains(
            REPO_ROOT / "src" / "hangul.rs",
            ["compose"],
        ),
        glob_check(
            args.screenshot_dir,
            "screenshot artifacts",
            ["*gui*.png", "*hangul*.png", "*gemini*.png"],
        ),
        contains(
            REPO_ROOT / "tools" / "x86_gui_dynamic_tcp_smoke.ps1",
            [
                "/gui-capabilities",
                "/gui-dynamic-demo",
                "/skill-run skill_gui_modern_shell_v1",
                "/gui-reset all",
                "dynamic_handle_range: 120..159",
            ],
        ),
        transcript_check(args.log),
    ]

    blocking_failures = [
        check.name
        for check in checks
        if check.status != "passed"
        and not (args.allow_missing_runtime and check.name == "runtime transcript")
    ]
    report = {
        "status": "passed" if not blocking_failures else "failed",
        "checks": [asdict(check) for check in checks],
        "blocking_failures": blocking_failures,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(report, indent=2, ensure_ascii=False))
    return 0 if not blocking_failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
