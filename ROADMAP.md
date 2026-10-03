# OpenRhiza Roadmap

Historical bootstrap roadmap after the 2026-10-03 new-implementation reset.
Use `next/docs/DIRECTION.md`, `next/docs/REQUIREMENTS.md` and
`next/docs/ROADMAP.md` for active development. The existing runtime and all
completion statements below remain reference material, not new OS progress.

Architecture review on 2026-10-03: use `OPENRHIZA_MODULE_ARCHITECTURE.md` for the
minimal-core and core-survival contracts and `OPENRHIZA_REBUILD_PLAN.md` for the
active six-stage sequence. Isolated processes, preemption, capability enforcement,
resource reservations, and user-mode Wasm workers precede service and GUI migration.
The baseline and migration backlog below describe the earlier embedded-runtime
implementation. Network/storage/GUI recovery exceptions are temporary; they do not
authorize new permanent kernel service logic.

This is the current working roadmap for making OpenRhiza usable before wider public release.

The goal is not to build a traditional OS with an AI chatbot attached.
The goal is to build the best autonomous AI OS: a small survival core plus sandboxed capability objects that the OS can discover, generate, validate, activate, roll back, and improve.

## Non-Negotiable Direction

- Keep the core small.
- Keep recovery input, recovery display, storage bootstrap, network bootstrap, sandbox execution, and rollback gates alive.
- Move drivers, filesystems, GUI behavior, programs, workflows, and most autonomy logic into sandboxed capabilities whenever possible.
- Treat every capability as an object with identity, lifecycle, validation status, rollback target, and a request surface.
- The user should express intent in prompts. OpenRhiza should handle lookup, generation, validation, activation, reporting, and learning.

## Current Baseline

OpenRhiza currently has:

- bare-metal x86_64 boot and recovery shell
- native e1000 networking path
- in-repo TLS path for OpenRhiza/Gemini API calls
- embedded Wasm runtime with multiple named module support
- bounded round-robin Wasm polling for input and capability modules
- scheduler metrics, bounded batch execution, and dropped-wake rescan recovery
- FAT16 bootstrap read/write floor with sector verification and flush
- fixed-slot skill disk and local capability cache
- 1920x1080 bootstrap GUI with object-scoped scene/mutation model
- Korean-capable GUI text path and UTF-8-safe autonomy context extraction
- autonomy mode commands with off/assist/council modes and a Gemini-backed council loop
- autonomy stale-cycle timeout recovery so failed LLM requests cannot block later cycles
- `/wasm-status`, `/semantic-status`, and `/registry-context` inspection commands
- SMP discovery/heartbeat substrate, but not real AP startup yet
- x86_64 QEMU remains the reference platform before broader ARM64 and phone expansion
- ARM64 now has a standalone `aarch64-qemu-virt` serial recovery ELF that builds and smoke-boots under QEMU
- voice input now has user commands, route state, a host-assisted bridge, and a sandbox route chain; real in-guest microphone capture is still pending

## Public-Usable Minimum

Before presenting OpenRhiza as publicly usable, the minimum target is:

1. Boot reliability
   - QEMU boot should consistently reach the GUI.
   - Recovery shell must remain accessible after GUI failure.
   - Keyboard and mouse input must survive display handoff.

2. Prompt-first workflow
   - Plain input should route to Gemini.
   - Local `/` commands should remain available for recovery and inspection.
   - The OS should query registry context before generating new capabilities.

3. Capability registry loop
   - Drivers, skills, workflows, policies, and evaluations must be queryable.
   - Downloaded artifacts must be cached and validated before activation.
   - Generated artifacts must be uploaded with evaluation/comment/vote when safe.

4. Sandbox object discipline
   - GUI objects, input parsers, drivers, skills, filesystem bridges, and autonomy agents should have explicit identities.
   - Activation must be live and reversible where possible.
   - Persistence must be a separate decision from live activation.

5. Storage foundation
   - The bootstrap cache must remain stable.
   - Filesystem family skills should probe and validate image-backed filesystems without mutating the recovery disk.
   - A semantic graph layer should index managed files so the LLM can reason over filesystem contents.

6. Autonomy foundation
   - Autonomy defaults to off.
   - User controls mode and interval.
   - Council mode should present inferred intent, goal, blocker, evidence, proposal, and approval requirement.
   - Autonomous work must stay bounded, reversible, and approval-gated for risky actions.

7. Voice-ready input foundation
   - Voice input must be optional and off by default.
   - Keyboard and recovery input must remain available.
   - Audio capture, VAD, transcription, and confirmation should be sandbox skills/workflows.
   - Remote multimodal transcription must show a transcript before action unless the user explicitly enables a trusted hands-free mode.

## Immediate Engineering Priorities

1. Enforce the boundary tags in [`CORE_BOUNDARY_AUDIT.md`](CORE_BOUNDARY_AUDIT.md).
2. Migrate native `e1000` toward a sandbox driver using `DRIVER_HOST_ABI.md`, keeping native fallback until sustained registry/Gemini traffic passes.
3. Migrate xHCI/HID and richer keymap/IME behavior toward sandbox input drivers over raw HID handoff.
4. Promote GUI shell/layout/render policy from bootstrap core helpers toward sandbox-owned `skill_gui_*` capabilities.
5. Complete filesystem bridge smoke tests inside OpenRhiza for FAT32, exFAT, NTFS, ext2, ext3, and ext4 images.
6. Move filesystem read/write implementation behind `skill_fs_bridge` rather than adding heavy filesystem logic to the core.
7. Move autonomy agents, evidence gathering, and proposal/vote logic into workflow skills while keeping core user-controlled mode and interval gates.
8. Add stronger per-module Wasm quotas: CPU budget, memory-page accounting, host ABI call limits, and automatic quarantine.
9. Stabilize the GUI after long Gemini conversations, including scroll persistence and message history retention.
10. Implement semantic index primitives in `skill_semantic_graph_index_v1`: file identity, content hash, summary, entities, links, freshness, and confidence.
11. Expose semantic graph query results to Gemini without dumping raw filesystem contents.
12. Continue SMP work from heartbeat-only to AP startup and later per-core scheduling.
13. Unify dedicated Nexus fetch with the generic TLS/API response path.
14. Harden the first x86_64 voice input workflow: transcript import, route policy, GUI confirmation, and no regression to keyboard or recovery input.
15. Continue the platform expansion path in [`PLATFORM_EXPANSION.md`](PLATFORM_EXPANSION.md): keep x86_64 stable, extend the ARM64 serial recovery kernel with sandbox host ABI stubs, then add virtio-mmio handles before real ARM boards or Android phones.

## Documentation And Release Tracking

- Use [`DOC_INDEX.md`](DOC_INDEX.md) to decide which documents are authoritative and which are historical.
- Use [`VALIDATION_MATRIX.md`](VALIDATION_MATRIX.md) to track pass/partial/design/blocker status for runtime capabilities.
- Use [`PUBLIC_ALPHA_CHECKLIST.md`](PUBLIC_ALPHA_CHECKLIST.md) as the minimum bar before wider public release.
- Historical Gemini notes remain useful context, but they must not override `OS.md`, this roadmap, or the core boundary audit.

## Near-Term User-Facing Goals

- A user can ask: "install the right driver for this device" and OpenRhiza handles registry lookup, validation, activation, persistence, and reporting.
- A user can ask: "make the GUI better" and OpenRhiza inspects the scene, proposes object-scoped mutations, applies safe changes, and rolls back failures.
- A user can ask: "what files and skills are available" and OpenRhiza answers from local cache, registry context, filesystem bridge, and semantic graph.
- A user can enable autonomy and receive useful bounded suggestions every configured interval without losing control.
- A user can speak a prompt in keyboard-limited contexts and still get transcript confirmation, rollback safety, and normal prompt-first workflow execution.

## Things Not To Do

- Do not add large filesystem implementations directly to the core.
- Do not make GUI layout a pile of global mutable state.
- Do not trust downloaded/generated artifacts without sandbox validation.
- Do not let autonomy change its own interval or mode.
- Do not persist new drivers or skills just because they loaded once.
- Do not remove or weaken the recovery shell to make the GUI look cleaner.

## Definition Of Progress

OpenRhiza improves when:

- the core gets smaller or more clearly bounded
- more behavior moves behind sandbox/object boundaries
- more actions become reversible
- more state becomes inspectable by the AI
- more generated work is validated before activation
- the user has fewer system-level details to manage manually
