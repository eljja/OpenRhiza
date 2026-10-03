# Astra Design-Readiness Review

Date: 2026-10-03 (Asia/Seoul).
Reviewer: actual `gpt-6-astra` agent, explicitly requested by the user.
Reviewer identity was checked against the current local session's
`turn_context.model = gpt-6-astra` metadata (2026-10-02T23:45:23.719Z,
effort `max`); it is not inferred just from the requested role.

Scope: document-review evidence at reset R0, before new kernel implementation.
Read all of `next/README.md`, `DIRECTION.md`, `REQUIREMENTS.md`,
`LEGACY_REVIEW.md`, `ROADMAP.md`, and the exact supporting metadata verifier.
Consulted the primary specifications linked below for narrow technical questions.
No other agents, new runtime code, source relocation, builds, deployments,
commits, paid external model calls, or messages to another chat were used.

Proposed disposition: **CONDITIONAL**. Proceed with Stage 1.1 qualification and
contract work; do not declare it complete or start Stage 1.2 on this baseline
alone. The direction is coherent, but not yet an implementation-entry contract.
Later hostile-execution, device and release prerequisites must not all be made
prerequisites to the first diagnostic boot.

Integration status: **PENDING**, for the parent to incorporate into canonical docs.
Re-review status: **PENDING**, after those changes and the Stage 1.1 decisions.
No new-OS stage is passed by this review. Future kernel implementation remains
assigned to Astra by the user's request; this report neither starts that work nor
changes another chat's model selection.

## Findings

These are concrete gaps in a design contract, not observed defects in nonexistent
new kernel code. Failure scenarios are reviewer inferences from the cited plan.
P1 means resolve before the indicated trust boundary; P2 means a bounded
sequencing/contract correction. No P0 or P3 is asserted. In particular, there is
no evidence here of an unfixable architectural contradiction or a safe runtime.

| ID | Priority | Finding | Binding deadline |
| --- | --- | --- | --- |
| F01 | P1 | Bootstrap trust and ownership have no selected handoff contract | Stage 1.1, before 1.2; admission enforcement before first initial-module launch |
| F02 | P1 | Reserved recovery is not yet an independent, enforceable dependency path | Minimal dependency/authority design in 1.1; survival evidence before hostile Stage 2 workloads |
| F03 | P1 | Quotas and generation tags do not define accounting or revocation completion | Ownership principles in 1.1; exact state machine before hostile Stage 2 execution; SMP extension before enabling SMP |
| F04 | P1 | IPC, syscall and process semantics leave blocking, races and partial effects undecided | Draft minimal ABI in 1.1; executable semantics before hostile Stage 2 execution |
| F05 | P1 | No mandatory device-isolation gate precedes raw DMA/MMIO/IRQ authority | Profile decision in 1.1; enforcement and fault evidence before first driver device grant in Stage 3 |
| F06 | P1 | Offline admission, freshness, revocation and permitted rollback are not reconciled | Fixed-image trust in Stage 2; operational contract before Stage 3 network enrollment/install |
| F07 | P1 | Atomic binding and durable state are names, not a crash/concurrency protocol | Before Stage 3 writable user data and real module/device cutover |
| F08 | P1 | Progress, recovery and oracle independence lack falsifiable thresholds | Minimal harness contract in 1.1; numeric thresholds before each corresponding gate |
| F09 | P2 | Typed authority substrate is needed before the Stage 5 planner | Local command/authorization subset in Stage 2; full intent/approval contract before Stage 5 effects |
| F10 | P2 | Early freeze and coarse sequential stages risk locking untested assumptions | Correct stage wording in 1.1; qualify runtime profile before its Stage 2 integration |

### F01 - Bootstrap trust and memory ownership need a concrete handoff

Evidence: [DIRECTION.md:47](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:47),
[DIRECTION.md:55](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:55),
[ROADMAP.md:26](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:26),
[ROADMAP.md:157](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:157).
The plan correctly requires an audited boot contract and recognizes the broader
TCB, but does not yet choose who authenticates what, who creates first authority,
or which boot allocations remain owned after entry.

Failure scenario and consequence: an image signed by a key that is itself
replaceable with that image supplies an apparently valid recovery module; or
the allocator reuses pages still containing the initial module set, page tables
or firmware runtime data. The first makes recovery authenticity circular; the
second defeats the core's ownership foundation before user isolation exists.

Bounded fix: one boot/TCB ADR, not a general package manager in kernel. Specify
the trusted QEMU host/firmware/loader assumption, external root of image trust,
the verified bytes including grants/configuration, the first user principal and
its authority ceiling, and which loader/verifier/parser components are in the
TCB. Define a versioned handoff with bounded lengths, checked nonoverlapping
regions, reservations, entry state, stack/page-table ownership and the immutable
resident initial-image descriptor. Choose who parses the initial ELF/image;
later untrusted ELF loading must not silently extend this trusted parser surface.

For UEFI, qualification must cover the memory-map/ExitBootServices transition,
not continued use of firmware boot-console services after it. UEFI requires the
current map key and prohibits further boot-service calls after successful exit.
This is a concrete constraint on the proposed handoff, not a demand to implement
a new firmware. [UEFI 2.10, sections 7.2.3 and 7.4.6](https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html?highlight=exitbootservice)

Stage/declaration: settle the ADR before 1.2. A trusted-host development image
with explicit lack of verified-boot/physical-attacker claims is acceptable for
bring-up. Installed EDK2 is not itself evidence of image authentication. Before
launching Stage 2 modules, test malformed/overlapping handoffs and altered
image/grant bytes against the chosen verifier. Neither TPM enrollment nor the
full Stage 6 release service is a prerequisite to a diagnostic boot.

### F02 - Recovery reservations need a dependency graph and authority ceiling

Evidence: [DIRECTION.md:29](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:29),
[DIRECTION.md:55](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:55),
[DIRECTION.md:87](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:87),
[ROADMAP.md:49](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:49).
Reserved budgets and sealed relaunch are sound, but the recovery set, dependency
closure, restart limit and non-revocable minimum authority are not specified.

Failure scenario and consequence: the failed supervisor owns the only console
endpoint or allocator quota; recovery's executable is reachable only through a
failed filesystem; or repeated crashes consume the process/handle pool while
relaunch loops. The core can keep ticking while no user can recover. Suspending
every application to make recovery appear responsive would also miss the stated
continued-progress requirement.

Bounded fix: list a small resident recovery dependency set and reserve its CPU,
frames, kernel object slots, queues and diagnostic capacity at admission time.
For the initial QEMU profile, select an independent serial input/output path in
user modules; do not depend on the future GUI, network, model, mutable cache or
normal supervisor to launch it. Keep only fixed-set relaunch, reservation and
capability-ceiling mechanisms in core. Define a bounded restart/backoff policy
and an exhausted-recovery fallback with an explicit degraded result, not an
infinite relaunch loop or successful-survival label after a reboot.

Distinguish three assumptions: faulty/untrusted ordinary modules; privileged
modules with tightly limited authority; and trusted admission/key-policy logic.
An admission-module compromise may invalidate authorization claims, but ordinary
supervisor or driver crashes must not be excluded merely by calling every
privileged module trusted. State which maximum grants cannot remove core or
recovery reservations. Define an affected application's dependencies; a failed
shared service may return bounded errors, while unrelated computation and the
recovery path must continue. Do not promise availability of the failed service
itself throughout its replacement.

Stage/test: write the dependency and authority sketch in 1.1; before the hostile
Stage 2 gate, saturate ordinary pools and crash the supervisor and its normal
logger/console path. Independently exercise recovery commands and unrelated
work, with the F08 deadlines and unchanged boot identity. Re-run with failed
storage/compositor dependencies when those are introduced.

### F03 - Define who pays, what revocation stops, and when memory is reusable

Evidence: [DIRECTION.md:75](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:75),
[ROADMAP.md:47](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:47),
[ROADMAP.md:54](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:54).
Generation-tagged handles, quotas and reclamation are listed without their
ownership and completion semantics.

Failure scenario and consequence: a small user allocation creates large kernel
mapping/handle/IPC metadata; a dying process leaves shared grants charged to
nobody; or a revoked frame is returned to another principal while an in-flight
operation or stale CPU translation still uses it. Single-core preemption and
interrupts already require decisions about such interleavings; postponing all
of them to SMP is not sufficient.

Bounded fix: make a compact resource ledger part of the Stage 2 contract. Name
the payer and hard limit for committed pages, page tables, protected stacks,
processes/threads, handles, queued bytes/capabilities, shared/pinned buffers,
fault/log records and cleanup work. Count descendants and service work against
an explicit sponsor or server budget; a client must not escape limits by
offloading unlimited work. Debit before publication, make allocation failure
ordinary, and preserve global/recovery reserves. Shared ownership must retain
a payer after exit; zero cross-principal reused pages before exposing them.

Specify create/lookup/transfer/revoke/close/destroy linearization points. Revoke
must invalidate delegated aliases as defined by the grant model, stop new uses,
and separately drain or cancel admitted operations. Logical revocation is not
physical reclamation. Choose a bounded asynchronous reclaim protocol, generation
wrap/slot-retirement rule, and completion token. Do not free until references,
CPU access and required translation invalidations have completed; add the DMA
conditions from F05 for device pages. Cleanup must retain an execution budget
after the killed client's budget has gone to zero.

Stage/test: name ownership principles in 1.1, finalize the ledger/state machine
before hostile native tests in Stage 2. Race grant transfer, lookup, death and
reuse; exhaust each metadata pool, not just user heap; check stale handles,
cross-principal data and the ledger after repeated cycles. Add cross-CPU
shootdown/acknowledgment and concurrent reuse tests before enabling SMP.

### F04 - Bounded IPC is not yet a syscall or lifecycle specification

Evidence: [DIRECTION.md:76](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:76),
[ROADMAP.md:27](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:27),
[ROADMAP.md:48](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:48).
The no-callback-under-lock rule is useful, but does not settle blocking, priority
inversion, cancellation or user-memory races.

Failure scenario and consequence: a high-priority recovery client waits for a
low-priority server starved by medium-priority work; a caller edits a shared
request after its rights/length are checked; or a timeout retries a write that
already committed. A size-bounded message does not prevent any of those outcomes.

Bounded fix: a small operation table must state validated input/output shapes,
maximum kernel work, errors, blocking/deadline behavior, cancellation point,
capability-transfer ownership and effects on peer death. Define full-queue and
partial-copy behavior without partial authority transfer. Use one immutable
request snapshot for authorization/effect selection; pin or mediate data buffers
with explicit concurrent-mutation rules. User-copy faults must become process
errors, not an uncontrolled kernel fault path. Define spawn/start/exit/kill/wait
and restart epochs, including how waiters and outstanding replies are released.

For the first implementation, bounded asynchronous request/reply with explicit
request IDs is a defensible small choice. If synchronous call chains are chosen,
specify bounded priority/budget donation or another demonstrable inversion bound,
including cycles and death; do not add a complex real-time framework by default.
No wait or unbounded revoke walk may hold a core/IRQ-critical lock. Cancellation
after effect commit must return an explicit completed/unknown outcome, not claim
that arbitrary effects were undone.

Native isolation also needs a selected CPU/ELF profile: user entry/return state,
page permissions, privileged I/O restrictions, and preservation of all enabled
per-thread architectural state, including FP/SIMD where enabled. A ring-3 label
alone is not the corresponding test oracle.

Stage/test: minimal shapes/version/error conventions in 1.1; complete the first
implemented operations before hostile Stage 2 execution. Test priority inversion,
nested waits, peer death, full queues, faulting copies, double replies, late
cancellation, concurrent request mutation and two native workers using distinct
register/memory state. Preserve versioning rather than freezing future syscalls.

### F05 - Device authority needs a fail-closed gate before user-mode drivers

Evidence: [DIRECTION.md:81](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:81),
[DIRECTION.md:185](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:185),
[ROADMAP.md:64](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:64),
[ROADMAP.md:76](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:76).
The risk is recognized, but "where supported" is not an admission rule and no
explicit isolation gate precedes the Stage 3 driver grant.

Failure scenario and consequence: a protected driver programs a descriptor for
core memory, leaves DMA running after its death, or floods interrupts that delay
the core timer. This can defeat same-core survival even inside a disposable VM.
Host isolation and intra-guest isolation are different claims. VirtIO's access-
platform feature and actual platform translation must be qualified: its absence
permits physical addresses rather than establishing restricted guest DMA.
[VirtIO 1.2, section 6](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)

Bounded fix: qualify one complete requester/domain/MMIO/interrupt profile and
deny raw device access unless it enforces the declared threat model. Reserve
IOMMU tables, routing and interrupt-control authority from untrusted drivers;
keep device protocols/reset policy in isolated modules. Pin exact device feature
negotiation and bus-master enable ordering. If the profile lacks enforceable
isolation, continue CPU-only work or clearly labeled trusted-driver experiments,
but do not pass hostile-driver containment. Bounce buffers alone do not restrict
what an unrestricted descriptor/MMIO writer can ask the device to access.

Define revocation order: fence new submissions and MMIO authority, stop/mask the
source as appropriate, remove DMA mappings, complete applicable invalidations
and outstanding-access barriers, establish device reset/quiescence, then reuse
memory or rebind. The exact sequence must match the selected transport, not this
generic list. VirtIO reset completion has a device-status acknowledgment and a
post-reset queue/notification guarantee; merely writing reset is not the gate.
[VirtIO 1.2, section 2.4](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)

Interrupt source validation/remapping and translated-access draining are distinct
mechanisms in VT-d. The contract must identify which are implemented by the
selected profile, rather than treating an IOMMU-present flag as sufficient.
[Intel VT-d, chapter 5](https://cdrdv2-public.intel.com/831418/vt-directed-io-spec.pdf),
[Intel VT-d, translation invalidation/drain definitions](https://cdrdv2-public.intel.com/671081/vt-directed-io-spec.pdf)

On timeout, quarantine the device and any memory not proven safe to reuse within
a fixed reservation; report degraded service, not complete reclamation. Define
per-source interrupt work/rate bounds, overflow/masking and re-enable rules, and
a core-owned preemption timer independent of driver cooperation. Exclude actual
hardware nonconformance explicitly, not ordinary malicious programming of a
conforming device.

Stage/test: decide required capabilities in 1.1 without demanding IOMMU code for
first boot. Add a Stage 3 entry subgate before device grants: forbidden DMA and
MMIO attempts, interrupt storms, in-flight DMA during driver death, failed reset,
late completions and buffer reuse must preserve core/unrelated witness progress.
Physical hardware remains a separate later qualification.

### F06 - Separate fixed-image trust from online freshness and offline operation

Evidence: [DIRECTION.md:36](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:36),
[DIRECTION.md:108](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:108),
[ROADMAP.md:49](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:49),
[ROADMAP.md:65](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:65),
[ROADMAP.md:74](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:74),
[ROADMAP.md:123](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:123).
Stage 3 already requires revoked/replayed package rejection, while key rotation,
rollback and related release machinery are grouped in Stage 6. Clock/entropy
modules are named, but their dependency and failure policies are not specified.

Failure scenario and consequence: an offline machine either accepts stale
authorization as current, or expiry disables all installed applications and
recovery. A valid old signature can also be confused with an authorized recovery
rollback. Restoring all locally persisted version counters with an old disk
snapshot can defeat an anti-rollback claim that has no independent anchor.

Bounded fix: Stage 2 can verify a fixed image against a pinned external root;
that alone does not require on-device key generation or a wall-clock service.
Separate core monotonic deadlines from wall-clock trust and entropy quality.
Before Stage 3 TLS/enrollment, choose the trusted time/bootstrap policy and an
entropy source with an unavailable/failure state; prevent key generation and
security-sensitive sessions from silently using fixture seeds or invalid time.
Keep fixture keys visibly separate from production authority.

Use an audited update-verification design, with a precise signed-byte encoding,
domain/schema separation and bounded parsing. TUF is a primary reference for
out-of-band root trust, persisted versions and time-based freshness checks; it is
not a mandate to put its complete implementation in core.
[TUF specification 1.0.36, sections 5.1-5.6](https://theupdateframework.github.io/specification/v1.0.36/)

Proposed offline contract: installed artifacts run under the last accepted local
authorization unless already locally revoked; stale/unknown freshness prevents
new admission or privilege expansion, not an automatic loss of the sealed
recovery path. An offline node cannot know a revocation it has not received;
state that limit explicitly. Define how new signed metadata/removable media
updates local knowledge. Keep an authorization epoch/security floor separate
from payload version: an explicitly authorized compatible older payload is not
the same event as replaying older trust metadata. Full hostile-disk rollback
resistance needs an independent freshness anchor or an explicit exclusion from
the initial QEMU threat model; a TPM is not required merely to begin development.

Stage/test: fix the image root before Stage 2 admission; bring the minimal trust
state/rollback rules into Stage 3, leaving operational rotation drills and system
A/B release qualification in Stage 6. Test no clock, backwards clock, unavailable
entropy, expired metadata, known revocation, offline relaunch, old signed payload,
unauthorized privilege change and interrupted persistence of trust state.

### F07 - Define the crash and concurrency protocol for activation and storage

Evidence: [DIRECTION.md:99](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:99),
[DIRECTION.md:114](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:114),
[ROADMAP.md:64](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:64),
[ROADMAP.md:74](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:74),
[ROADMAP.md:124](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:124).
The live/preferred split and migration warning are right. No commit point or
allowed post-crash state is yet given for their implementation.

Failure scenario and consequence: version B becomes active while version A still
has write/device authority; an interrupted install persists preference before
payload/trust data; or rollback restores A against a schema already irreversibly
changed by B. Each can corrupt data despite individually valid signatures and an
apparently atomic in-memory pointer update.

Bounded fix: define a small lifecycle with operation epochs and one documented
linearization point: stage immutable candidate, validate/prepare, fence old
requests, drain/cancel, transfer compatible state, switch binding, verify health,
and separately commit next-boot preference. Define what happens to requests at
each edge, including an unavailable/unknown result after a crash; do not promise
exactly-once external effects. Persistence must record enough intent/commit state
to select an authorized, fully present, state-compatible version after restart.
Never resurrect a revoked artifact just because it was previously healthy.

For the first writable service, narrow the supported migration policy to
compatible schemas or explicit backup/restore; rejecting a destructive migration
is acceptable. Specify what a durable acknowledgment means for file flush,
rename and preference commit. Identify ordering/cache assumptions of the chosen
block device and the fault model for lost/reordered/torn writes. A QEMU reboot
alone is not the defined power-loss experiment. Recovery must be able to expose
the last valid state or a clear failure without depending on the failed candidate.

Stage/test: resolve before Stage 3 user data or device/service cutover, not only
Stage 6 system upgrades. Inject interruption at every persistence/activation
edge and keep concurrent readers/writers active. Use a host-held expected-data
ledger and allowed-state model: old or committed-new as specified, never missing
acknowledged durable data, unauthorized code or mixed incompatible state. System
image A/B implementation can remain at Stage 6.

### F08 - Survival gates need independent oracles and predeclared budgets

Evidence: [DIRECTION.md:87](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:87),
[REQUIREMENTS.md:44](D:/python/github/OpenRhiza/OpenRhiza/next/docs/REQUIREMENTS.md:44),
[ROADMAP.md:35](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:35),
[ROADMAP.md:54](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:54),
[ROADMAP.md:89](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:89),
[ROADMAP.md:100](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:100).
Process counts and an eight-hour soak are concrete, but "progress", "bounded",
"health" and "measures latency" still admit incompatible pass criteria.

Failure scenario and consequence: an attacker prints a witness line, the harness
accepts a counter from the system under test, a stopped guest clock never reaches
its timeout, or a slow monotonic leak survives a short demonstration. Independent
tests that a generator cannot edit do not suffice if their oracle still trusts
the candidate's self-reported success.

Bounded fix: define the harness/oracle trust boundary in 1.1. Issue externally
timed challenges to separate principals and recovery through production paths;
attribute responses to protected channels rather than free-form shared logs.
Bind results to source/image/test/profile hashes and record reset/boot events
externally. Observe useful completed work and errors, not only scheduler ticks.
Keep a host-monotonic deadline as well as guest-monotonic measurements. Test code,
fixtures and result-signing authority must be outside generator/candidate write
access; add deliberately broken negative controls to show the oracle rejects
missing progress, reset, corruption and failed admission. This checks the oracle,
not the absence of all implementation defects.

Before each gate, publish an acceptance sheet containing exact admission limits,
latency maxima, repetitions, resource baselines and permitted stable cache slack.
The following are reviewer-proposed calibration starting points, not measured
results or previously accepted human requirements. Adjust for the qualified
profile with written rationale before the acceptance run, never after seeing a
failure merely to relabel it as a pass.

| Gate | Proposed falsifiable contract |
| --- | --- |
| Stage 1.3 | Existing three fresh boots, each with a declared host timeout (initial candidate: 30 s); expected panic is a separate diagnostic case; no reused image evidence |
| Stage 2 CPU-only stress | Candidate guest quantum <= 10 ms; each unrelated admitted worker completes useful challenged work with maximum gap <= 100 ms guest time; ordinary recovery-command response <= 250 ms guest and <= 5 s host |
| Stage 2 recovery | Healthy reserved command path returns within <= 2 s guest and <= 10 s host after injected supervisor failure, with no boot/reset change |
| Stage 2 reclamation | CPU-only resources reach the ledger baseline plus explicitly fixed cache allowance within <= 1 s guest and <= 5 s host after the stop/revoke request is accepted; 1,000 create/fault/revoke cycles show no accumulating uncharged resources |
| Stage 2 admission | For each named finite limit L, L succeeds within the declared envelope and L+1 fails with the documented error and no partial authority/resource publication; sums include core/recovery headroom |
| Stage 3 devices | Separate configured interrupt-rate/work and quiesce deadlines; distinguish successful reclamation from bounded quarantined pages; F05 forbidden-access and late-completion checks pass |
| Stages 3-6 persistence | Host oracle enumerates permitted crash states and verifies durable acknowledgments, artifact/trust epochs and data hashes; a log saying rollback succeeded is insufficient |
| Stages 4-5 UX | Before their gates, set input/render latency, audio underrun, resource-growth and game-clock drift tolerances for the selected profile; eight hours or nonblank pixels alone cannot establish usable multitasking or playable Doom |

A calibrated slow host may produce an explicitly invalid infrastructure run, but
not a passing survival result without observations. Do not remove the external
timeout to accommodate a frozen guest. Stage 1 need not implement all these
tests; it needs the small harness contract and ownership of their future gates.
Start boundary fuzz/negative tests with the boundary in Stage 2/3; keep extensive
campaigns and multi-day qualification in Stage 6.

### F09 - Authority must bind identity and object version, not model wording

Evidence: [DIRECTION.md:121](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:121),
[DIRECTION.md:170](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:170),
[ROADMAP.md:49](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:49),
[ROADMAP.md:72](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:72),
[ROADMAP.md:98](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:98).
The policy boundary is correct, but the exact-command/typed-action substrate is
scheduled with AI in Stage 5 despite earlier terminal, supervisor and admission
needs. The plan also lacks an approval-to-effect identity/version binding.

Failure scenario and consequence: the user approves a plan to modify a particular
file or module, but a pathname/binding changes before execution; a saved workflow
or semantic summary later replays the approval as ambient authority; or an
authorized query remains usable after its source ACL changes. Typed JSON alone
does not resolve those authorization races.

Bounded fix: implement only the deterministic local command/action subset needed
by the Stage 2 terminal and Stage 3 module manager. Stage 5 adds intent planning,
not a second authority mechanism. Bind each effect to authenticated principal,
capability, stable object identity/generation, operation arguments, policy epoch,
approval scope, deadline and request/transaction ID. Validate at the effect's
commit boundary, or use a reservation that preserves the approved conditions.
Unknown operations and changed preconditions fail or require new authorization.

Models, retrieved text, cached workflows and signed packages are input, never
user-approval or signer principals. The approval service must receive confirmation
through a trusted user path and construct the displayed effect from validated
fields, not a model's reassuring prose. Semantic cache hits recheck current source
permissions; raw-content/model-upload permission remains separate. Cancellation
and retries follow F04/F07 rather than silently duplicating external effects.

Stage/test: local substrate before Stage 2 terminal acceptance; full contract and
injection tests before Stage 5 external effects. Test approval followed by object
replacement, privilege/ACL revocation, service restart, forged confirmation,
replayed cached plan and prompt injection in both content and tool results.
This does not bring voice, semantic indexing or model integration into Stage 1.

### F10 - Freeze narrow contracts after experiments, not the whole future system

Evidence: [REQUIREMENTS.md:8](D:/python/github/OpenRhiza/OpenRhiza/next/docs/REQUIREMENTS.md:8),
[DIRECTION.md:108](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:108),
[ROADMAP.md:20](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:20),
[ROADMAP.md:26](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:26),
[ROADMAP.md:47](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:47).
Requirements allow details to mature at their stages, but Stage 1.1 calls for
manifest freeze before native/runtime/device experiments. Stage 2 combines first
processes, recovery, Wasm and SMP under a single gate.

Failure scenario and consequence: speculative dependency/state/device fields get
locked into v0, or a Wasm runtime's platform needs force unplanned kernel services.
Alternatively all useful single-core service work waits for unrelated SMP work,
making honest intermediate progress difficult to report.

Bounded fix: freeze only a boot-image envelope and initial bounded-message
fixtures in 1.1. Mark the native ABI and evolving module manifest experimental,
version them, reject unknown authority-bearing fields, and freeze their small
operational subset after representative successful and failure-path round trips.
Keep comprehensive IDL, compatibility layers and generalized migration out of
this prerequisite. Do not relax the core boundary to meet an arbitrary runtime's
default feature set.

Qualify one pinned Wasm worker profile after native process/broker isolation.
Wasmtime is a possible module, not a required kernel dependency: its primary
documentation describes a restricted `no_std` feature set and custom platform
hooks, with native compilation/features needing separate consideration. Check
the chosen pinned version rather than assuming desktop defaults will port.
An audited interpreter or externally built AOT profile can be considered without
adding another OS underneath OpenRhiza.
[Wasmtime platform support](https://docs.wasmtime.dev/stability-platform-support.html)

Stage fix: explicitly split Stage 2 into native single-core isolation, fixed
recovery/Wasm-worker qualification, and an SMP subgate. Allow subsequent work on
the explicitly single-vCPU profile after the first two pass; enabling or claiming
SMP still requires its own concurrency/TLB/interrupt tests. This is a proposed
amendment to the six-sequential-stage wording, not permission to bypass it before
the parent integrates the change. Preserve all later product goals without
making their full implementation a first-boot condition.

## Minimal Stage Amendments

These are proposed subgates within the existing roadmap, not additional product
features or an instruction to create a second competing roadmap.

| Boundary | Minimum closure | What deliberately stays later |
| --- | --- | --- |
| K: enter Stage 1.2 | Qualified candidate boot/tool profile; F01 handoff/TCB; F02 recovery dependency and authority sketch; F03 resource ownership principles; F04 initial ABI shapes/errors; F05 device denial/profile rule; F08 small independent harness and initial timeout; F10 experimental-versioning decision | Complete scheduler proofs, hostile tests, IOMMU implementation, dynamic registry, full manifests, writable FS, Wasm JIT, GUI and AI |
| N: claim hostile CPU-only execution in Stage 2 | Native entry/fault/preemption isolation; full first-operation semantics; F02-F04 enforcement; F08 calibrated bounds and independent witness/reclaim tests; fixed-image trust from F01/F06; local deterministic commands | Raw DMA/IRQ authority; arbitrary external native packages; generated privileged services |
| W: add Wasm and expand to SMP | Native N gate first; a pinned isolated runtime/broker profile and worker-failure tests; separately pass two-vCPU race/TLB/interrupt tests before SMP is enabled | No need for JIT, full WASI, native compiler self-hosting or SMP merely to qualify the initial one-vCPU service profile |
| D: Stage 3 device access | F05 enforced requester/mapping/interrupt/reset boundaries and adversarial device tests before raw grants | Universal hardware support, physical fault immunity, vendor graphics |
| A: Stage 3 admission and durable services | F06 time/entropy/trust state precede their consumers; F07 bounded install/cutover/durability protocol; local policy; production-path oracles | Full public registry social features, operational key-rotation drills and system A/B release certification |
| Intent: Stage 5 | F09 effect/approval/TOCTOU contract and generator-independent evidence pipeline; audio, knowledge and game acceptance values agreed before their gates | Always-listen voice, broad autonomy, general local acceleration |
| Release: Stage 6 | Complete system-image upgrades, backup/restore, key compromise/rotation, long soak, public registry/operator security and exact scope claims | Expansion goals unless separately qualified |

Stages N/D/A are trust-boundary labels proposed here. No current stage has already
met them. A hand-written or signed native module is not automatically safe to
give device authority; artifact origin and execution privileges are separate.

The Stage 1.1 outcome can be a few short ADRs and machine-readable fixtures owned
by the parent, not a complete implementation specification for the entire OS.
Resolve the choices that constrain entry and ownership now; test and refine
unimplemented later behavior before its own trust boundary.

## What Is Sound

- The restart boundary is clear, portable audited dependencies are allowed, and
  another OS is not a guest runtime dependency. Legacy artifacts and old success
  counters cannot be promoted to new evidence.
  [README.md:17](D:/python/github/OpenRhiza/OpenRhiza/next/README.md:17)
- Hardware process isolation underneath Wasm, a small mechanism core, and explicit
  inclusion of boot/build/privileged modules in the TCB are appropriate starting
  boundaries. No rewrite into a Linux/BSD/Windows/Android runtime is needed to
  address these findings.
  [DIRECTION.md:45](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:45),
  [DIRECTION.md:65](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:65)
- Recovery policy outside core, unchanged-boot survival evidence, and refusal to
  treat a watchdog reset as containment are the right distinctions to preserve.
  [DIRECTION.md:87](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:87)
- Offline installed behavior, independent build/test/sign/admit responsibilities,
  and signature-as-identity rather than correctness avoid major trust shortcuts.
  [DIRECTION.md:36](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:36),
  [REQUIREMENTS.md:49](D:/python/github/OpenRhiza/OpenRhiza/next/docs/REQUIREMENTS.md:49)
- Command-first interaction, Korean/English IME, real application/process/window
  state, background budgets and a downloaded playable game are preserved as
  behavioral goals. Semantic knowledge remains a private rebuildable sidecar;
  it does not force a new filesystem format.
  [DIRECTION.md:119](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:119),
  [DIRECTION.md:168](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:168)
- One pinned QEMU profile, later physical qualification, and deferred platform,
  self-hosting and compatibility programs constrain the initial scope sensibly.
  The fixes above do not require formal verification of the whole system,
  universal hardware support or a public Nexus before first boot.
  [ROADMAP.md:137](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:137)

## Evidence and Limitations

Directly observed: the working tree is extensively dirty; the listed new baseline
contains documents/inventory/tools and no kernel. References above were checked
against numbered source lines. The verifier checks inventory hashes/exclusions
and required-file presence and explicitly disclaims an OS gate.
[verify-baseline.ps1:6](D:/python/github/OpenRhiza/OpenRhiza/next/tools/verify-baseline.ps1:6),
[verify-baseline.ps1:30](D:/python/github/OpenRhiza/OpenRhiza/next/tools/verify-baseline.ps1:30)

Parent-reported, not rerun or independently certified by this reviewer:
`next/tools/verify-baseline.ps1` passed; QEMU 10.2.0, EDK2 x86_64 firmware files
and Rust stable/nightly are installed; the old image's `/status` and `/recovery`
smoke passed with log
[x86-astra-plan-review-reference-20261003.log](D:/python/github/OpenRhiza/OpenRhiza/logs/x86-astra-plan-review-reference-20261003.log).
These facts do not qualify a new pin/profile or advance a new-OS gate. The parent
also confirmed canonical review documents would remain unchanged during review.
The prior legacy run has the same evidentiary limit stated in
[LEGACY_REVIEW.md:125](D:/python/github/OpenRhiza/OpenRhiza/next/docs/LEGACY_REVIEW.md:125).

This is not a line-by-line audit of legacy code, crypto implementations, firmware,
hardware or a new kernel. No new build, boot, hostile test, performance
measurement or power-loss experiment was performed. Intel PDF references were
checked through primary-source indexed excerpts for the cited mechanisms; full
PDF retrieval was limited by the web tool. Those excerpts do not qualify the
installed QEMU IOMMU/interrupt behavior. Public Wasmtime documentation is a
selection aid, not a substitute for pin-specific port qualification.

Reviewed source snapshots (SHA-256, before the report was added):

| Source | SHA-256 |
| --- | --- |
| `next/README.md` | `B224CF3D8B9F76679143308E1D152AE9E50243DE6A23CC83B738C7A26B2FC348` |
| `next/docs/DIRECTION.md` | `52049A58B8344F3EE9CF70BA90313D4108980D6D569A3BB6FA2C3D899A6790E1` |
| `next/docs/REQUIREMENTS.md` | `2099B7BC15F3A2FA7DF4A706F7E7131D16C11CCDC7B29DEF11C4A38EE01CFD34` |
| `next/docs/LEGACY_REVIEW.md` | `F9D2E559E0428F68C6CE3BA7433B07931B50FB139416732B949A844B41EFC53B` |
| `next/docs/ROADMAP.md` | `F71BD9BE0EF936E2AD266F29233A789756E76321AE2E03F2D8BAE0B58BD671BA` |
| `next/tools/verify-baseline.ps1` | `1A27187034A5E8A0AC4F8B4261F368E3286904BC74827C34F4F1D01F1360E7DA` |

Post-write document checks: all 58 local links resolve and numbered references
are within the target files; the six source hashes above remain unchanged.
These are document-integrity checks, not runtime tests. This reviewer wrote only
`next/docs/ASTRA_PLAN_REVIEW.md`; concurrent parent work was not reverted.

Final disposition: **CONDITIONAL for continued planning; not ready now for
Stage 1.2 entry, hostile native/device execution or release.** Close only K for
first entry, then the corresponding later gates before increasing authority.
The reviewed direction is worth retaining; completeness and defect-freedom are
not established by this review or by any finite set of passing tests.

## Plan Integration Re-review

Date: 2026-10-03. Reviewer: the same `gpt-6-astra` reviewer.
Scope: the six current documents hashed below and their F01-F10 integration;
no renewed primary-source research, model introspection or Stage 1.1 execution.
The initial findings, original snapshot hashes/references and historical verdict
above are preserved unchanged. References in this section address the new snapshot.

**Plan-level verdict: ACCEPTED FOR STAGE 1.1 K CONTRACT WORK.** The amendments
are sufficient to proceed with that work; no blocking integration contradiction
was found. **Actual K selections/fixtures and entry review remain outstanding;
Stage 1.2 entry is NOT AUTHORIZED by this re-review.** This accepts the amended
plan, not the implementation or passage of K, N, W, SMP, D, A, Intent or release
gates. The distinction is explicit in
[ROADMAP.md:208](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:208),
[REQUIREMENTS.md:8](D:/python/github/OpenRhiza/OpenRhiza/next/docs/REQUIREMENTS.md:8)
and [README.md:30](D:/python/github/OpenRhiza/OpenRhiza/next/README.md:30).

### Finding Integration

The disposition's mapping is substantively accurate; F08 has one nonblocking
timing-wording drift identified below. These are incorporated planning obligations,
not ten completed implementations.
[ASTRA_REVIEW_DISPOSITION.md:40](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ASTRA_REVIEW_DISPOSITION.md:40)

| Finding | Re-review result and current contract evidence |
| --- | --- |
| F01 | Incorporated: external trust, authenticated configuration, parser/handoff and first-principal ownership are K decisions. [IMPLEMENTATION_GATES.md:15](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:15) |
| F02 | Incorporated: resident recovery closure/ceilings are selected at K, with protected reservations and failure tests at N. [IMPLEMENTATION_GATES.md:25](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:25), [IMPLEMENTATION_GATES.md:76](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:76) |
| F03 | Incorporated: payer/delegation ledger, alias revocation, funded cleanup and reclamation completion precede hostile qualification. [IMPLEMENTATION_GATES.md:54](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:54) |
| F04 | Incorporated: bounded operations, snapshots, faults, transfer/death/cancellation, inversion and enabled native state are explicit N obligations. [IMPLEMENTATION_GATES.md:64](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:64) |
| F05 | Incorporated: D gates generic device authority; the initial console has a separate narrow contract, not an unrestricted exemption. [IMPLEMENTATION_GATES.md:103](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:103) |
| F06 | Incorporated: time/entropy failure policy, offline authority, security epochs and rollback scope precede relevant consumers; external admission awaits durable trust state. [IMPLEMENTATION_GATES.md:131](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:131), [ROADMAP.md:115](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:115) |
| F07 | Incorporated: fenced ownership, live/preferred commit separation, compatible migration and host-held crash-state oracle precede real cutover/user data. [IMPLEMENTATION_GATES.md:146](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:146) |
| F08 | Incorporated: protected external challenges, dual clocks, negative controls and predeclared calibration remain requirements, not results. Correct the reclamation time origin below before N acceptance. [IMPLEMENTATION_GATES.md:178](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:178) |
| F09 | Incorporated: Stage 2 local typed actions precede Stage 5 planning; effect/approval identity, generation and commit checks remain mandatory. [IMPLEMENTATION_GATES.md:80](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:80), [IMPLEMENTATION_GATES.md:165](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:165) |
| F10 | Incorporated: narrow boot-envelope freeze, experimental operational subsets, and distinct N/W/SMP progression replace premature broad freeze. [IMPLEMENTATION_GATES.md:40](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:40), [ROADMAP.md:27](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:27) |

### Focused Consistency Checks

- **Initial console:** consistent. K selects a narrow non-DMA polling-UART grant
  or equivalently bounded transport; N requires its authority audit/denial tests.
  Generic devices remain subject to D. This avoids making recovery depend on
  future generic drivers without treating non-DMA alone as isolation evidence.
  [IMPLEMENTATION_GATES.md:27](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:27),
  [IMPLEMENTATION_GATES.md:111](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:111),
  [DIRECTION.md:118](D:/python/github/OpenRhiza/OpenRhiza/next/docs/DIRECTION.md:118)
- **External enrollment/install:** consistent. Stage 3.2 protocol/trust-policy
  development does not authorize external enrollment/install; the explicit hold
  lasts until minimum trust persistence/recovery is qualified in 3.3. Network
  development is not evidence of durable authenticated node identity.
  [ROADMAP.md:98](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:98),
  [ROADMAP.md:115](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:115),
  [ASTRA_REVIEW_DISPOSITION.md:60](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ASTRA_REVIEW_DISPOSITION.md:60)
- **Single-vCPU/Wasm/SMP:** consistent. N precedes W; the qualified single-vCPU
  profile may proceed after both. SMP remains disabled/unclaimed until its
  independent two-vCPU gate passes. Neither N alone nor written W requirements
  constitute the completed single-vCPU Stage 2 profile.
  [ROADMAP.md:65](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:65),
  [IMPLEMENTATION_GATES.md:92](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:92)

### Nonblocking Correction And Remaining K Work

**P2 drafting drift, not a blocker to K contract work:**
[IMPLEMENTATION_GATES.md:199](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:199)
starts the reclamation deadline at "acknowledged teardown", whereas the initial
F08 candidate at
[ASTRA_PLAN_REVIEW.md:390](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ASTRA_PLAN_REVIEW.md:390)
starts it when the stop/revoke request is accepted. If acknowledgment means final
teardown completion, an arbitrarily long teardown could occur before the measured
interval begins. Before freezing the N acceptance sheet, restore "after the
stop/revoke request is accepted" and define that acceptance event and its bounded
request latency. The numeric candidates remain subject to profile calibration;
this correction must not be used to claim a measured bound now.

Actual K work still requires selecting and reviewing the pinned one-vCPU
toolchain/firmware/machine/device profile and isolated invocation; boot trust,
parser and memory-handoff ADR/fixtures; first-principal grants and resident
recovery/console authority; initial ownership/ABI shapes and device denial; and
independent harness channels, boot/reset observations and initial deadlines.
Those concrete selections and fixtures need an entry review before Stage 1.2.
Later runtime enforcement and hostile tests stay at their stated gates; they are
not prerequisites silently added to this plan-level approval.
[IMPLEMENTATION_GATES.md:10](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:10),
[IMPLEMENTATION_GATES.md:45](D:/python/github/OpenRhiza/OpenRhiza/next/docs/IMPLEMENTATION_GATES.md:45),
[ROADMAP.md:46](D:/python/github/OpenRhiza/OpenRhiza/next/docs/ROADMAP.md:46)

### Re-reviewed Snapshot

SHA-256 of the current inputs, before the parent's subsequent disposition update:

| Document | SHA-256 |
| --- | --- |
| `next/README.md` | `6474275AC728038FC0F7F297361DC8E02B5D4D69CD5A86C5650AAB05B824075B` |
| `next/docs/DIRECTION.md` | `BC41FED0BF55F76FF313676801D38191A40F11C4DF1B854F6A10BFA839B7CB34` |
| `next/docs/ROADMAP.md` | `033B11B902059B4328687BB8D6450D89155F0E0DEC062D8625AD2D5D4B89245F` |
| `next/docs/REQUIREMENTS.md` | `FF36DB8942E65130ED6A89603221DA6B056090C468D92FAE7BB43DD3724C775F` |
| `next/docs/IMPLEMENTATION_GATES.md` | `748E12BAC62FC57E9ACB732F9243141ADD8B10D9F8C1902916AFC30E255DD538` |
| `next/docs/ASTRA_REVIEW_DISPOSITION.md` | `8E9EC19474641FC5EBA086EDE028127025A30D139F8567E1BF74DB08E55597A5` |

Limitations: document-only re-review. No K qualification, kernel implementation,
build, new-runtime execution, fault test or performance measurement was performed.
Only this section was appended; other documents remain parent-owned. The parent
will record this exact plan-level verdict in the disposition; actual K entry
review and all runtime gates remain pending.
