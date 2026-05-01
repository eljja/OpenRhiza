# Codex Operating Memory

This file is for Codex / ChatGPT handoff and persistent working memory.
It is intended to help future Codex sessions continue OpenRhiza work without drifting away from the project's core rules.

Gemini may keep its own notes, but this file is the Codex-side operational memory.

## 1. Non-Negotiable Rules

These rules override convenience:

1. Keep only the minimum survival path in the core.
2. Everything that can run as a sandbox capability should become a sandbox capability.
3. Drivers, skills, workflows, programs, services, and GUI items should behave like isolated objects.
4. One broken object must not silently break unrelated objects.
5. Recovery input and recovery display must remain available even if higher-level GUI work fails.
6. The final goal is self-hosting:
   - OpenRhiza should eventually improve its own drivers, workflows, and GUI from inside OpenRhiza itself.
7. External development help is allowed during bootstrap.
   - It is not the end state.

## 2. Authoritative Documents

When there is a conflict, prefer these files in this order:

1. [OS.md](D:/python/github/OpenRhiza/OpenRhiza/OS.md)
2. [ROADMAP.md](D:/python/github/OpenRhiza/OpenRhiza/ROADMAP.md)
3. [ARCHITECTURE.md](D:/python/github/OpenRhiza/OpenRhiza/ARCHITECTURE.md)
4. [DISPLAY_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/DISPLAY_ABI.md)
5. [DRIVER_HOST_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/DRIVER_HOST_ABI.md)
6. [GUI_DEVELOPMENT.md](D:/python/github/OpenRhiza/OpenRhiza/GUI_DEVELOPMENT.md)
7. [MODULE_MAP.md](D:/python/github/OpenRhiza/OpenRhiza/MODULE_MAP.md)
8. [KNOWN_ISSUES.md](D:/python/github/OpenRhiza/OpenRhiza/KNOWN_ISSUES.md)
9. [BUILD_AND_TEST.md](D:/python/github/OpenRhiza/OpenRhiza/BUILD_AND_TEST.md)
10. [PROGRAM_COMPATIBILITY_GOALS.md](D:/python/github/OpenRhiza/OpenRhiza/PROGRAM_COMPATIBILITY_GOALS.md)
11. [SEMANTIC_GRAPH_LAYER.md](D:/python/github/OpenRhiza/OpenRhiza/SEMANTIC_GRAPH_LAYER.md)
12. [STORAGE_HOST_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/STORAGE_HOST_ABI.md)
13. [SKILL_FS_BRIDGE.md](D:/python/github/OpenRhiza/OpenRhiza/SKILL_FS_BRIDGE.md)
14. [STORAGE_IMAGE_HARNESS.md](D:/python/github/OpenRhiza/OpenRhiza/STORAGE_IMAGE_HARNESS.md)
15. [AUTONOMY_MODE.md](D:/python/github/OpenRhiza/OpenRhiza/AUTONOMY_MODE.md)
16. [CORE_RUNTIME_FOUNDATION.md](D:/python/github/OpenRhiza/OpenRhiza/CORE_RUNTIME_FOUNDATION.md)
17. [QEMU_DRIVER_SET.md](D:/python/github/OpenRhiza/OpenRhiza/QEMU_DRIVER_SET.md)

Historical Gemini files should not override these.

## 3. What Codex / ChatGPT Has Completed

As of May 1, 2026, Codex-side work has already pushed OpenRhiza through these milestones:

### Display / GUI

- Recovery console -> framebuffer validation -> GUI bootstrap staged flow
- `1920x1080` bootstrap GUI
- object-based GUI runtime with:
  - sidebar
  - conversation surface
  - composer
  - footer
- object-scoped hover, focus, selection, and redraw behavior
- modern GUI font pipeline using Noto Sans KR assets
- LVGL-style bridge scaffold over the same scene contract

### Input / Stability

- GUI input deadlock fixed by removing VGA writer re-entry from pointer interaction flow
- GUI composer input restored after GUI handoff
- pointer shape, size, and interaction visuals refined
- major GUI flicker sources reduced by:
  - removing large redraws on pointer motion
  - separating interaction-state redraw from layout redraw
  - removing pointer-coordinate redraw churn from the GUI status bar

### Capability / Bootstrapping

- fixed-slot skill seed model:
  - `SK000.WAS` -> display console bootstrap
  - `SK001.WAS` -> GUI session bootstrap
  - `SK002.WAS` -> framebuffer mode
  - `SK003.WAS` -> GUI compositor seed
- `SK004.WAS` -> registry lookup
- `SK005.WAS` -> GUI scene mutator
- `SK006.WAS` -> filesystem image probe bootstrap
- `SK007.WAS` -> GUI modern shell seed
- `SK008.WAS` -> QEMU baseline driver binding pack
- optional `fs_harness.img` secondary-slave raw disk path for sandbox filesystem validation
- FAT read path adjusted so fixed-slot artifacts can be found more reliably
- padded Wasm slots trimmed back to real module length before parsing
- boot autorun stabilized enough to reach the GUI bootstrap path again

### Autonomy / Runtime Substrate

- `AUTONOMY_MODE.md` created as the autonomy design contract
- `src/autonomy.rs` added with:
  - default `off` mode
  - `assist` and `council` modes
  - user-controlled interval
  - Gemini-backed role-separated council prompts
  - council response summarization
- CLI commands added:
  - `/autonomy-status`
  - `/autonomy-mode <off|assist|council>`
  - `/autonomy-interval <minutes>`
  - `/autonomy-run-now`
- autonomy prompt origin tracking added so council responses do not execute machine-action JSON
- UTF-8-safe GUI context extraction added for Korean and other multi-byte text

### Scheduler / Core Runtime

- executor queue capacity increased
- batch execution budget added
- scheduler metrics added
- dropped wakes now request full task rescan instead of becoming silent lost wakeups
- multiple named Wasm modules now use bounded round-robin polling
- SMP bootstrap status and heartbeat reporting added
- FAT16 bootstrap writes now include sector verification and cache flush
- `cargo build` and `cargo bootimage` pass after the latest stabilization patch

### GUI Mutation / Self-Hosting Direction

- GUI scene contract created
- GUI mutation host imports added:
  - label
  - style
  - bounds
  - interaction
  - focus
  - session selection
  - reset
- local CLI inspection/mutation commands added
- `skill_gui_scene_mutator_v1` bootstrap skill added
- `skill_gui_modern_shell_v1` added as the correct path for polished GUI layout work: sandbox-owned object mutations, not hardcoded core GUI branches.
- `skill_qemu_driver_pack_v1` added so QEMU baseline driver bindings are activated by sandbox skill through `os_driver_activate_binding`, not by adding device-specific driver policy to the core.
- `DRIVER_HOST_ABI.md` and `src/driver_host.rs` added as the correct path for low-level sandbox drivers: device claim, PCI config, MMIO, PIO, DMA, and IRQ polling through opaque handles.
- Legacy raw Wasm imports `read_mmio`, `write_mmio`, and `alloc_dma_page` are intentionally denied; new drivers must use handle-based `os_driver_*` calls.

## 4. What Still Needs Work

These are the main live follow-up items:

### High Priority

1. QEMU regression test the latest autonomy/scheduler/UTF-8 changes.
2. Keep physical input stable through GUI handoff without regressions.
3. Stabilize long-running GUI conversations: persistence, scroll, and message retention.
4. Continue moving GUI ownership away from bootstrap-only core rendering and toward sandbox-owned scene mutation.
5. Move e1000, xHCI, and storage protocol logic into separate sandbox driver artifacts using `DRIVER_HOST_ABI.md`.
6. Add autonomy cycle timeout/stale-cycle recovery.

### Medium Priority

1. Better conversation scroll behavior
2. Better composer auto-resize and text editing behavior
3. More robust object-local redraw and invalidation
4. Stronger self-hosted GUI mutation from internal prompts
5. Better separation between temporary bootstrap presenters and long-term GUI engine behavior

### Longer Term

1. Multi-capability Wasm/runtime ownership cleanup with per-module accounting
2. SMP substrate -> AP bring-up -> per-core scheduling progression
3. Storage write floor -> sandbox filesystem bridge -> real file operations progression
4. Final transport unification for Nexus fetch on the same HTTPS/TLS response path
5. Stronger capability promotion/evaluation/report loop
6. Program compatibility through sandboxed compatibility skills rather than core expansion
7. Sidecar semantic graph layer over managed filesystems so the OS can query structured file knowledge before raw scans
8. First-boot autonomy UX with a bounded three-agent council and explicit approval gates

## 5. Filesystem Bridge Milestone

Current storage/filesystem progress now includes:

- external validation lab for FAT32, exFAT, NTFS, ext2, ext3, ext4
- `STORAGE_HOST_ABI.md`
- `SKILL_FS_BRIDGE.md`
- `STORAGE_IMAGE_HARNESS.md`
- optional in-OS `fs_harness.img`
- `skill_fs_image_probe_v1`

The current in-OS milestone is not full mount/file APIs yet.
It is:

- raw block exposure from core
- sandbox filesystem family detection
- bounded scratch write/read/restore validation

## 6. Current Known Important Caveats

- `skill_gui_compositor_seed_v1` may still need fixed-slot seed-path regression testing.
- Some older docs still exist for historical reasons and mention `host_brain.py`, bottom-row VGA CLI, or earlier migration phases.
- Those docs should be treated as historical unless refreshed.

## 7. Codex Working Style For This Repo

When continuing OpenRhiza work:

1. Preserve the minimal-core rule first.
2. Prefer object-scoped fixes over global hacks.
3. Prefer sandbox capability evolution over core feature growth.
4. Preserve recovery shell and recovery input whenever experimenting with GUI or display changes.
5. Build and boot-test after meaningful changes.
6. Update the authoritative docs when architecture or behavior changes.
7. If a historical doc no longer matches reality, either refresh it or explicitly mark it historical.

## 8. Suggested Split With Gemini

If Gemini and Codex split work:

### Good Codex Responsibilities

- architectural cleanup
- implementation refactors
- sandbox/core boundary enforcement
- GUI runtime and object model stabilization
- doc consistency and operating rules
- build/test/run harness work

### Good Gemini Responsibilities

- exploration
- alternative implementation proposals
- broader capability brainstorming
- candidate skill/workflow ideas
- long-form planning artifacts

### Shared Rule

Neither assistant should treat a historical planning note as more authoritative than the current baseline docs.

## 9. Quick Commands

Main QEMU test command:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu.ps1 target\x86_64-unknown-none\debug\bootimage-OpenRhiza.bin
```

Main local verification:

```powershell
cargo build
cargo bootimage
pwsh.exe -ExecutionPolicy Bypass -File .\build_sandbox_skills.ps1
cd openrhiza-nexus
npm run build
```

## 10. Last Principle Reminder

If a new feature breaks an old feature, assume the boundary is wrong.

The answer should usually be:

- narrower core
- cleaner object boundaries
- safer sandbox ownership
- stronger rollback

not “more feature logic in the core.”
