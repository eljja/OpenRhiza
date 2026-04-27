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
2. [DISPLAY_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/DISPLAY_ABI.md)
3. [GUI_DEVELOPMENT.md](D:/python/github/OpenRhiza/OpenRhiza/GUI_DEVELOPMENT.md)
4. [ARCHITECTURE.md](D:/python/github/OpenRhiza/OpenRhiza/ARCHITECTURE.md)
5. [MODULE_MAP.md](D:/python/github/OpenRhiza/OpenRhiza/MODULE_MAP.md)
6. [KNOWN_ISSUES.md](D:/python/github/OpenRhiza/OpenRhiza/KNOWN_ISSUES.md)
7. [BUILD_AND_TEST.md](D:/python/github/OpenRhiza/OpenRhiza/BUILD_AND_TEST.md)
8. [PROGRAM_COMPATIBILITY_GOALS.md](D:/python/github/OpenRhiza/OpenRhiza/PROGRAM_COMPATIBILITY_GOALS.md)
9. [SEMANTIC_GRAPH_LAYER.md](D:/python/github/OpenRhiza/OpenRhiza/SEMANTIC_GRAPH_LAYER.md)

Historical Gemini files should not override these.

## 3. What Codex / ChatGPT Has Completed

As of late April 2026, Codex-side work has already pushed OpenRhiza through these milestones:

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
- FAT read path adjusted so fixed-slot artifacts can be found more reliably
- padded Wasm slots trimmed back to real module length before parsing
- boot autorun stabilized enough to reach the GUI bootstrap path again

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

## 4. What Still Needs Work

These are the main live follow-up items:

### High Priority

1. Finish residual GUI flicker elimination, especially at object-boundary transitions.
2. Stabilize `skill_gui_compositor_seed_v1` from the fixed-slot seed path.
3. Keep physical input stable through GUI handoff without regressions.
4. Continue moving GUI ownership away from bootstrap-only core rendering and toward sandbox-owned scene mutation.

### Medium Priority

1. Better conversation scroll behavior
2. Better composer auto-resize and text editing behavior
3. More robust object-local redraw and invalidation
4. Stronger self-hosted GUI mutation from internal prompts
5. Better separation between temporary bootstrap presenters and long-term GUI engine behavior

### Longer Term

1. Multi-capability Wasm/runtime ownership cleanup
2. Storage write path
3. TLS path integration into the active Nexus fetch path
4. Stronger capability promotion/evaluation/report loop
5. Program compatibility through sandboxed compatibility skills rather than core expansion
6. Sidecar semantic graph layer over managed filesystems so the OS can query structured file knowledge before raw scans

## 5. Current Known Important Caveats

- `skill_gui_compositor_seed_v1` can still fail from `SK003.WAS` with a bad-magic parse problem.
- Some older docs still exist for historical reasons and mention `host_brain.py`, bottom-row VGA CLI, or earlier migration phases.
- Those docs should be treated as historical unless refreshed.

## 6. Codex Working Style For This Repo

When continuing OpenRhiza work:

1. Preserve the minimal-core rule first.
2. Prefer object-scoped fixes over global hacks.
3. Prefer sandbox capability evolution over core feature growth.
4. Preserve recovery shell and recovery input whenever experimenting with GUI or display changes.
5. Build and boot-test after meaningful changes.
6. Update the authoritative docs when architecture or behavior changes.
7. If a historical doc no longer matches reality, either refresh it or explicitly mark it historical.

## 7. Suggested Split With Gemini

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

## 8. Quick Commands

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

## 9. Last Principle Reminder

If a new feature breaks an old feature, assume the boundary is wrong.

The answer should usually be:

- narrower core
- cleaner object boundaries
- safer sandbox ownership
- stronger rollback

not “more feature logic in the core.”
