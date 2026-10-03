# OpenRhiza Platform Progress

This document summarizes platform-specific progress, remaining work, and common interface rules for OpenRhiza across `x86_64` and `ARM64`.

The rule is architecture-independent:

- The core stays minimal: boot, survival console, bounded capability handles, sandbox host ABI, validation gates, rollback, and recovery.
- Drivers, storage/filesystems, GUI policy, workflows, autonomy behavior, and higher-level services must move into sandbox drivers, skills, workflows, or registry-backed capabilities.
- A feature is not considered production-ready only because a proof gate passes. It needs repeatable evidence, rollback, persistence if required, and architecture-equivalent CLI/GUI behavior.

## Common User Contract

The user-facing OpenRhiza experience must converge across all supported architectures.

### CLI Baseline

Every architecture must provide:

- A recovery/survival prompt that can accept commands even if GUI, network, or sandbox drivers fail.
- The same command naming model where local control commands are slash-prefixed, for example `/status`, `/help`, `/vosstat`, `/skill-load`, and `/driver-map`.
- A prompt path for natural-language requests that can query OpenRhiza.com, use LLM assistance, load skills, run validation, and report evidence.
- Clear separation between user-visible output and debug logs. Debug should prefer serial/log windows; user-facing CLI should stay readable.
- Scroll/history behavior sufficient to inspect previous output.
- Input reliability across keyboard, mouse, and future voice/hands-free paths.

Architecture-specific internals may differ, but the visible CLI behavior should not.

### GUI Baseline

Every architecture must eventually provide the same GUI contract:

- A high-resolution display path, with `1920x1080` as the current desktop-class target.
- A modern object-scoped GUI surface: sessions/sidebar, conversation history, dynamic composer/input area, and a recovery path.
- Pixel-level pointer behavior without text-cell snapping.
- Flicker-free redraw behavior, with object-level invalidation rather than whole-screen repaint on pointer movement.
- Korean and English text rendering, input, spacing, cursor, and wrapping behavior that remains stable during long conversations.
- GUI policy delivered by sandbox skills where possible. The core may provide only the survival display, framebuffer handoff, object handles, validation, and rollback.

Internal rendering can vary by architecture, but the user must be able to ask the same tasks and receive equivalent results.

## x86_64 / QEMU PC Status

`x86_64-qemu-pc` remains the reference platform.

### What Works

- Boots into the OpenRhiza kernel and recovery console.
- Network path is usable through the native e1000 fallback and sandbox e1000 probe/cutover work.
- OpenRhiza.com HTTPS/API access has been validated from the OS path.
- Gemini/LLM prompt path is usable through the configured proxy/direct request flow.
- Driver query, hardware report, driver upload, driver comment, driver vote, and related registry flows have been exercised.
- Persistent driver map/cache concepts exist, including live binding and rollback commands.
- Sandbox keyboard and mouse bootstrap drivers can load, promote, persist, and rollback.
- GUI bootstrap path exists:
  - display skill loading
  - framebuffer/high-resolution path
  - object-scoped GUI session
  - modern shell skill direction
  - conversation/composer surface
  - pointer rendering and flicker reduction work
- Korean input and font rendering have initial support, though text layout and long-run behavior still need regression coverage.
- Voice input design and host-assisted bridge direction exist, but native in-guest capture is not complete.
- Storage/PIIX sandbox probe and scratch read/write/restore proof exist.
- Filesystem skill designs exist for FAT/exFAT/NTFS/ext-family support, with storage host ABI and image harness scaffolding.
- QEMU driver/skill packs have been generated and uploaded/seeded in the local registry cache path.

### Current x86_64 Limitations

- Native fallbacks still carry too much production responsibility:
  - e1000 live traffic
  - xHCI/HID survival input
  - ATA/PIIX storage bootstrap
  - portions of display/GUI policy
- Sandbox-active cutover exists in multiple places, but not every normal OS request path is fully routed through sandbox owners by default.
- GUI is useful as a bootstrap object runtime, not yet a polished final desktop.
- Long conversation persistence, scrollback, Korean layout, cursor behavior, and GUI state retention need more soak/regression tests.
- Filesystem support is not yet broad, safe, production-grade read/write across FAT32, exFAT, NTFS, and ext2/3/4.
- Semantic graph indexing is still design/early scaffold, not a full always-available LLM filesystem memory layer.
- RShift and some USB keyboard/numpad edge cases remain known input issues.
- Boot-default autonomous behavior must remain user-controlled and evidence-gated.

### x86_64 Next Work

1. Route normal e1000 RX/TX through sandbox-active owner by default only after sustained traffic and rollback evidence.
2. Finish xHCI/HID sandbox transport ownership with command-ring, interrupter, HID event, keyboard, mouse, numpad, and RShift validation.
3. Route normal storage host block requests through the sandbox-active PIIX/storage owner, not only scratch selftests.
4. Keep moving GUI layout/composer/shell policy out of core into sandbox GUI skills.
5. Finish filesystem bridge skills with read/write validation and image-backed in-OS harness tests.
6. Add long-run GUI, input, LLM conversation, and storage soak tests.
7. Keep recovery console and rollback paths independent from GUI and sandbox experiments.

## ARM64 / QEMU Virt Status

`aarch64-qemu-virt` is not yet equivalent to x86_64, but it has moved well past a static serial survival kernel.

### What Works

- Builds a standalone ARM64 recovery ELF.
- Boots under QEMU `virt` with PL011 serial recovery console.
- Provides a CLI-level recovery surface with `input>` prompt, `/help`, `/clear`, `/status`, `/registry-context`, `/skill-cache`, `/driver-host-status`, `/display-status`, `/input-routing-status`, `/driver-map`, `/skill-load`, `/skill-run`, `/poweroff`, previous-command recall, and natural-language prompt stub behavior.
- Provides a visible ARM64 GUI bootstrap through QEMU `ramfb`:
  - `/gui-start`
  - `/gui-redraw`
  - `/gui-status`
  - local display skill aliases `skill_display_framebuffer_mode_v1`, `skill_gui_session_bootstrap_v1`, and `skill_gui_modern_shell_v1`
  - the rendered surface mirrors the x86_64 session/sidebar/conversation/composer contract at `1920x1080`
- Installs bounded virtio-mmio capability handles without adding virtio drivers to core.
- Detects QEMU virtio-mmio transports as read-only capability candidates.
- Provides sandbox-driver-oriented virtio paths for:
  - virtio-net
  - virtio-block
  - virtio-input
  - virtio-gpu
- Uses tiny executor/full Wasm proof paths to validate feature negotiation, queue programming, queue notify, used-ring/IRQ completion, and rollback gates.
- Has passed extensive proof gates for:
  - sustained device evidence
  - per-device counter windows
  - sandbox-source live/soak
  - detected-device source live/soak
  - external normal-subsystem submit
  - normal-source boot-live gates
  - hardware payload policy gates
  - storage write-intent gates
  - volatile storage capability proof
  - provider seal/read/clear proof
  - provider restore proof after runtime capsule clear
- Current provider restore proof validates that a sandbox provider object can rehydrate runtime policy state from its slot after the runtime capsule is cleared.

### Current ARM64 Limitations

- ARM64 is still recovery-first. It now has a visible `ramfb` GUI bootstrap, but it does not yet offer the same full interactive GUI/CLI desktop-class experience as x86_64.
- ARM64 CLI is now usable as a serial recovery interface, but remote OpenRhiza.com/Gemini prompt execution is still deferred until virtio-net, TLS, and registry policy are sandbox-promoted.
- Boot-default live virtio routing remains disabled.
- Many gates are proof-only, opt-in, and volatile.
- Provider restore is not disk persistence. It does not survive process/QEMU reboot yet.
- Real production subsystem queues still need to be connected to the sandbox virtio owner paths.
- Real disk-backed sandbox storage/filesystem provider and reboot readback are not complete.
- GUI/display for ARM64 is visually bootstrapped through `ramfb`, but full interaction parity is not complete. Input remains PL011 serial until virtio-input/display event routing is promoted through sandbox gates.
- Input, network, block, and GPU live promotion must remain guarded until longer device-specific traffic windows pass under realistic workload.

### ARM64 Next Work

1. Replace volatile provider proof with a real sandbox storage/filesystem provider that writes the policy capsule to a block object.
2. Add reboot/process readback validation: seal policy, persist through sandbox provider, reboot/restart, read back, verify, then allow gated promotion.
3. Connect real ARM64 net/block/input/gpu subsystem producers to `virtio_production_request_*` paths instead of diagnostic producer ticks.
4. Run longer device-specific traffic windows for virtio-net RX/TX, virtio-block read/write, virtio-input events, and virtio-gpu control completions.
5. Add per-device live promotion gates that are independent, reversible, and disabled by default until persisted policy exists.
6. Extend ARM64 CLI from recovery-level parity to network-backed parity: keep the existing `input>` slash-command surface, then connect remote registry/LLM prompt routing only after virtio-net/TLS are sandbox-promoted.
7. Move ARM64 from visible `ramfb` GUI bootstrap to full x86_64 GUI interaction parity after virtio-gpu and input paths are stable.
8. Keep virtio drivers out of core. Core may expose capability handles, DMA/IRQ windows, evidence counters, and rollback gates only.

## Why ARM64 Is Taking Longer Than x86_64

ARM64 is slower to reach parity for structural reasons, not because the goal changed.

### 1. x86_64 Started As The Reference Platform

The x86_64 path already had:

- boot image flow
- visible QEMU display
- keyboard/mouse path
- e1000 network path
- storage bootstrap
- GUI experiments
- OpenRhiza.com and LLM API validation

ARM64 started later from a serial survival kernel, so it needs the lower layers before GUI parity is meaningful.

### 2. ARM64 Uses Virtio-MMIO Instead Of PCI-Centric QEMU PC Devices

x86_64 QEMU PC work centered on devices such as e1000, xHCI, PIIX/ATA, and VGA/framebuffer.

ARM64 QEMU `virt` uses virtio-mmio devices. That requires different host ABI boundaries:

- MMIO capability handles instead of PCI-only handles
- virtqueue ownership
- feature negotiation
- queue address programming
- queue notify
- used-ring completion
- IRQ evidence

Those must be exposed safely without putting virtio drivers into core.

### 3. ARM64 Required Runtime Bring-Up Work

ARM64 needed extra work before sandbox drivers could run reliably:

- no_std ARM64 recovery runtime
- allocator/runtime setup
- PL011 serial recovery
- MMIO discovery
- Wasm execution path
- FP/SIMD runtime access issue resolution
- tiny executor fallback/proof path

x86_64 already had a richer kernel/runtime environment when these experiments began.

### 4. The ARM64 Work Is More Strictly Sandbox-First

The x86_64 path still has practical native fallback code carrying real behavior.

For ARM64, the current direction intentionally avoids adding virtio-net/block/input/gpu drivers to core. That is architecturally cleaner, but slower because each capability needs:

1. bounded host ABI
2. sandbox driver proof
3. payload proof
4. completion evidence
5. rollback proof
6. source/producer routing
7. persistence policy
8. reboot/readback proof

### 5. ARM64 Is Building The Future Pattern, Not Just A Port

ARM64 is being used to prove the general rule:

> Device logic belongs in sandbox drivers; core only grants bounded capability handles and validates evidence.

That means ARM64 work is not just "make it boot"; it is building the reusable sandbox device ownership model for future architectures.

## Cross-Architecture Completion Criteria

An architecture is not considered OpenRhiza-ready until it satisfies these criteria:

1. It boots reliably into a recovery console.
2. It can load sandbox skills and drivers.
3. It can reach OpenRhiza.com or an equivalent registry path.
4. It can run LLM-assisted prompt workflows.
5. It has stable keyboard/mouse or equivalent input.
6. It has a usable high-resolution GUI path or a documented display-limited mode.
7. It can persist and reload selected sandbox driver policy.
8. It can rollback failed sandbox drivers without losing recovery input/display.
9. Its CLI command set is compatible with the reference x86_64 command set.
10. Its GUI object/session/composer behavior is compatible with the reference x86_64 GUI behavior.

## Immediate Priority

The next platform milestone is:

1. Keep x86_64 as the visible reference UX.
2. Bring ARM64 from proof-only virtio routing to persisted, reboot-verified sandbox provider policy.
3. Preserve CLI/GUI parity requirements as non-negotiable architecture acceptance criteria.
4. Do not move virtio/e1000/xHCI/storage/GUI policy into permanent core to speed up a demo.
