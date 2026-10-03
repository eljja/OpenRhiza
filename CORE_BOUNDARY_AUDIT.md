# OpenRhiza Core Boundary Audit

This document is the working audit for deciding what belongs in the OpenRhiza core and what must move into sandboxed capabilities.

The goal is not to make the current core look ideal. The goal is to label reality clearly so future Codex, Gemini, and OpenRhiza-internal work keeps shrinking the core instead of normalizing bootstrap debt.

## Boundary Tags

### Permanent Core

Code that may remain in the core because the machine cannot safely boot, recover, isolate, validate, or roll back without it.

Allowed responsibilities:

- CPU boot and interrupt substrate
- allocator and minimal runtime substrate
- recovery display and recovery input
- Wasm sandbox execution boundary
- bounded host ABIs
- hardware discovery and capability handle creation
- minimal registry/network bootstrap until sandbox networking can replace it
- validation, rollback, and persistence gates

### Bootstrap Fallback

Code that is acceptable only while OpenRhiza is still bootstrapping itself.

Rules:

- it must have a documented replacement path
- it must stay narrow
- it must not absorb policy or high-level behavior
- it must preserve recovery paths
- it should become a sandbox driver, skill, workflow, or object capability as soon as the host ABI supports it

### Must Move To Skill

Code that is not a permanent survival requirement and should not be treated as core architecture.

Allowed temporary reason:

- no sandbox artifact exists yet
- no host ABI exists yet
- moving it now would break recovery input, display, storage, or registry access

Otherwise, it should become a skill, workflow, driver, program, or sandbox-owned object.

## Current Module Classification

| Module or Area | Current Tag | Why | Target Boundary |
| --- | --- | --- | --- |
| `src/main.rs` | Bootstrap Fallback | It owns too much orchestration, boot autorun, service API phase routing, skill load flow, GUI transition flow, and autonomy scheduling. | Split into minimal boot core plus sandbox-owned boot workflow/orchestrator. |
| `src/core/seed.rs` | Permanent Core | In-kernel Wasm runtime and host import boundary are central to the OS model. It now tracks per-module health, consecutive traps, quarantines repeatedly trapping modules so one broken capability cannot keep consuming poll/frame-pump budget, and exposes explicit operator recovery for quarantined modules. | Keep, but reduce policy. Add quota/accounting and keep imports handle-scoped. |
| `src/task/executor.rs` | Permanent Core | Scheduler substrate is required for basic runtime progress. | Keep; evolve to quotas and SMP without embedding capability policy. |
| `src/task/keyboard.rs` | Bootstrap Fallback | Recovery input is core, but rich keyboard decoding, IME, CLI routing, and debug commands are larger than the final core should own. | Keep minimal recovery decoder; move input parsing/IME/keymaps to sandbox input skills over HID handoff. |
| `src/vga.rs` | Permanent Core / Bootstrap Fallback | Recovery text surface is permanent. Shared GUI composer state and richer input editing are bootstrap debt. | Keep recovery shell only; move rich shell/composer behavior behind display/input skills. |
| `src/display.rs` | Bootstrap Fallback, Must Shrink | It contains framebuffer presenter, GUI layout, object runtime, pointer redraw, text rendering, and GUI mutation application. Too much long-term GUI behavior is in core. | Keep display handoff, recovery framebuffer, object handles, validation, rollback. Move GUI shell/layout/render policy to skills/renderers. |
| `src/gui_contract.rs` | Permanent Core Candidate | Shared object contract is acceptable as ABI/schema if kept small. | Keep as stable scene/mutation ABI. Avoid toolkit policy. |
| `src/gui_lvgl_bridge.rs` | Must Move To Skill | A toolkit-style bridge is not survival core. | Move to sandbox renderer/skill or keep only as host-side reference. |
| `src/gui_font.rs` | Bootstrap Fallback | Rendering requires glyph access, but font ingestion/selection is not core. | Keep only validated atlas reader if required; move font parsing/atlas generation to font skill/workflow. |
| `src/hangul.rs` | Must Move To Skill | Korean IME is valuable, not a survival core requirement. | Move to input/text skill once sandbox text input ABI is stable. |
| `src/input_handoff.rs` | Permanent Core | Raw input handoff, routing gates, and per-kind packet/event counters are core boundary responsibilities. | Keep transport/observability only; move parsing policy. |
| `src/input_runtime.rs` | Bootstrap Fallback | Runtime activation state is needed now; policy should not grow here. | Keep activation/rollback gate; move parser behavior to sandbox input drivers. |
| `src/arch/x86_64/usb.rs` | Bootstrap Fallback, Must Move Driver Logic | Native xHCI/HID support keeps input alive today, but real USB driver logic belongs outside core. | Keep emergency transport/handoff if needed; move xHCI/HID parsing to sandbox driver artifacts. |
| `src/keyboard.rs` | Bootstrap Fallback | PS/2 set 1 decoding is a recovery fallback, but full keyboard logic is not final core. | Keep only minimal recovery decoder; move layout/keymap/IME to skills. |
| `src/e1000.rs` | Must Move To Sandbox Driver | Native NIC driver is large device-specific logic. It remains only because registry/Gemini access currently depends on it. | Replace with `drv_e1000_*` sandbox driver using `DRIVER_HOST_ABI.md`. |
| `src/net.rs` | Bootstrap Fallback | Network stack is currently required for OpenRhiza/Gemini access. | Keep minimal packet queues and network ABI; move device driver and higher policy out. |
| `src/dns.rs` | Bootstrap Fallback | DNS is needed for bootstrap network access but is not a kernel principle. | Eventually capability/transport skill or small reusable network service object. |
| `src/https.rs` | Bootstrap Fallback | OpenRhiza/Gemini access is currently required. HTTP/API policy should not become core. | Keep only until sandbox transport/client capabilities can carry API calls. |
| `src/tls.rs` | Bootstrap Fallback | TLS is needed now for registry/Gemini. Long-term it should be a transport capability if feasible. | Keep narrow; isolate from API policy; consider sandbox transport later. |
| `src/api_v1.rs` | Must Move To Workflow/Skill | Registry payload assembly, comments, votes, and API domain logic are policy/application behavior. | Move to registry workflow skill; core keeps only signed/validated transport gate if needed. |
| `src/prompt_orchestrator.rs` | Must Move To Workflow | Prompt routing and action planning are not core. | Move to workflow skill and autonomy council objects. |
| `src/autonomy.rs` | Must Move To Workflow | Autonomy planning/council behavior is high-level policy. | Core keeps mode/interval gate only; agents and evidence gathering become skills/workflows. |
| `src/storage.rs` | Bootstrap Fallback | ATA/FAT fixed-slot cache is necessary now, but general storage/filesystem logic must not grow here. | Keep minimal fixed-slot bootstrap read/write floor; move storage protocol and filesystems to sandbox drivers/skills. |
| `src/storage_host.rs` | Permanent Core | Bounded block-object host ABI is the correct core boundary for filesystem skills. | Keep; expand carefully with handles, quotas, and scratch guards. |
| `src/driver_host.rs` | Permanent Core | Handle-scoped PCI/MMIO/PIO/DMA/IRQ ABI is the correct boundary for sandbox drivers. It now enforces per-handle capability bits. | Keep; harden quotas, validation, and promotion gates. |
| `src/driver_runtime.rs` | Permanent Core Candidate | Binding lifecycle, validation stage, and rollback gate are core governance. | Keep only lifecycle/gate state; driver policy belongs to workflows. |
| `src/runtime_bindings.rs` | Permanent Core Candidate | Binding registry is needed for live object activation. | Keep; avoid embedding driver-specific policy. |
| `src/component_runtime.rs` | Permanent Core Candidate | Object lifecycle stage and rollback model match OpenRhiza philosophy. | Keep small and generic. |
| `src/sandbox_lifecycle.rs` | Permanent Core Candidate | Generic sandbox lifecycle is core boundary state. | Keep small and generic. |
| `src/skill_runtime.rs` | Bootstrap Fallback / Permanent Boundary | Skill activation, lifecycle gates, and artifact recovery are needed; skill semantics should not live here. | Keep loader/recovery/gates generic; move skill behavior to artifacts. |
| `src/skill_cache.rs` | Bootstrap Fallback | Fixed-slot cache is bootstrap storage policy. Artifact-keyed payload caching and signed metadata ranking are needed so recovery can retry replacement files without stale Wasm bytes or preferring unsigned stale candidates; `/skill-cache-rank-selftest` keeps this generic ranking rule testable from recovery. | Keep until richer storage/registry skill cache exists; richer replacement policy belongs in cache/registry workflow logic. |
| `src/driver_cache.rs` | Bootstrap Fallback | Local driver cache helps later boots, but rich cache policy belongs outside core. | Keep fixed-slot minimum; move policy to registry/cache workflow. |
| `src/capability_cache.rs` | Bootstrap Fallback | Useful for local context, but policy and semantic ranking should move out. | Keep minimal cache read/write; move ranking/query policy to skill. |
| `src/semantic_graph.rs` | Must Move To Skill | Semantic indexing is explicitly non-core. | Move to `skill_semantic_graph_index_v1`; core may expose storage/query ABI. |
| `src/security.rs` | Permanent Core | Trust anchors and signature verification gates are core. | Keep minimal and auditable. |
| `src/identity.rs` | Permanent Core | Hardware identity and node identity are required for registry trust and matching. | Keep, but avoid policy-heavy profiling. |
| `src/firmware.rs` | Bootstrap Fallback | Local firmware/artifact scan is bootstrap storage policy. | Move to cache/registry skill when stable. |
| `src/wifi_mac.rs` | Bootstrap Fallback | Synthetic identity helper, not core principle. | Keep only if needed for identity; otherwise move to identity workflow. |
| `src/smp.rs` | Permanent Core | CPU topology and AP bring-up are core runtime substrate. | Keep; move scheduling policy out where possible. |
| `src/allocator.rs` | Permanent Core | Required runtime substrate. | Keep. |
| `src/crypto/*` | Permanent Core / Bootstrap Fallback | Crypto is required for trust/TLS now. | Keep trust primitives; avoid protocol policy. |
| `src/arch/x86_64/discovery.rs` | Permanent Core | Hardware discovery and identity are necessary. | Keep; expose handles instead of policy. |
| `src/arch/x86_64/interrupts.rs` | Permanent Core | Required CPU/IRQ substrate. | Keep. |
| `src/arch/x86_64/apic.rs` | Permanent Core | Required interrupt/SMP substrate. | Keep. |
| `src/arch/x86_64/serial.rs` | Permanent Core | Recovery/debug output is survival infrastructure. | Keep. |
| `src/arch/x86_64/port.rs` | Permanent Core | Low-level primitive needed by bounded ABIs. | Keep internal; do not expose raw access to drivers. |

## Platform Kernel Boundary

| Module or Area | Current Tag | Why | Target Boundary |
| --- | --- | --- | --- |
| `platform-kernels/aarch64-recovery` | Permanent Core Prototype | It is a standalone ARM64 survival proof for CPU entry, stack, PL011 serial recovery, and local prompt. | Keep deliberately tiny. Add only sandbox host ABI stubs and platform handles needed for survival. |
| `run_qemu_arm64.ps1` and `tools/smoke_arm64_recovery.py` | Tooling | These validate the ARM64 recovery path from the host. | Keep as verification tools; do not imply the full OS is ARM64-ready. |

The ARM64 path must not absorb virtio-net, virtio-block, virtio-gpu, GUI, filesystem, voice, or policy logic into the core. Those belong behind sandbox host ABIs and registry-loaded capabilities.

Current enforcement note: the default ARM64 recovery ELF now denies GUI, virtio payload, production routing, policy persistence, and LLM policy commands in core. Any future reintroduction must be a sandbox skill/workflow or an explicit proof harness outside the survival kernel path.

## Priority Migration Sequence

The migration order is intentionally practical. OpenRhiza must stay usable after every step.

### 1. e1000 native driver to sandbox driver

Why first:

- `src/e1000.rs` is large, device-specific, and already maps well to `DRIVER_HOST_ABI.md`.
- Network is essential for registry/LLM access, so the migration must be staged with native fallback.

Target:

- create or fetch `drv_e1000_sandbox_v1`
- claim `pci:8086:100e`
- allocate DMA through `os_driver_dma_*`
- drive RX/TX through network host queues
- keep native e1000 as fallback until sandbox driver passes sustained traffic
- use the current `sandbox-candidate` stage first; only `sandbox-active` may reroute TX/RX to Wasm

Success criteria:

- OpenRhiza reaches OpenRhiza.com/Gemini through sandbox e1000
- native driver can be disabled without losing recovery path
- failed sandbox driver rolls back to native fallback

Current migration progress:

- `qemu_driver_pack_seed` now marks e1000 as `sandbox-candidate` without disrupting live traffic and only claims baseline PCI identity. Device-specific MMIO/PIO/DMA/IRQ validation is delegated to dedicated sandbox probe skills.
- `skill_e1000_sandbox_probe_v1` now claims `pci:8086:100e` from a separate Wasm artifact, probes PCI/MMIO/DMA/IRQ/TX queue host imports, verifies sandbox DMA readback for descriptor memory, and remains non-disruptive.
- Driver handles now enforce requested capability bits (`PCI config`, `MMIO`, `PIO`, `DMA`, `IRQ`) instead of treating a claimed device as broad raw access.
- A dedicated network promotion ABI now accepts sandbox e1000 health evidence but denies `sandbox-active` until claimed device, PCI/MMIO/DMA/IRQ, TX handoff, and sustained RX/TX validation are all present.
- `src/net.rs` has explicit `native-fallback`, `sandbox-candidate`, and `sandbox-active` stages.
- `/net-driver-status` reports the active stage and native/sandbox RX/TX counters.
- The legacy stage import only permits `native-fallback` and `sandbox-candidate`; `sandbox-active` now requires the gated promotion ABI and complete traffic validation evidence.
- The driver host ABI now supports releasing temporary handles so probe skills do not keep broad raw access after validation.
- Probe-only e1000 validation now releases its device handle after promotion is denied, leaving native e1000 as the live fallback.
- Sandbox driver-host handles and sandbox e1000 ownership now require lease heartbeats; stale authority is automatically released or rolled back to native fallback.
- Native e1000 still carries live traffic until a real sandbox NIC driver can take exclusive descriptor-ring ownership, sustain RX/TX, and promote `sandbox-active`.

### 2. xHCI/HID native path to sandbox input drivers

Why second:

- input regressions are high risk
- OpenRhiza already has HID handoff and sandbox input modules

Target:

- keep minimal emergency keyboard/mouse fallback
- expose raw USB/HID reports as bounded objects
- move keymap, IME, mouse scaling, and HID parsing to sandbox input drivers

Success criteria:

- keyboard and mouse remain usable through GUI handoff
- broken sandbox input parser automatically falls back
- right Shift / numpad / layout issues can be fixed by replacing input capability, not changing core

Current migration progress:

- `KEYBDRV.WAS` and `MOUSEDRV.WAS` are built from `sandbox-input-drivers/` source by `build_sandbox_skills.ps1`.
- Core retains raw HID handoff, routing gates, and rollback.
- `/input-routing-status` reports keyboard/mouse raw packet queue/fetch counters and sandbox-emitted key/mouse event counters. This lets driver validation prove that parsing is happening outside the xHCI/HID fallback path.
- Sandbox input drivers cannot directly mark themselves active through the Wasm ABI; `/input-activate` now requires post-load raw fetch and emitted event evidence for the target device kind.
- Sandbox input drivers can report health evidence through a dedicated input health import, but self-promotion is still denied unless core/user-approved validation gates are satisfied.
- `skill_xhci_sandbox_probe_v1` claims QEMU xHCI `pci:1b36:000d`, reads PCI/MMIO capability, operational, runtime, doorbell, and PortSC topology registers, polls IRQ through the driver host ABI, and deliberately leaves native USB/HID fallback active.
- The xHCI probe releases its temporary handle after validation. A future active sandbox xHCI transport driver must keep a handle only after passing input and transport promotion gates.
- Driver-host handle leases now prevent a failed xHCI sandbox module from keeping raw MMIO/IRQ authority after it stops heartbeating.
- Boot autorun now loads sandbox keyboard and mouse drivers as candidates, but does not promote them; fallback input remains active until `/input-activate` validates post-load evidence.
- The next step is to add an exclusive xHCI event-ring handoff gate, then reduce `src/arch/x86_64/usb.rs` to recovery transport/enumeration fallback and move device parser logic into replaceable sandbox input drivers.

### 3. GUI shell and renderer policy to sandbox skills

Why third:

- `src/display.rs` is currently one of the largest core modules
- GUI styling/layout should be replaceable without rebuilding the kernel

Target:

- core keeps recovery display, framebuffer handoff, object handles, validation, and dirty-region presentation
- GUI shell, layout, widgets, text behavior, and renderer policy move to `skill_gui_*`

Success criteria:

- a GUI skill can replace shell layout live
- object failure is isolated
- recovery console survives GUI skill failure

Current migration progress:

- `skill_gui_modern_shell_v1` owns the modern shell mutation path.
- `skill_gui_modern_shell_v1` now explicitly claims GUI shell-policy ownership and must keep a heartbeat lease while it owns shell layout policy.
- The active core scene report no longer imports the LVGL-style bridge; renderer/toolkit mapping is treated as a sandbox-skill target.
- The object-capability handle allowlist now includes every built-in session object used by the sandbox modern shell, including the recovery-shell session item.
- GUI skills can query object bounds through `os_gui_get_object_bounds`, so sandbox-owned layouts can adapt to the active surface instead of hardcoding 1920x1080 coordinates in core or skill code.
- `src/display.rs` still owns too much presenter, layout, text, scroll, and composer behavior and remains a major shrink target.
- The core now records shell policy ownership separately from framebuffer presentation so future GUI skills can replace layout policy without expanding the display core.
- `/input-routing-status` now includes raw HID handoff/event counters so input migration failures can be observed without adding parser policy to core.

### 4. Filesystem bridge and common filesystem families

Why fourth:

- storage writes are high risk
- current FAT16 fixed-slot floor is stable enough for bootstrap but not a general filesystem model

Target:

- core exposes bounded block objects and scratch regions only
- FAT32, exFAT, NTFS, ext2/3/4 probing/read/write live in `skill_fs_bridge`

Success criteria:

- filesystem skills can probe and validate image-backed disks inside OpenRhiza
- scratch write/read/restore works without mutating recovery disk
- no filesystem family parser grows inside core

Current migration progress:

- `/fs-bridge-status` exposes storage host ABI counters for image opens, block reads, block writes, flushes, and errors.
- The host still exposes bounded raw block transport only; filesystem interpretation remains a sandbox skill responsibility.
- `skill_storage_host_probe_v1` validates image-backed block object open/read/scratch write/read/restore/flush from a Wasm skill, proving storage validation can run outside core.
- `skill_piix_ide_sandbox_probe_v1` claims `pci:8086:7010` through the driver host ABI, reads PCI config and bounded PIO status, performs a sandbox-owned ATA PIO LBA0 read, and performs secondary-slave scratch write/read/restore directly through PIO. It deliberately does not take over live storage because the storage runtime cannot yet dispatch active block operations to a Wasm-owned controller driver.
- The PIIX IDE probe releases its temporary handle after reporting storage controller evidence.
- `flush_image` is best-effort on the QEMU secondary-slave harness; promotion relies on readback validation and rollback, not blind ATA cache-flush trust.
- `src/storage_host.rs` now has explicit `native-fallback`, `sandbox-candidate`, and `sandbox-active` stages for block drivers.
- Sandbox storage skills can report health evidence, but `sandbox-active` promotion still requires a block-operation dispatch path to the sandbox owner. The PIIX probe now proves controller PIO read/write/restore, but intentionally withholds `DRIVER_OWNS_BLOCK_IO` until live storage requests can be routed to it.
- Sandbox storage-active ownership also requires a heartbeat lease so broken block drivers automatically fall back to the native bootstrap floor.
- This keeps the native ATA/FAT bootstrap floor alive while storage drivers migrate behind the driver host ABI.

### 5. Autonomy workflow out of core

Why fifth:

- autonomy is policy/planning, not kernel substrate
- it must remain user-controlled and approval-gated

Target:

- core keeps autonomy mode, interval, permission gate, and audit log
- council agents, evidence gathering, proposals, and votes move to workflow skills

Success criteria:

- autonomy can be upgraded/replaced as a capability
- mode/interval cannot be changed by the AI itself
- risky actions still require approval

## Enforcement Rules

- New device-specific protocol code should not be added to core unless it is a temporary survival fallback and is listed in this audit.
- New GUI behavior should enter as object-scoped scene mutations or sandbox renderer skills, not as another hardcoded mode in `display.rs`.
- New filesystem family code must enter through `storage_host` and `skill_fs_bridge`, not `storage.rs`.
- New autonomy behavior must enter as workflow/council capability, not as more kernel policy.
- Any new fallback added to core must include:
  - why it is needed for survival
  - what sandbox replacement will remove it
  - how rollback is preserved

## Current Overall Assessment

OpenRhiza is aligned with its philosophy at the ABI and documentation level.

The implementation is still in a transitional state:

- Permanent core boundaries exist.
- Sandbox skills and driver host ABI exist.
- Several large bootstrap fallbacks remain because the OS still needs network, input, display, and storage while the sandbox replacements mature.

The next stage should not add large new user-facing features directly to core. It should convert the existing bootstrap fallbacks into sandbox-owned capabilities in the priority order above.
