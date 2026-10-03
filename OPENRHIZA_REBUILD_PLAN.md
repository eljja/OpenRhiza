# OpenRhiza Independent OS Rebuild Plan

Superseded by the 2026-10-03 new-implementation reset. Read
`next/docs/DIRECTION.md` and `next/docs/ROADMAP.md` for the active plan.
The text below is retained as a historical incremental-rebuild proposal.
Its instruction to use the existing Rust kernel as the starting point no longer
applies. No completion or implementation evidence carries into the new OS.

Status: revised on 2026-08-14; core survival and execution order reviewed on 2026-10-03

Current checkpoint: architecture and baseline investigation are complete. Stage 1
implementation gates have not been completed. Existing boot/CLI smoke tests do not
prove process isolation, core survival under module faults, or desktop completeness.

Review evidence on 2026-10-03: `cargo bootimage` passed. The existing x86_64 QEMU
TCG profile booted and accepted `/status`, `/scheduler-status`, `/smp-status`,
`/wasm-status`, and `/recovery`; see
`logs/x86-core-direction-review-20261003.log`. This is a bootstrap command smoke
result only. The runtime remains a cooperative executor with in-kernel Wasmi,
one online CPU, and AP bring-up marked `stub`. No ring-3 fault-containment or
core-survival gate was passed by this run.

Source findings to address first: `os_rx_packet` allocates kernel heap using a
module-provided length before validating Wasm memory; executor spawn panics on a
full queue; panic/page-fault handlers loop forever; driver host MMIO/DMA paths
still operate in kernel context. These findings keep the survival claim unproven.

## 1. Non-negotiable product decision

OpenRhiza will be a new operating system with its own kernel and userspace. The
public product will not boot or host itself on Linux, BSD, Windows, Android, or an
existing distribution. The existing OpenRhiza Rust kernel is the starting point,
not a demo that will be replaced by another OS.

OpenRhiza must eventually own these layers:

- boot and recovery;
- interrupt, timer, memory, process, thread, and SMP management;
- syscall and capability ABI;
- IPC and service supervision;
- device discovery and driver lifecycle;
- block storage, VFS, filesystems, and data integrity;
- network and security protocols;
- graphics, input, window composition, desktop, and applications;
- audio, power, update, package, and recovery services;
- Wasm skill isolation and LLM action policy.

The primary architectural invariant is **module-first ownership**. Except for the
minimal Rhiza Core kernel, every driver, filesystem, protocol, service, policy,
input pipeline, desktop component, application, Wasm runtime, and LLM integration
is a signed sandbox module. A feature is not complete if its product logic is
implemented as a new kernel branch or a permanently privileged global subsystem.

The canonical module contract, acquisition pipeline, and input/intent architecture
are defined in `OPENRHIZA_MODULE_ARCHITECTURE.md`. That document is normative for
all six stages in this plan.

Core survival is the first release criterion. Faulty or malicious non-core modules
must not panic, corrupt, starve, or reboot the kernel under the declared hardware
assumptions. Watchdog restart and recovery boot are recorded separately from
survival. No stage may infer this property from a module signature or a successful
startup log.

AI generates/acquires implementations and interprets intent. Installed modules run
their service contracts with no mandatory inference in the data path. Boot, direct
commands, scheduling, input, storage, and installed applications work with the model
and registry disconnected. Model jobs remain bounded, replaceable modules.

KDE Plasma and GNOME cannot be imported as the initial OpenRhiza desktop. Their
current implementations assume a large Linux/POSIX graphics, input, process,
device, and desktop stack. Porting one before OpenRhiza has a stable userspace ABI,
shared memory, process model, libc/POSIX layer, dynamic loader, graphics stack, and
Wayland services would be harder than building the initial OpenRhiza desktop.

OpenRhiza will therefore build a native desktop first. A Wayland-compatible server
or KDE/GNOME port can be evaluated later as an application-compatibility project,
not as a kernel dependency.

## 2. What "from scratch" means

From scratch does not mean ignoring standards or rewriting every algorithm. It
means OpenRhiza controls the runtime and can boot without another operating system.

Allowed after audit:

- public specifications such as UEFI, ACPI, PCIe, APIC, USB, NVMe, AHCI, VirtIO,
  Ethernet, ELF, DWARF, TLS, Unicode, and Wayland wire protocols;
- `no_std` Rust crates and portable libraries that are linked into OpenRhiza or
  ported to its ABI;
- bootstrapping with QEMU and cross-compilers hosted on a development machine;
- firmware interfaces needed to start supported hardware;
- source reuse with compatible licenses, pinned versions, tests, and ownership.

Not allowed as a shipped dependency:

- a Linux or other third-party kernel under the OpenRhiza services;
- Linux processes, syscalls, namespaces, cgroups, device nodes, or service manager;
- delegating normal boot, drivers, filesystems, networking, desktop, or recovery to
  another installed OS;
- claiming a mocked or host-only implementation is an OpenRhiza guest feature.

Firmware is part of the hardware platform and is not an OS dependency. Build tools
are development dependencies and are not part of the runtime architecture.

## 3. Re-audited starting point

The existing code is a valuable bootstrap kernel, but the earlier desktop 1-6
completion record describes a static demonstration rather than a modern OS.

| Area | Current implementation | Required state |
| --- | --- | --- |
| Boot | x86_64 bootimage and QEMU path | UEFI boot, recovery, image/update contract |
| Scheduling | Cooperative async executor | Preemptive threads and isolated processes |
| SMP | Discovery and heartbeat scaffolding | AP startup, scheduler, TLB shootdown, locking |
| Memory | One static 32 MiB kernel heap | Physical/virtual allocators and per-process VM |
| Faults | Basic IDT and fatal loops | User-fault containment and kernel crash capture |
| User ABI | No ring-3 process/syscall model | Versioned syscall, handles, IPC, ELF loader |
| Wasm | In-kernel Wasmi with broad imports | User process host with quotas and capabilities |
| Desktop | Four static seed windows | Real client surfaces and independent apps |
| Input | Handoff mirror; QEMU reports inactive sandbox routes and raw drops | Bounded module routes, focus, and backpressure |
| Voice | Persisted mode/route and log-only skill seed | Real audio capture, ASR, privacy, and intent routing |
| Commands | Exact local commands; other text forwarded to one model | Typed registry, direct actions, planner, policy, and undo |
| Storage | ATA PIO and fixed-slot FAT16 cache | Async block layer, VFS, directories, safe writes |
| Drivers | Native paths, probes, and mocks | Restartable drivers with real guest/device tests |
| ARM64 | Separate recovery/framebuffer proof | Architecture-neutral core and platform parity |
| Registry | Guest signature verification | Authenticated, revocable, auditable supply chain |
| Testing | Host probes and serial-string QEMU smoke | Unit, fault, input, pixel, fuzz, and soak gates |
| CI | Nexus deployment only | Kernel, userspace, image, and QEMU release pipeline |

The largest structural risks are concentrated ownership in `src/main.rs` and
`src/display.rs`, an in-kernel high-level network/LLM loop, missing process
isolation, and tests that can pass without exercising a guest hardware path.

## 4. Target architecture

OpenRhiza should use a capability-oriented hybrid microkernel architecture. The
kernel remains small enough to audit but can keep performance-critical memory,
scheduler, IPC, and interrupt mechanisms in kernel space.

```text
UEFI firmware
  -> OpenRhiza boot manager
  -> signed kernel + initial service image
  -> kernel
       architecture / interrupts / timers / SMP
       physical and virtual memory
       threads / processes / scheduler
       capabilities / handles / IPC
       interrupt and DMA mediation
       debug console / crash capture
  -> signed bootstrap module host (isolated user process)
       -> supervisor and module resolver modules
       -> device and user-mode driver modules
       -> block / VFS / filesystem modules
       -> network / TLS / name-service modules
       -> display / input / audio / power modules
       -> package / update / recovery modules
       -> Wasm worker and LLM action-policy modules
       -> native compositor and independent application modules
```

### Kernel boundary

The kernel is responsible only for mechanisms that cannot safely live elsewhere:

- privilege transitions and process address spaces;
- preemptive scheduling, timers, SMP coordination, and synchronization;
- capabilities, handle tables, IPC queues, and shared-memory grants;
- interrupt routing and bounded mapping of MMIO, I/O ports, and DMA resources;
- minimal early console, panic capture, and recovery handoff.

The kernel must not contain LLM prompts, registry workflows, GUI application state,
TLS policy, package selection, or general filesystem policy.

Its API accepts bounded opaque messages and capability transfers; service-specific
schemas and parsers execute in modules. Recovery resource reservations and a sealed
mechanism to reinstantiate the signed recovery set are kernel mechanisms. General
supervision, module selection, and recovery UI remain module-owned policies.

The kernel loads one signed bootstrap user process from the initial image. That
process is not part of the kernel and has no ambient authority; it receives only
the initial capability set needed to resolve, launch, supervise, and replace
modules. Bootstrap and recovery modules use the same package and sandbox rules as
downloaded modules.

### Process and capability model

- Native applications and services use ELF64 executables.
- Every process has a separate page table and explicit handle table.
- Syscalls operate on unforgeable handles, not global paths or raw device numbers.
- IPC carries typed messages and transferred capabilities.
- A process can access files, sockets, devices, display surfaces, and LLM tools only
  through capabilities granted by its launcher or user policy.
- Process exit, crash, timeout, and resource exhaustion are observable and contained.
- Compatibility syscalls are adapters above the native ABI, never the security root.

### Driver model

Boot-critical interrupt and platform setup starts in the kernel. Device logic runs
in supervised user-mode drivers whenever hardware permits it.

The first QEMU platform is deliberately narrow:

- x86_64, ACPI, local APIC, I/O APIC, HPET/TSC;
- PCI discovery and MSI/MSI-X;
- VirtIO block, network, GPU, input, and sound;
- serial console and a basic UEFI framebuffer fallback.

The first real-hardware platform adds NVMe, xHCI HID, one Ethernet controller, and
UEFI GOP software composition. Wi-Fi, Bluetooth, accelerated vendor GPUs, broad
laptop power management, and arbitrary hardware are later release tracks.

DMA-capable drivers receive only explicitly mapped buffers. IOMMU isolation is a
release requirement for hardware that exposes it; driver restart and device reset
must not require a kernel reboot where the device supports recovery.

### Storage and VFS

- A block service provides asynchronous, bounded requests and flush semantics.
- The VFS owns mounts, paths, directories, metadata, permissions, handles, and cache
  coherency above filesystem services.
- Initial filesystems are `tmpfs`, `devfs`, a read-only boot image, and a small
  crash-tested writable system/user filesystem.
- The writable filesystem must define atomic rename, durable flush, journal or
  copy-on-write recovery, checksums, fsck behavior, and power-loss tests before it
  stores user data.
- FAT is limited to the EFI/transfer role; the current fixed-slot FAT16 cache does
  not become the general filesystem.

### Graphics and desktop

- The display service owns scan-out buffers and mode information.
- The compositor owns surfaces, z-order, damage, focus, workspaces, decorations,
  pointer capture, keyboard routing, clipboard, notifications, and accessibility.
- Applications render into granted shared-memory buffers and submit bounded damage.
- The initial renderer is an optimized software compositor over UEFI GOP and
  VirtIO-GPU 2D. Hardware 3D is not a first-alpha claim.
- The native surface protocol is versioned and transport-independent. A later
  Wayland compatibility service may translate common Wayland/xdg-shell requests to
  native surfaces without changing the kernel ABI.

### Application and development model

- The native SDK provides process, IPC, file, network, time, input, surface, audio,
  and capability APIs.
- A custom Rust target and a port of the required Rust standard-library portions
  are maintained as release artifacts.
- A small libc/POSIX compatibility library is added only after native contracts are
  stable; broad POSIX compatibility is not a Stage 1-4 completion criterion.
- Native system services use ELF64; untrusted apps and skills prefer Wasm workers.
- Packages contain immutable content digests, manifests, capabilities, signatures,
  dependencies, and rollback metadata.

### Wasm and LLM boundary

Wasmi or a later runtime must execute in an ordinary isolated user process. A Wasm
trap, infinite loop, or runtime defect must not execute in kernel context.

The LLM never receives raw MMIO, DMA, syscall, filesystem, network, or package
installation authority. All model-driven changes follow this path:

```text
user intent -> typed action proposal -> policy check -> optional confirmation
            -> bounded service transaction -> result and audit -> optional undo
```

Model output, documents, terminal output, web content, and downloaded skills are
untrusted data. Deterministic services outside the model enforce every permission.

## 5. Six sequential stages

Each stage begins with a baseline report and ends with changed files, test results,
QEMU evidence, known limitations, and the next stage. The next stage cannot start
until the current QEMU gate passes.

### Stage 1: Reproducible kernel foundation

Objective: turn the current prototype into a trustworthy kernel baseline without
adding more desktop demonstrations.

Deliverables:

- Record architecture decisions for the native ABI, capability model, process
  boundary, module invariant, bootstrap host, service ownership, supported QEMU
  machine, and dependency policy.
- Separate generated binaries, images, screenshots, logs, caches, certificates,
  and test fixtures from source; revoke and replace exposed/demo keys.
- Split oversized bootstrap modules into kernel mechanism, service protocol, and
  temporary compatibility ownership without behavior changes. Inventory boundaries
  first; defer large service moves until Stage 2 supplies isolated processes.
- Pin toolchains and dependencies; create one clean build command and image layout.
- Add kernel unit tests where host-independent and deterministic QEMU CI for boot,
  serial CLI, panic capture, and recovery.
- Define the signed module manifest, interface IDL, capability vocabulary, lifecycle,
  provenance, and compatibility rules before moving more code into modules.
- Audit externally reachable host functions for unchecked lengths, fatal allocations,
  global grants, queue panics, and unsafe MMIO/DMA. Give each temporary kernel-side
  path a removal gate; interim hardening does not count as hardware isolation.
- Define executable core invariants and independent negative-test fixtures before
  accepting generated code. Keep initial v0 interfaces small enough to test without
  first building a new general-purpose IDL compiler.
- Replace conflicting completion documents with one evidence-based validation
  matrix that distinguishes mock, host, guest, emulator, and hardware evidence.

QEMU gate:

- A clean clone builds the same x86_64 image with pinned tools.
- The image boots to the recovery command console three consecutive times.
- A deliberate kernel test panic writes a diagnostic record and reboots to recovery.
  This proves last-resort recovery only and explicitly fails core survival.
- CI captures QEMU version, command line, serial log, exit status, and timeout.
- No hard-coded GUI label or unconditional mock is counted as feature completion.
- A boundary audit proves that high-level policy and product logic did not move into
  Rhiza Core while the baseline was being cleaned up.

### Stage 2: Preemptive multiprocess kernel

Objective: establish the minimum security and multitasking substrate of a modern OS.

Deliverables:

- Physical page allocator, kernel virtual memory manager, per-process page tables,
  guarded stacks, shared-memory mappings, and copy-in/copy-out validation.
- Preemptive priority scheduler with timer accounting, sleep/wake, cancellation,
  bounded queues, and starvation tests.
- Ring-3 ELF64 process loader, threads, process lifecycle, stable syscall numbers,
  handle tables, capabilities, and typed IPC.
- Real AP startup, per-CPU state, scheduler participation, synchronization, and TLB
  shootdown for the supported QEMU CPU count.
- User-fault termination, kernel panic capture, process dumps, and supervisor restart.
- A minimal user-mode `init`, command shell, logger, and test processes.
- A signed bootstrap module host that launches the shell, logger, and supervisor as
  replaceable sandbox modules with no ambient kernel authority.
- An isolated user-mode Wasm runtime and host broker with per-module capabilities,
  fallible memory limits, fuel/deadlines, bounded IPC, and termination. This foundation
  is required here, before driver/services/desktop migration in Stages 3 and 4.
- Reserved recovery execution and memory budgets plus a sealed path to replace
  failed bootstrap workers with the signed recovery set.
- Property/model checks for handle ownership and revocation, scheduler progress,
  allocator boundaries, and syscall validation; record the limits of each proof.

Substage order: single-core ring-3 execution and timer preemption; protected IPC and
resource admission; isolated Wasm workers and negative tests; recovery under worker
and supervisor failure; then SMP with separate concurrency and TLB gates. Each
substage passes before the next. Full SMP is not a prerequisite to proving the
first single-core module-containment boundary.

QEMU gate:

- Four isolated user processes make observable progress under timer preemption.
- Invalid pointer, illegal instruction, divide-by-zero, stack overflow, and process
  crash terminate only the offending process.
- A busy-loop process cannot starve the recovery shell or watchdog.
- Two or more virtual CPUs schedule work and pass IPC/memory stress tests.
- Capability transfer succeeds only through the defined IPC path; forged handles
  and cross-process memory access fail closed.
- Infinite-loop, invalid-pointer, huge-allocation, queue-flood, malformed-syscall,
  runtime-crash, and supervisor-failure fixtures preserve the same boot ID and an
  independent witness/recovery process. A watchdog reset is a failed survival run.

### Stage 3: User-mode platform services

Objective: move from a kernel demo to an OS that can persist data and communicate
through restartable services.

Deliverables:

- Service supervisor, names, dependency ordering, readiness, health, restart,
  backoff, and critical-service recovery policy.
- Registry resolver and local module-cache services with digest, signature,
  compatibility, provenance, capability, and rollback enforcement.
- Device manager with PCI enumeration, resource grants, interrupt delivery, DMA
  buffers, driver binding, reset, and crash recovery.
- User-mode VirtIO block and network drivers; bounded kernel bootstrap fallback only
  where necessary to preserve recovery.
- Block service, cache, VFS, mount namespace, `tmpfs`, `devfs`, read-only boot image,
  and a tested writable filesystem.
- Network service with Ethernet, ARP, IPv4, ICMP, UDP, TCP, DHCP, DNS, entropy, and
  TLS client support behind socket capabilities.
- Versioned file, socket, clock, random, and service IPC APIs for applications.
- Every service and driver above the minimal kernel boundary is packaged and tested
  through the same module lifecycle; no direct static fallback becomes permanent.

QEMU gate:

- Create directories/files, rename, flush, reboot, remount, and verify content.
- Inject power loss at defined write points and recover without silent corruption.
- Download a signed test package over TLS and reject a bad certificate or signature.
- Kill block and network drivers; the supervisor recovers them or enters recovery
  without corrupting memory or hanging the shell.
- A process without file/socket capability cannot use the relevant service.

### Stage 4: Native graphics, input, and desktop

Objective: replace static seed windows with a genuine multitasking desktop.

Deliverables:

- User-mode VirtIO-GPU/display and VirtIO-input drivers plus framebuffer fallback.
- Keyboard and pointer input modules that normalize device events into one bounded
  input protocol without granting applications raw device access.
- Shared surfaces, damage tracking, software composition, cursor, font/text service,
  input routing, focus, pointer capture, and keyboard layouts.
- Compositor process with move, resize, close, minimize, maximize, workspaces,
  launcher, task switcher, clipboard, notifications, and multiple resolutions.
- Independent terminal, file manager, settings, system monitor, and skill manager
  processes using the native SDK.
- Screenshot/pixel tests, synthetic keyboard/mouse input, layout tests, crash tests,
  frame-time metrics, and long-duration desktop soak tests.
- Recovery console remains available if the compositor or display driver crashes.

QEMU gate:

- At least five independent applications run concurrently as separate processes.
- Automated input focuses, drags, resizes, minimizes, switches, and closes windows.
- Pixel checks prove expected surfaces are visible, nonblank, correctly ordered, and
  non-overlapping at every supported QEMU resolution.
- Crashing an application, compositor, or display driver preserves the kernel and
  recovery path; supervised components restart according to policy.
- An eight-hour QEMU soak has no unbounded memory growth or stalled input.

### Stage 5: Secure applications, Wasm, and LLM operations

Objective: make OpenRhiza AI-native without making the model or skill runtime a
privileged part of the kernel.

Deliverables:

- Isolated Wasm worker processes with per-module imports, memory ceilings, fuel or
  epoch interruption, deadlines, output limits, cancellation, and quarantine, built
  on the already-passing Stage 2 foundation. This stage expands application and
  generation integration rather than introducing isolation for the first time.
- A module forge workflow where OpenRhiza.com can return an existing signed module
  or request LLM-assisted source generation, but cannot skip hermetic build,
  analysis, QEMU tests, signature policy, canary activation, or rollback.
- Signed package and skill manifests with explicit file, network, device, display,
  audio, and model capabilities.
- Real terminal/LLM, files, settings, monitor, skill store, and package applications;
  no placeholder data or kernel-drawn application panels.
- A downloadable Doom/FreeDoom demonstration consisting of a GPL-compliant engine
  module and a separately signed FreeDoom content module. It must install from the
  registry, launch in its own surface/process, accept real input, play at the
  correct game clock, produce audio, persist saves, survive minimize/restore, and
  uninstall without adding Doom-specific code or imports to Rhiza Core.
- Typed action catalog, policy engine, confirmation, transaction, cancellation,
  audit, and undo framework.
- Prompt-injection boundaries and tests for terminal, document, web, package, skill,
  and tool output; deterministic policy is always authoritative.
- A unified Intent Router: exact registered commands execute locally through typed
  module actions; unknown, ambiguous, or multi-step requests go to an LLM planner,
  whose typed action plan is independently authorized and bounded.
- Push-to-talk voice modules for audio capture, visible recording state, bounded
  buffering, ASR, transcript review, and routing through that same Intent Router.
  Always-listen and direct-audio model upload remain later opt-in capabilities.
- Offline operation: boot, desktop, files, settings, recovery, and installed apps
  work when no model or network is available.

QEMU gate:

- Infinite-loop, memory-exhaustion, trap, invalid-import, and forged-signature Wasm
  modules are contained without blocking the desktop or recovery shell.
- LLM-requested actions cannot exceed the invoking user's capabilities.
- Destructive actions require policy-defined confirmation and produce complete
  audit records; interrupted transactions recover to a defined state.
- The full desktop remains usable with the network and model service disabled.
- Keyboard, pointer, and voice transcripts produce the same typed intent envelope;
  disabling voice cannot affect keyboard/mouse command handling.
- The Doom demo passes `DOOM_MODULE_ACCEPTANCE.md`, including registry download,
  signature rejection, deterministic input/state/pixel evidence, 35 Hz simulation,
  focused keyboard/mouse control, save/load after reboot, audio, resource limits,
  crash isolation, and offline relaunch after installation.

### Stage 6: Update, hardware, security, and public release

Objective: turn the independent QEMU OS into a narrowly supported public alpha,
then a beta, without overstating compatibility.

Deliverables:

- Signed UEFI boot chain, signed kernel/service/package images, key rotation,
  revocation, immutable digests, transparent history, SBOM, provenance, and license
  inventory.
- A/B system images, health confirmation, rollback, interrupted-update recovery,
  factory reset, user-data migration, backup, and restore.
- Authenticated registry administration, least-privilege roles, schema and size
  limits, abuse controls, signing-service isolation, and staged release channels.
- First hardware target with UEFI GOP, ACPI/SMP, NVMe, xHCI keyboard/mouse, one
  Ethernet controller, RTC, shutdown/reboot, and documented limitations.
- Install media, diagnostics, crash collection, test mode, privacy controls,
  security reporting, support lifecycle, developer SDK, and application signing.
- Fuzzing, adversarial review, power-loss testing, repeated install/update/rollback,
  multi-day soak, and release-candidate dogfooding.
- Module-generation provenance records including specification, source and binary
  digests, toolchain, dependency graph, test evidence, signer, model identity where
  applicable, promotion history, and revocation state.

QEMU and hardware gate:

- A clean install completes two signed upgrades and one rollback in QEMU.
- Corrupt, replayed, revoked, downgraded, or unsigned privileged artifacts fail
  closed while recovery remains available.
- Power loss during installation, filesystem mutation, or update never leaves the
  declared machine unrecoverable.
- Every declared hardware model passes the same boot, input, display, storage,
  network, update, recovery, and soak checklist.
- No critical security finding, demo key, unsigned privileged component, silent data
  corruption, or manual-only recovery step remains.

## 6. Completion criteria and scope control

"Complete" means every published claim has a repeatable test, failure behavior,
and recovery path. It does not mean every PC, GPU, Wi-Fi chipset, POSIX application,
or Linux desktop application is supported.

The first public alpha should declare a narrow environment:

- x86_64 QEMU as the primary reference platform;
- one explicitly named real hardware target only after Stage 6 qualification;
- software-composited desktop with no promise of accelerated 3D;
- OpenRhiza native and Wasm applications, not arbitrary Linux binaries;
- wired network before broad Wi-Fi support;
- one stable display, storage, input, and update configuration.

Broad GPU, Wi-Fi, Bluetooth, laptop sleep, battery, audio-device, ARM64, POSIX, and
third-party desktop compatibility are post-alpha programs with their own gates.

For a small experienced team, a credible QEMU developer preview is a multi-quarter
effort and a hardware-limited public beta is a multi-year program. A solo effort
must narrow hardware and application scope further. These are engineering planning
constraints, not reasons to compromise the independent-OS requirement.

## 7. Rules for every future prompt

At the start, report:

- current branch and dirty-worktree summary;
- last completed stage and substage;
- last passing build and QEMU evidence;
- exact objective and gate for the current prompt.

At completion, report:

- completed stage/substage and changed files;
- commands and tests run;
- QEMU configuration, evidence, and result;
- limitations, regressions, and blockers;
- whether the gate permits the next substage.

Never count static labels, direct test-only exports, unconditional mocks, host-only
probes, or serial substring presence as completion of an interactive guest feature.
Visual and input behavior requires guest execution, synthetic input, and pixel
evidence. Hardware behavior requires declared emulator or physical-device evidence.

## 8. Immediate next action

The 2026-10-03 ecosystem review in `OPENRHIZA_COM_PLAN.md` extends the inventory
beyond the kernel: Nexus/public boards, shared operational evidence, module forge,
host generation tools, device candidate simulations, self-hosting, semantic
storage, voice, compatibility and platform expansion. Stage 1 must freeze the
OS/server manifest/API/evidence contracts together, not as unrelated projects.

Joint stage gates remain sequential: Stage 2 proves local containment; Stage 3
requires authenticated enrollment/TLS, mandatory signed immutable delivery and
real driver/service activation; Stage 4 supplies real app/window lifecycles;
Stage 5 adds the independently validated forge, intent/voice/semantic capabilities
and Doom; Stage 6 qualifies public governance, privacy, backups and releases.
Security prerequisites for Stage 3 must not be postponed until Stage 6.

Review verification: Nexus lint passed with two unused-input warnings; public
boards/health returned HTTP 200. The existing x86_64 QEMU TCG image passed
`logs/x86-ecosystem-review-20261003.log`, including native DNS and public
skill/workflow queries plus recovery return. This is connectivity/command
evidence, not signed delivery, forge completion, or core-survival evidence.
No Stage 1 gate is newly complete and no runtime source was changed by this review.

Begin Stage 1 only. The first substage is an evidence-preserving repository and
architecture baseline:

1. Inventory tracked/generated artifacts, keys, binaries, caches, dirty changes, and
   module-controlled paths that can currently panic, allocate, block, or access
   device registers in kernel context.
2. Write ADRs for the independent OS boundary, hybrid microkernel, native ABI,
   process/capability model, universal module invariant, bootstrap module host, and
   first QEMU platform.
3. Define the clean image layout and pinned one-command build.
4. Freeze module manifest v0, interface IDL v0, capability vocabulary v0, and the
   local/cache/registry/LLM resolution state machine.
5. Add a deterministic QEMU baseline that verifies boot, recovery, and panic capture.
   Specify boot-ID/witness-process fault tests for Stage 2 separately; the legacy
   kernel cannot claim those gates before its process boundary exists.
6. Update the validation matrix so later work cannot be marked complete by mocks.

No additional desktop feature work starts until this baseline gate passes.
