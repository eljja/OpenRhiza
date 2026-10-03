# Implementation Entry and Acceptance Gates

Status: planning requirements incorporated from Astra findings F01-F10 on
2026-10-03. These gates are not passed. `DIRECTION.md` remains the product and
architecture authority; `ROADMAP.md` owns sequencing. This document defines the
decisions and evidence required at each boundary, not a second roadmap.

## K: Before First Kernel Implementation (Stage 1.1)

Record a small reviewed set of ADRs and bounded fixtures covering:

- Exact host toolchain, firmware/loader, QEMU machine/CPU/devices, memory and
  one-vCPU configuration, build isolation from parent Cargo/rustup settings,
  dependency/license pins and reproducible invocation.
- Boot TCB and trust mode: which component verifies which bytes and which
  external root is trusted. Authenticate grants/configuration as well as code
  before initial module admission. A trusted-host development image is permitted
  for diagnostic bring-up with its verified-boot/physical-attacker exclusions
  explicit; installed firmware alone establishes no authenticity claim.
- Versioned entry envelope with bounded lengths, checked regions/overlaps,
  entry CPU/stack/page-table state, firmware exit rules, owned/reserved frames,
  immutable resident initial-image descriptor and parser ownership. If UEFI is
  selected, qualify memory-map/ExitBootServices behavior and post-exit console
  ownership. Later untrusted ELF parsing must not silently expand the core TCB.
- First user principal, maximum initial grants and protected core/recovery
  reservations. No ordinary or delegated grant may remove these reservations.
- Resident recovery dependency graph, including input/output, relaunch trigger,
  restart limit/backoff and explicit exhausted/degraded state. Its initial
  console must not depend on mutable storage, a model, network, GUI or a healthy
  normal supervisor. Choose and qualify an isolated serial module with a narrow
  non-DMA polling-UART grant or an equivalently bounded console transport.
- Resource ownership/accounting principles, draft version/error/message shapes,
  selected native entry/return and enabled architectural-state profile, plus a
  deny-by-default device-authority rule. These are initial decisions; complete
  syscall, scheduler and reclamation contracts belong to N before hostile tests.
- Independent harness ownership, externally observed boot/reset identity,
  authenticated/attributed test channels, host-monotonic timeouts and initial
  three-boot/panic diagnostic cases. Tests cannot depend only on guest log text.

Freeze the boot-image envelope and initial fixtures after their qualification;
keep native ABI and broader module manifests explicitly experimental/versioned.
Reject unknown authority-bearing fields. Freeze each operational subset after
representative valid/invalid round trips, before admitting its consumers.

K does not require a full filesystem, dynamic registry, IOMMU implementation,
Wasm runtime, SMP, desktop, AI or production release operation. Selections and
entry evidence are still outstanding; this list is not evidence that K passed.

## N: Hostile CPU-Only Execution (Stage 2.1)

Before admitting hostile CPU-only test modules, implement the reviewed subset
and define its exact operation and resource contracts:

- Charge pages, page tables, stacks, process/thread slots, handles, queued bytes
  and capabilities, shared/pinned buffers, fault/log records and cleanup work.
  Name the payer, sponsor for delegated/service work and each hard limit. Debit
  before publishing an object. Shared resources retain an owner after exit;
  failed requests leave no partial charge/authority. Zero reused private pages.
- Define create/lookup/transfer/revoke/close/destroy linearization points, alias
  revocation, slot generation exhaustion/retirement, cleanup budget and completion
  token. Logical revocation stops new use; reclamation also requires draining or
  cancelling admitted work and completing required CPU translation invalidation.
  Keep cleanup bounded and funded even after the failed principal is exhausted.
- Specify each syscall's bounded work, immutable request snapshot, validation,
  user-copy fault handling, queue-full/error/partial-output behavior, capability
  transfer, blocking/deadline/cancellation and peer-death effects. Shared data
  buffers need explicit concurrent-mutation and pinning rules.
- Prefer a small bounded asynchronous request/reply contract with request IDs.
  Any chosen synchronous dependency must demonstrate a priority/budget-inversion
  bound, including cycles and death. No unbounded walk or wait holds a core or
  IRQ-critical lock. A timeout after commit reports completed/unknown outcome;
  it does not imply effects were undone or authorize a duplicate retry.
- Define spawn/start/exit/kill/wait, restart epochs and outstanding reply/waiter
  cleanup. Verify user entry/return, W^X, privileged I/O denial, fault disposition
  and all enabled per-thread architectural state, including FP/SIMD if enabled.
- Enforce reserved recovery CPU, frames, object/queue slots and diagnostics.
  Crash the normal supervisor/logger/console and saturate ordinary resources;
  test recovery and unrelated useful work. A dependent client may get a bounded
  service error, but ordinary failures cannot revoke recovery's minimum grants.
- Admit the fixed initial image under the K trust root. Deliver a small typed
  local command/action registry for the terminal and supervisor, independent of
  LLMs. Ordinary supervisor crashes remain in the fault model; compromise of
  explicitly named authorization/key-policy TCB logic is a separate assumption.

Before calling N passed, publish numeric limits and pass production-path fault,
race, saturation, stale-handle, metadata-pool and reclamation tests under the
independent oracle below. Restrict devices to the specifically qualified initial
console contract; N does not permit unqualified raw DMA/MMIO/IRQ access.

## W And SMP: Runtime Qualification (Stages 2.2 and 2.3)

After N, pin and qualify one isolated Wasm worker/broker profile. Explicitly
select interpreter or AOT/JIT features, platform hooks and host-call contracts;
neither desktop runtime defaults nor full WASI are assumed portable. Worker and
broker crashes, infinite loops and invalid host requests must preserve N.

The initial single-vCPU profile may progress after N and W. Report SMP as pending
until the separate two-vCPU gate passes concurrent ownership, TLB shootdown and
acknowledgment, interrupts, reclamation/reuse and architectural-state tests.
Only then enable or claim SMP. This preserves the six-stage sequence for the
qualified profile without making untested SMP an implicit part of every result.

## D: Before Generic Device Grants (Stage 3.1)

Qualify the exact requester/domain/MMIO/interrupt profile before granting a
driver raw device authority. Enforce DMA address translation/access-platform
negotiation where required, bus-master ordering, IRQ source isolation, masking,
rate/work bounds and a core-owned preemption timer. Keep translation tables and
interrupt-control authority outside the untrusted driver's grants.

The fixed non-DMA console contract selected at K is a narrow exception to the
generic driver sequence, with its own authority audit and denial tests at N.
It is not permission to expose other I/O ports or device registers. Without an
enforceable generic-device profile, continue CPU-only work or label a trusted
driver experiment explicitly; hostile-driver containment cannot pass. Running
inside QEMU protects the host separately from protecting the guest kernel.

Specify transport-correct revocation: fence new submissions/MMIO, control the
source, remove mappings, complete required invalidation/access barriers and
establish reset/quiescence before memory reuse or rebinding. Test the actual
acknowledgment, not only the reset write. On timeout, quarantine the device and
unproven pages within a fixed budget; do not report full reclamation. Device
protocol/reset policy stays in modules; the core enforces bounded grants.

Inject forbidden DMA/MMIO, interrupt storms, driver death during I/O, failed
reset, late completions and buffer reuse. Hardware nonconformance is a declared
exclusion; malicious programming of a conforming admitted device is covered.

## A: Network Admission, Storage and Cutover (Stages 3.2-3.3)

Before TLS, enrollment or external package admission, select trusted wall-clock
bootstrap and entropy sources with explicit failure states. Keep monotonic
scheduling time separate. Invalid/unavailable time or entropy must not silently
permit security-sensitive sessions or key generation with test seeds.

Choose an audited update-verification design, external root, signed-byte/schema
encoding, bounded parsers and persisted authorization/security epochs. Installed
artifacts run under their last accepted local authority unless locally revoked.
Unknown/stale freshness denies new admission and privilege expansion while
preserving sealed recovery and authorized installed operation. Offline nodes
cannot know unreceived revocations. Define signed metadata/removable-media
refresh, authorized older payload rollback and recovery trust separately from
replaying old authorization. Full hostile-disk rollback resistance requires an
independent freshness anchor or an explicit initial-profile exclusion.

For lifecycle/persistence, define request epochs and the commit point through:
stage immutable candidate, validate/prepare, fence old work, drain/cancel, move
compatible state, switch active binding, verify health, and separately persist
next-boot preference. Specify each interrupted state and its allowed result;
avoid simultaneous writers and never revive a revoked artifact during rollback.
Initially permit compatible state schemas or explicit backup/restore; reject
unsupported destructive migrations. A later healthy state cannot erase evidence
of already lost acknowledged data.

Define durable acknowledgment for file flush/rename and preferences, block/cache
ordering assumptions and the lost/reordered/torn-write fault model. Before user
data or real cutover, inject interruption at every transition with concurrent
clients and compare to a host-held expected-data/authorized-state ledger. A QEMU
reboot alone is not a power-loss experiment. Minimal trust-state recovery belongs
here; operational key-rotation drills and system-image A/B certification remain
in Stage 6.

## Intent: AI Effects (Stage 5)

Extend the existing local typed actions with model planning. Bind effects to the
authenticated principal, capability, stable object/generation, arguments, policy
epoch, approval scope, deadline and transaction ID. Revalidate at commit or hold
a reservation preserving the approved conditions. The trusted confirmation path
shows effects from validated fields and supplies confirmation; model text,
signed packages and cached workflows cannot impersonate it.

Test object replacement, ACL/capability revocation, restart, replay, forged
confirmation and injected documents/tool results. Semantic cache hits recheck
current source permissions; content upload is separately authorized. Apply N/A
cancellation and retry semantics to external effects. Freeze profile-specific
voice/privacy, game timing/audio and UI response limits before their gates.

## Acceptance Sheets And Independent Oracles

Each gate records source/image/test/profile hashes, authority assumptions, exact
limits and thresholds, workload/input, repetitions, result/error observations and
resource baselines with fixed cache/quarantine allowances. Tests, fixtures and
result-signing keys are outside candidate/generator write authority. Attribute
externally challenged useful work to protected principals/channels; record
boot/reset externally and use both guest and host-monotonic deadlines. Include
broken negative controls that must fail for lost progress, reset, corruption and
bad admission. Shared guest success logs alone cannot pass a gate.

Astra proposed the following calibration candidates. They are not measured
results or already frozen requirements. Qualify them against the named profile
and record any change/rationale before an acceptance run, preserving failures.

| Boundary | Initial calibration candidate |
| --- | --- |
| Stage 1.3 | Three fresh boots, <= 30 s host per boot; separate expected-panic case |
| N scheduling | <= 10 ms guest quantum; unrelated useful-work gaps <= 100 ms guest |
| N commands | <= 250 ms guest and <= 5 s host for normal recovery command responses |
| N recovery relaunch | Healthy reserved command path within <= 2 s guest and <= 10 s host of injected supervisor failure |
| N reclamation | Ledger baseline plus fixed allowance within <= 1 s guest and <= 5 s host after the stop/revoke request is accepted; 1,000 create/fault/revoke cycles |
| N admission | Every limit L admits L within its defined combined envelope; L+1 rejects without partial resources/authority |

For reclamation, acceptance is the protected operation's linearization point:
the authorized stop/revoke request is recorded and cleanup ownership is committed
before asynchronous teardown. Final teardown acknowledgment is not that event.
Record externally issued request, protected acceptance and observed ledger
baseline separately. The acceptance sheet must also set a numeric maximum for
request-submission-to-acceptance latency and a host end-to-end timeout; a delayed
acceptance cannot hide an unbounded teardown or stalled request path.

D requires its own IRQ and quiesce deadlines and quarantine ceiling. A requires
enumerated permitted crash states and durable-data checks. Desktop/voice/game
gates require input/render latency, underrun, memory growth and game-clock drift
tolerances before the soak. An infrastructure timeout may invalidate a run; it
cannot turn missing observations into a pass or remove the external deadline.
Start boundary fuzzing/negative tests with N/D/A; Stage 6 adds longer campaigns.
