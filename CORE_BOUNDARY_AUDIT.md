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
| `src/core/seed.rs` | Permanent Core | In-kernel Wasm runtime and host import boundary are central to the OS model. | Keep, but reduce policy. Add quota/accounting and keep imports handle-scoped. |
| `src/task/executor.rs` | Permanent Core | Scheduler substrate is required for basic runtime progress. | Keep; evolve to quotas and SMP without embedding capability policy. |
| `src/task/keyboard.rs` | Bootstrap Fallback | Recovery input is core, but rich keyboard decoding, IME, CLI routing, and debug commands are larger than the final core should own. | Keep minimal recovery decoder; move input parsing/IME/keymaps to sandbox input skills over HID handoff. |
| `src/vga.rs` | Permanent Core / Bootstrap Fallback | Recovery text surface is permanent. Shared GUI composer state and richer input editing are bootstrap debt. | Keep recovery shell only; move rich shell/composer behavior behind display/input skills. |
| `src/display.rs` | Bootstrap Fallback, Must Shrink | It contains framebuffer presenter, GUI layout, object runtime, pointer redraw, text rendering, and GUI mutation application. Too much long-term GUI behavior is in core. | Keep display handoff, recovery framebuffer, object handles, validation, rollback. Move GUI shell/layout/render policy to skills/renderers. |
| `src/gui_contract.rs` | Permanent Core Candidate | Shared object contract is acceptable as ABI/schema if kept small. | Keep as stable scene/mutation ABI. Avoid toolkit policy. |
| `src/gui_lvgl_bridge.rs` | Must Move To Skill | A toolkit-style bridge is not survival core. | Move to sandbox renderer/skill or keep only as host-side reference. |
| `src/gui_font.rs` | Bootstrap Fallback | Rendering requires glyph access, but font ingestion/selection is not core. | Keep only validated atlas reader if required; move font parsing/atlas generation to font skill/workflow. |
| `src/hangul.rs` | Must Move To Skill | Korean IME is valuable, not a survival core requirement. | Move to input/text skill once sandbox text input ABI is stable. |
| `src/input_handoff.rs` | Permanent Core | Raw input handoff and routing gates are core boundary responsibilities. | Keep; reduce parsing policy. |
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
| `src/driver_host.rs` | Permanent Core | Handle-scoped PCI/MMIO/PIO/DMA/IRQ ABI is the correct boundary for sandbox drivers. | Keep; harden capability checks and quotas. |
| `src/driver_runtime.rs` | Permanent Core Candidate | Binding lifecycle, validation stage, and rollback gate are core governance. | Keep only lifecycle/gate state; driver policy belongs to workflows. |
| `src/runtime_bindings.rs` | Permanent Core Candidate | Binding registry is needed for live object activation. | Keep; avoid embedding driver-specific policy. |
| `src/component_runtime.rs` | Permanent Core Candidate | Object lifecycle stage and rollback model match OpenRhiza philosophy. | Keep small and generic. |
| `src/sandbox_lifecycle.rs` | Permanent Core Candidate | Generic sandbox lifecycle is core boundary state. | Keep small and generic. |
| `src/skill_runtime.rs` | Bootstrap Fallback / Permanent Boundary | Skill activation and lifecycle gates are needed; skill semantics should not live here. | Keep loader/gates; move skill behavior to artifacts. |
| `src/skill_cache.rs` | Bootstrap Fallback | Fixed-slot cache is bootstrap storage policy. | Keep until richer storage/registry skill cache exists. |
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

Success criteria:

- OpenRhiza reaches OpenRhiza.com/Gemini through sandbox e1000
- native driver can be disabled without losing recovery path
- failed sandbox driver rolls back to native fallback

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
