# OpenRhiza Product and Architecture Direction

Status: new-development authority, 2026-10-03. This supersedes the earlier
incremental migration plan. Requirements are traced in `REQUIREMENTS.md`;
implementation stages and gates are in `ROADMAP.md`.

The 2026-10-03 Astra review is incorporated through `IMPLEMENTATION_GATES.md`.
Its K/N/W/SMP/D/A/Intent boundaries define required decisions and evidence at the
corresponding roadmap stages. They are not passed by writing these documents.
Initial findings and their disposition are preserved in `ASTRA_PLAN_REVIEW.md`
and `ASTRA_REVIEW_DISPOSITION.md`.

## Product decision

OpenRhiza is an independent, AI-native, general-purpose operating system that
can safely acquire, construct, run, replace and improve its capabilities.
Its foundation is a small independently audited kernel. All higher-level
behavior is packaged as sandbox modules. AI interprets intent and helps create
implementations; deterministic mechanisms outside the model enforce authority.

The product is not a Linux distribution, an agent running on another OS, a
cloud-dependent thin client, an in-kernel interpreter demo, or a collection of
generated panels that only appear to be working applications. Edge/IoT and
commercial sandbox services may be later uses, not a change of primary mission.

"From zero" means new first-party implementation, interfaces, build and tests.
Existing code remains a source of observations, not the new implementation base.
Standards and independently audited portable libraries are allowed. This is an
engineering restart, not a claim of legally independent clean-room provenance.

## Non-negotiable invariants

- OpenRhiza boots and executes without a Linux/BSD/Windows/Android kernel or
  third-party OS services underneath it.
- Only Rhiza Core is kernel code. Normal recovery UI, drivers, VFS/filesystems,
  protocols, Wasm runtime, compositor, applications, voice, model clients,
  supervisors, policies and module resolution are isolated modules.
- A module fault, infinite loop, resource flood or runtime defect cannot corrupt,
  panic, starve or reboot the core under the declared fault model.
- A module receives explicit object capabilities, not ambient filesystem,
  network, display, input, device, model, signing or administration authority.
- Installed functionality runs without mandatory model inference or registry
  access in its normal data path. Offline boot/recovery remain possible.
- Model output and all retrieved content are untrusted. Generation, evaluation,
  signing, deployment and local authorization are separate responsibilities.
- User agency is authoritative. Irreversible/destructive changes and private
  publication obey explicit policy/approval, even in autonomous modes.
- Every public claim requires repeatable artifact-specific behavior and failure
  evidence; labels, flags, votes and logs alone are not completion.

## Rhiza Core boundary

Core mechanisms are CPU privilege transitions, address spaces, protected stacks,
physical/virtual memory, timer preemption, thread/process lifecycle, capability
tables, bounded IPC, interrupt routing, and bounded device/DMA mediation.
Keep only minimal early diagnostics and a sealed recovery-launch mechanism in
addition to those mechanisms. No general command parser, device-specific protocol,
GUI object renderer, filesystem format, TLS client, model prompt or package
workflow belongs in kernel space.

The initial signed image supplies ordinary user-mode bootstrap/recovery modules
with explicit grants. Their authority and resource reservations are visible and
reviewable; packaging does not make their logic kernel code. The kernel can
reinstantiate the fixed approved recovery set if its supervisor fails. It does
not ask an LLM which recovery implementation to trust.

The practical trusted computing base includes firmware/boot verification,
kernel implementation, initial privileged modules, keys and build/release tools.
Do not hide those dependencies by describing only kernel source size.

Before kernel entry implementation, name the external image-trust root, verifier
and parser ownership, first user principal, authority ceiling and boot memory
ownership in the K contract. Authenticate authority-bearing configuration as well
as code before module admission. Diagnostic bring-up may use an explicitly
trusted-host image; it does not establish verified boot or physical security.
The bounded handoff must preserve resident image/page-table/firmware reservations
and specify the firmware-exit and post-exit console rules.

Recovery requires a resident dependency closure and reserved CPU, frames, object
slots, queues and diagnostics that ordinary grants cannot remove. Its initial
isolated console has no dependency on the normal supervisor, GUI, storage,
network or AI. Fixed-set relaunch has a restart limit and a defined degraded
outcome. A shared service's clients may receive bounded errors during recovery;
unrelated useful work and the reserved recovery path must continue. Ordinary
privileged-module crashes remain in the fault model; explicitly trusted
authorization/key-policy compromise is a separate assumption.

## Isolation and survival contract

Start with single-core protected user execution and preemption. Move Wasm
interpretation/JIT/AOT and host-function brokers into isolated user workers before
introducing generated device/service code. Native ELF modules also use protected
user processes and the same manifest, grants and lifecycle rules. Wasm is a
portable execution format, not the only sandbox or a replacement for hardware
isolation. A widget need not be a process; effects remain object-scoped within
an application, while independent privilege/failure domains are separate workers.

Kernel admission checks use bounded sizes, checked arithmetic, fallible resource
allocation, generation-tagged handles, revocation, queue limits and copy-in/out.
No module callbacks execute while kernel locks or IRQ-critical sections are held.
Fuel alone does not bound a blocking host call; preemption, deadlines and bounded
IPC enforce termination/progress outside cooperative module behavior.

Every resource has a payer and a limit, including kernel metadata, delegated
service work, shared grants and cleanup. Debit before publication and retain
cleanup budgets after client death. Logical revocation stops new access;
physical reclamation waits for admitted work, references and required CPU/DMA
invalidation. Specify handle retirement and revoke completion rather than
assuming a generation tag alone makes reuse safe.

The native syscall/lifecycle contract defines bounded work, immutable request
snapshots, user-copy faults, queue errors, capability transfer, deadlines, peer
death and post-commit cancellation. Prefer bounded asynchronous request/reply;
any synchronous path must establish a priority/budget-inversion bound. Native
entry/return and every enabled per-thread architectural state are tested. Keep
the ABI experimental until its implemented subset passes round-trip/fault tests.

DMA-capable drivers need enforceable device isolation and reset/quiesce contracts.
Generic device grants remain denied until D qualifies the exact requester,
translation, MMIO, interrupt, timer and reset profile. The specifically qualified
initial non-DMA console has its own narrow authority contract. An IOMMU-present
flag does not establish containment, and host isolation in QEMU does not protect
the guest core from an unrestricted emulated DMA device. Failed reset/drain
quarantines unproven pages within a fixed budget rather than reusing them.
Unqualified generated hardware experiments remain explicitly trusted-driver
experiments in disposable emulators, not hostile-driver containment evidence.
Physical faults, firmware defects and hardware outside the declared assumptions
are not covered by an absolute no-crash claim; malicious use of an admitted,
conforming device is within the selected device fault model.

Survival evidence requires unchanged boot identity, independent witness progress,
bounded recovery response, and reclaimed resources after faults. A watchdog
reboot is recovery evidence and fails a same-core survival gate. Formal/model
checks, fuzzing, fault injection and soak tests supplement one another; passing
tests alone does not prove the absence of all kernel defects.

Before each gate, freeze profile-specific limits, latency/reclamation deadlines,
repetitions and permitted resource slack. Independent, protected challenge
channels and host-monotonic deadlines verify useful work and boot/reset identity.
Candidate-written success logs cannot certify a pass. Broken negative controls
must demonstrate that the oracle rejects stopped work, resets and corruption.

## Local capability lifecycle

```text
user intent + local state
  -> approved local cache -> registry compatibility/evidence lookup
  -> reuse OR bounded AI source-generation job
  -> pinned isolated build + independent analysis/contract/adversarial tests
  -> approved immutable digest + signed manifest + release channel
  -> local policy + capability/resource admission
  -> sandbox canary + device/service quiesce + versioned state transfer
  -> health verification + atomic active-binding commit
  -> persist preference OR revoke/quiesce/rollback
  -> consented redacted evaluation for the exact artifact and platform
```

Package identity is not a file slot or GUI number. Define schema/version, artifact
digest, execution profile, interfaces, dependency digests, platform/device matches,
requested capabilities, budgets, state schema, rollback targets, license,
provenance, independent test evidence, signer, channel and revocation state.
Freeze only the initial boot envelope and bounded fixtures at K. Broader module
manifest and native ABI subsets remain experimental/versioned until their valid
and invalid operational round trips pass; reject unknown authority-bearing fields.
A general-purpose IDL toolchain is not a first-boot requirement.

Live state and next-boot preferred state are separate transactions. Data migration
and irreversible device effects require explicit compatibility/rollback policies.
Core updates follow independently reviewed signed system-image updates; no AI
hot-patching of kernel memory is a normal capability.

Define operation epochs, the active-binding commit point and allowed interrupted
states before real cutover. Fence/drain the old owner before granting a new writer.
Initially allow compatible schemas or explicit backup/restore, rejecting unsafe
migrations. Durable file and preference acknowledgment has an explicit block/cache
fault model and host-held expected-state oracle; reboot alone does not test power
loss. Minimal trust-state recovery precedes Stage 3 admission; system-image A/B
certification and operational release drills remain at Stage 6.

Installed artifacts keep their last accepted local authority unless locally
revoked. Stale/unknown metadata freshness blocks new admission or privilege
expansion while preserving authorized installed operation and sealed recovery.
Offline nodes cannot know revocations they have not received. Separate security
epochs from payload versions so authorized rollback cannot replay old trust.
Specify trusted wall-clock bootstrap and entropy failure behavior before TLS or
key generation; core monotonic deadlines do not depend on either. Hostile-disk
rollback resistance needs an independent anchor or an explicit scope exclusion.

## Input and user experience

The default session is a useful command terminal, not a marketing screen. Exact
registered commands execute locally through typed actions. Unknown, ambiguous
or multi-step requests go to a replaceable model planner. Its typed plan passes
deterministic authorization, confirmation where required, cancellation, audit
and undo/transaction boundaries before any service acts.

The local typed command/action subset begins with the Stage 2 terminal; Stage 5
adds model planning to that same authority mechanism. Effects bind principal,
capability, object generation, arguments, policy epoch, approval scope, deadline
and transaction ID, revalidated at commit or preserved by a reservation. Trusted
confirmation displays validated effects and cannot be supplied by model prose,
a package, a cached workflow or a stale semantic permission decision.

Keyboard and pointer events, including focus and pointer capture, are normalized
by input modules. Text services own UTF-8, Korean/English rendering, IME,
composition, selection, wrapping and accessible text. They are not kernel policy.
Voice adds a visible opt-in capture/VAD/ASR/transcript-confirmation path to the same
intent router. Default off/push-to-talk comes before always-listen or direct
multimodal audio. Background agents cannot enable a microphone themselves.

A native desktop runs independent applications with surfaces, focus, move/resize,
close/minimize/maximize, workspaces, clipboard, notifications and task switching.
MDI/document windows and background jobs are real client state. Foreground QoS
must preserve recovery and other admitted service budgets rather than suspend
all work to improve a game's frame counter. AI-editable GUI scenes use bounded
object actions above the compositor, not arbitrary kernel rendering imports.

Initial apps are terminal/LLM, files, settings, monitor and module manager. A real
Doom/FreeDoom module tests installation, focused keyboard/mouse, game timing,
pixels, sound, durable saves, backgrounding, offline relaunch and crash isolation.
No game-specific kernel ABI, WAD parser, game loop or persistence shim is allowed.

## Nexus and module forge

OpenRhiza.com is shared operational knowledge, not just a download bucket.
Unify module families (drivers, services, apps, skills, compatibility runtimes,
models/assets) with workflows, policies, nodes, hardware/platform records,
artifact-specific evaluations, comments, votes, provenance and release history.
Human boards and machine APIs use the same model with public/private projections.

Separate registry, model gateway, forge workers, independent validators, approval
and release signing. Authenticate authors/nodes and verify proof of key possession;
TPM trust requires verified attestation. Contributions begin quarantined, not
active. Apply schemas, sizes, quotas, deadlines, abuse controls and least privilege.
Votes and LLM analyses help discover/prioritize candidates; they never replace
mandatory tests, authority, compatibility or revocation checks.

The cloud service may use ordinary server infrastructure during bootstrap. That
is not an OS dependency of the OpenRhiza guest. A future OpenRhiza-hosted service
is a self-hosting milestone, not a prerequisite to a usable independent OS.
Model-provider choice remains replaceable and user-controlled; neither Gemini
nor any specific model is the kernel architecture. Never embed user API keys in
kernel images or public packages. Secret access is a granted module capability.

## Private knowledge and autonomy

Managed files remain the source of truth. A rebuildable semantic sidecar indexes
content identity/hash, chunks, summaries, entities, relationships and freshness.
Queries respect the invoking user's permissions and return small references;
loading raw content or uploading it is a separate authorized operation. File
events drive bounded refresh. The graph is not a mandatory new filesystem.

Off/assist/council modes remain user-controlled. Independent proposals, evidence,
disagreement and bounded jobs improve useful planning; majority voting does not
grant authority or establish correctness. Timers, model failures and resource
limits cannot disable the user's session. Diagnostics, firewall and forensics
become real capability-backed applications; inspecting an authorized process dump
is different from unrestricted kernel-memory patching.

## Scope and long-term expansion

The first supported environment is one pinned x86_64 QEMU profile. Prefer UEFI,
VirtIO devices and software composition; qualify firmware/toolchain/device choices
in Stage 1 before freezing them. A native writable filesystem needs durability and
power-loss gates before storing user data; FAT transfer/boot and common foreign
filesystem import modules have different roles.

Self-hosted SDK/compiler/build/testing, optional local inference/acceleration,
native browser/research tools, broad filesystem interoperability, one qualified
real PC, ARM64 QEMU/board, RISC-V, a specific unlocked phone, Wi-Fi/Bluetooth,
USB tethering, vendor graphics and foreign program/Wayland compatibility remain
part of the roadmap. Each is a module/capability project with separate evidence.
Do not make universal hardware/program compatibility an alpha claim.

Public release means the declared scope passes isolation, functionality,
durability, supply-chain, recovery, privacy and long-duration tests. Open source
availability and a working demonstration are not equivalent to a safe public OS.
