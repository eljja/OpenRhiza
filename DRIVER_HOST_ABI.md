# OpenRhiza Sandbox Driver Host ABI

This ABI is the boundary that lets sandboxed driver skills control hardware without moving driver logic into the core.

Core rule:
- The core owns only capability checks, address translation, bounded memory movement, and rollback-visible runtime state.
- Device logic stays in sandbox drivers or skills.
- Raw host access is denied unless a sandbox module first claims a device and receives a driver handle.

## Claim Flow

1. A sandbox driver calls `os_driver_activate_binding(match_key, driver_id)`.
2. The driver calls `os_driver_claim_device(match_key, driver_id, caps)`.
3. The core matches `match_key` against enumerated PCI devices and returns an opaque handle.
4. All MMIO, PIO, DMA, PCI config, and IRQ calls require that handle.
5. The handle is limited to the requested capability bits. An unclaimed capability is denied even when the handle is otherwise valid.
6. A temporary probe driver releases its handle with `os_driver_release_device(handle)` after validation if it is not the live owner.

Supported match keys:
- `pci:vendor:device`, for example `pci:8086:100e`.
- `pci:class:ccss`, for example `pci:class:0101`.
- `virtio-mmio:0x<base>`, for example `virtio-mmio:0x0a000000`.
- `virtio:mmio`, for the first platform-installed virtio-mmio transport window.

Capability bits:
- `0x01` PCI configuration space
- `0x02` MMIO window
- `0x04` PIO window
- `0x08` DMA allocation and bounded DMA buffer copy
- `0x10` IRQ poll/ack

The core masks unknown bits. Reclaiming the same `(match_key, driver_id)` can only add recognized bits to the same handle; it cannot grant access outside this list. A complete production promotion path must additionally validate the driver before any live traffic or device ownership moves from native fallback to sandbox-active.

## ABI Surface

Device:
- `os_driver_claim_device(key_ptr, key_len, id_ptr, id_len, caps) -> handle`
- `os_driver_claim_mmio_device(key_ptr, key_len, id_ptr, id_len, caps) -> handle`
- `os_driver_release_device(handle) -> ok`
- `os_driver_handle_heartbeat(handle) -> ok`
- `os_driver_activate_binding(key_ptr, key_len, id_ptr, id_len) -> ok`

Release revokes the handle and drops DMA grants owned by that handle. Sandbox drivers should release temporary discovery/probe handles when they are not the active owner of a device. A live owner must submit handle heartbeats; stale handles are automatically released by the core watchdog.

`os_driver_claim_mmio_device` is for non-PCI platform devices such as ARM64 `virtio-mmio`.
It grants only MMIO, DMA, and IRQ bits, never PCI config or PIO authority.
The core owns only the capability window and lease. It does not implement virtio-net, virtio-block, virtio-input, or virtio-gpu logic.

PCI config:
- `os_driver_pci_config_read32(handle, offset) -> value`
- `os_driver_pci_config_write32(handle, offset, value) -> ok`

MMIO:
- `os_driver_mmio_read32(handle, offset) -> value`
- `os_driver_mmio_write32(handle, offset, value) -> ok`

PIO:
- `os_driver_pio_read8/16/32(handle, offset) -> value`
- `os_driver_pio_write8/16/32(handle, offset, value) -> ok`

DMA:
- `os_driver_dma_alloc(handle, byte_len, align) -> dma_handle`
- `os_driver_dma_phys(handle, dma_handle) -> physical_address`
- `os_driver_dma_len(handle, dma_handle) -> byte_len`
- `os_driver_dma_write(handle, dma_handle, offset, wasm_ptr, len) -> copied`
- `os_driver_dma_read(handle, dma_handle, offset, wasm_ptr, len) -> copied`
- `os_driver_dma_barrier(handle, dma_handle) -> ok`

IRQ:
- `os_driver_irq_poll(handle) -> pending_mask`
- `os_driver_irq_ack(handle, mask) -> ok`
- `os_driver_wait_used_idx(handle, dma_handle, used_offset, previous_idx, poll_limit) -> completed`

Virtio live gate:
- `os_virtio_prepare_live_gate(handle, expected_device_id, queue_index, dma_handle, queue_size) -> gate_or_zero`
- `os_virtio_record_feature_negotiation(gate, device_features, driver_features, status) -> ok`
- `os_virtio_record_queue_programming(gate, queue_index, queue_size, desc_phys, driver_phys, device_phys) -> ok`
- `os_virtio_record_queue_notify(gate, queue_index) -> ok`
- `os_virtio_record_irq_completion(gate, irq_status, ack_mask) -> ok`
- `os_virtio_commit_live_gate(gate) -> ok`
- `os_virtio_rollback_live_gate(gate) -> ok`

The virtio gate is a rollback-first promotion guard. `prepare` verifies the handle is a `virtio-mmio` MMIO/DMA/IRQ grant, checks the standard transport identity, verifies the expected device ID, confirms the requested bounded queue can support the requested size, and ties the candidate to sandbox-owned DMA memory. The record calls collect feature negotiation, queue address programming, queue notification, and completion evidence. `os_virtio_record_irq_completion` accepts real IRQ status or a sandbox-observed used-ring completion marker; it does not build or inspect device payloads. `commit` succeeds only after all required evidence bits are present. `rollback` removes the gate record. This keeps virtio device logic in sandbox drivers while the core tracks only bounded promotion evidence.

Network handoff:
- `os_fetch_tx_packet(ptr, max_len) -> byte_len`
- `os_rx_packet(ptr, len) -> void`
- `os_net_get_sandbox_ethernet_stage() -> stage`
- `os_net_set_sandbox_ethernet_stage(stage) -> ok`
- `os_net_report_sandbox_ethernet_health(driver_id_ptr, driver_id_len, flags) -> ok`
- `os_net_sandbox_ethernet_heartbeat(driver_id_ptr, driver_id_len, flags) -> ok`
- `os_net_request_sandbox_ethernet_promotion(driver_id_ptr, driver_id_len) -> ok`

Safety note:
- The legacy stage setter only permits `0` (`native-fallback`) and `1` (`sandbox-candidate`).
- `2` (`sandbox-active`) is intentionally denied through the legacy setter.
- The promotion ABI requires a matching driver id plus all health flags below.
- A promoted sandbox NIC must keep submitting lease heartbeats. If heartbeats stop, the core automatically restores native fallback.
- This prevents an arbitrary sandbox module from cutting live networking over to an incomplete NIC driver.

Network promotion health flags:
- `0x01` device was claimed through the driver host ABI
- `0x02` PCI config access was exercised
- `0x04` MMIO access was exercised
- `0x08` DMA allocation/copy was exercised
- `0x10` IRQ poll/ack path was exercised
- `0x20` TX handoff queue was observed
- `0x40` sustained RX validation passed
- `0x80` sustained TX validation passed

The current e1000 probe intentionally does not satisfy sustained RX/TX, so promotion is denied and native fallback continues carrying live traffic.

## Current State

`skill_qemu_driver_pack_v1` registers QEMU baseline bindings, validates only baseline PCI identity, and releases its temporary handles. Broad MMIO/PIO/DMA/IRQ probing has moved to dedicated sandbox driver skills so a generic pack cannot accidentally accumulate raw device power.

`skill_e1000_sandbox_probe_v1` separately claims `pci:8086:100e`, heartbeats its temporary handle, reads PCI/MMIO state, allocates/writes/reads DMA descriptor memory, verifies DMA readback, builds sandbox-owned TX/RX descriptor records, verifies descriptor and payload readback, injects a bounded synthetic RX frame through the network ABI, polls IRQ, probes the TX handoff queue, and leaves networking in `sandbox-candidate` mode. It submits candidate lease heartbeats, releases its handle after promotion is denied, and remains a migration probe rather than a complete NIC driver. It now also exports `prepare_live_e1000_cutover`, which can allocate sandbox DMA and program live TDBAL/TDBAH/TDLEN/TDH/TDT and RDBAL/RDBAH/RDLEN/RDH/RDT registers. After that export is called, its `poll_net` path can pull TX packets from the core handoff queue into sandbox-owned TX descriptors and harvest completed RX descriptors back through `os_rx_packet`.

The core now exposes a manual cutover gate:

- `/net-e1000-cutover`
- `/net-sandbox-cutover skill_e1000_sandbox_probe_v1 drv_e1000_sandbox_probe_v1`

That gate runs the candidate probe, temporarily quiesces native RX/TX by entering a core-owned cutover trial, injects a bounded probe TX frame, invokes `prepare_live_e1000_cutover`, polls the sandbox NIC path, then either promotes `sandbox-active` through the health gate or rolls back to native fallback. Boot autorun still does not perform this cutover automatically.

`skill_xhci_sandbox_probe_v1` claims QEMU xHCI `pci:1b36:000d`, heartbeats its temporary handle, reads PCI/MMIO capability, operational, runtime offset, doorbell offset, and PortSC topology state, allocates DMA, builds and verifies an event-ring segment table plus transfer-event TRB layout, probes the raw HID handoff queue, polls IRQ, releases its handle, and leaves native USB/HID input active. It now exports `prepare_live_xhci_event_ring`, which allocates sandbox DMA and prepares an event-ring segment table without programming live xHCI registers. The dangerous live register write path is isolated behind `commit_live_xhci_event_ring` and is not called by boot autorun or the normal CLI gate.

The core now exposes a recovery-safe xHCI prepare gate:

- `/xhci-sandbox-prepare`
- `/xhci-sandbox-prepare skill_xhci_sandbox_probe_v1 drv_xhci_sandbox_probe_v1`

That gate forces keyboard and mouse routing back to handoff-mirror fallback before and after the sandbox prepare, runs the candidate probe, calls only the safe prepare export, and reports the input handoff/runtime state. Full xHCI cutover still requires a second gate that can restore native controller ownership after command-ring/interrupter/HID validation.

The next manual xHCI gate is intentionally a rollback drill, not promotion:

- `/xhci-rollback-drill`
- `/xhci-rollback-drill skill_xhci_sandbox_probe_v1 drv_xhci_sandbox_probe_v1`

That gate auto-loads the local xHCI sandbox skill if needed, runs the candidate probe, prepares sandbox event-ring DMA, calls the isolated `commit_live_xhci_event_ring` export, then immediately reinitializes the native xHCI fallback using the saved controller tuple. It clears stale HID runtime DMA context before fallback reinit, does not promote sandbox input ownership, and is QEMU-verified with `committed=1 native_restored=1` plus post-drill keyboard input. Its only purpose is to prove that a live-register sandbox experiment can be recovered before any future command-ring, interrupter, or HID event routing cutover is allowed.

`skill_piix_ide_sandbox_probe_v1` claims `pci:8086:7010` with PCI-config and PIO capabilities, heartbeats its handle, reads the controller ID and secondary-channel status, performs a real sandbox-owned ATA PIO `READ SECTOR` on LBA0, then performs a secondary-slave scratch-sector write/read/restore proof through the host PIO ABI. It also exports `storage_read_block`, `storage_write_block`, and `storage_flush`, allowing the OS runtime to call sandbox-owned block operations directly for a scratch read/write/restore selftest. It reports every storage health flag except `DRIVER_OWNS_BLOCK_IO`, submits storage candidate lease heartbeats, retains only a candidate lease, and leaves native bootstrap storage active after normal probes.

The core now exposes a manual storage owner gate:

- `/storage-piix-cutover`
- `/storage-sandbox-cutover skill_piix_ide_sandbox_probe_v1 drv_piix_ide_sandbox_probe_v1`
- `/storage-route-selftest`

That gate runs the PIIX candidate, calls the sandbox block exports through the `OpenRhizaSeed` broker for a scratch read/write/restore proof, adds `DRIVER_OWNS_BLOCK_IO` evidence only after that proof passes, then promotes the sandbox block owner through the storage health gate. Normal `storage_host::read_blocks/write_blocks` calls still need a request route into the active sandbox owner before the native ATA fallback can be removed.

After promotion, the core records the active sandbox module key. `/storage-route-selftest` uses that active module, not a hardcoded driver name, to perform a brokered scratch read/write/restore route check. If the active route fails, the core clears the active owner and restores native storage fallback. This keeps storage cutover recovery deterministic while the normal storage host path is still being migrated.

`/driver-host-status` reports each active handle with its granted capability mask so migration work can verify that sandbox drivers are not relying on accidental raw access.
It also reports claim/release/lease-expiry counters, which are used to confirm probe-only skills return temporary handles instead of accumulating device authority.

## ARM64 virtio-mmio Capability Stubs

ARM64 support must not add virtio drivers to the core.

The ARM64 QEMU `virt` path now has a host-ABI scaffold for platform MMIO windows:

- `src/arch/aarch64/mod.rs::install_sandbox_host_abi_stubs()`
- `src/driver_host.rs::install_aarch64_qemu_virtio_mmio_stubs()`
- Wasm import `os_driver_claim_mmio_device(...)`
- Sandbox seed `skill_virtio_mmio_probe_v1`
- Sandbox driver candidates `skill_virtio_net_driver_v1`, `skill_virtio_block_driver_v1`, `skill_virtio_input_driver_v1`, and `skill_virtio_gpu_driver_v1`

The seeded probe scans the bounded QEMU `virt` MMIO transport range `0x0a000000..0x0a003fff`, claims one opaque handle at a time, reads only standard transport identity registers, polls the declared IRQ line, releases the handle, and exits.
It is a transport proof, not a device driver.

The device-specific virtio sandbox seeds use the same bounded handle contract but filter by standard virtio device ID:

- `virtio-net`: device ID `1`
- `virtio-block`: device ID `2`
- `virtio-gpu`: device ID `16`
- `virtio-input`: device ID `18`

These are candidate drivers only. They may claim an MMIO capability, read identity/features/queue limits, poll IRQ metadata, allocate sandbox DMA queue memory, build a device-specific virtqueue payload in sandbox DMA, read the layout back, negotiate feature bits, program queue addresses, notify a queue, and commit only after the used ring or IRQ provides completion evidence. The core still does not know how to form a packet, block request, input event buffer, or GPU command; those payloads are owned by the sandbox drivers.

Current executor scaffolds:

- `virtio-net`: RX queue `0` arms and reads a writable RX buffer when the used ring advances; TX queue `1` submits a small synthetic Ethernet frame and verifies device completion.
- `virtio-block`: request queue `0` submits a sector-0 `VIRTIO_BLK_T_IN` read request and a guarded scratch-LBA `VIRTIO_BLK_T_OUT` write request. The write path is denied for read-only devices or devices that cannot contain the scratch LBA.
- `virtio-input`: event queue `0` submits a writable `virtio_input_event` buffer and reads back event type/code/value only after completion.
- `virtio-gpu`: control queue `0` submits `VIRTIO_GPU_CMD_GET_DISPLAY_INFO` and reads back the response header only after completion.

The live gate is intentionally strict: a queue may notify without promotion, but `commit` stays denied unless real completion evidence is recorded. RX/input queues can therefore remain armed without falsely claiming success when no packet or input event has arrived.

ARM64 recovery also has `/virtio-payload-selftest` as a temporary diagnostic harness. It is not a production driver and must not become the long-term place for virtio policy. It verifies the same payload shapes against QEMU before the full ARM64 Wasm runtime is available; current positive runtime evidence covers virtio-gpu display-info control completion, virtio-block sector read/write completion, virtio-net TX completion, virtio-net RX completion when the smoke harness injects a raw Ethernet frame through a socket netdev, and virtio-input event completion when the smoke harness injects QMP/VNC pointer events.

ARM64 recovery also exposes `/virtio-wasm-selftest`, injected/rollback/promotion proof gates, 12-round traffic windows, independent device gates, `/virtio-wasm-live-status`, `/virtio-wasm-live-cutover-*`, `/virtio-wasm-live-rollback-*`, `/virtio-wasm-live-cutover-rollback-*`, `/virtio-wasm-live-dispatch-*`, `/virtio-wasm-live-route-all`, `/virtio-wasm-live-rollback-all`, `/virtio-wasm-live-routing-selftest`, `/virtio-wasm-live-route-all-selftest`, `/virtio-wasm-live-service-*`, `/virtio-wasm-live-service-loop-selftest`, `/virtio-wasm-live-os-pump-selftest`, the long-window alias `/vospl`, the OS producer API alias `/vosrq`, the opt-in live runtime commands `/vosstart`, `/vossub`, `/vosstat`, `/vosstop`, `/voslive`, the subsystem producer bridge alias `/vossys`, the persistent subsystem runtime soak alias `/vosps`, the production inbox queue alias `/vosprod`, the production inbox runtime commands `/vosprod-start` and `/vosprod-all`, the manual subsystem submit aliases `/vosman` and `/vossall`, `/virtio-wasm-live-boot-service-selftest`, and a marker-gated Rust payload probe as explicit experiments. The custom target now builds `core+alloc+compiler_builtins`, links `wasmi` in no_std mode, registers the same driver-host imports, and can instantiate and call sandbox modules. The previous `wasmi::TypedFunc::call()` stall was caused by ARM64 FP/SIMD access trapping before `CPACR_EL1` enabled FP/SIMD for the recovery runtime. `/virtio-wasm-enable-gate-*` runs a logged warm-up that is not counted, then requires 12 positive device evidence rounds. Block/GPU/net evidence windows are strict; input may retry bounded no-evidence rounds because external event injection can miss a polling window, but only positive used-ring event completions count and the gate fails closed if 12 are not collected. `/virtio-wasm-live-cutover-rollback-*` consumes that gate, records the device owner bit only if the gate passes, verifies the owner bit, clears it through rollback, and requires a rollback proof. `/virtio-wasm-live-routing-selftest` then runs the next safety step: explicit cutover, four sandbox-owner dispatch ticks, owner-active verification, rollback, fallback proof, and final owner-mask zero for block/gpu/net/input. `/virtio-wasm-live-route-all-selftest` is stricter: it cuts all four owners over together, verifies the route-all owner mask, dispatches all four owners, then rolls all owners back. `/virtio-wasm-live-boot-service-selftest` boots to recovery, starts a bounded post-boot sandbox-owner service pump, runs four all-device service ticks, then stops through rollback-all with final owner mask zero. `/virtio-wasm-live-service-loop-selftest` goes one step further: it returns from the command handler and proves the ARM64 main idle loop drains four bounded request/event ticks before rollback-all. `/virtio-wasm-live-os-pump-selftest` keeps that main-loop shape but uses typed OS work items: block read, block write, GPU control, net RX, net TX, and input event dispatch. `/vospl` validates the same path for a longer 16-tick/96-work-item window. `/vosrq` validates the OS-side producer boundary by prequeuing typed block/net/input/GPU work through explicit producer functions, returning to the main loop, draining the work through sandbox-active owners, and rolling all owners back. `/voslive` starts the opt-in sandbox-active runtime, prequeues producer-owned typed work, lets the normal ARM64 main loop pump only while work is pending, then proves exact drain counters and rollback-all. `/vossys` moves one boundary closer to production by exercising subsystem-facing producer functions for block read/write, GPU control, net RX/TX, and input events over a 16-tick/96-work-item window. `/vosps` removes the prequeue shortcut: it keeps the opt-in sandbox-active runtime alive and lets a persistent subsystem producer feed one work round at a time from the ARM64 main loop for 32 ticks/192 work items before rollback-all. `/vosprod` adds a normal-subsystem inbox between subsystem work generation and sandbox-owner dispatch; it accepts eight rounds/48 typed work items, forwards them into sandbox-active owners, validates `enqueued=48 forwarded=48 rejected=0`, and rolls all owners back. `/vosprod-start` keeps that production inbox runtime active after the command returns, `/vosprod-all` submits external normal-subsystem batches into it, the main loop forwards those batches through sandbox-active owners, `/vosstat` proves `enqueued=48 forwarded=48 rejected=0` for the current eight-batch diagnostic, and `/vosstop` rolls every owner back. `/vosman` arms the same opt-in runtime but requires external subsystem submit commands such as `/vossall` to feed typed work after the command returns; the main loop drains those externally submitted batches through sandbox-active owners and rolls all owners back. This is still diagnostic and operator-triggered, but it is now a callable subsystem bridge, runtime producer shape, normal inbox queue, opt-in production inbox runtime, and external submit boundary rather than only a synthetic owner-state bit.

Subsystem producer bridge functions in the ARM64 recovery runtime:

- `virtio_subsystem_request_block_read()`
- `virtio_subsystem_request_block_write()`
- `virtio_subsystem_request_gpu_control()`
- `virtio_subsystem_request_net_rx()`
- `virtio_subsystem_request_net_tx()`
- `virtio_subsystem_request_input_event()`

These are not device drivers. They are the narrow handoff points that future real ARM64 net/block/input/GPU subsystems should call after they have a normal OS request or event to submit. The bridge rejects work unless the opt-in live runtime and matching sandbox owner are active, counts every accepted/rejected request, drains only through sandbox-active virtio owners, and rolls all owners back during the selftest.

The production inbox queue (`/vosprod`) and opt-in runtime (`/vosprod-start` plus `/vosprod-all`) are the current preferred replacement target for real subsystem producers. They are still diagnostic queues, but the shape is intentionally close to production: normal subsystem work enters a separate inbox, the main loop forwards accepted items into the sandbox-active owner queue, counters prove accepted/forwarded/rejected behavior, and rollback-all clears every owner. The focused wrapper `tools/run_arm64_production_runtime_demo.ps1` runs this path with a configurable batch count, writes the full QEMU log under `target`, and prints the key counters; the 64-batch run has passed with 384 submitted/forwarded work items and zero failures.

`/vosprod-external-normal-source-live` plus `/vosprod-external-normal-submit-all` is an ARM64 external normal-subsystem boundary proof. It does not generate work internally and does not open production boot routing. Instead, it arms a proof owner mask, waits for externally submitted normal-subsystem work, validates that six typed work items pass through `VirtioArm64NormalSubsystemQueues -> normal device/ingress/source/descriptor queues -> production inbox -> sandbox virtio owners`, keeps the runtime alive only as a validated keepalive proof, and clears proof owners on `/stop` without running a live device rollback. `/vosprod-external-normal-submit-{block,gpu,net,input}` proves the same path one device at a time: block submits read/write, GPU submits one control command, net submits RX/TX, and input submits one event. `/vosprod-external-normal-submit-device-round` repeats the same device-shaped submission as a complete batch and is used by the repeated proof window to validate that multiple externally produced batches drain through the same sandbox-owner route without relying on generated source ticks. The external poller intentionally waits until all enabled device work is present before draining, so partial device submissions cannot produce false health evidence. On 2026-05-10 the focused wrapper `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionExternalNormalDeviceSubmitSoak` passed eight repeated rounds with exact 48-item queue/inbox/drain counters and rollback. This proves the shape needed for real net/block/input/GPU subsystem producers while live device promotion remains closed.

The source-live production gate (`/vosprod-source-live`) is the first ARM64 path where production routing can be opened on demand and kept active after validation. It uses an explicit ARM64 subsystem queue in front of the production inbox: recovery storage/display/network/input source ticks submit typed work, the queue records pending work, a separate drain step dispatches that work through `virtio_production_request_*`, the production inbox forwards into sandbox-active virtio owners, and `/stop` performs rollback-all. Local evidence shows `ARM64 subsystem queue enqueued=192 dispatched=192 rejected=0`, production inbox `enqueued=192 forwarded=192 rejected=0`, `production source-live keepalive active=1`, `production queue runtime running=1`, live owner mask `0x0000000f`, and rollback to `live service running after stop=0`. `/vosprod-source-live-soak` extends the same enqueue/drain path to 128 source ticks and has passed with `ARM64 subsystem queue enqueued=768 dispatched=768 rejected=0`, production inbox `enqueued=768 forwarded=768 rejected=0`, `production source-live keepalive active=1`, and `/stop` rollback. `/vosprod-real-source-live` and `/vosprod-real-source-live-soak` add the next safety step: they first detect actual QEMU virtio-mmio device IDs (`net=1`, `block=2`, `gpu=16`, `input=18`), narrow the production owner mask to the detected devices, enqueue/drain only those detected source paths, and require device-specific sandbox driver seed completion evidence for block read/write, GPU control, net TX/RX, and input events before counting a drain as complete. On 2026-05-08 both focused gates passed locally with `device_mask=0x0000000f`; the short gate validated 32 ticks/192 work items and the long soak validated 128 ticks/768 work items with sandbox driver seed payload evidence, zero rejected work, and `/stop` rollback. `/vosprod-normal-source-live` is the next boundary: it uses a distinct `normal-subsystem-producer` source mode and validates the future normal subsystem producer path through `VirtioArm64NormalDescriptorQueue`, then drains those typed descriptors through `virtio_production_request_*` into the sandbox-owned production inbox without reusing the recovery source adapters. Those descriptors now carry per-device request metadata such as block LBA/generation, GPU target size/frame, net packet/length, and input event tuple; virtio queue ownership and device execution still remain in sandbox drivers. It passed locally on 2026-05-09 with `normal_source=1`, `ARM64 normal subsystem producer source device_mask=0x0000000f`, exact 32-tick counters, normal producer source health `storage_ticks=32 display_ticks=32 network_ticks=32 input_ticks=32 rejected=0`, `source_mode=validated-keepalive boot_default_live=0`, and `/stop` rollback; the 128-tick normal-source soak also passed after the descriptor queue split, after request metadata was added, and after source-health counters became mandatory. This is opt-in production routing, not boot-default policy.

The opt-in production boot promotion gate (`/vosboot`) starts that production inbox runtime, auto-feeds normal-subsystem block read/write, GPU control, net RX/TX, and input event work for 64 ARM64 main-loop ticks, drains through sandbox-active virtio owners, validates exact runtime/subsystem/inbox counters, then performs rollback-all. `/vosboot-block`, `/vosboot-gpu`, `/vosboot-net`, and `/vosboot-input` run the same 64-tick gate for one selected device mask. These gates have passed locally, but they remain explicit operator gates; boot-default live routing still requires persisted policy plus longer real-workload soak.

Hardware-payload boot policy is deliberately split into three gates before any future persisted boot-default routing:

- `/vospolicy-payload-{block,gpu,net,input}` records a runtime-only candidate only after matching hardware-payload evidence exists.
- `/vospolicy-capsule-seal-{block,gpu,net,input}` seals that candidate into a verified runtime capsule and `/vospolicy-capsule-verify` checks mask/evidence/checksum consistency.
- `/vospolicy-storage-stage`, `/vospolicy-storage-commit`, and `/vospolicy-storage-rollback` exercise the persistence boundary without granting persistence. The current ARM64 recovery core has no storage capability, so commit must be denied with `storage_capability=0 persisted=0`; rollback must clear the write-intent. This passed for block, GPU, net, and input via `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionHardwarePayloadPolicyStorageDevice`.
- `/vospolicy-storage-attach-proof`, `/vospolicy-storage-verify`, and `/vospolicy-storage-clear-proof` exercise the capability-present path using a volatile proof slot only. This proves that a future sandbox storage skill can attach a policy-storage capability, accept a staged capsule, verify it, and clear it without turning on boot-default live routing. It passed for block, GPU, net, and input via `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionHardwarePayloadPolicyStorageProofDevice`.
- `/vospolicy-storage-provider-attach-proof`, `/vospolicy-storage-provider-seal-proof`, `/vospolicy-storage-provider-read-proof`, and `/vospolicy-storage-provider-clear-proof` split that volatile path into the provider ABI shape that a sandbox storage/filesystem skill must implement. The recovery core stages the write-intent, the provider proof accepts/seals it, readback verifies the checksum, and clear removes the provider slot. This passed for block, GPU, net, and input via `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionHardwarePayloadPolicyStorageProviderProofDevice`.
- `/vospolicy-storage-provider-restore-proof` adds the next reboot-readback-shaped contract. After the provider seals a staged capsule, `/vospolicy-capsule-clear` can remove the runtime capsule state, then provider restore rehydrates the candidate/capsule from the provider slot and `/vospolicy-storage-provider-read-proof` verifies it again. This passed for block, GPU, net, and input via `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionHardwarePayloadPolicyStorageProviderRestoreProofDevice`.

These storage ABI gates are not disk persistence. They are the rollback-safe write-intent, capability-present, provider readback, and provider restore contracts that a future sandbox storage/filesystem skill must satisfy before any boot policy can be saved and reloaded across a real reboot.

Production inbox producer functions in the ARM64 recovery runtime:

- `virtio_production_request_block_read()`
- `virtio_production_request_block_write()`
- `virtio_production_request_gpu_control()`
- `virtio_production_request_net_rx()`
- `virtio_production_request_net_tx()`
- `virtio_production_request_input_event()`

These are the boundary real ARM64 net/block/input/GPU subsystems should call once they have normal OS work to submit. They do not contain device policy; they only enqueue typed work into the production inbox when the opt-in sandbox-active runtime is accepting work. The main loop is responsible for forwarding inbox work to the active sandbox owner and for preserving rollback.

Sandbox-driver source import for ARM64 virtio proof gates:

- `os_arm64_submit_normal_work(kind, a, b, c) -> status`

This import is intentionally narrower than raw virtio access. A sandbox virtio seed may call it only after it has produced device-specific payload evidence through the sandbox driver path. The ARM64 recovery runtime accepts it only while `/vosprod-sandbox-source-live` has explicitly enabled sandbox-source submission; otherwise it returns `2` to mark proof-only execution and does not enqueue work. Accepted calls return `1`, rejected calls return `0`, and accepted work still flows through the normal subsystem queue, device queue, ingress queue, source queue, descriptor queue, production inbox, sandbox-active owner, and rollback-visible counters. This prevents generated/core producers from being mistaken for sandbox-driver submitted work.

Manual submit commands `/vossall`, `/vossblk`, `/vossgpu`, `/vossnet`, and `/vossinp` are diagnostic stand-ins for real subsystem producers. They intentionally require an active `/vosman` runtime gate and are useful for proving that work submitted after runtime activation is accepted, drained, counted, and rolled back without moving virtio policy into core.

Virtio rollback cleanup must be conservative. A sandbox seed may reset the device status and release the capability handle, but it must not write `QueueNum=0` as cleanup after a completed request; QEMU virtio-mmio can panic or wedge during full Wasm block-write proof when queue size is rewritten as part of rollback.

Next ARM64 work:

1. Replace the placeholder normal-subsystem producer adapters with real OS virtio-net/block/input/gpu subsystem queues connected to the `/vosprod-start`, `/vosprod-source`, `/vosprod-source-live`, `/vosprod-source-live-soak`, `/vosprod-real-source-live`, `/vosprod-real-source-live-soak`, `/vosprod-normal-source-live`, and `/vosprod-normal-source-live-soak` production inbox runtime shape.
2. Keep block/GPU/net counted evidence strict. For input, allow only bounded no-evidence retries caused by asynchronous event injection; never count those retries as evidence, and fail closed if 12 positive event completions are not collected.
3. Promote only after device-specific payload completion evidence is collected through the sandbox runtime path and longer real-workload soak, not from boot-time direct diagnostics alone.
4. Keep `wasmi::TypedFunc::call()` in the regression suite so FP/SIMD enablement or runtime upgrades cannot regress Rust-generated module execution.
5. Keep all virtio feature negotiation, queue setup, payload construction, and device-specific behavior in those sandbox drivers.

The current `/vosprod-source` path proves the next boundary without diagnostic auto-feed: ARM64 recovery source ticks call `virtio_production_request_*`, the production inbox forwards typed work, sandbox-active owners drain it, and rollback clears ownership. `/vosprod-source-live` proves the same source path can remain active after validation until `/stop` rollback. `/vosprod-real-source-live` proves the source path is gated by actual detected virtio devices rather than an unconditional all-device mask and now requires sandbox driver seed payload completion evidence before drain counters advance. `/vosprod-normal-source-live` proves the non-recovery producer boundary can enqueue typed per-device source work, forward it through a normal descriptor queue, and drain it into the same production inbox. `/vosprod-sandbox-source-live` proves the next stricter boundary: generated normal producers stay disabled while sandbox virtio driver seeds submit six accepted normal work items through `os_arm64_submit_normal_work` after block read/write, GPU control, net RX/TX, and input event payload evidence. It passed as a one-tick proof gate with exact six-item queue and production-inbox counters, but it is not a long soak and not boot-default. The normal producer ticks now feed normal subsystem queues, then the explicit normal subsystem API, then normal device queues, then a normal subsystem ingress queue; the ingress drain calls device-specific submit functions (`block_read`, `block_write`, `gpu_control`, `net_rx`, `net_tx`, `input_event`) instead of writing descriptors directly. Real ARM64 subsystems must replace those generated ticks by feeding the same normal subsystem queue boundary or by passing sandbox-driver source evidence through `os_arm64_submit_normal_work`. `/vosboot-normal-live` and `/vosboot-normal-live-soak` are the subsystem-queue/API/device/ingress/source/descriptor-backed boot-live promotion proof: they validate the normal subsystem queue counters, normal subsystem API counters, normal device queues, normal ingress queue, normal source queue, normal descriptor queue, production inbox, per-device normal producer source-health counters, and sandbox-owner keepalive path, then leave owners active after `production boot-live keepalive active=1` while still reporting `boot_default_live=0`. The next step is replacing generated subsystem queue work with real normal virtio RX/TX, block request, input event, and GPU control subsystem sources while retaining explicit rollback to the recovery path. Boot-default live routing remains disabled until long-run traffic and rollback soak are proven.

Device-scoped detected-source gates are also available as `/vosprod-real-source-live-{block,gpu,net,input}` and `/vosprod-real-source-live-soak-{block,gpu,net,input}`. They narrow the owner mask to one detected device at a time, prove the same production inbox and sandbox driver seed evidence path, and keep rollback explicit. Device-scoped sandbox-source gates are available as `/vosprod-sandbox-source-live-{block,gpu,net,input}` and `/vosprod-sandbox-source-live-soak-{block,gpu,net,input}`; these keep generated normal producers disabled and require the selected sandbox virtio seed to submit normal work through `os_arm64_submit_normal_work`.

The current production-candidate bundle is `tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyProductionCandidate`. It passed locally on 2026-05-09 with exit code `0` after the normal-source soak gate moved to the descriptor queue boundary, again after typed per-device request metadata was added to normal descriptors, again after normal producer source-health counters became mandatory, and again after normal source queue insertion. It now covers all-device detected-source live, all-device detected-source long soak, source/descriptor-queued normal-subsystem producer long soak, all-device sandbox-source live/soak, device-scoped sandbox-source long soak, and device-scoped detected-source long soak for block/GPU/net/input. Treat this as the validation baseline for ARM64 sandbox virtio production routing. Do not treat it as permission to boot-autorun live routing; the remaining boundary is to replace generated normal source work with real normal subsystem producers connected to the same inbox.

`/vosstat` now reports `source_mode` explicitly. `detected-recovery-adapter` means source work is still generated by the temporary recovery adapter after real QEMU virtio devices are detected. `normal-subsystem-producer` means the explicit normal producer boundary is feeding the production inbox. `validated-keepalive` means source validation finished and sandbox owners remain active until `/stop`. `boot_default_live=0` must remain visible until persisted policy and real normal subsystem queue soak are proven. Normal subsystem queue, API, device, ingress, source, and descriptor evidence is now observable in the same status output through `ARM64 normal subsystem queues ...`, `ARM64 normal subsystem API ...`, `ARM64 normal device queues ...`, `ARM64 normal ingress queue ...`, `ARM64 normal source queue ...`, `ARM64 normal descriptor queue ...`, and `ARM64 normal producer health ...`, so an operator can confirm subsystem queue generation, subsystem API calls, producer isolation, ingress submission, descriptor forwarding, and rejected counts without searching the full validation log.

Compatibility imports `read_mmio`, `write_mmio`, and `alloc_dma_page` are intentionally denied and return no direct raw access. New drivers must use the handle-based `os_driver_*` ABI.
