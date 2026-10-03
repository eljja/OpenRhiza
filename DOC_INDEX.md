# OpenRhiza Documentation Index

새 개발 기준 문서 8개의 한국어 대응본: [한국어 문서 목차](next/README.ko.md).
영문 원문과 같은 요구사항·단계·판정을 따르며 별도의 계획이 아닙니다.

This file defines which documents are authoritative, which are implementation references, and which are historical notes.

When documents disagree, prefer the highest-priority current document listed here.

## Authoritative Current Documents

These documents define the active direction of OpenRhiza.

| Priority | Document | Role |
| --- | --- | --- |
| 1 | `next/docs/DIRECTION.md` | Independent AI-native OS reset; normative product, core, module, Nexus and safety decisions. |
| 2 | `next/docs/REQUIREMENTS.md` | Preserved user intent, source traceability and new acceptance boundaries. |
| 3 | `next/docs/ROADMAP.md` | Fresh six-stage development sequence; no legacy completion is inherited. |
| 4 | `next/docs/IMPLEMENTATION_GATES.md` | Stage-specific entry decisions, authority boundaries and independent acceptance evidence. |
| 5 | `next/docs/LEGACY_REVIEW.md` | Investigation scope, concrete implementation gaps and reference-only evidence. |
| 6 | `next/README.md` | New workspace boundary, R0 checkpoint and metadata verification. |

The 2026-10-03 reset supersedes the previous incremental rebuild plan. Existing
source is preserved, not migrated into the new runtime. New development has not
passed a kernel bring-up gate. `next/docs/legacy-inventory.json` records the
reviewed project corpus metadata, not a line-by-line security certification.

`next/docs/ASTRA_PLAN_REVIEW.md` preserves the initial Astra findings against
the original document snapshots. `next/docs/ASTRA_REVIEW_DISPOSITION.md` maps
them to current plan changes and records follow-up review. These are review
evidence; incorporation does not pass Stage 1.1 or a runtime gate.

## Legacy Baseline And Design References

The following priority list belonged to the earlier implementation. Use it only
to understand that implementation; it does not override the reset documents.

| Legacy Priority | Document | Role |
| --- | --- | --- |
| 1 | `OPENRHIZA_MODULE_ARCHITECTURE.md` | Minimal core, sandbox modules, AI acquisition, and core-survival contracts. |
| 2 | `OPENRHIZA_REBUILD_PLAN.md` | Independent-OS six-stage sequence and QEMU acceptance gates. |
| 3 | `OS.md` | Runtime rules for the internal OpenRhiza intelligence. |
| 4 | `ROADMAP.md` | Bootstrap baseline and migration backlog; current ordering is in the rebuild plan. |
| 5 | `ARCHITECTURE.md` | Earlier layer model; target boundaries follow the module architecture. |
| 6 | `CORE_BOUNDARY_AUDIT.md` | Source-level boundary tags: permanent core, bootstrap fallback, must move to skill. |
| 7 | `README.md` | Public-facing project overview and current status. |
| 8 | `KNOWN_ISSUES.md` | Active technical risks and unresolved constraints. |
| 9 | `BUILD_AND_TEST.md` | Current build, run, and verification flow. |
| 10 | `VALIDATION_MATRIX.md` | Pass/partial/fail state for important runtime capabilities. |
| 11 | `PUBLIC_ALPHA_CHECKLIST.md` | Minimum work before wider public release. |
| 12 | `Codex.md` | Codex/ChatGPT handoff memory. |
| 13 | `SANDBOX_MIGRATION_STATUS.md` | Current status of moving e1000, xHCI/HID, storage, GUI, filesystems, and autonomy out of core. |
| 14 | `PLATFORM_PROGRESS.md` | x86_64 and ARM64 progress split, remaining work, parity rules, and ARM64 delay rationale. |

## Subsystem Reference Documents

Use these to reconstruct intent and old implementation details. Their ABIs,
runtime ownership and completion claims are not contracts for the new OS.

| Area | Documents |
| --- | --- |
| Display and GUI | `DISPLAY_ABI.md`, `GUI_DEVELOPMENT.md`, `FONT_SKILL.md` |
| Drivers and hot-swap | `DRIVER_HOST_ABI.md`, `HOT_SWAP_RUNTIME.md`, `PERSISTENT_DRIVER_CACHE.md`, `QEMU_DRIVER_SET.md` |
| Input | `SANDBOX_INPUT_DRIVER.md`, `USB_HID_HANDOFF.md` |
| Storage and filesystems | `STORAGE_HOST_ABI.md`, `SKILL_FS_BRIDGE.md`, `STORAGE_IMAGE_HARNESS.md`, `FILESYSTEM_SKILLS.md` |
| Semantic storage | `SEMANTIC_GRAPH_LAYER.md` |
| Sandbox applications | `DOOM_SKILL.md`, `DOOM_MODULE_ACCEPTANCE.md` |
| Autonomy | `AUTONOMY_MODE.md` |
| Voice | `VOICE_INPUT.md` |
| Platform expansion | `PLATFORM_PROGRESS.md`, `PLATFORM_EXPANSION.md`, `CROSS_PLATFORM_BRINGUP.md` |
| Ecosystem/registry/API | `OPENRHIZA_COM_PLAN.md` (OS/Nexus/forge re-audit), `CAPABILITY_REGISTRY.md`, `OPENRHIZA_API_V1.md` (unfrozen draft) |
| Compatibility | `PROGRAM_COMPATIBILITY_GOALS.md` |
| Security | `SECURITY.md` |

## Historical Or Secondary Notes

These files may still contain useful context, but they must not override the current baseline documents:

- `Gemini.md`
- `Gemini_stepbystep#1.md`
- `Gemini_task.md`
- `Gemini_walkthrough.md`
- `Gemini_implementation_plan.md`
- `Gemini_openrhiza_readiness_nic_analysis.md`
- `PROTOCOL.md` when it discusses the older `host_brain.py` flow

Historical documents should either be marked historical in-place or moved under a future `docs/archive/` directory once the active docs are stable.

## Current Snapshot

Reset checkpoint R0: new direction, requirements, legacy review and a fresh
six-stage roadmap are in `next/docs/`. The next task is Stage 1.1: qualify the
boot/toolchain profile and specify the new core/module contracts before bring-up.
The older checkpoints below are historical reference only.

2026-10-03 review checkpoint: use the six-stage rebuild and module architecture
for current completion claims. `OPENRHIZA_COM_PLAN.md` records the broader
OS/Nexus/public-site/forge investigation, security blockers and live/QEMU evidence.
The public site and guest registry queries respond, but isolated execution,
authenticated signed delivery and self-hosted generation are not complete.
The older snapshot below describes bootstrap work, not passed rebuild stages.

As of May 17, 2026:

- `x86_64-qemu-pc` is the reference platform.
- `aarch64-qemu-virt` has a standalone serial recovery ELF, an `input>` recovery CLI surface, virtio-mmio capability handles, sandbox virtio proof gates, production-routing proof paths, and volatile provider restore proof. It is not yet network-backed prompt or GUI parity with x86_64.
- Voice input has a route-aware sandbox chain and a host-assisted bridge, but no native in-guest microphone capture yet.
- The GUI is a bootstrap object runtime with sandbox mutation paths, not a finished desktop.
- Filesystem support has a storage host ABI and image harness path, but broad filesystem read/write is still a skill-validation target.
- The largest core-reduction priorities remain e1000, xHCI/HID, GUI shell policy, filesystem bridge logic, and autonomy workflows.
- `SANDBOX_MIGRATION_STATUS.md` is the current cross-subsystem migration checkpoint for that work.
- `PLATFORM_PROGRESS.md` is the current split summary for x86_64 versus ARM64 progress, remaining work, and cross-architecture CLI/GUI parity rules.

## Maintenance Rule

When a new architecture, runtime path, or capability becomes active, update these files in the same change:

1. `ROADMAP.md`
2. `KNOWN_ISSUES.md`
3. `VALIDATION_MATRIX.md`
4. the subsystem design document
5. `Codex.md` when the change affects future handoff context
