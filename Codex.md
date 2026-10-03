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

User language preference recorded on 2026-10-03: provide Korean versions of the
new planning and review documents. `next/README.ko.md` indexes all eight complete
counterparts. Keep Korean counterparts synchronized when updating current plans;
preserve identifiers, acceptance values, historical evidence and English originals.

User model preference recorded on 2026-10-03: use an explicitly selected
`gpt-6-astra` reviewer for the new plan and `gpt-6-astra` for new kernel
implementation. The coordinating model may prepare context, integrate reviewed
work and run verification. Report the actual model used; do not silently replace
Astra or equate model choice with correctness. Kernel implementation includes
iterative build, fault-test and repair work within the accepted stage gates.

The user's 2026-10-03 reset request supersedes the incremental rebuild plan:
start a new first-party OS implementation, preserving existing code as reference
only. Read `next/README.md`, `next/docs/DIRECTION.md`,
`next/docs/REQUIREMENTS.md`, `next/docs/LEGACY_REVIEW.md`, and
`next/docs/ROADMAP.md` first. `DOC_INDEX.md` records this precedence.

Current checkpoint: R0 with Astra plan-review amendments; see
`next/docs/ASTRA_PLAN_REVIEW.md`, `next/docs/ASTRA_REVIEW_DISPOSITION.md` and
`next/docs/IMPLEMENTATION_GATES.md` for findings, review status and entry gates.
Stage 1.1/K selections and fixtures are still outstanding. The next task is to
close that small boot/toolchain/ownership/recovery/harness contract before Astra
implements Stage 1.2. Runtime, device and release gates remain unpassed.
Do not import the old kernel, patched bootloader,
embedded secrets, fixed-slot ABI, images, compiled modules or success counters.
Future source/build/test configuration belongs to a separate workspace in
`next/`; it must not silently inherit parent Cargo configuration/build scripts.
Keep offline operation, native process isolation, user-mode Wasm workers and
bounded deterministic authority. Report a watchdog reboot as recovery, not
core survival. Do not expose generated drivers to real hardware without a
qualified device isolation/reset model.

Existing-image QEMU command smoke passed in
`logs/x86-legacy-reference-reset-review-20261003.log` for status, scheduler,
Wasm, desktop, platform and recovery commands. It is legacy reference evidence;
no new OS image or process-isolation gate was tested. No live registry write,
model generation, release publication or source deletion was requested.

The Astra review turn also passed the existing-image `/status` and `/recovery`
smoke in `logs/x86-astra-plan-review-reference-20261003.log`. This does not
qualify a new profile or kernel. Model choice, a plan review, inventory checks
and old-image boots cannot substitute for the new acceptance gates.

### Earlier Review Checkpoints (Reference Only)

The broader 2026-10-03 ecosystem checkpoint is in `OPENRHIZA_COM_PLAN.md`:
OpenRhiza includes the local independent OS, Nexus shared capability/evidence
registry, and a separately authorized module forge. Also preserve self-hosting,
semantic storage, voice, autonomy, Doom, compatibility and platform-expansion
goals. Existing host/micro-compiler and device mock work is not guest completion.
Nexus source currently lacks authenticated contributions, mandatory signed
immutable delivery and verified node trust; guest TLS skips peer verification.
Native DNS exists, unlike older integration-plan text. QEMU TCG connectivity and
status/recovery smoke passed in `logs/x86-ecosystem-review-20261003.log`; public
boards/health returned 200 and Nexus lint passed with two warnings. No runtime
source changed, no live writes/model generation were requested by this review,
and no rebuild stage was completed. Continue with Stage 1, not more GUI demos.

The following order belonged to the bootstrap implementation. These files now
serve as references and cannot override the new reset direction:

1. [OS.md](D:/python/github/OpenRhiza/OpenRhiza/OS.md)
2. [ROADMAP.md](D:/python/github/OpenRhiza/OpenRhiza/ROADMAP.md)
3. [DOC_INDEX.md](D:/python/github/OpenRhiza/OpenRhiza/DOC_INDEX.md)
4. [VALIDATION_MATRIX.md](D:/python/github/OpenRhiza/OpenRhiza/VALIDATION_MATRIX.md)
5. [PUBLIC_ALPHA_CHECKLIST.md](D:/python/github/OpenRhiza/OpenRhiza/PUBLIC_ALPHA_CHECKLIST.md)
6. [ARCHITECTURE.md](D:/python/github/OpenRhiza/OpenRhiza/ARCHITECTURE.md)
7. [CORE_BOUNDARY_AUDIT.md](D:/python/github/OpenRhiza/OpenRhiza/CORE_BOUNDARY_AUDIT.md)
8. [DISPLAY_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/DISPLAY_ABI.md)
9. [DRIVER_HOST_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/DRIVER_HOST_ABI.md)
10. [GUI_DEVELOPMENT.md](D:/python/github/OpenRhiza/OpenRhiza/GUI_DEVELOPMENT.md)
11. [MODULE_MAP.md](D:/python/github/OpenRhiza/OpenRhiza/MODULE_MAP.md)
12. [KNOWN_ISSUES.md](D:/python/github/OpenRhiza/OpenRhiza/KNOWN_ISSUES.md)
13. [BUILD_AND_TEST.md](D:/python/github/OpenRhiza/OpenRhiza/BUILD_AND_TEST.md)
14. [SANDBOX_MIGRATION_STATUS.md](D:/python/github/OpenRhiza/OpenRhiza/SANDBOX_MIGRATION_STATUS.md)
15. [PROGRAM_COMPATIBILITY_GOALS.md](D:/python/github/OpenRhiza/OpenRhiza/PROGRAM_COMPATIBILITY_GOALS.md)
16. [SEMANTIC_GRAPH_LAYER.md](D:/python/github/OpenRhiza/OpenRhiza/SEMANTIC_GRAPH_LAYER.md)
17. [STORAGE_HOST_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/STORAGE_HOST_ABI.md)
18. [SKILL_FS_BRIDGE.md](D:/python/github/OpenRhiza/OpenRhiza/SKILL_FS_BRIDGE.md)
19. [STORAGE_IMAGE_HARNESS.md](D:/python/github/OpenRhiza/OpenRhiza/STORAGE_IMAGE_HARNESS.md)
20. [AUTONOMY_MODE.md](D:/python/github/OpenRhiza/OpenRhiza/AUTONOMY_MODE.md)
21. [CORE_RUNTIME_FOUNDATION.md](D:/python/github/OpenRhiza/OpenRhiza/CORE_RUNTIME_FOUNDATION.md)
22. [QEMU_DRIVER_SET.md](D:/python/github/OpenRhiza/OpenRhiza/QEMU_DRIVER_SET.md)
23. [PLATFORM_EXPANSION.md](D:/python/github/OpenRhiza/OpenRhiza/PLATFORM_EXPANSION.md)
24. [VOICE_INPUT.md](D:/python/github/OpenRhiza/OpenRhiza/VOICE_INPUT.md)
25. [CROSS_PLATFORM_BRINGUP.md](D:/python/github/OpenRhiza/OpenRhiza/CROSS_PLATFORM_BRINGUP.md)

Historical Gemini files should not override these.

## 3. What Codex / ChatGPT Has Completed

As of May 2, 2026, Codex-side work has already pushed OpenRhiza through these milestones:

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
- e1000, PIIX IDE, and xHCI probe-only sandbox skills now release temporary device handles after validation so raw device authority does not accumulate.
- Driver-host handles, sandbox e1000 ownership, sandbox storage ownership, sandbox input active ownership, and GUI shell policy ownership now have heartbeat/lease watchdogs so stale sandbox authority rolls back instead of persisting silently.
- `src/storage_host.rs` now has staged storage driver promotion gates. PIIX controller evidence and image-backed block-object validation are intentionally separated until a real sandbox block driver can prove both.
- Sandbox input drivers now report health evidence through dedicated input imports, but they still cannot self-promote without validation and recovery fallback preservation.
- `/core-boundary-status` reports the live migration state from inside the OS.
- `SANDBOX_MIGRATION_STATUS.md` records the current cross-subsystem migration status and next order.

## 4. What Still Needs Work

These are the main live follow-up items:

### High Priority

1. Enforce `CORE_BOUNDARY_AUDIT.md` when adding or moving features.
2. Move native e1000 toward a sandbox driver artifact using `DRIVER_HOST_ABI.md`, with native fallback retained until traffic validation passes.
3. Move xHCI/HID parsing, richer keymaps, and IME behavior toward sandbox input drivers without regressing recovery input.
4. Continue moving GUI ownership away from bootstrap-only core rendering and toward sandbox-owned scene mutation.
5. Move filesystem family read/write behavior behind `skill_fs_bridge`.
6. Move autonomy planning/council behavior out to workflow skills while core keeps only user-controlled gates.
7. QEMU regression test the latest autonomy/scheduler/UTF-8 changes.
8. Keep physical input stable through GUI handoff without regressions.
9. Stabilize long-running GUI conversations: persistence, scroll, and message retention.

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
9. Platform expansion in the sequence: x86_64 QEMU stabilization -> aarch64 QEMU `virt` -> open ARM board -> Android phone
10. Voice input as a sandbox capability chain: audio capture bridge -> VAD -> transcription -> transcript confirmation -> normal prompt path

### Voice And Platform Expansion

- Do not start phone support by targeting Android directly. Start with `qemu-system-aarch64 -machine virt`.
- Use `platforms/openrhiza-platforms.json`, `tools/platform_matrix.py`, and `/platform-status` as the shared source of truth for platform expansion state.
- `platform-kernels/aarch64-recovery` is the first ARM64 survival kernel. Build it with `build_arm64_recovery.ps1` and smoke-test with `tools/smoke_arm64_recovery.py`.
- Current ARM64 rule: the default recovery ELF must stay minimal. GUI, virtio payload, production routing, boot-live, storage-policy, and LLM-policy behavior are denied in recovery core and must be rebuilt as sandbox skills/workflows or explicit external proof harnesses.
- For direct ARM64 virtio sandbox gate validation, run `powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1`. Use `-SkipBuild -Quick` for a fast local recheck, add `-LiveCutover` to verify explicit device owner cutover plus rollback for block/gpu/net/input, add `-LiveRouting` to verify sequential cutover plus sandbox-owner dispatch windows plus rollback, add `-LiveRouteAll` to verify all four owners active together before dispatch and rollback-all, add `-LiveBootService` to verify boot-to-post-boot-service-pump with rollback-all, add `-LiveServiceLoop` to verify the ARM64 main idle loop pumps sandbox-owner request/event ticks after the command handler returns, add `-LiveOsPump` to verify typed block/net/input/gpu OS work through that main loop, add `-LiveOsPumpLong` for the 16-tick/96-work-item typed pump soak, add `-LiveOsRequestApi` to verify `/vosrq` OS producer APIs prequeue typed work before the main loop drains it through sandbox owners, add `-LiveOsRuntime` to verify `/voslive` opt-in live runtime mode with pending-work main-loop pumping and rollback-all, add `-LiveOsSubsystemBridge` to verify `/vossys` subsystem-facing producer APIs, add `-LiveSubsystemRuntime` to verify `/vosps` persistent subsystem producer runtime, add `-LiveProductionQueue` to verify `/vosprod` normal-subsystem inbox forwarding into sandbox owners, add `-LiveProductionRuntime` to verify `/vosprod-start` plus external `/vosprod-all` submit batches into the production inbox, add `-LiveProductionSubsystemSource` to verify `/vosprod-source` recovery subsystem sources feeding `virtio_production_request_*`, add `-LiveProductionBootGate` to verify `/vosboot` 64-tick opt-in production boot gate, add `-LiveProductionDeviceBootGates` to verify `/vosboot-{block,gpu,net,input}`, and add `-LiveManualSubsystem` to verify `/vosman` plus external `/vossall` subsystem submits. Use `-OnlyLiveServiceLoop`, `-OnlyLiveOsPump`, `-OnlyLiveOsPumpLong`, `-OnlyLiveOsRequestApi`, `-OnlyLiveOsRuntime`, `-OnlyLiveOsSubsystemBridge`, `-OnlyLiveSubsystemRuntime`, `-OnlyLiveProductionQueue`, `-OnlyLiveProductionRuntime`, `-OnlyLiveProductionSubsystemSource`, `-OnlyLiveProductionBootGate`, `-OnlyLiveProductionDeviceBootGates`, or `-OnlyLiveManualSubsystem` for focused reruns.
- Newer ARM64 source-live validation flags are `-LiveProductionSubsystemSourceLive`, `-OnlyLiveProductionSubsystemSourceLive`, `-LiveProductionSubsystemSourceLiveSoak`, `-OnlyLiveProductionSubsystemSourceLiveSoak`, `-LiveProductionRealSubsystemSourceLive`, `-OnlyLiveProductionRealSubsystemSourceLive`, `-LiveProductionRealSubsystemSourceLiveSoak`, `-OnlyLiveProductionRealSubsystemSourceLiveSoak`, `-LiveProductionNormalSubsystemSourceLive`, `-OnlyLiveProductionNormalSubsystemSourceLive`, `-LiveProductionNormalSubsystemSourceLiveSoak`, `-OnlyLiveProductionNormalSubsystemSourceLiveSoak`, `-LiveProductionSandboxSourceLive`, `-OnlyLiveProductionSandboxSourceLive`, `-LiveProductionSandboxSourceLiveSoak`, `-OnlyLiveProductionSandboxSourceLiveSoak`, `-LiveProductionExternalNormalSourceLive`, `-OnlyLiveProductionExternalNormalSourceLive`, `-LiveProductionExternalNormalDeviceSubmitLive`, `-OnlyLiveProductionExternalNormalDeviceSubmitLive`, `-LiveProductionExternalNormalDeviceSubmitSoak`, `-OnlyLiveProductionExternalNormalDeviceSubmitSoak`, `-LiveProductionSandboxSourceDeviceLive`, `-OnlyLiveProductionSandboxSourceDeviceLive`, `-LiveProductionSandboxSourceDeviceLiveSoak`, and `-OnlyLiveProductionSandboxSourceDeviceLiveSoak`. They validate `/vosprod-source-live`, `/vosprod-source-live-soak`, `/vosprod-real-source-live`, `/vosprod-real-source-live-soak`, `/vosprod-normal-source-live`, `/vosprod-normal-source-live-soak`, `/vosprod-sandbox-source-live`, `/vosprod-sandbox-source-live-soak`, `/vosprod-external-normal-source-live`, repeated `/vosprod-external-normal-submit-device-round`, and device-scoped sandbox-source gates: ARM64 subsystem/normal queue -> production inbox -> sandbox-active virtio owners, then keep owners active until `/stop` rollback. The `real-source` variants first detect actual QEMU virtio-mmio device IDs, narrow source work to those devices, and require sandbox driver seed payload completion evidence before drain counters advance. The `normal-source` variants mark the future normal subsystem producer boundary separately from the temporary recovery adapter. The `sandbox-source` variants disable generated normal producers and require sandbox virtio seeds to submit accepted work through `os_arm64_submit_normal_work`. The `external-normal` proofs disable internal generation and accept explicit `/vosprod-external-normal-submit-all`, repeated `/vosprod-external-normal-submit-device-round`, or per-device `/vosprod-external-normal-submit-{block,gpu,net,input}` work only; they clear proof owners on `/stop` and do not open live device promotion. The 2026-05-10 all-device 32-tick soak, block/GPU/net/input device-scoped live/soak gates, external-normal one-batch proof, and external-normal per-device submit proof passed, but these remain opt-in evidence, not boot-default live promotion.
- `run_qemu_arm64.ps1` runs the generated ARM64 serial recovery ELF when present. It defaults to serial recovery (`-nographic`) because ARM64 GUI/framebuffer is not the active recovery path yet; use `-Graphic` only for display/GPU experiments where an inactive QEMU display window is expected until a sandbox display skill owns a framebuffer.
- The ARM64 serial recovery milestone is complete enough to be treated as a real survival proof, not just a design note.
- ARM64 now has the first virtio-mmio capability-handle scaffold (`os_driver_claim_mmio_device`) plus `skill_virtio_mmio_probe_v1`. The standalone ARM64 recovery kernel also has `/abi-status` and read-only `/virtio-mmio-probe` diagnostics. The remaining ARM64 work is wiring the handle ABI into the real ARM64 sandbox runtime after MMIO mappings exist, then adding virtio-net/block/input/gpu as sandbox drivers, not core code.
- ARM64 no_std `wasmi::TypedFunc::call()` works under QEMU after enabling ARM64 FP/SIMD through `CPACR_EL1`. Keep the tiny hand-authored Wasm executor for deterministic bridge diagnostics, but Rust-generated sandbox payloads now have a full-runtime regression path.
- ARM64 `/virtio-wasm-rollback-selftest` proves ten repeated tiny-executor reset/rollback cycles for virtio-gpu, virtio-block read/write, and virtio-net TX. `/virtio-wasm-promotion-gate-selftest` proves full Wasm block/gpu/net/input gate evidence. `/virtio-wasm-sustained-gate-selftest` repeats the full Wasm block/gpu/net/input gate across five rollback-window rounds and currently passes with `sustained gate rounds=5 passed=1`. Treat all of these as rollback/promotion-gate proofs, not live promotion.
- ARM64 `/vosprod` is the current production-queue shape proof: a separate normal-subsystem inbox accepts typed block read/write, GPU control, net RX/TX, and input event work, forwards that inbox into sandbox-active owners from the ARM64 main loop, validates `8` ticks / `48` enqueued / `48` forwarded / `0` rejected, then rollback-all clears every owner. `/vosprod-start` keeps that production inbox runtime active after the command returns; external `/vosprod-all` submit batches have passed with `48` enqueued / `48` forwarded / `0` rejected across eight batches before `/vosstop` rollback. `/vosprod-source` disables diagnostic auto-feed and lets ARM64 recovery storage/display/network/input source ticks call `virtio_production_request_*`; it has passed with `32` ticks, exact source/runtime/subsystem counters at `32` per path, and production inbox `enqueued=192 forwarded=192 rejected=0`. The focused wrapper `tools/run_arm64_production_runtime_demo.ps1 -SkipBuild -Batches 64` has also passed with `64` externally submitted batches, exact runtime counters `block_read=64 block_write=64 gpu_control=64 net_rx=64 net_tx=64 input_events=64 failures=0`, and production inbox counters `enqueued=384 forwarded=384 rejected=0`. `/vosboot` and `/vosboot-{block,gpu,net,input}` now pass 64-tick opt-in production boot promotion gates and rollback every owner after exact counter checks. `/vosboot-live` runs the same 64-tick all-device gate, then keeps sandbox-active virtio owners alive until `/vosstop` after `production boot-live keepalive active=1`. `/vosboot-live-soak` runs a longer 128-tick all-device boot-live validation before keeping sandbox-active owners alive. `/vosboot-live-{block,gpu,net,input}` now runs the same keepalive gate per device and keeps only that sandbox owner active after exact device-scoped counters pass. `/vosman` plus `/vossall` remains the manual subsystem submit proof. These are opt-in handoff gates, not boot-default routing.
- `/vosprod-source-live` has passed as the first opt-in ARM64 production routing keepalive gate: it validates the enqueue/drain ARM64 subsystem queue with `ARM64 subsystem queue enqueued=192 dispatched=192 rejected=0`, production inbox `enqueued=192 forwarded=192 rejected=0`, `production source-live keepalive active=1`, live owner mask `0x0000000f`, and `/stop` rollback to `live service running after stop=0`. `/vosprod-source-live-soak` extends the same path to 128 ticks with `ARM64 subsystem queue enqueued=768 dispatched=768 rejected=0` and production inbox `enqueued=768 forwarded=768 rejected=0`. `/vosprod-real-source-live` and `/vosprod-real-source-live-soak` now add actual detected-device gating before source enqueue and sandbox driver seed payload completion evidence during drain; both passed on 2026-05-08 with `device_mask=0x0000000f`, exact 192/768 work-item counters, block read/write, GPU control, net TX/RX, input event evidence, zero rejected work, and rollback. `/vosprod-normal-source-live` and `/vosprod-normal-source-live-soak` passed on 2026-05-09 after the normal producer path was split away from recovery adapters and routed through `VirtioArm64NormalSubsystemQueues -> arm64_normal_subsystem_submit_* -> VirtioArm64NormalDeviceQueues -> VirtioArm64NormalIngressQueue -> submit API -> VirtioArm64NormalSourceQueue -> VirtioArm64NormalDescriptorQueue`; they validate `normal_source=1`, `ARM64 normal subsystem producer source device_mask=0x0000000f`, subsystem queue counters, subsystem API counters, device queue enqueue/submit, ingress enqueue/submit, source enqueue/forward, descriptor enqueue/drain into `virtio_production_request_*`, exact 32/128-tick counters, normal subsystem queues `enqueued=192/768 dispatched=192/768 rejected=0`, normal subsystem API `block_read=32/128 block_write=32/128 gpu_control=32/128 net_rx=32/128 net_tx=32/128 input_events=32/128 rejected=0`, normal device queues `enqueued=192/768 submitted=192/768 rejected=0`, normal ingress queue `enqueued=192/768 submitted=192/768 rejected=0`, normal source queue `enqueued=192/768 forwarded=192/768 rejected=0`, normal producer source health `storage_ticks=32/128 display_ticks=32/128 network_ticks=32/128 input_ticks=32/128 rejected=0`, `source_mode=validated-keepalive boot_default_live=0`, and `/stop` rollback. `/vosboot-normal-live` and `/vosboot-normal-live-soak` also passed on 2026-05-09 as the subsystem-queue/API/device/ingress/source/descriptor-backed boot-live proof, keeping sandbox owners active after exact normal subsystem queue/API/device/ingress/source/descriptor queue evidence (`192/768` enqueued/dispatched/submitted/forwarded, zero rejected), exact normal producer health, `virtio live production boot gate passed=1`, and `production boot-live keepalive active=1`. The descriptor now carries per-device request metadata such as block LBA/generation, GPU target size/frame, net packet/length, and input event tuple. Treat these as production routing opened on demand, not boot-default.
- ARM64 virtio gate is directly runnable with `powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1`. Use `-SkipBuild -Quick` for fast local rechecks, add `-Sustained` to include the sustained rollback-window gate, add `-TrafficWindow` to include the 12-round all-device counter gate, add `-DeviceGates` to run independent block/gpu/net/input proof gates, add `-LiveCutover` to run explicit live-owner cutover/rollback selftests, add `-LiveRouting` to run explicit owner dispatch windows and rollback, add `-LiveRouteAll` to run all-device route-all dispatch and rollback, add `-LiveBootService` to run the post-boot live service pump selftest, add `-LiveServiceLoop` to prove the ARM64 main idle loop itself drains bounded request/event ticks, add `-LiveOsPump` to prove typed block read/write, GPU control, net RX/TX, and input event work through the main loop, add `-LiveOsPumpLong` to prove the same path over 16 ticks, add `-LiveOsRequestApi` to prove typed OS producer APIs feed the main-loop drain path, add `-LiveOsRuntime` to prove the opt-in runtime path, add `-LiveOsSubsystemBridge` to prove subsystem-facing producer APIs, add `-LiveSubsystemRuntime` to prove the persistent subsystem runtime producer, add `-LiveProductionQueue` to prove the normal-subsystem inbox queue path, add `-LiveProductionRuntime` to prove an opt-in production inbox runtime can accept external `/vosprod-all` batches after activation, add `-LiveProductionSubsystemSource` to prove `/vosprod-source` production work from ARM64 subsystem sources, add `-LiveProductionRealSubsystemSourceLive` or `-LiveProductionRealSubsystemSourceLiveSoak` to prove detected-device source work entering the production inbox, add `-LiveProductionNormalSubsystemSourceLive` or `-LiveProductionNormalSubsystemSourceLiveSoak` to prove the normal-subsystem producer boundary, add `-LiveProductionSandboxSourceLive`, `-LiveProductionSandboxSourceLiveSoak`, `-LiveProductionSandboxSourceDeviceLive`, or `-LiveProductionSandboxSourceDeviceLiveSoak` to prove sandbox virtio seeds submit normal work through `os_arm64_submit_normal_work`, add `-LiveProductionNormalBootLive` or `-LiveProductionNormalBootLiveSoak` to prove descriptor-backed normal producer boot-live keepalive, add `-LiveProductionRealSubsystemSourceDeviceLive` or `-LiveProductionRealSubsystemSourceDeviceLiveSoak` to prove the same path narrowed to block/gpu/net/input independently, add `-ProductionCandidate` to run the current production-candidate bundle, add `-LiveProductionBootGate` to prove `/vosboot`, add `-LiveProductionBootLive` to prove `/vosboot-live` keepalive after the gate passes, add `-LiveProductionBootLiveSoak` to prove the longer `/vosboot-live-soak` keepalive window, add `-LiveProductionDeviceBootGates` to prove `/vosboot-{block,gpu,net,input}`, add `-LiveProductionDeviceBootLiveGates` to prove `/vosboot-live-{block,gpu,net,input}`, or add `-LiveManualSubsystem` to prove external manual subsystem submits into an armed runtime. Use `-ProductionRuntimeBatches <n>` with `-LiveProductionRuntime` or `-OnlyLiveProductionRuntime` to raise the batch count. Use `tools/run_arm64_production_runtime_demo.ps1` when you need the focused production-runtime demo with summarized output and a full log file. Use `-OnlyLiveServiceLoop`, `-OnlyLiveOsPump`, `-OnlyLiveOsPumpLong`, `-OnlyLiveOsRequestApi`, `-OnlyLiveOsRuntime`, `-OnlyLiveOsSubsystemBridge`, `-OnlyLiveSubsystemRuntime`, `-OnlyLiveProductionQueue`, `-OnlyLiveProductionRuntime`, `-OnlyLiveProductionSubsystemSource`, `-OnlyLiveProductionRealSubsystemSourceLive`, `-OnlyLiveProductionRealSubsystemSourceLiveSoak`, `-OnlyLiveProductionNormalSubsystemSourceLive`, `-OnlyLiveProductionNormalSubsystemSourceLiveSoak`, `-OnlyLiveProductionSandboxSourceLive`, `-OnlyLiveProductionSandboxSourceLiveSoak`, `-OnlyLiveProductionSandboxSourceDeviceLive`, `-OnlyLiveProductionSandboxSourceDeviceLiveSoak`, `-OnlyLiveProductionNormalBootLive`, `-OnlyLiveProductionNormalBootLiveSoak`, `-OnlyLiveProductionRealSubsystemSourceDeviceLive`, `-OnlyLiveProductionRealSubsystemSourceDeviceLiveSoak`, `-OnlyProductionCandidate`, `-OnlyLiveProductionBootGate`, `-OnlyLiveProductionBootLive`, `-OnlyLiveProductionBootLiveSoak`, `-OnlyLiveProductionDeviceBootGates`, `-OnlyLiveProductionDeviceBootLiveGates`, or `-OnlyLiveManualSubsystem` when you need a focused check of the newest main-loop paths.
- 2026-05-07 ARM64 long soak evidence: `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionBootLiveSoak` passed with 128 all-device production ticks, exact counters `block_read=128 block_write=128 gpu_control=128 net_rx=128 net_tx=128 input_events=128 failures=0`, production inbox `enqueued=768 forwarded=768 rejected=0`, and `production boot-live keepalive active=1`. `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSubsystemSource` also passed with 32 subsystem-source ticks, `production subsystem source counters block_read=32 block_write=32 gpu_control=32 net_rx=32 net_tx=32 input_events=32 rejected=0`, and production inbox `enqueued=192 forwarded=192 rejected=0`. These are still explicit opt-in gates, not boot-default live routing.
- 2026-05-08 ARM64 source-live evidence: `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSubsystemSourceLive` passed with 32 subsystem-source ticks, `ARM64 subsystem queue enqueued=192 dispatched=192 rejected=0`, production inbox `enqueued=192 forwarded=192 rejected=0`, `production queue runtime running=1`, live owner mask `0x0000000f`, and `/stop` rollback to `live service running after stop=0`. `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSubsystemSourceLiveSoak` passed with 128 subsystem-source ticks, `ARM64 subsystem queue enqueued=768 dispatched=768 rejected=0`, production inbox `enqueued=768 forwarded=768 rejected=0`, and `/stop` rollback.
- 2026-05-08 ARM64 real-device source-live evidence: `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionRealSubsystemSourceLive` and `-OnlyLiveProductionRealSubsystemSourceLiveSoak` passed after wiring live drain to sandbox driver seed payload evidence. Both detected actual QEMU virtio-mmio devices as `device_mask=0x0000000f`; the short path validated 192 work items and the long soak validated 768 work items through the safe enqueue/drain/production-inbox path, with block read/write, GPU control, net TX/RX, input event completions, and `/stop` rollback. `-OnlyLiveProductionRealSubsystemSourceDeviceLive` also passed for block `0x00000001` with 32 ticks/64 work items, GPU `0x00000002` with 32 ticks/32 work items, net `0x00000004` with 32 ticks/64 work items, and input `0x00000008` with 32 ticks/32 work items. `-OnlyLiveProductionRealSubsystemSourceDeviceLiveSoak` passed for the same device-scoped gates over the longer 128-tick window.
- 2026-05-09/10 ARM64 production-candidate bundle evidence: `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyProductionCandidate` passed with exit code `0` after the normal-source gate moved to the descriptor queue boundary, passed again after typed per-device request metadata was added to normal descriptors, passed again after normal producer source-health counters became mandatory, passed again after the normal source queue was inserted, passed again after generated normal producers were forced through device-specific submit functions, passed again after generated normal producers were moved behind the ingress queue, passed again after generated normal producers were isolated behind normal device queues, passed again after generated normal producers were forced through `arm64_normal_subsystem_submit_*`, and passed again after generated producers were isolated behind `VirtioArm64NormalSubsystemQueues`. It combines all-device detected-source live, all-device detected-source long soak, subsystem-queue/API/device/ingress/source/descriptor-queued normal-source long soak with `ARM64 normal subsystem queues len=0 enqueued=768 dispatched=768 rejected=0`, `ARM64 normal subsystem API block_read=128 block_write=128 gpu_control=128 net_rx=128 net_tx=128 input_events=128 rejected=0`, `ARM64 normal device queues len=0 enqueued=768 submitted=768 rejected=0`, `ARM64 normal ingress queue len=0 enqueued=768 submitted=768 rejected=0`, `ARM64 normal source queue len=0 enqueued=768 forwarded=768 rejected=0`, and `ARM64 normal producer health storage_ticks=128 display_ticks=128 network_ticks=128 input_ticks=128 rejected=0`, plus all-device sandbox-source live/soak, block/GPU/net/input device-scoped sandbox-source long soak, and block/GPU/net/input device-scoped detected-source long soak. This is the current baseline to rerun before claiming ARM64 sandbox virtio production routing is healthy. It is not boot-default live routing.
- 2026-05-09/10 ARM64 sandbox-source evidence: `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSandboxSourceLive` and direct `tools/smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live --timeout 180` passed first. This is the first ARM64 gate where sandbox virtio driver seeds submit normal work through `os_arm64_submit_normal_work` while `generated_normal_source=0`. Evidence required `sandbox_source=1`, `device_mask=0x0000000f`, six sandbox-source submissions, exact six-item normal queue counters, production inbox `enqueued=6 forwarded=6 rejected=0`, keepalive active, and `/stop` rollback. On 2026-05-10 `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSandboxSourceLiveSoak` passed with 32 all-device ticks / 192 sandbox-source submissions and exact normal queue, production inbox, live pump, keepalive, and rollback evidence. `-OnlyLiveProductionSandboxSourceDeviceLive` and `-OnlyLiveProductionSandboxSourceDeviceLiveSoak` also passed for block, GPU, net, and input. Do not describe this as production boot-live or boot-default; it is opt-in sandbox-source soak evidence that still requires real subsystem queues and persisted recovery policy before any automatic promotion.
- 2026-05-10 ARM64 external-normal repeated submit evidence: `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionExternalNormalDeviceSubmitSoak` passed with eight explicit `/vosprod-external-normal-submit-device-round` batches, exact normal subsystem/device/ingress/source/descriptor totals `48`, production inbox `enqueued=48 forwarded=48 rejected=0`, live pump counters `block_read=8 block_write=8 gpu_control=8 net_rx=8 net_tx=8 input_events=8 failures=0`, validated keepalive, and `/stop` rollback. This is proof-owner evidence only; it proves externally submitted work can repeatedly drain through sandbox owners, not that ARM64 boot-default live routing is open.
- In virtio sandbox seeds, rollback/reset must be device-status based. Do not write `QueueNum=0` as a rollback cleanup; QEMU virtio-mmio can panic or wedge during full Wasm block write proof.
- ARM64 virtio-input full seed now receives injected pointer-event used-ring completion directly after legacy virtio-mmio queue programming was added. The ARM64 full Wasm traffic-window gate has passed with `block_read=12 block_write=12 gpu_control=12 net_rx=12 net_tx=12 input_events=12`. Independent proof-only device gates now pass with block `block_read=12 block_write=12`, GPU `gpu_control=12`, net `net_rx=12 net_tx=12`, and input `input_events=12`. `/virtio-wasm-live-cutover-rollback-{block,gpu,net,input}` records a diagnostic owner bit only after the matching 12-round gate passes, then clears it and proves fallback execution. `/virtio-wasm-live-routing-selftest` cuts over each device sequentially, runs four sandbox-owner dispatch ticks, rolls back, and requires final owner mask zero with zero dispatch failures. `/virtio-wasm-live-route-all-selftest` cuts over all four owners together, dispatches all four, rolls back all, and requires final owner mask zero with zero dispatch failures. `/virtio-wasm-live-boot-service-selftest` boots recovery, starts the explicit sandbox-owner service pump, runs four all-device service ticks, then stops through rollback-all and requires final owner mask zero with zero dispatch failures. `/virtio-wasm-live-service-loop-selftest` returns from the command handler and proves the ARM64 main idle loop itself drains four bounded request/event ticks before rollback-all. `/virtio-wasm-live-os-pump-selftest` proves the same main-loop path can route typed OS work with `block_read=4 block_write=4 gpu_control=4 net_rx=4 net_tx=4 input_events=4 failures=0` and `enqueued=24 drained=24` before rollback-all. The long-window typed pump uses `/vospl` and has passed with `block_read=16 block_write=16 gpu_control=16 net_rx=16 net_tx=16 input_events=16 failures=0` and `enqueued=96 drained=96`. `/vosrq` has passed with OS producer APIs prequeuing 24 typed work items, returning to the ARM64 main loop, draining through sandbox-active owners, and rolling back with zero failures. `/voslive` has passed with opt-in runtime mode, 24 producer-owned typed work items, pending-work main-loop pumping, exact counters, and rollback-all. `/vossys` has passed with subsystem-facing producer APIs prequeuing 16 rounds/96 typed work items, exact runtime counters, exact subsystem counters, and rollback-all. `/vosps` has passed with a persistent subsystem producer feeding one work round at a time for 32 rounds/192 typed work items from the ARM64 main loop, exact runtime counters, exact subsystem counters, zero subsystem runtime failures, and rollback-all. `/vosprod` has passed with a separate normal-subsystem inbox feeding eight rounds/48 typed work items into sandbox-active owners, exact runtime counters, exact subsystem counters, `enqueued=48 forwarded=48 rejected=0`, zero production queue failures, and rollback-all. `/vosprod-source` has passed with diagnostic auto-feed disabled and ARM64 recovery subsystem source ticks feeding `virtio_production_request_*` for 32 rounds/192 typed work items, exact source/runtime/subsystem counters, `enqueued=192 forwarded=192 rejected=0`, zero production queue failures, and rollback-all. `/vosprod-normal-source-live` and its soak now prove the non-recovery normal producer boundary through `VirtioArm64NormalDescriptorQueue` for 32/128 rounds with exact counters and rollback. `/vosprod-start` plus external `/vosprod-all` has passed with production runtime active after command return, eight submit batches, exact runtime counters `block_read=8 block_write=8 gpu_control=8 net_rx=8 net_tx=8 input_events=8 failures=0`, production inbox counters `enqueued=48 forwarded=48 rejected=0`, zero production queue failures, and `/vosstop` rollback. `/vosboot` has passed with 64 all-device production-inbox ticks, exact counters `block_read=64 block_write=64 gpu_control=64 net_rx=64 net_tx=64 input_events=64 failures=0`, production inbox counters `enqueued=384 forwarded=384 rejected=0`, and rollback-all. `/vosboot-block`, `/vosboot-gpu`, `/vosboot-net`, and `/vosboot-input` have each passed 64 device-specific production boot gate ticks. `/vosman` has passed with external `/vossall` submit commands feeding four typed work batches into an already-armed sandbox-active runtime, exact runtime/subsystem counters, and rollback-all. Logged warm-up misses are never counted. Block/GPU/net counted evidence remains strict; input may use bounded no-evidence retries because QMP/VNC event injection is asynchronous, but it must still collect 12 positive event completions and must fail closed if that target is not reached. The next step is replacing placeholder normal producer functions with real full-subsystem queue/event sources, then running longer real-workload soak windows before boot-default live routing.
- Do not put speech recognition, wake-word logic, route selection, codec policy, or multimodal prompt policy into the core.
- The x86_64 voice milestone should prove that spoken prompts can become editable transcripts without breaking keyboard, GUI, or recovery input.
- Default voice routing is `text-first`. Add direct-audio only through sandbox skills such as `skill_voice_router_policy_v1` and `skill_voice_audio_llm_bridge_v1`, and prefer `hybrid` when audio context is sometimes useful but bandwidth and auditability still matter.
- `/voice-test` now queues the route-aware sandbox chain. It does not yet prove native in-guest microphone capture.
- The long-term phone value is high because keyboard may be absent, but phone audio/display/touch stacks must still be capability-driven.

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

## 11. ARM64 Current Handoff Notes

- ARM64 is serial-first by design. A QEMU graphical window can show `Display output is not active` until a sandbox display/GPU skill owns a real framebuffer path; do not treat that as a recovery-core failure.
- The current ARM64 evidence baseline is `tools/run_arm64_virtio_gate.ps1 -SkipBuild -Quick`, `-OnlyLiveProductionRuntime -ProductionRuntimeBatches 32`, `-OnlyLiveProductionSubsystemSource`, `-OnlyLiveProductionSubsystemSourceLive`, `-OnlyLiveProductionSubsystemSourceLiveSoak`, `-OnlyLiveProductionRealSubsystemSourceLive`, `-OnlyLiveProductionRealSubsystemSourceLiveSoak`, `-OnlyLiveProductionNormalSubsystemSourceLive`, `-OnlyLiveProductionNormalSubsystemSourceLiveSoak`, `-OnlyLiveProductionNormalBootLive`, `-OnlyLiveProductionNormalBootLiveSoak`, `-OnlyLiveProductionRealSubsystemSourceDeviceLive`, `-OnlyLiveProductionRealSubsystemSourceDeviceLiveSoak`, `-OnlyLiveProductionDeviceBootLiveGates`, and `-OnlyLiveProductionBootLiveSoak`.
- The latest local all-device soak reached 128 production ticks with `block_read=128 block_write=128 gpu_control=128 net_rx=128 net_tx=128 input_events=128 failures=0`, `enqueued=768 forwarded=768 rejected=0`, and `production boot-live keepalive active=1`.
- Device-scoped boot-live was corrected after finding that it previously left all owners active. It now narrows to exactly one owner mask: block `0x1`, gpu `0x2`, net `0x4`, input `0x8`.
- Keep boot-default promotion disabled until real subsystem queues, persisted policy, and longer real workload soak pass. The current `/vosboot*` paths are opt-in proof gates, not production boot policy.
