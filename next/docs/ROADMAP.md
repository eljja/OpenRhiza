# OpenRhiza New Development Roadmap

This plan starts with no inherited runtime implementation or passed milestones.
The old kernel/Nexus/proofs remain references. `DIRECTION.md` is normative;
requirement IDs come from `REQUIREMENTS.md`.

The Astra review amendments are incorporated in `IMPLEMENTATION_GATES.md`.
Gate names below refer to that document; no gate has passed. Kernel code will
be implemented with the explicitly requested `gpt-6-astra` model and validated
through build, QEMU, fault injection and repair, with results reported per gate.

## Reset checkpoint R0

R0 establishes source inventory, consolidated intent, architecture decisions,
legacy lessons, development boundary and new test sequence. No new kernel image,
process isolation, module loader, wire ABI or public release is implemented here.
The old QEMU smoke is reported separately and does not advance new stages.

New work stays in `next/`. Do not move/delete old source or reverse local edits.
Do not import parent Cargo patches, host-embedded keys or old binary fixtures.
Kernel, modules, SDK, registry/forge, schemas and tests get new ownership and
independent builds. Independently audited standards/portable dependencies are
selected explicitly with licenses, pins and provenance.

## Six stages for the qualified profile

Keep the stage order for the declared profile. Stage 2.3 is a separate SMP
qualification: a qualified single-vCPU profile may proceed to Stage 3 after
2.1-2.2, while SMP remains explicitly unimplemented/unqualified until 2.3 passes.
This does not permit enabling untested SMP or claiming the full SMP stage complete.

### Stage 1 New boot and reproducible substrate

Requirements: R01-R03, R14, R24-R25.

1.1: close K with short reviewed ADRs and fixtures: one x86_64 QEMU/firmware/
toolchain profile; boot trust/parser and bounded memory handoff; first-principal
authority; resident recovery dependency/reservation sketch; resource ownership;
draft native ABI/error shapes; default device denial; independent harness and
initial deadlines. Qualify a minimal isolated non-DMA recovery console contract.
Record credentials/test-fixture rules. No reused patched bootloader or inherited
parent Cargo/toolchain configuration. Freeze the qualified boot envelope and
initial fixtures; keep broader manifests/native ABI experimental until their
implemented subsets pass round-trip tests. Stage 1.1 has not yet passed K.

1.2: create a new standalone workspace and image builder; add fresh kernel entry,
early diagnostics, memory-map validation, IDT/fault-stack foundation, timer and
bounded diagnostic records. Document every unsafe boundary. No feature code.
Begin only after the K choices and entry review are recorded.

1.3: build from a clean checkout and boot three times through a new QEMU harness.
Capture tool/firmware versions, source/image hashes, command line, boot identity,
serial logs, timeout/exit status and a deliberate kernel-panic diagnostic. A panic
test proves diagnostic/recovery behavior only, never same-core survival. Use an
external boot/reset oracle and a predeclared host timeout; the initial calibration
candidate is 30 seconds per fresh boot, to be qualified before acceptance.

Gate: reproducible fresh image, honest failure capture and dependency/boundary
audit. Stage 2 cannot start merely because the legacy image boots.

### Stage 2 Protected execution and module survival

Requirements: R02-R03, R08, R13-R14, R25.

2.1: implement N on one CPU: owned page allocation, protected address spaces and
stacks, native ELF entry/return, preemption, bounded syscalls/IPC and principal
handles. Specify resource payers/limits, revocation/reclaim completion, request
snapshots, peer death, cancellation and inversion bounds before hostile execution.
Add the authenticated fixed initial/recovery set, protected reservations and
isolated logger/terminal/supervisor with a small deterministic command registry.

N gate: four isolated processes complete independently challenged useful work.
Bad pointers/instructions, divide errors, stack overflow, busy loops, metadata
and heap exhaustion, handle/IPC races and normal-supervisor crashes preserve
boot identity, unrelated work and recovery. Predeclare numeric deadlines,
L/L+1 admission behavior, ledger baselines and repeated reclamation checks;
negative controls must make the harness fail. No unqualified device grants.

2.2: close W by pinning and qualifying one isolated Wasm worker/broker profile
after N. Test runtime/host-call failure, quota exhaustion and preemption through
the production worker path; desktop default runtime features are not assumed.

2.3: qualify SMP separately before enabling it: two-vCPU concurrent ownership,
TLB shootdown/acknowledgment, interrupts, reclaim/reuse and architectural-state
tests. Report the 2.1-2.2 single-vCPU gate and pending SMP gate separately.
Watchdog resets fail containment in either profile.

### Stage 3 Real services and secure module delivery

Requirements: R04, R10-R15, R24-R25.

3.1: close D before generic raw device grants: exact DMA/requester/domain/MMIO/
interrupt isolation, core timer independence, reset/drain acknowledgment and
bounded quarantine. Inject forbidden addresses, IRQ floods and late I/O during
driver death/reuse. Lack of enforceable isolation blocks hostile-driver claims.
Then introduce restartable device-manager and isolated VirtIO block/net modules.

3.2: implement A's minimum trust, time and entropy contracts before their network
consumers. Fixed-image trust does not depend on a wall clock; dynamic enrollment,
TLS and key generation require the selected trust/failure policies. Specify
offline installed authority, stale metadata, revocation, permitted rollback and
persisted security epochs. Add DHCP/DNS/TLS/socket and resolver/cache modules.
No kernel format parsers or registry orchestration.

3.3: add VFS and a small durable writable filesystem with real files/directories/
rename/flush/recovery. Define lifecycle epochs, fencing, commit points, compatible
state transfer and allowed interrupted states before writable user data or real
cutover. Test lost/reordered/torn writes under the selected block/cache assumptions
against a host-held data/trust oracle; reboot by itself is insufficient.

Build a fresh minimal Nexus slice around shared schemas: authenticated enrollment,
immutable artifact/digest/manifest/signature, channels, ownership and revocation.
Use synthetic independently tested packages first; no untrusted generation yet.
Local admission enforces policy/grants/budgets even if the registry is compromised.
External enrollment/install remains disabled until minimum trust-state persistence
and recovery are qualified in 3.3; network protocol development alone does not
authorize external package admission.

Gate: real file content survives reboot/power-loss injection; bad TLS peers,
unsigned/corrupt/revoked/replayed packages fail closed; signed module download,
offline cache relaunch and actual service/device cutover work. Driver crashes
revoke resources/reset devices or enter recovery without core corruption.

### Stage 4 Multitasking desktop and native SDK

Requirements: R05, R07-R08, R17, R19-R20, R25.

User-mode display/input/text modules, shared surfaces/damage, native compositor,
focus/pointer capture, window/document lifecycle, workspaces, launcher, clipboard,
notifications and accessible text. Deliver Korean/English font/IME/composition
as services, not kernel logic. Build native SDK and independent terminal, files,
settings, monitor and module manager processes with real data/background jobs.

Gate: at least five concurrent apps; synthetic input and pixel/state tests prove
focus, drag/resize, minimize/restore, MDI/workspace switches and close. Application,
compositor and display faults preserve recovery and other budgets. Eight-hour
soak measures input latency, memory, queues and resources; labels alone fail.
Predeclare profile-specific latency/resource-growth limits before the gate.

### Stage 5 AI operations and complete application demo

Requirements: R04, R06-R07, R09-R10, R12, R16-R18, R20-R21, R25.

Provider-neutral bounded model jobs and protected secrets extend the existing
local action registry with typed plans, deterministic policy/approval,
transactions, cancellation, audit and undo. Close Intent: bind approved effects
to principal/object generation/arguments/policy epoch and revalidate at commit;
test replacement, revocation, replay and forged confirmation. Add quarantined
forge workers with pinned build environments and tests the
generator cannot modify, independently authorized signing and local canaries.

Add permission-aware semantic graph/query/refresh, opt-in assist/council and
visible push-to-talk audio/VAD/ASR/transcript modules. Direct audio and always-listen
remain later opt-ins. Bring SDK/build capabilities into the OS incrementally;
external forge tools are marked development dependencies until self-hosted gates.

Install Doom engine and separately licensed FreeDoom content from the registry.
Test 35 Hz simulation, focused keyboard/mouse, nonblank pixels, sound, save/load
after reboot, background pause/throttle, offline launch, uninstall and containment.
Optimize via shared buffers/batching or isolated AOT/native workers; never add
game branches to core or starve network/recovery to improve a frame metric.

Gate: a generated nonprivileged module passes independent negative tests and
canary/rollback; injected documents/tool/model output cannot gain authority;
voice denial/cancellation/privacy and semantic permissions hold; full game gate
passes against predeclared game-clock, audio and responsiveness tolerances.
Offline installed apps and direct commands remain usable.

### Stage 6 Qualified public system and ecosystem

Requirements: R10-R12, R14-R15, R22-R25.

Signed system/module releases, provenance/SBOM/licenses, key rotation/revocation,
A/B updates and data migration/backup/restore. Qualify contributor/admin/signer
roles, evidence ingestion, abuse/cost controls, moderation, public/private
projections, deployment/database backup/rollback and security reporting.

Run fuzz/adversarial review, multi-day soak, interrupted installs/updates and
repeated upgrades/rollback. Qualify one explicitly named physical PC if claimed;
otherwise release only the tested QEMU scope. Publish exact supported features,
limits, evidence and recovery instructions, not universal compatibility.

Gate: two signed upgrades and rollback, revoked/downgraded/corrupt artifact
rejection, durable interrupted-update recovery, evidence-bound public registry
and no critical unresolved security or silent corruption issue in declared scope.

## Expansion programs preserved

These remain product goals, not prerequisites for calling an early QEMU milestone
complete: broader foreign filesystems, full self-hosted Rust/C toolchain, browser
and research workflows, local models/GPU/NPU acceleration, Wi-Fi/Bluetooth/USB
tethering, laptop power/suspend, accelerated graphics, ARM64 QEMU and a qualified
board, RISC-V, one unlocked phone, foreign program runtimes and Wayland/KDE/GNOME
compatibility. Each receives its own module contracts and acceptance matrix.

## Reporting rules

At each prompt start: new checkpoint/stage/substage, existing dirty-worktree
boundary, last **new** build/QEMU evidence, objective and gate. At completion:
changed files, test commands/results, exact QEMU profile/logs, limitations and
whether the gate permits progression. Keep old-reference evidence separate.
Report the qualified vCPU/device profile and any deferred SMP/device gate.

Do not cross a stage gate without new-runtime QEMU evidence. Documentation or
host-only tests can complete a preparatory subtask, not an OS stage. Failed gates
remain failed; preserve diagnostic evidence and narrow the cause before moving.

Immediate next task: Stage 1.1, closing K with explicit reviewed selections and
fixtures. Only then begin Stage 1.2 kernel implementation with Astra. Plan-review
integration does not itself select those contracts or complete a runtime gate.
