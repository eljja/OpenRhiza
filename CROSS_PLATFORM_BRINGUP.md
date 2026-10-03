# Cross-Platform Bring-Up

OpenRhiza must support multiple processors without turning the core into a large traditional OS.

For the current split status of `x86_64` and `ARM64`, remaining work, parity rules, and why ARM64 is taking longer, see `PLATFORM_PROGRESS.md`.

The invariant is:

Core contains only survival boot, recovery I/O, memory/interrupt setup, and sandbox host ABI.
Everything else is a driver, skill, workflow, policy, or evaluation record that can be fetched from OpenRhiza.com or generated and validated in a sandbox.

## Current Platform Matrix

The machine-readable matrix lives at:

```text
platforms/openrhiza-platforms.json
```

Inspect it with:

```powershell
python .\tools\platform_matrix.py
python .\tools\platform_matrix.py --registry-keys
```

Current targets:

- `x86_64-qemu-pc`: active reference target.
- `aarch64-qemu-virt`: serial recovery ELF plus sandbox-first virtio-mmio proof gates. It is not yet CLI/GUI parity with x86_64.
- `android-unlocked-aarch64`: research target after ARM64 QEMU works.
- `riscv64-qemu-virt`: parking-lot target after ARM64 path is stable.

## ARM64 First Milestone

Target:

```text
aarch64-qemu-virt
```

The first ARM64 boot must be deliberately small:

1. CPU entry.
2. Minimal page tables/MMU.
3. Exception vectors.
4. GIC interrupt gate.
5. PL011 UART recovery log/input.
6. Sandbox host ABI stubs.

The first milestone now exists as a standalone survival kernel:

```text
platform-kernels/aarch64-recovery
```

Build it with:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\build_arm64_recovery.ps1
```

Smoke-test it with:

```powershell
python .\tools\smoke_arm64_recovery.py
python .\tools\smoke_arm64_recovery.py --cli-baseline
python .\tools\smoke_arm64_recovery.py --virtio-probe
python .\tools\smoke_arm64_recovery.py --virtio-payload
# Experimental: validates ARM64 Wasm module instantiation plus tiny hand-authored executor host-import entry
# and verified virtio-gpu/block/net payload smokes submitted through that tiny executor path.
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm
# Experimental injected path: proves virtio-net RX and virtio-input event payloads through the tiny executor.
python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-injected
# Rollback drill: repeats device-specific tiny-executor payload/reset cycles before any live promotion is allowed.
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-rollback
# Proof-only promotion gate: full Wasm block/gpu/net/input seeds plus injected net/input evidence.
python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-promotion-gate
# Sustained rollback window: repeats the full Wasm block/gpu/net/input proof gate before any live promotion.
python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-sustained-gate --timeout 240
# Traffic-counter window: requires 12 rounds of block read/write, GPU control, net RX/TX, and input event evidence.
python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-traffic-window --timeout 360
# Independent proof-only device gates. These split traffic evidence by device and still do not enable live promotion.
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate block --timeout 360
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate gpu --timeout 360
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate net --timeout 360
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate input --timeout 360
# Explicit device live-owner cutover plus rollback selftests. These record a device owner bit only
# after the matching 12-round gate passes, then clear it and prove fallback execution.
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-live-cutover block --timeout 360
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-live-cutover gpu --timeout 360
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-live-cutover net --timeout 360
python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-live-cutover input --timeout 360
```

The `--cli-baseline` path validates the ARM64 recovery user surface: `input>` prompt, grouped `/help`, `/status`, registry/skill/driver/display/input state commands, and natural-language prompt stub behavior. It does not prove remote OpenRhiza.com or Gemini access; those stay blocked until virtio-net, TLS, and registry policy are sandbox-promoted.

On Windows, if Python resolves to the Microsoft Store shim or the Python smoke path is unavailable, use the PowerShell TCP serial harness:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\arm64_cli_tcp_smoke.ps1 -KeepLog
```

The harness uses QEMU serial `tcp:127.0.0.1:<port>,server` so QEMU waits for the test client before booting. Do not use `server,nowait` for this CLI smoke because it can discard early serial output before the client attaches.

Validate the visible ARM64 GUI bootstrap with:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\arm64_gui_tcp_smoke.ps1 -KeepLog
```

This launches QEMU with `-device ramfb`, runs `/gui-start` over TCP serial, and requires the `1920x1080` x86-compatible OpenRhiza session/sidebar/conversation/composer scene to be rendered. This is only the visible display bootstrap; it does not enable boot-default virtio-gpu or virtio-input live routing.

Run the current directly-runnable ARM64 sandbox virtio gate suite with:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1
```

For a faster smoke that skips rebuilds and runs only the rollback plus promotion proof gates:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -Quick
```

For the current explicit live-owner cutover/rollback suite:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -Quick -LiveCutover
```

For the current explicit live-routing diagnostic suite:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -Quick -LiveRouting
```

For the stricter route-all diagnostic, where all four virtio owners are active at the same time:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -Quick -LiveRouteAll
```

For the current post-boot live service pump diagnostic:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -Quick -LiveBootService
```

For the current ARM64 main-loop service pump diagnostic:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -Quick -LiveServiceLoop
```

For a focused rerun of only that main-loop path:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveServiceLoop
```

For the current typed OS request/event pump diagnostic:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -Quick -LiveOsPump
```

For a focused rerun of only that typed OS pump path:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveOsPump
```

For the longer typed OS pump window:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveOsPumpLong
```

For the opt-in live OS runtime producer path:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveOsRuntime
```

For the subsystem-facing producer bridge path:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveOsSubsystemBridge
```

For the persistent subsystem runtime producer path:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveSubsystemRuntime
```

For the production-style normal subsystem inbox path:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionQueue
```

For the focused external normal-subsystem submit proof, where the runtime waits for explicit normal work instead of generating source ticks internally:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionExternalNormalSourceLive
```

Validated locally on 2026-05-10 with `external normal subsystem submit-all accepted=6`, normal subsystem/device/ingress/source/descriptor counters all at `6` with zero rejects, production inbox `enqueued=6 forwarded=6 rejected=0`, `production external-normal keepalive active=1`, `external normal proof owners cleared`, and `live service running after stop=0`. This is a proof-owner boundary only. It validates the route real subsystems should use to submit normal work, but it does not enable boot-default live promotion.

For the per-device version of the same proof:

```powershell
tools/run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionExternalNormalDeviceSubmitLive
```

Validated locally on 2026-05-10 with `/vosprod-external-normal-submit-block`, `-gpu`, `-net`, and `-input` producing accepted counts `2/1/2/1`, then draining exactly six work items through the same normal subsystem queue chain and sandbox virtio owners. The poller waits for the complete enabled-device batch before dispatch, so single-device commands cannot create partial false-positive producer health. This remains proof-owner evidence only; boot-default live promotion is still closed.

For repeated device-shaped external normal submissions:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionExternalNormalDeviceSubmitSoak
```

This gate uses `/vosprod-external-normal-submit-device-round` repeatedly after `/vosprod-external-normal-source-live`. Each round queues block read/write, one GPU control item, net RX/TX, and one input event through the normal subsystem producer boundary, then waits for the ARM64 main loop to drain the complete six-item batch before submitting the next round. It passed locally on 2026-05-10 over eight rounds with exact normal subsystem/device/ingress/source/descriptor queue totals `48`, production inbox `enqueued=48 forwarded=48 rejected=0`, live pump counters `block_read=8 block_write=8 gpu_control=8 net_rx=8 net_tx=8 input_events=8 failures=0`, keepalive status, and `/stop` rollback. It is still proof-owner evidence, not production live routing.

For the opt-in production inbox runtime with a longer externally-submitted batch window:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_production_runtime_demo.ps1 -SkipBuild -Batches 64
```

For the production subsystem-source path, which disables diagnostic auto-feed and lets ARM64 recovery storage/display/network/input source ticks enqueue through `virtio_production_request_*` before sandbox owners drain the production inbox:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSubsystemSource
```

Validated locally on 2026-05-07 with exact counters:

```text
production subsystem source counters block_read=32 block_write=32 gpu_control=32 net_rx=32 net_tx=32 input_events=32 rejected=0
production inbox enqueued=192 forwarded=192 rejected=0
```

For the opt-in production subsystem-source live route, which validates the same source-to-production-inbox path and then leaves the sandbox virtio owners active until an explicit stop command:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSubsystemSourceLive
```

Validated locally on 2026-05-08 with exact counters:

```text
ARM64 subsystem queue enqueued=192 dispatched=192 rejected=0
production subsystem inbox enqueued=192 forwarded=192 rejected=0
production source-live keepalive active=1
live service running after stop=0
```

This is the first ARM64 path where production routing can be opened on demand and kept live after validation. It is still not boot-default policy; it must remain opt-in until real subsystem queues replace the temporary recovery source ticks and longer workload soak passes.

For the longer source-live soak gate:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSubsystemSourceLiveSoak
```

Validated locally on 2026-05-08 with exact counters:

```text
ARM64 subsystem queue enqueued=768 dispatched=768 rejected=0
production subsystem inbox enqueued=768 forwarded=768 rejected=0
production source-live keepalive active=1
live service running after stop=0
```

For the detected-device source-live gate, which first detects actual QEMU virtio-mmio device IDs and only then enqueues source work for those devices:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionRealSubsystemSourceLive
```

Validated locally on 2026-05-08 with:

```text
ARM64 real subsystem source detected device_mask=0x0000000f
ARM64 subsystem queue enqueued=192 dispatched=192 rejected=0
production subsystem inbox enqueued=192 forwarded=192 rejected=0
live service running after stop=0
```

For the longer detected-device source-live soak gate:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionRealSubsystemSourceLiveSoak
```

Validated locally on 2026-05-08 with 128 ticks / 768 work items, zero rejected work, `device_mask=0x0000000f`, sandbox driver seed payload evidence, and explicit `/stop` rollback. This still uses recovery source ticks gated by actual device presence; the next production step is connecting real normal subsystem queues to the same production inbox.

For the normal-subsystem producer boundary gate, which uses a separate source mode for the future net/block/input/GPU subsystem queues while preserving the same sandbox-owner rollback gate:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionNormalSubsystemSourceLive
```

Validated locally on 2026-05-09 with `normal_source=1`, `ARM64 normal subsystem producer source device_mask=0x0000000f`, exact 32-tick counters, normal producer source health `storage_ticks=32 display_ticks=32 network_ticks=32 input_ticks=32 rejected=0`, `source_mode=validated-keepalive boot_default_live=0` after validation, and `/stop` rollback. The normal producer path now bypasses recovery adapters and enqueues typed block/GPU/net/input descriptors into `VirtioArm64NormalDescriptorQueue`; each descriptor carries per-device request metadata such as block LBA/generation, GPU target size/frame, net packet/length, and input event tuple. The main-loop drain submits those descriptors through `virtio_production_request_*` into the sandbox-owned production inbox. This is still a boundary proof: the placeholder producer adapters must be replaced by real normal subsystem queues/events before boot-default live routing can be considered.

For the longer normal-subsystem producer boundary soak:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionNormalSubsystemSourceLiveSoak
```

Validated locally on 2026-05-09 after the normal producer path was split into `normal subsystem queues -> normal subsystem API -> normal device queues -> normal subsystem ingress queue -> device submit API -> normal source queue -> normal descriptor queue -> production inbox -> sandbox driver` with 128 ticks / 768 work items, `normal_source=1`, exact source/runtime/inbox counters, normal subsystem queues `enqueued=768 dispatched=768 rejected=0`, normal subsystem API `block_read=128 block_write=128 gpu_control=128 net_rx=128 net_tx=128 input_events=128 rejected=0`, normal device queues `enqueued=768 submitted=768 rejected=0`, normal ingress queue `enqueued=768 submitted=768 rejected=0`, normal source queue `enqueued=768 forwarded=768 rejected=0`, normal producer source health `storage_ticks=128 display_ticks=128 network_ticks=128 input_ticks=128 rejected=0`, and `/stop` rollback. Revalidated after adding per-device request metadata to the descriptor shape, after source-health counters became mandatory, after the source queue boundary was inserted ahead of descriptors, after generated producers were forced through device-specific submit functions, after generated producers were moved behind the ingress queue, after generated producers were isolated behind the normal device queue boundary, after the normal subsystem API counter boundary was inserted before device queues, and after the normal subsystem queue boundary was inserted before the API. The production-candidate wrapper includes this normal-source soak gate so regressions in the next subsystem-producer boundary are caught alongside detected-source gates.

For the sandbox-driver source proof gate, which disables generated normal producers and requires sandbox virtio driver seeds to submit work through the host import after payload evidence:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSandboxSourceLive
```

Validated locally on 2026-05-09 with `sandbox_source=1 generated_normal_source=0`, `ARM64 sandbox-driver source device_mask=0x0000000f`, six accepted sandbox-source submissions, normal subsystem/device/ingress/source/descriptor counters all at `6` with zero rejects, production inbox `enqueued=6 forwarded=6 rejected=0`, keepalive active, and `/stop` rollback. This is the first gate where sandbox driver seeds, not generated core producers, submit the normal work. It is intentionally only a one-tick proof gate; do not claim boot-default live routing or long-soak readiness from this result.

For the longer sandbox-driver source soak and the device-scoped sandbox-source gates:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSandboxSourceLiveSoak
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSandboxSourceDeviceLive
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionSandboxSourceDeviceLiveSoak
```

Validated locally on 2026-05-10. The all-device sandbox-source soak passed with `sandbox_source=1 generated_normal_source=0`, `ARM64 sandbox-driver source device_mask=0x0000000f`, 32 ticks / 192 sandbox-submitted work items, normal subsystem/device/ingress/source/descriptor counters all matching `192` with zero rejects, production inbox `enqueued=192 forwarded=192 rejected=0`, live pump counters `block_read=32 block_write=32 gpu_control=32 net_rx=32 net_tx=32 input_events=32 failures=0`, keepalive active, and `/stop` rollback. The device-scoped sandbox-source live and soak wrappers also passed for block, GPU, net, and input. The external-normal one-batch proof also passed, proving explicit normal-subsystem submit commands can feed the same queue chain without generated source work. This proves the sandbox driver seeds and external normal submit path can be sources of normal work over bounded validation windows, but boot-default live routing remains disabled until real subsystem queues and persisted recovery policy are connected.

For the descriptor-backed normal boot-live gate, which keeps the sandbox virtio owners active after normal producer descriptor evidence passes:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionNormalBootLive
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionNormalBootLiveSoak
```

Validated locally on 2026-05-09. The short gate passed with `ARM64 normal subsystem queues len=0 enqueued=192 dispatched=192 rejected=0`, `ARM64 normal subsystem API block_read=32 block_write=32 gpu_control=32 net_rx=32 net_tx=32 input_events=32 rejected=0`, `ARM64 normal device queues len=0 enqueued=192 submitted=192 rejected=0`, `ARM64 normal ingress queue len=0 enqueued=192 submitted=192 rejected=0`, `ARM64 normal source queue len=0 enqueued=192 forwarded=192 rejected=0`, `ARM64 normal descriptor queue len=0 enqueued=192 dispatched=192 rejected=0`, exact 32-tick runtime/source/inbox counters, normal producer health `storage_ticks=32 display_ticks=32 network_ticks=32 input_ticks=32 rejected=0`, `virtio live production boot gate passed=1`, `production boot-live keepalive active=1`, live owner mask `0x0000000f`, and `source_mode=validated-keepalive boot_default_live=0`. The long soak gate passed with 128 ticks / 768 work items through the same normal subsystem, API, device, ingress, source, and descriptor queues and normal producer health `storage_ticks=128 display_ticks=128 network_ticks=128 input_ticks=128 rejected=0`. This is the current closest ARM64 boot-live handoff proof, but boot-default policy remains disabled until those subsystem queue entries are fed by real long-running subsystem sources instead of generated normal producer work.

For device-scoped detected-device source-live gates:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionRealSubsystemSourceDeviceLive
```

Validated locally on 2026-05-08 for block `0x00000001` with 32 ticks / 64 work items, GPU `0x00000002` with 32 ticks / 32 work items, net `0x00000004` with 32 ticks / 64 work items, and input `0x00000008` with 32 ticks / 32 work items. These gates prove that the same production inbox path can be narrowed to one sandbox-active virtio owner at a time.

For the longer device-scoped detected-device source-live gates:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionRealSubsystemSourceDeviceLiveSoak
```

Validated locally on 2026-05-08 for the same block/GPU/net/input owner masks over a 128-tick window.

For the current production-candidate bundle:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyProductionCandidate
```

This runs the all-device detected-source live gate, all-device detected-source long soak, the normal-subsystem producer long soak, the all-device sandbox-source live/soak gates, the four device-scoped sandbox-source long-soak gates, and the four device-scoped detected-source long-soak gates. It is still an explicit opt-in validation bundle, not boot-default live routing.

Validated locally on 2026-05-09 with exit code `0` after the normal-source soak gate was moved onto the descriptor queue boundary, again after typed request metadata was added to normal descriptors, again after normal producer source-health counters became mandatory, and again after inserting a normal source queue ahead of descriptors. The bundle confirmed the detected all-device live path, the detected all-device 128-tick soak path, the source-queued and descriptor-queued normal-source 128-tick soak path with `storage/display/network/input=128` and `rejected=0`, the sandbox-driver source proof gate with generated normal producers disabled, and the block/GPU/net/input device-scoped 128-tick detected-source soak paths. On 2026-05-10 the production-candidate wrapper was extended to include the all-device sandbox-source soak and block/GPU/net/input device-scoped sandbox-source soak gates, so the bundle now catches regressions where sandbox virtio seeds stop submitting normal work through `os_arm64_submit_normal_work`. The remaining production step is not another diagnostic counter: replace generated normal producer work with real normal subsystem queues/events or sandbox-driver source submissions over long soak windows while preserving the same sandbox-owner rollback gate.

Status output must make this distinction visible. `/vosstat` reports `source_mode=detected-recovery-adapter` while detected recovery source adapters are feeding the queue, `source_mode=normal-subsystem-producer` while the normal producer boundary is feeding the queue, `source_mode=validated-keepalive` after validation leaves sandbox owners active, and `boot_default_live=0` while automatic live routing remains blocked. When normal source work has run, `/vosstat` also prints `ARM64 normal subsystem queues ...`, `ARM64 normal subsystem API ...`, `ARM64 normal device queues ...`, `ARM64 normal ingress queue ...`, `ARM64 normal source queue ...`, `ARM64 normal descriptor queue ...`, and `ARM64 normal producer health ...`; all seven must remain zero-reject before any future promotion policy can be considered.

For the current opt-in production boot promotion gate, which starts the production inbox runtime, feeds 64 main-loop ticks through sandbox-active block/GPU/net/input owners, verifies exact counters, then rolls back:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionBootGate
```

For the opt-in boot-live gate, which runs the same 64-tick validation but keeps sandbox-active virtio owners alive after the gate passes until `/vosstop`:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionBootLive
```

For the longer opt-in boot-live soak gate, which runs a 128-tick all-device production window before keeping sandbox-active virtio owners alive:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionBootLiveSoak
```

Validated locally on 2026-05-07 with exact counters:

```text
block_read=128 block_write=128 gpu_control=128 net_rx=128 net_tx=128 input_events=128 failures=0
production inbox enqueued=768 forwarded=768 rejected=0
```

For device-specific boot promotion gates:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionDeviceBootGates
```

For device-specific boot-live keepalive gates, which validate one sandbox owner at a time and keep only that owner active after the gate passes until `/vosstop`:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveProductionDeviceBootLiveGates
```

For the manual subsystem submit path:

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyLiveManualSubsystem
```

Expected serial output:

```text
OpenRhiza ARM64 recovery core
platform=aarch64-qemu-virt serial=PL011
arm64>
```

Do not add virtio-net, virtio-block, virtio-gpu, GUI, filesystem, or voice logic to the ARM core.
Those must be sandbox capabilities once the survival path exists.

## ARM64 Sandbox Host ABI

The ARM64 path now has the first virtio-mmio capability contract without adding a virtio driver to the core.

Core-owned pieces:

- CPU/MMU/exception/GIC/PL011 survival path.
- Platform MMIO descriptor installation for QEMU `virt`.
- Opaque capability handles for `virtio-mmio` windows.
- Bounded `os_driver_mmio_read32`, `os_driver_mmio_write32`, DMA, and IRQ-poll imports.
- Lease/release bookkeeping.

Sandbox-owned pieces:

- `skill_virtio_mmio_probe_v1` transport identity probe.
- `skill_virtio_net_driver_v1`, `skill_virtio_block_driver_v1`, `skill_virtio_input_driver_v1`, and `skill_virtio_gpu_driver_v1` candidate drivers.
- Feature negotiation, queue setup, request dispatch, and device policy.

The core API is:

```text
os_driver_claim_mmio_device(key_ptr, key_len, id_ptr, id_len, caps) -> handle
```

Supported first match keys:

```text
virtio:mmio
virtio-mmio:0x0a000000
```

The current transport probe is intentionally read-only. It checks the virtio transport identity registers and releases the handle. It must not become a generic virtio implementation.

The four device-specific virtio seeds now submit bounded device payloads from sandbox-owned DMA. Each one claims bounded `virtio-mmio:0x<base>` handles, filters by standard virtio device ID, reads identity/features/queue limits, builds and reads back a device-specific virtqueue payload, negotiates feature bits, programs queue addresses, notifies the queue, and commits only if the used ring or IRQ provides completion evidence plus device-specific response readback. `virtio-net` submits/arms RX/TX packet buffers, `virtio-block` submits sector read plus guarded scratch-sector write requests, `virtio-input` arms and reads an event buffer, and `virtio-gpu` submits a display-info control command and reads the response header. If completion evidence does not appear, promotion remains denied and the gate is rolled back.

The ARM64 serial recovery kernel exposes matching diagnostic commands:

```text
/abi-status
/virtio-mmio-probe
/virtio-payload-selftest
/virtio-wasm-selftest
/virtio-wasm-injected-selftest
/virtio-wasm-rollback-selftest
/virtio-wasm-promotion-gate-selftest
/virtio-wasm-sustained-gate-selftest
/virtio-wasm-traffic-window-selftest
/virtio-wasm-enable-gate-block
/virtio-wasm-enable-gate-gpu
/virtio-wasm-enable-gate-net
/virtio-wasm-enable-gate-input
/virtio-wasm-live-status
/virtio-wasm-live-cutover-block
/virtio-wasm-live-cutover-gpu
/virtio-wasm-live-cutover-net
/virtio-wasm-live-cutover-input
/virtio-wasm-live-rollback-block
/virtio-wasm-live-rollback-gpu
/virtio-wasm-live-rollback-net
/virtio-wasm-live-rollback-input
/virtio-wasm-live-cutover-rollback-block
/virtio-wasm-live-cutover-rollback-gpu
/virtio-wasm-live-cutover-rollback-net
/virtio-wasm-live-cutover-rollback-input
/virtio-wasm-live-dispatch-block
/virtio-wasm-live-dispatch-gpu
/virtio-wasm-live-dispatch-net
/virtio-wasm-live-dispatch-input
/virtio-wasm-live-route-all
/virtio-wasm-live-rollback-all
/virtio-wasm-live-routing-selftest
/virtio-wasm-live-route-all-selftest
/virtio-wasm-live-service-status
/virtio-wasm-live-service-start
/virtio-wasm-live-service-tick
/virtio-wasm-live-service-window
/virtio-wasm-live-service-stop
/virtio-wasm-live-service-loop-selftest
/virtio-wasm-live-os-pump-selftest
/virtio-wasm-live-os-runtime-start
/virtio-wasm-live-os-submit-all
/virtio-wasm-live-os-runtime-status
/virtio-wasm-live-os-runtime-stop
/virtio-wasm-live-os-runtime-selftest
/virtio-wasm-live-os-subsystem-bridge-selftest
/vossys
/virtio-wasm-live-subsystem-runtime-soak
/vosps
/virtio-wasm-live-production-queue-selftest
/vosprod
/virtio-wasm-live-production-boot-gate
/vosboot
/virtio-wasm-live-production-boot-live
/vosboot-live
/virtio-wasm-live-production-boot-live-soak
/vosboot-live-soak
/vosboot-block
/vosboot-gpu
/vosboot-net
/vosboot-input
/vosboot-live-block
/vosboot-live-gpu
/vosboot-live-net
/vosboot-live-input
/virtio-wasm-live-production-runtime-start
/vosprod-start
/virtio-wasm-live-production-submit-all
/vosprod-all
/virtio-wasm-live-boot-service-selftest
/virtio-wasm-rust-probe
```

`/virtio-mmio-probe` reads only the transport identity registers from the bounded first MMIO window. It does not negotiate features, configure queues, or drive packets/blocks/input.
`/virtio-payload-selftest` is a temporary runtime verification harness, not a production driver. It exists to prove that ARM64 QEMU `virt` can execute the same class of sandbox-owned virtqueue payloads before the full Wasm runtime is online. With `-VirtioPayloadDevices`, current QEMU coverage proves virtio-gpu `GET_DISPLAY_INFO`, virtio-block sector read, guarded virtio-block scratch write, and virtio-net TX completion. `tools/smoke_arm64_recovery.py --virtio-payload --inject-net-rx` switches the netdev to a socket backend, injects a raw Ethernet frame, and proves virtio-net RX completion plus payload readback. `tools/smoke_arm64_recovery.py --virtio-payload --inject-input` attaches QMP/VNC input frontends, injects pointer events, and proves virtio-input event queue completion plus event readback.

`/virtio-wasm-selftest` wires a no_std `wasmi` runtime into the ARM64 recovery ELF behind a diagnostic path. The runtime can build, instantiate, and call sandbox modules with host imports after the recovery core enables ARM64 FP/SIMD access through `CPACR_EL1`. The build path includes `alloc` for the custom ARM64 target. The ARM64 recovery kernel also keeps a tiny hand-authored Wasm executor for a deliberately small opcode subset so diagnostic payloads can stay deterministic while the full runtime matures. That executor proves `run_skill`-style return, host-import entry, and completion-proven virtio-gpu `GET_DISPLAY_INFO`, virtio-block sector-read, guarded virtio-block scratch-write, and virtio-net TX payload smokes submitted through a host import under QEMU.

`/virtio-wasm-injected-selftest` is the explicit injected-payload tiny-executor path. The smoke harness boots only virtio-net plus virtio-input, injects a raw Ethernet frame and pointer events, then proves virtio-net RX completion/readback and virtio-input event completion/readback behind the tiny executor import `os_virtio_injected_payload_smoke`. This closes the previous tiny-executor coverage gap for input and net RX. It is still a diagnostic bridge, not a production virtio driver in core, and live promotion remains blocked until sustained sandbox-owner rollback tests pass.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-rust-probe` adds a test-only virtio-rng marker and runs the Rust-generated Wasm diagnostic at boot. This proves full `wasmi::TypedFunc::call()` on ARM64 with a return-only module, a memory-backed host import, a Rust-generated log module, and the Rust-generated virtio-gpu payload seed. The previous stall was caused by FP/SIMD access trapping before `CPACR_EL1` enabled FP/SIMD for the recovery runtime.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-rollback` runs the tiny executor through ten repeated device-specific payload/reset cycles for virtio-gpu, virtio-block read/write, and virtio-net TX. This is a rollback drill, not live promotion.

`tools/smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-promotion-gate` runs proof-only live-gate checks through the full Wasm sandbox runtime. Current positive evidence covers full Wasm virtio-block read/write, virtio-gpu control, virtio-net RX/TX, and virtio-input event readback. The virtio-input full seed now handles both modern and legacy virtio-mmio queue programming, claims the device, programs status/event queues, notifies queue `0`, waits through `os_driver_wait_used_idx`, receives a used-ring completion, reads back the injected pointer event, and rolls back safely. This command does not enable live promotion.

`tools/smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-sustained-gate --timeout 240` runs `/virtio-wasm-sustained-gate-selftest`. It repeats the full Wasm block/gpu/net/input promotion proof across five rollback-window rounds and requires `sustained gate rounds=5 passed=1`. This is a sustained rollback-window proof, but it still does not enable live promotion.

`tools/smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-traffic-window --timeout 360` runs `/virtio-wasm-traffic-window-selftest`. It repeats the full Wasm proof path across 12 rounds and requires exact counters for `block_read`, `block_write`, `gpu_control`, `net_rx`, `net_tx`, and `input_events`. The latest passing evidence is `traffic counters block_read=12 block_write=12 gpu_control=12 net_rx=12 net_tx=12 input_events=12` with `traffic window rounds=12 passed=1`. An earlier run caught a block counter miss at 11/12 and correctly blocked promotion, which is the intended behavior for this gate.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate <block|gpu|net|input> --timeout 360` runs one independent proof-only device gate. Each gate runs a logged warm-up that is not counted, then requires 12 positive evidence rounds for one device: block requires `block_read=12 block_write=12`, GPU requires `gpu_control=12`, net requires `net_rx=12 net_tx=12`, and input requires `input_events=12`. Block/GPU/net remain strict counted-round gates. Input may retry bounded no-evidence rounds because external QMP/VNC event injection can miss a polling window, but no retry is counted as evidence and the gate still fails closed if 12 positive event completions are not collected. The warm-up exists because virtio-block scratch writes can miss the very first completion while the QEMU device settles; this is logged and not treated as evidence.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-live-cutover <block|gpu|net|input> --timeout 360` runs an explicit live owner cutover plus rollback selftest for one device. The command records the device owner bit only after the matching 12-round evidence gate passes, verifies `owner active after cutover=1`, clears the owner bit through rollback, requires `rollback proof=1`, and verifies `owner active after rollback=0`. This is still an operator-triggered diagnostic path, not boot autorun and not yet a continuous production owner loop.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-routing --timeout 720` runs `/virtio-wasm-live-routing-selftest`. It explicitly cuts over block, GPU, net, and input one device at a time, runs four sandbox-owner dispatch ticks per device, verifies the owner stays active during dispatch, rolls the owner back, and requires the owner to be cleared afterward. The current passing evidence is `production live routing diagnostic passed=1`, final owner mask `0`, and dispatch counters `block_ok=4 block_fail=0 gpu_ok=4 gpu_fail=0 net_ok=4 net_fail=0 input_ok=4 input_fail=0`. This is the first end-to-end owner dispatch loop, but it is still diagnostic and operator-triggered. The next production step is to connect these dispatch ticks to the real OS virtio request queues/event pumps rather than a bounded selftest window.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-route-all --timeout 720` runs `/virtio-wasm-live-route-all-selftest`. It cuts over all four virtio devices together, verifies the route-all owner mask is active, runs four sandbox-owner dispatch ticks per device, then runs rollback-all and requires the final owner mask to be zero. This is stricter than the sequential routing diagnostic because the owner mask contains all device owners at once before dispatch begins.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-boot-service --timeout 900` runs `/virtio-wasm-live-boot-service-selftest`. It boots the ARM64 recovery kernel, explicitly starts the sandbox-owner service pump after the prompt is available, routes all four virtio owners, runs four service ticks across block/GPU/net/input, then stops through rollback-all. Passing evidence requires `live service started=1`, `live service window ticks=4 passed=1`, zero dispatch failures, `live service running after stop=0`, and `virtio live boot service selftest passed=1`. This is the first boot-to-service-pump path; it is still operator-triggered and must not become default boot autorun until the service tick is connected to real OS request queues/event pumps and long-run rollback soak passes.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-service-loop --timeout 900` runs `/virtio-wasm-live-service-loop-selftest`. It starts the same sandbox-owner service, returns from the command handler, and requires the ARM64 recovery main idle loop to pump four bounded request/event ticks. Passing evidence requires `live service mainloop selftest armed=1`, `command returned; ARM64 main loop must pump service ticks.`, `live service mainloop target ticks=4 queue_ok=1 counters_ok=1`, rollback-all with owner mask zero, and `virtio live service mainloop selftest passed=1`. This proves service dispatch is no longer only a command-local window. It is still a bounded synthetic request/event pump, not default production live routing.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-pump --timeout 900` runs `/virtio-wasm-live-os-pump-selftest`. It uses the same main idle loop path but queues typed OS work instead of one generic request per device: block read, block write, GPU control, net RX, net TX, and input event work. Passing evidence requires `live OS pump target ticks=4 queue_ok=1 counters_ok=1`, `live OS pump queue block_read_pending=0 block_write_pending=0 gpu_control_pending=0 net_rx_pending=0 net_tx_pending=0 input_event_pending=0 enqueued=24 drained=24`, `live OS pump counters block_read=4 block_write=4 gpu_control=4 net_rx=4 net_tx=4 input_events=4 failures=0`, rollback-all with owner mask zero, and `virtio live OS pump selftest passed=1`. This is still a bounded diagnostic, not default boot autorun.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-pump-long --timeout 1200` runs the short recovery alias `/vospl` for the same typed pump. The short alias is intentional because it avoids long PL011 command-string fragility while validating the same path. Passing evidence requires `live OS pump profile=long target_ticks=16`, `live OS pump target ticks=16 queue_ok=1 counters_ok=1`, `enqueued=96 drained=96`, and exact counters `block_read=16 block_write=16 gpu_control=16 net_rx=16 net_tx=16 input_events=16 failures=0`.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-request-api --timeout 900` runs the short recovery alias `/vosrq`. It prequeues typed block read/write, GPU control, net RX/TX, and input event work through OS producer APIs, returns from the command handler, and requires the ARM64 main loop to drain the producer-owned work through sandbox-active virtio owners before rollback-all. Passing evidence requires `OS producer API prequeued items=24`, `OS producer API selftest armed=1`, `live OS pump target ticks=4 queue_ok=1 counters_ok=1`, `enqueued=24 drained=24`, and exact counters `block_read=4 block_write=4 gpu_control=4 net_rx=4 net_tx=4 input_events=4 failures=0`. This proves the request boundary shape for future real OS subsystem producers, but it is still bounded and operator-triggered.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-runtime --timeout 900` runs the short recovery alias `/voslive`. It starts the opt-in sandbox-active runtime mode, prequeues 24 producer-owned typed work items, returns from the command handler, lets the normal ARM64 main loop pump only while pending work exists, then rolls all virtio owners back. Passing evidence requires `live OS runtime selftest armed=1`, `live OS runtime selftest prequeued items=24`, `command returned; ARM64 main loop must drain live runtime OS work.`, `live OS pump target ticks=4 queue_ok=1 counters_ok=1`, `enqueued=24 drained=24`, exact counters `block_read=4 block_write=4 gpu_control=4 net_rx=4 net_tx=4 input_events=4 failures=0`, and `virtio live OS runtime selftest passed=1`. This is the current closest safe step to live production routing, but it is still operator-triggered and intentionally not boot-default.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-subsystem-bridge --timeout 1200` runs the short recovery alias `/vossys`. It validates the next boundary after `/voslive`: subsystem-facing producer functions enqueue typed block read/write, GPU control, net RX/TX, and input event work into the opt-in sandbox-active runtime. Passing evidence requires `subsystem bridge selftest armed=1`, `subsystem bridge prequeued items=96`, `command returned; ARM64 main loop must drain subsystem-backed OS work.`, `live OS pump target ticks=16 queue_ok=1 counters_ok=1`, `enqueued=96 drained=96`, exact runtime counters `block_read=16 block_write=16 gpu_control=16 net_rx=16 net_tx=16 input_events=16 failures=0`, exact subsystem producer counters `block_read=16 block_write=16 gpu_control=16 net_rx=16 net_tx=16 input_events=16 rejected=0`, and `virtio live OS subsystem bridge selftest passed=1`. This proves the callable boundary that real net/block/input/GPU subsystems should use, but the work source is still a bounded diagnostic producer rather than mature production subsystem queues.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-subsystem-runtime --timeout 1800` runs the short recovery alias `/vosps`. It validates the next boundary after `/vossys`: subsystem work is no longer prequeued in one burst. The opt-in sandbox-active runtime stays alive, the ARM64 idle main loop asks the subsystem producer for one typed work round only when the queue is empty, sandbox virtio owners drain that work, and the cycle repeats for 32 ticks/192 work items before rollback-all. Passing evidence requires `subsystem runtime soak armed=1 target_ticks=32`, `command returned; subsystem producer will feed sandbox owners from the main loop.`, `live OS pump target ticks=32 queue_ok=1 counters_ok=1`, `enqueued=192 drained=192`, exact runtime counters `block_read=32 block_write=32 gpu_control=32 net_rx=32 net_tx=32 input_events=32 failures=0`, exact subsystem producer counters `block_read=32 block_write=32 gpu_control=32 net_rx=32 net_tx=32 input_events=32 rejected=0`, `subsystem runtime failures=0`, and `virtio live subsystem runtime soak passed=1`. This is a persistent-runtime proof, but boot-default live routing is still disabled.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-queue --timeout 1200` runs the short recovery alias `/vosprod`. It validates the current production-routing candidate shape: a normal-subsystem inbox accepts typed block read/write, GPU control, net RX/TX, and input event work, then forwards that work into sandbox-active owners from the ARM64 main loop. Passing evidence requires `production subsystem queue selftest armed=1 target_ticks=8`, `live OS pump target ticks=8 queue_ok=1 counters_ok=1`, `enqueued=48 drained=48`, exact runtime counters `block_read=8 block_write=8 gpu_control=8 net_rx=8 net_tx=8 input_events=8 failures=0`, exact subsystem producer counters `block_read=8 block_write=8 gpu_control=8 net_rx=8 net_tx=8 input_events=8 rejected=0`, production inbox counters `enqueued=48 forwarded=48 rejected=0`, `production queue failures=0`, and `virtio live production subsystem queue passed=1`. This is the preferred handoff shape for real ARM64 net/block/input/GPU subsystem queues, but it is still explicit and rollback-gated.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-runtime --production-runtime-batches <n> --timeout 1200` runs `/vosprod-start`, then feeds `/vosprod-all` submit batches after the command handler returns. It validates that the production inbox can stay active as an opt-in runtime, accept external normal-subsystem work, forward that work into sandbox-active owners from the ARM64 main loop, report status, and roll back through `/vosstop`. Passing evidence requires `production inbox runtime active=1`, `<n>` `production inbox submit-all accepted=6` events, `<n>` `production subsystem inbox accepted=0 forwarded=6` drain rounds, exact runtime counters `block_read=<n> block_write=<n> gpu_control=<n> net_rx=<n> net_tx=<n> input_events=<n> failures=0`, production inbox counters `enqueued=<n*6> forwarded=<n*6> rejected=0`, `production queue runtime running=1 failures=0` before stop, and `virtio live service stop complete`. `tools/run_arm64_production_runtime_demo.ps1` is the focused runnable wrapper; it defaults to 64 batches, writes the full QEMU log to `target/arm64-production-runtime-demo.log`, and prints the key counters. The 64-batch path has passed locally with `block_read=64 block_write=64 gpu_control=64 net_rx=64 net_tx=64 input_events=64 failures=0` and `enqueued=384 forwarded=384 rejected=0`. This is now the preferred opt-in runtime shape for future real subsystem producers. It is still not boot-default live routing.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-boot-gate --timeout 1800` runs `/vosboot`. It starts the opt-in production inbox runtime, auto-feeds normal-subsystem work for 64 ARM64 main-loop ticks, drains through sandbox-active virtio block/GPU/net/input owners, validates exact counters `block_read=64 block_write=64 gpu_control=64 net_rx=64 net_tx=64 input_events=64 failures=0`, validates production inbox counters `enqueued=384 forwarded=384 rejected=0`, prints `virtio live production boot gate passed=1`, then rolls every owner back. `/vosboot-block`, `/vosboot-gpu`, `/vosboot-net`, and `/vosboot-input` run the same 64-tick boot gate for one device mask at a time. These gates are now passing locally, but they remain explicit opt-in promotion gates; boot-default live promotion still requires persisted policy and longer real-workload soak.

`tools/smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-manual-subsystem --timeout 1200` runs the short recovery alias `/vosman`, then feeds `/vossall` from outside the runtime. It validates the next boundary after `/vosps`: the sandbox-active runtime stays armed after the command returns, external subsystem submit commands enqueue typed block read/write, GPU control, net RX/TX, and input event work, and the ARM64 idle main loop drains that work through the active sandbox owners. Passing evidence requires `manual subsystem selftest armed=1 required_submit_all=4`, at least four `subsystem manual submit-all queued=6` events, `live OS pump target ticks=4 queue_ok=1 counters_ok=1`, `enqueued=24 drained=24`, exact runtime counters `block_read=4 block_write=4 gpu_control=4 net_rx=4 net_tx=4 input_events=4 failures=0`, exact subsystem producer counters `block_read=4 block_write=4 gpu_control=4 net_rx=4 net_tx=4 input_events=4 rejected=0`, and `virtio live manual subsystem selftest passed=1`. This proves external subsystem-facing calls can feed sandbox owners after runtime activation, but it is still an explicit diagnostic gate rather than production boot routing.

The input harness uses `virtio-mouse-device` because the QMP key path reports no key handler for that device class. The passing path sends pointer events, prepares both virtio-input status and event queues, posts multiple event buffers, and requires non-empty event readback before declaring success.

## ARM64 QEMU Runner

The scaffolded runner is:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1 -VirtioProbeDevice
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1 -VirtioPayloadDevices
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1 -Graphic -VirtioPayloadDevices
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1 -Graphic -Ramfb
```

The default ARM64 runner is serial recovery mode (`-nographic`). This is intentional: PL011 serial remains the recovery-safe control path. For a visible ARM64 GUI bootstrap, run with `-Graphic -Ramfb` and then execute `/gui-start` or `/skill-run skill_gui_session_bootstrap_v1` at the ARM64 prompt. The `ramfb` path mirrors the x86 GUI layout while keeping full GUI policy/input ownership out of permanent core.

To open the visible QEMU GTK window and inject `/gui-start` automatically:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\launch_arm64_gui.ps1
```

If the generated ARM64 recovery ELF exists, the runner defaults to:

```text
target/aarch64-openrhiza-none/debug/openrhiza-arm64.elf
```

If the ELF is missing, the runner validates QEMU availability and reports the expected build/run shape instead of mutating the x86 path.

Manual run:

```powershell
pwsh.exe -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1 .\target\aarch64-openrhiza-none\debug\openrhiza-arm64.elf
```

## Android Direction

Android is not the first ARM target.

Use Android later for:

- an unlocked-device recovery core
- Android app/user-space compatibility experiments
- audio/touch/display bridge skills
- device-specific registry metadata

The phone goal is not "replace every Android device."
The first phone goal is one known unlocked device with a minimal recovery core and sandbox-loaded capabilities.

## Registry Keys

Every platform capability should use explicit match keys:

- `arch:x86_64`
- `arch:aarch64`
- `arch:riscv64`
- `machine:qemu-pc`
- `machine:qemu-aarch64-virt`
- `machine:qemu-riscv64-virt`
- `android-device:<vendor>:<model>:<codename>`
- `dt:<compatible-string>`
- `virtio:mmio`
- `virtio:<device-class>`

Drivers and skills must declare:

- supported architecture
- supported machine or board
- required host ABI version
- whether it is boot-critical, recovery-critical, or optional
- validation status per platform

## Required OS Commands

OpenRhiza exposes:

```text
/platform-status
```

This command reports the active platform plan inside the OS so the LLM and operator can keep architecture expansion aligned with the core-minimal rule.

## Work Queue

1. Keep x86_64 QEMU stable.
2. Split x86-specific boot code out of `src/main.rs` into an x86 platform entry module.
3. Keep `platform-kernels/aarch64-recovery` as the serial survival baseline.
4. Split x86-specific boot code out of the main kernel path.
5. Wire ARM64 sandbox host ABI stubs into the real ARM64 runtime after MMIO mappings exist.
6. Run `skill_virtio_mmio_probe_v1` against QEMU `virt` transport windows.
7. Load virtio driver skills from OpenRhiza.com.
8. Add rollback-safe live gates for virtio-net, virtio-block, virtio-input, and virtio-gpu queue ownership.
9. Add platform evaluation uploads per boot target.

## Validation Status

The current ARM64 milestone is still a serial-first recovery kernel, but it now
validates sandbox-owned virtio-mmio payload routing under QEMU before any boot
promotion is allowed. Display remains inactive until a display/GPU skill owns a
real framebuffer path; this is intentional and keeps the survival core minimal.

It proves:

- QEMU ARM64 launch path
- CPU entry
- stack setup
- PL011 serial output/input
- local recovery prompt
- QEMU virtio-mmio capability handle probe with `--virtio-probe`
- runtime virtio payload harness with `--virtio-payload`: gpu control, block read/write, and net TX completion
- injected runtime payload harnesses: virtio-net RX with socket netdev injection and virtio-input event readback with QMP/VNC pointer injection
- directly runnable ARM64 gate suite: `tools/run_arm64_virtio_gate.ps1` validates rollback drill, full Wasm proof gate, injected tiny-executor input/net RX path, and Rust-generated Wasm call probe
- locally built virtio-net/block/input/gpu sandbox driver candidates
- experimental ARM64 `wasmi` runtime linkage and module instantiation behind `/virtio-wasm-selftest`
- deterministic tiny hand-authored Wasm executor reaching host imports and submitting gpu/block/net-TX plus injected net-RX/input payloads under QEMU
- repeated ten-cycle tiny-executor rollback drill for gpu, block read/write, and net TX payload candidates
- detected-device source-live routing where `/vosprod-real-source-live` and `/vosprod-real-source-live-soak` only enqueue work for actual QEMU virtio-mmio devices and require sandbox driver seed block read/write, GPU control, net TX/RX, and input event payload completion before drain counters advance
- proof-only full Wasm promotion gates for virtio-block, virtio-gpu, virtio-net, and virtio-input
- five-round sustained full Wasm rollback-window gate for virtio-block, virtio-gpu, virtio-net, and virtio-input
- 12-round full Wasm traffic-counter window for virtio-block read/write, virtio-gpu control, virtio-net RX/TX, and virtio-input events
- independent 12-round full Wasm device gates for virtio-block, virtio-gpu, virtio-net, and virtio-input
- ARM64 main-loop typed OS pump dispatch for block read/write, GPU control, net RX/TX, and input event work before rollback-all, including a 16-tick/96-work-item long-window proof
- ARM64 OS producer API boundary proof with `/vosrq`: producer-owned typed work is queued before returning to the main loop, then drained through sandbox-active virtio owners and rolled back
- ARM64 opt-in live runtime proof with `/voslive`: sandbox-active owners stay in runtime mode, producer-owned typed work drains from the normal main loop only when pending, then rollback-all clears every owner
- ARM64 subsystem producer bridge proof with `/vossys`: subsystem-facing functions queue 16 rounds/96 typed work items into the opt-in sandbox-active runtime, the main loop drains them through sandbox owners, and rollback-all clears every owner
- ARM64 persistent subsystem runtime proof with `/vosps`: the main loop produces one subsystem-backed work round at a time for 32 rounds/192 typed work items, drains through sandbox owners, and rollback-all clears every owner
- ARM64 manual subsystem submit proof with `/vosman` plus `/vossall`: external subsystem submit commands feed the already-armed sandbox-active runtime, the main loop drains the work through sandbox owners, and rollback-all clears every owner
- ARM64 opt-in production boot promotion gates with `/vosboot` and `/vosboot-{block,gpu,net,input}`: production inbox work drains for 64 main-loop ticks through all devices or one selected device, exact counters are checked, and rollback-all clears every owner
- ARM64 opt-in production boot-live gates with `/vosboot-live-soak` and `/vosboot-live-{block,gpu,net,input}`: all-device soak passed locally on 2026-05-07 for 128 ticks/768 production-inbox items with exact counters and `production boot-live keepalive active=1`; device-specific gates now narrow the live owner mask to exactly one device (`0x1`, `0x2`, `0x4`, or `0x8`) and validate only that device's counters before keepalive

It does not yet prove:

- main x86_64 kernel portability
- mature production net/block/input/GPU subsystem queues replacing the temporary `/vosprod-source`, `/vosps`, `/vosman`, `/vosboot`, and `/vosboot-live*` recovery producers on ARM64
- persistent boot-default virtio-mmio ownership
- ARM64 display/input/storage
- registry-loaded ARM64 capabilities

Track the latest status in `VALIDATION_MATRIX.md`.
