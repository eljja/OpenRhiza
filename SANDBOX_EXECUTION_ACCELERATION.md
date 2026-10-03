# Sandbox Execution Acceleration

OpenRhiza can now run original Doom as a sandbox skill above 60 fresh frames per
second in the current performance profile. The remaining boundary problem is
quality headroom: guest-side `wasmi` interpretation is still too slow for large
C ports to keep larger viewports or high-detail rendering above 60.

## Current Doom Evidence

Command:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\validate_doom_frame_rate.ps1
```

Current x86_64 TCG result:

```text
[doom-fps] pass min=90 steady_min=209 steady_max=227 last=210 fresh_min=113 fresh_max=126 fresh_last=113
```

Interpretation:

- `fps` is the generic frame-resource presentation rate. This is above 60 after
  warmup.
- `fresh_fps` is the rate at which the sandbox produced a new frame sequence.
  The current performance profile also keeps this above 60 after warmup.
- Foreground frame-resource QoS pauses background Wasm `poll_net` work and
  reduces sandbox input polling while a live sandbox frame lease is active.
- Foreground busy-pump scheduling removes the normal one-tick sleep while the
  frame lease is active, skips API/network/driver/GUI background work, and leaves
  only frame pumping plus minimal input alive.
- The TCG launch path uses `tcg,thread=multi`; plain single-threaded TCG is too
  sensitive to host load and can drop fresh frames below 60.
- The frame-resource runtime tracks post-warmup `min_fps`,
  `min_fresh_fps`, and `floor_violations`; any window below the 60fps floor
  emits `[FrameResource][WARN]` on serial output and requests sandbox-frame
  close so a below-floor frame session does not keep running.
- Warmup windows are logged as `[FrameResourceWarmup]`; `[FrameResource]`
  samples are post-warmup values and must stay above the floor.
- Doom reads frame-resource metrics from the OS and adaptively lowers view size
  before `fps` or `fresh_fps` reaches the 60fps floor, then restores quality
  only after sustained headroom.
- Full-size `screenblocks=10` rendering still falls below 60 fresh frames under
  guest `wasmi`, so larger/high-detail visual modes need a generic compiled
  sandbox tier or sandbox-owned GPU/compositor acceleration.
- Post-link `wasm-opt -O4` reduced artifact size but caused a kernel allocation
  panic while loading SKDOGEN.WAS, so it is disabled by default and kept only as
  an explicit build experiment.
- WHPX did not reach Doom runnable state within the validation timeout on the
  current host, so it is not a dependable fix.

## Required Root Fix

The fix is a generic sandbox execution backend, not Doom-specific core code.

Target shape:

1. Keep Wasm as the portable skill artifact and validation format.
2. Add an optional compiled execution tier for hot sandbox modules:
   - AOT-compile the Wasm module to a bounded native code object, or
   - run a native sandbox worker with the same host ABI surface and capability
     handles.
3. Keep host calls object-scoped and capability-checked:
   - frame resources
   - block/file handles
   - input packets
   - driver handles
   - rollback-visible health counters
4. Promote a compiled worker only after deterministic equivalence checks:
   - import/export signature match
   - memory bounds proof
   - frame-resource sequence counters
   - timeout watchdog
   - rollback to interpreted Wasm
5. Never move Doom, virtio, filesystem, GUI shell policy, or device-specific
   logic into core.

## Minimum Acceptance Gate

Before calling the current adaptive low-detail Doom profile fixed:

- `tools\validate_doom_frame_rate.ps1` must pass with its default `90/90`
  visible/fresh FPS gate on x86_64 TCG. The script rejects thresholds below 60
  so the minimum-frame requirement cannot be weakened accidentally. Warmup is
  logged separately and is not counted as a post-warmup frame sample.
- A longer visible run must keep recovery shell rollback working through F12 or
  the sandbox frame close button.

Before calling larger Doom visual modes performance fixed:

- `tools\validate_doom_frame_rate.ps1 -MinFps 110 -MinFreshFps 110` must pass on
  x86_64 TCG or another default-supported accelerator.
- The compiled tier must fail closed: if load, ABI validation, watchdog, or
  frame sequencing fails, the OS returns to interpreted sandbox execution or the
  recovery shell.

The current 113-126 fresh-FPS result is achieved by reducing Doom's original
viewport and detail cost inside the sandbox skill, exposing frame-resource
metrics back to the sandbox for adaptive quality control, generic foreground
frame-resource QoS, busy-pump scheduling, indexed8 frame submission, QEMU High
process priority during validation, conservative quality restore after 60s of
sustained headroom, and multi-threaded TCG. A source-port-
quality 60 Hz visual experience at larger viewport/high-detail settings still
requires renderer interpolation in a sandbox-owned Doom/source-port skill or a
compiled sandbox backend fast enough to support that rendering budget.
