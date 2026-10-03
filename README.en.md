# OpenRhiza

**An independent AI-native operating system built around a small core and isolated modules.**

[한국어](README.md) · [English documents](next/README.md) · [한국어 문서](next/README.ko.md) · [Roadmap](next/docs/ROADMAP.md)

## Status

OpenRhiza is restarting its implementation, preserving the earlier prototype as reference material.
**R0 direction, requirements, legacy investigation, and Astra plan re-review are complete.**
**The new kernel does not boot yet.** This branch is not an installable OS or finished product.

Next is **Stage 1.1, gate K**: select and review the boot profile, trust boundary, memory ownership,
minimum recovery path, and independent test harness before kernel implementation. Plan acceptance
does not mean an implementation gate has passed.

## What We Are Building

- An independent OS, not a Linux distribution or an agent layered on another OS.
- A small core for CPU, memory, authority, preemption, and bounded IPC; isolated drivers, storage, GUI, apps, and AI.
- A command-first session, an optional multiwindow desktop, background work, and MDI.
- Direct local execution of exact commands; replaceable LLM planning for other intents through the same authorized action path.
- Keyboard, mouse, and Korean/English input first; opt-in voice afterward.
- Modules reused from OpenRhiza.com/Nexus or produced with AI, then independently built, tested, and admitted.
- A downloadable, actually playable Doom/FreeDoom module with persistence and offline relaunch as an integration test.

```text
User command / optional voice
              |
Local actions + LLM plans -> deterministic policy and approval
              |
Isolated modules: apps, desktop, storage, drivers, models
              |
Rhiza Core: protection, resource ownership, preemption, bounded IPC
```

AI does not hot-patch the core or grant its own authority. Signatures establish provenance and
authorization, not correctness. Installed functionality, boot, and recovery must tolerate unavailable
models and networks. We do not promise absolute crash immunity: survival without reboot, recovery,
and reclamation must be demonstrated within a declared fault model.

## Sequence

| Stage | Goal | Status |
| --- | --- | --- |
| R0 | Direction, requirements, legacy investigation, plan review | Complete; not runtime evidence |
| 1 | New boot and reproducible foundation | Stage 1.1 K contract pending |
| 2 | Protected native execution, recovery, Wasm, separate SMP gate | Not implemented |
| 3 | Isolated devices, storage, network, safe module delivery | Not implemented |
| 4 | Real multitasking desktop and native SDK | Not implemented |
| 5 | AI actions, voice, knowledge layer, playable game | Not implemented |
| 6 | Qualified updates, recovery, long-running tests, public system | Not implemented |

The [roadmap](next/docs/ROADMAP.md) defines ordering and exceptions; the
[implementation gates](next/docs/IMPLEMENTATION_GATES.md) define entry and acceptance evidence.
Qualified single-vCPU operation is distinct from unqualified SMP. Old completion counts and QEMU
results do not carry over to the new implementation.

## Documents

| Document | 한국어 | English |
| --- | --- | --- |
| Index and current baseline | [읽기](next/README.ko.md) | [Read](next/README.md) |
| Product and architecture | [읽기](next/docs/DIRECTION.ko.md) | [Read](next/docs/DIRECTION.md) |
| Requirements R01-R25 | [읽기](next/docs/REQUIREMENTS.ko.md) | [Read](next/docs/REQUIREMENTS.md) |
| Legacy investigation | [읽기](next/docs/LEGACY_REVIEW.ko.md) | [Read](next/docs/LEGACY_REVIEW.md) |
| Six-stage roadmap | [읽기](next/docs/ROADMAP.ko.md) | [Read](next/docs/ROADMAP.md) |
| Entry and acceptance gates | [읽기](next/docs/IMPLEMENTATION_GATES.ko.md) | [Read](next/docs/IMPLEMENTATION_GATES.md) |
| Astra initial and follow-up review | [읽기](next/docs/ASTRA_PLAN_REVIEW.ko.md) | [Read](next/docs/ASTRA_PLAN_REVIEW.md) |
| Review disposition | [읽기](next/docs/ASTRA_REVIEW_DISPOSITION.ko.md) | [Read](next/docs/ASTRA_REVIEW_DISPOSITION.md) |

## Repository and History

- **New development:** `codex/openrhiza-next`, containing current plans and future implementation.
- **Preserved work:** [codex/legacy-snapshot-20261003](https://github.com/eljja/OpenRhiza/tree/codex/legacy-snapshot-20261003), a source/document snapshot including previously uncommitted changes.
- **Earlier public history:** `main` and existing branches remain; no force update or deletion.

This branch does not import the old kernel, root Cargo configuration, or deployment workflow into
`next/`. [Archive notes](ARCHIVE.md) explain preservation, excluded local artifacts, and historical
paths. The old Nexus deployment does not run here. Website operations and new OS readiness are separate.

## Document Checks

From the repository root, with PowerShell 7 and Git:

```powershell
pwsh -NoProfile -File next/tools/verify-baseline.ps1
pwsh -NoProfile -File next/tools/test-baseline.ps1
git fetch --no-tags origin codex/legacy-snapshot-20261003
pwsh -NoProfile -File next/tools/verify-baseline.ps1 -LegacyRef FETCH_HEAD
```

Default checks cover required documents, translation structure, identifiers, and frozen inventory
integrity. The last command verifies the actual bytes in the pinned archive commit against that
inventory. **These are not OS builds, QEMU tests, or kernel safety evidence.** There is no new image
to run or download at this checkpoint.

[Contributing (한국어)](CONTRIBUTING.md) · [Security (한국어)](SECURITY.md) · [Apache-2.0 license](LICENSE)
