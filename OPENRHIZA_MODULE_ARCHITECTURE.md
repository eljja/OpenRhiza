# OpenRhiza Module-First Architecture

Reference only after the 2026-10-03 new-implementation reset. The independent-OS
and minimal-core intent is preserved, but active decisions and acceptance gates
are in `next/docs/DIRECTION.md` and `next/docs/ROADMAP.md`. Earlier ABI/ownership
proposals are not inherited by the new implementation.

Status: architecture baseline proposed on 2026-08-14; reviewed on 2026-10-03

The 2026-10-03 review preserves the independent-OS and module-first requirements.
The first implementation priority is a core that contains failures in arbitrary
module code. This document describes target contracts, not completed guarantees.

## 1. System invariant

Rhiza Core is the only non-module product component after firmware handoff. All
other behavior is implemented by signed, versioned, replaceable sandbox modules.

This includes:

- device discovery policy and device drivers;
- storage, VFS, and filesystem implementations;
- Ethernet, IP, TCP, DNS, TLS, and higher network protocols;
- keyboard, pointer, audio, speech, display, and compositor behavior;
- service supervision, package management, update, and recovery policy;
- command parsing, intent routing, LLM planning, and action execution;
- desktop shell, applications, developer tools, and compatibility layers.

A subsystem does not satisfy this invariant merely because a kernel function calls
a Wasm export. Product state, retry policy, parsing, protocol state machines, and
device behavior must be owned by an isolated module. Rhiza Core only exposes
bounded mechanisms.

## 2. Minimal trusted computing base

Rhiza Core contains only:

- architecture entry, privilege transitions, interrupts, exceptions, and timers;
- physical and virtual memory, page tables, and protected address spaces;
- preemptive threads, process lifecycle, SMP, and synchronization primitives;
- capability handle tables, typed IPC transport, shared-memory grants, and waitsets;
- bounded grants for IRQ, MMIO, I/O-port, DMA/IOMMU, framebuffer, and firmware data;
- signature and digest primitives required to authenticate the initial image;
- a minimal ELF loader for the first user process;
- early serial output, panic record, watchdog, and immutable recovery selector.

Rhiza Core does not contain:

- a general Wasm runtime or skill policy;
- hardware-specific device state machines beyond architectural interrupt/bootstrap
  mechanisms;
- a VFS, filesystem, TCP/IP, TLS, GUI, audio, speech, registry, or LLM client;
- application commands, natural-language prompts, workflow selection, or UI state;
- automatic network retrieval or LLM-generated code execution.

The kernel verifies and launches a small `bootstrap.module-host` ELF process from a
signed initial module image. The module host is separately address-spaced,
replaceable through A/B images, and receives an explicit root capability set. It is
not linked into the kernel.

### Core survival contract

For a declared machine, resource configuration, and hardware fault model, a faulty
or malicious module must not corrupt Rhiza Core, panic it, monopolize execution,
or exhaust the resources reserved for scheduling, fault handling, and recovery.
Invalid requests return bounded errors; the offending fault domain is terminated,
its grants are revoked, and unrelated runnable domains continue making progress.

This requires all of the following before arbitrary generated modules go live:

- separate user address spaces, guarded stacks, and W^X memory mappings;
- timer preemption and CPU budgets, including priority and interrupt flood controls;
- fallible allocation, admission limits, bounded queues, and reserved recovery memory;
- bounded syscall buffers, checked arithmetic, validated user pointers, and handle
  ownership checks on every request;
- no external-input `panic`, `expect`, unchecked allocation, or unbounded kernel wait;
- no module callback, Wasm execution, parser, or inference in an interrupt handler
  or while a kernel lock is held;
- complete exception entry and fault disposition, with independent emergency stacks
  for faults that cannot safely use the interrupted stack;
- generation-tagged handles, explicit revocation, and device quiescence before a
  DMA mapping or shared buffer is reclaimed;
- actual IOMMU enforcement before an untrusted module controls a DMA device.

Wasm validation is an additional boundary inside a user process. Its runtime and
host functions can fail too, so they share that worker's disposable address space.
Wasm fuel cannot bound a blocking native host function; kernel preemption and
bounded IPC are required even when a module uses fuel.

No software can promise continued execution through arbitrary physical hardware
failure, power loss, or an unspecified CPU/firmware defect. Even a formal proof
depends on stated assumptions. Core invariants must have executable specifications,
property tests, and progressively machine-checked proofs; a passing QEMU run alone
is not a correctness proof.

A kernel panic, hang, or watchdog-triggered reboot fails the core-survival gate.
Crash capture and boot rollback are separate last-resort recovery mechanisms. They
must never be reported as evidence that the original core kept running.

Reference verification assumptions: <https://sel4.systems/Verification/assumptions.html>.
This is a reference for verification methodology, not a runtime dependency.

### Critical recovery domains

The module host, supervisor, verifier, recovery shell, and recovery input/display
modules are separately budgeted failure domains. Failure of their normal instances
must not prevent activation of the signed local recovery set. A small kernel
mechanism can instantiate that predefined set with sealed, fixed grants; it cannot
interpret an AI proposal or load arbitrary replacement policy.

Privileged bootstrap modules, signing keys, firmware, and build tools also affect
end-to-end trust. Being outside kernel address space does not remove them from the
system's trust analysis. Root capabilities are split and delegated once; ordinary
workers and generated modules never inherit the bootstrap root authority.

CPU memory isolation and IOMMU protection do not guarantee that arbitrary device
register writes are safe for the hardware or system bus. Unknown driver experiments
run in disposable QEMU instances first. Live device grants require a tested hardware
profile and a bounded register/reset contract; unsupported isolation is reported
explicitly and cannot be counted as the full driver-containment guarantee.

## 3. Module execution profiles

Every installed component is a `RhizaModulePackage`. Two execution profiles are
permitted:

### Wasm sandbox module

The default for policies, protocols, parsers, applications, skills, command tools,
LLM adapters, speech stages, and most portable drivers.

- Runs in a dedicated user-mode Wasm worker process by default. Drivers, storage,
  recovery, and other critical services never share a worker with unrelated code.
  Optional pools for disposable applications must declare the entire pool as one
  failure domain.
- Receives only manifest-approved typed imports.
- Has fuel or epoch interruption, wall-clock deadline, memory/page limit, output
  limit, handle limit, and cancellation.
- Cannot issue a kernel syscall directly; the worker translates approved imports to
  handles already granted to that module instance.

### Native sandbox module

Reserved for the bootstrap host, runtime implementation, and measured hot paths
that cannot meet requirements in Wasm.

- Runs as an ordinary ring-3 ELF process with its own page table and handle table.
- Has no ambient device, file, network, display, or child-process authority.
- Must use the same package manifest, signature, capability, health, update, and
  rollback lifecycle as a Wasm module.
- Requires a stronger review and signing policy than Wasm.

No module is dynamically linked into kernel address space. A performance exception
must first demonstrate measured failure of shared-memory batching or a native
user-mode process. It cannot silently become kernel code.

## 4. Module interface contract

Module interfaces are declared in a versioned Rhiza Interface Definition Language
(RIDL). RIDL generates Rust bindings, message codecs, validation, compatibility
tests, and documentation.

Initial interface families:

- `rhiza.lifecycle`: start, ready, health, quiesce, stop, snapshot, restore;
- `rhiza.log`: structured event and bounded diagnostic stream;
- `rhiza.device`: match, resource request, interrupt event, reset, removal;
- `rhiza.block`: geometry, read, write, flush, trim, completion;
- `rhiza.vfs`: open, directory, metadata, read, write, rename, watch;
- `rhiza.net`: link, packet, socket, DNS, TLS session;
- `rhiza.input`: key, pointer, touch, text, accessibility, capture;
- `rhiza.audio`: stream format, capture/playback buffers, clock, state;
- `rhiza.surface`: buffers, damage, input region, focus, clipboard, notification;
- `rhiza.intent`: ingress, command schema, action plan, approval, result, undo;
- `rhiza.model`: bounded inference request, tool proposal, usage, cancellation;
- `rhiza.package`: resolve, fetch, verify, stage, activate, rollback, revoke.

Messages have explicit size and rate limits. Unknown fields and newer compatible
minor versions are handled according to the interface schema; incompatible major
versions fail before module activation.

## 5. Package manifest

Each package contains immutable content addressed by digest plus a signed manifest.
At minimum the manifest declares:

```text
module_id / version / execution_profile
target_arch / minimum_core_abi / interface versions
entrypoints / provided services / required services
requested capabilities and resource ceilings
device match keys where applicable
source digest / binary digest / build toolchain
dependencies / licenses / SBOM / provenance
test suite digest / test result digest / supported QEMU and hardware targets
publisher / signer / signing policy / creation and expiry time
update channel / rollback compatibility / state migration contract
LLM generation record when applicable
```

The LLM generation record identifies the model/provider, specification digest,
generation policy, source-review result, and builder. It need not expose private
user content, but it must make generated provenance distinguishable from reviewed
human-authored code.

## 6. Resolution and generation pipeline

When a service or driver is needed, the resolver follows one deterministic order:

```text
active compatible module
  -> signed local recovery/baseline module
  -> verified local cache candidate
  -> OpenRhiza.com registry candidate
  -> LLM-assisted module forge request
  -> unavailable/recovery behavior
```

OpenRhiza.com is a registry, build coordinator, evaluation store, and signing-policy
service. It is not a remote root shell and it cannot directly grant capabilities on
a user's machine.

An LLM may produce a requirement analysis, RIDL contract, implementation, tests,
and documentation. It may not sign, approve, activate, or expand the capabilities
of its own output.

AI serves two different functions: it builds replacement modules and it interprets
user intent. Already-installed modules execute their deterministic service contracts
without consulting a model on every packet, block operation, keystroke, or frame.
Inference failures consume only their own job budget and cannot gate basic OS work.
Local and remote model providers implement the same replaceable module interface.

Generated tests and a second model's review are useful evidence, but activation
also requires independent contract tests and adversarial fixtures the generator
cannot change. A signature proves artifact identity and provenance, not correctness.

Every generated or downloaded module passes:

1. Canonical requirement and interface contract creation.
2. Existing-module search and license/provenance evaluation.
3. Source generation or acquisition in an isolated build environment.
4. Hermetic compile with pinned toolchain and dependency lock.
5. Static validation, unsafe review, import/capability audit, SBOM, and license scan.
6. Unit, property, fuzz, simulator, and negative capability tests as applicable.
7. Disposable QEMU integration and fault-injection tests.
8. Signing by an independent policy-controlled builder after evidence is complete.
9. Guest-side digest/signature/provenance verification.
10. `Downloaded -> Verified -> Testing -> Canary -> Active` promotion.
11. Health monitoring, evaluation upload, quarantine, and automatic rollback.

No network or LLM availability is assumed during boot. The signed initial image
contains the module host, supervisor, serial input, display fallback, storage
reader, recovery shell, local verifier, and modules required for the declared boot
platform. Registry access improves the system; it is never required to recover it.

Initial recovery modules may have been AI-assisted during development, but their
tested binary digests are frozen for a release. Runtime generation replaces a
candidate only after validation; it does not regenerate the boot dependency graph.
Core changes follow a separately reviewed release and proof/test process rather
than the autonomous module-promotion loop.

Live replacement uses `stage -> test -> quiesce -> checkpoint -> transfer -> health
check -> commit`, with bounded timeouts and an explicit rollback image. Storage
writers must drain and flush before ownership transfer; input/display takeover must
preserve an independent recovery path. Irreversible state migrations cannot claim
live rollback until backward-compatible state or a recoverable snapshot exists.

## 7. Capability and sandbox policy

Capabilities are concrete handles, not descriptive strings checked only by a
module. Examples include one PCI function's BAR range, one interrupt vector, a
bounded DMA pool, one directory handle, one remote endpoint, one display surface,
one audio capture stream, or one model tool catalog.

- The launcher intersects manifest requests with user/device/system policy.
- Denied capabilities are absent rather than represented by a privileged no-op.
- Child modules receive an explicit subset; authority cannot be invented or widened.
- Raw MMIO, DMA, microphone, camera, package activation, and model action execution
  are separately visible and revocable capabilities.
- Every privileged grant, transfer, denial, activation, and rollback is audited.
- Resource quotas are enforced by the kernel and module host, not by module intent.

Driver modules never receive all physical memory. DMA uses granted shared pages and
IOMMU mappings where available. Protocol modules operate on packet or stream
interfaces and do not inherit the network driver's device capabilities.

## 8. Input architecture

All input reaches applications through sandbox modules and one normalized event
contract.

```text
device IRQ/DMA grant
  -> keyboard / pointer / audio driver module
  -> input or audio service module
  -> focus / capture / privacy policy module
  -> application event OR IntentIngress envelope
  -> Intent Router module
```

### Keyboard and pointer

- Boot recovery starts with a signed serial/keyboard baseline module.
- PS/2 or VirtIO-input is the first QEMU path; xHCI HID follows for hardware.
- Drivers emit normalized key usages, modifiers, pointer deltas/buttons, timestamps,
  and device identity. Text composition and keyboard layout are separate modules.
- The compositor's focus policy chooses the receiving surface. Apps cannot read
  global input unless explicitly granted an input-capture capability.
- Pointer gestures that map to direct UI operations do not require an LLM.

### Voice

Voice is added after reliable keyboard/pointer operation and audio capture.

```text
audio driver -> audio service -> push-to-talk capture -> optional VAD/filter
             -> local or remote ASR -> transcript preview
             -> IntentIngress -> same Intent Router used by keyboard text
```

The first voice mode is push-to-talk with an unmistakable recording indicator,
bounded duration, cancellation, and transcript review. Text-first routing is the
default. Direct audio upload to a multimodal model requires a separate capability,
visible destination, size/time bound, and explicit privacy policy. Always-listen is
not enabled for the first public release.

Audio capture, ASR, language detection, text normalization, and voice routing are
separate modules so they can be fetched, generated, benchmarked, and replaced
independently.

## 9. Command and LLM execution model

The current keyword-based prompt orchestration is a bootstrap aid and is not the
target command architecture.

Every command-capable module registers a typed command schema:

```text
command id / aliases / argument schema / completion provider
required capabilities / risk class / confirmation policy
action endpoint / timeout / undo endpoint / help metadata
```

The Intent Router uses this order:

1. Parse exact shell syntax, shortcuts, and registered deterministic commands.
2. Validate arguments and resolve the registered action endpoint.
3. Execute low-risk direct commands without an LLM after capability/policy checks.
4. Route unknown, ambiguous, conversational, or multi-step intent to an LLM planner.
5. Require the planner to return a typed `ActionPlan`, never executable free text.
6. Validate every action, argument, dependency, capability, risk, and resource bound.
7. Ask for confirmation where policy requires it.
8. Execute through module IPC with bounded retries, cancellation, observation, and a
   maximum planning/tool-call budget.
9. Record results and undo tokens; re-planning receives bounded structured results.

Emergency recovery commands remain deterministic and work without the model,
network, registry, compositor, or ordinary module cache. Destructive commands are
not made safe merely by being syntactically simple: policy and confirmation still
apply.

Keyboard text, a clicked command, and an ASR transcript all become the same
`IntentIngress` type. Input origin and trust metadata remain attached so policy can
distinguish a physical local keypress from remote, generated, or transcribed input.

## 10. Module classes and required failure behavior

| Class | Examples | Required containment |
| --- | --- | --- |
| Bootstrap | host, verifier, supervisor, recovery | Signed offline copy and A/B rollback |
| Driver | VirtIO, NVMe, xHCI, audio | Device reset/rebind or recovery without memory escape |
| Storage | block cache, VFS, filesystem | Transactional state, flush contract, crash recovery |
| Network | Ethernet, TCP, DNS, TLS | Endpoint capability, bounded buffers, parser fuzzing |
| UI | display, input, compositor, shell | Restart without kernel loss; recovery console survives |
| Voice | capture, ASR, router | Visible capture, privacy capability, bounded retention |
| Agent | intent, planner, action broker | Typed plans, policy enforcement, bounded iteration |
| Application | files, terminal, settings | Per-app capabilities and independent crash boundary |

Doom is the reference application-module stress test. It is not privileged and has
no game-specific kernel ABI. Its engine and content are separate packages, and it
uses only generic surface, focused input, audio playback, clock, read-only content,
and private save-directory capabilities. `DOOM_MODULE_ACCEPTANCE.md` is the
normative completion gate.

The supervisor maintains dependency graphs but modules own no hidden fallback in
the kernel. If no safe module is available, the system reports the missing
capability and enters the narrow recovery behavior defined for that service.

## 11. Validation gates

A module feature is complete only when evidence proves:

- the product logic executes outside kernel address space;
- the manifest requests only documented capabilities;
- denied capabilities fail closed;
- CPU, memory, handles, buffers, messages, and time are bounded;
- crash, trap, timeout, malformed input, and dependency loss are contained;
- replacement, state migration, rollback, quarantine, and revocation work;
- the relevant guest QEMU path is exercised, not only a host mock;
- module provenance and exact binary digest are captured in test evidence.

Core-survival evidence also records an unchanged boot ID, monotonic kernel progress,
continued work by an independent witness process, bounded recovery response time,
and complete grant/resource reclamation after fault injection. At minimum, inject
invalid pointers and handles, oversized IPC, allocation/queue exhaustion, infinite
loops, worker runtime crashes, IPC peer death, interrupted activation, and module
host/supervisor failure. Hardware/DMA claims additionally require an enabled IOMMU
and device-specific fault evidence on the declared platform. Every gate states
which fault cases were tested and which assumptions remain unverified.

For visual input features, QEMU evidence includes synthetic keyboard/mouse events
and pixel checks. For voice, it includes a deterministic virtual audio fixture,
recording-state pixels, bounded capture, known transcript fixtures, denial tests,
and routing to a typed action without granting ASR direct action authority.

## 12. Migration rule for current code

Existing code is classified into exactly one disposition:

- `CORE`: minimal kernel mechanism retained and hardened;
- `PROTOCOL`: data types and RIDL contracts shared across boundaries;
- `MODULE`: moved into an isolated package and process;
- `BOOTSTRAP TEMPORARY`: retained behind a measured removal gate;
- `DELETE/REPLACE`: mock, duplicate, unsafe fallback, or misleading completion path.

`src/main.rs`, `src/display.rs`, `src/storage.rs`, `src/net.rs`, `src/voice.rs`,
`src/prompt_orchestrator.rs`, and the broad host imports in `src/core/seed.rs` must be
audited first. Their high-level behavior belongs in modules; only bounded mechanism
and temporary recovery wiring may remain in Rhiza Core.
