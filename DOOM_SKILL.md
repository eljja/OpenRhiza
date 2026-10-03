# Doom On OpenRhiza

OpenRhiza should run Doom as a sandbox skill, not as core code.

## Current State

Re-audit on 2026-08-14: the original doomgeneric engine currently initializes in
QEMU, opens `DOOM1.WAD`, reaches the 320x200 renderer, and submits indexed frames.
This is an engine/rendering baseline, not yet an accepted playable application.
Automated gameplay input/state verification is missing, mouse and audio are not
wired, save/config writes are no-ops, and the latest baseline dropped below the
current frame-resource floor. The launcher also required a cache-capacity fix after
the module map grew beyond its old 1024-byte assumption.

- Official Doom source is available under `third_party/doom`.
- Doomgeneric portable source is available under `third_party/doomgeneric`.
- `sandbox-skills/doomgeneric_openrhiza_seed` builds the current
  `skill_doom_port_v1` artifact as `SKDOGEN.WAS`.
- The skill runs the original Doom renderer through doomgeneric and the legal
  Doom shareware IWAD (`DOOM1.WAD`) or a user-supplied compatible IWAD.
- The Doom game loop, renderer, WAD access, input mapper, and timing policy stay
  inside the sandbox skill. Core exposes only generic bounded host imports:
  frame resources, file reads, keyboard packets, monotonic time, and sandbox
  frame release/close.
- Frames are submitted through the generic indexed8 frame resource ABI
  (`os_frame_resource_submit_indexed8_seq`). The generic core frame pump calls
  the skill's `frame_pump` export at the requested cadence; no Doom-specific
  scheduler or renderer is in core.
- The experimental x86_64 performance profile uses original Doom low-detail mode,
  a smaller original viewport (`screenblocks=4`), foreground frame-resource
  scheduler QoS, foreground busy-pump scheduling, and QEMU TCG multi-threading
  to investigate performance under the interpreted Wasm runtime. It is not a
  current pass claim. Doom's simulation must run at its correct 35 Hz timebase;
  compositor presentation rate and new game frames are separate metrics.

## Run Path

One-command host launch:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1
```

This boots x86_64 OpenRhiza in QEMU, waits for the sandbox GUI session, loads
`skill_doom_port_v1`, starts the runtime, advances frames, and leaves the QEMU
window open.

To provide a legal IWAD or FreeDoom IWAD:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1 -WadPath C:\path\to\FREEDOOM.WAD
```

To download and install the latest FreeDoom release automatically:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1 -DownloadFreedoom
```

To download and install the verified Doom shareware IWAD:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1 -DownloadDoomShareware
```

The filename must be `DOOM1.WAD`, `DOOM.WAD`, or `FREEDOOM.WAD`. The launcher
copies it onto the sandbox driver disk, and the skill probes the WAD header
through the bounded generic `os_file_size_root_8_3` /
`os_file_read_root_8_3` imports. Core still does not parse WAD lumps.

Inside an already running OpenRhiza session:

```text
/skill-load skill_doom_port_v1
/skill-run skill_doom_port_v1
/skill-invoke skill_doom_port_v1 tick_skill
```

Expected behavior:

- the skill loads from `SKDOGEN.WAS`
- the GUI conversation surface shows the sandbox Doom runtime
- if a legal WAD is supplied, the skill logs WAD size, lump count, and directory
  offset, then renders `MAP01`/`E1M1` linedefs in first person from inside the
  sandbox
- visible QEMU launches rely on `poll_net` after `run_skill` to avoid repeated
  keyboard-injected `tick_skill` commands flickering the composer
- `tick_skill` advances and blits a forced frame
- no Doom engine, WAD parser, renderer, input mapper, audio mixer, or save logic
  is added to core

Headless validation:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\run_doom_skill.ps1 -Headless -ExitAfterStart -TimeoutSeconds 120 -TickCount 6
```

Frame-rate regression validation:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\validate_doom_frame_rate.ps1
```

Historical x86_64 headless result recorded by the earlier implementation:

```text
[Skill Runtime] skill:skill_doom_port_v1 frame_pump_hz=1000 interval_ticks=1
[doom-fps] pass min=90 steady_min=209 steady_max=227 last=210 fresh_min=113 fresh_max=126 fresh_last=113
```

This historical result is not sufficient proof of playability. The 2026-08-14
re-audit reached the original engine but recorded approximately 40 presented and
20 fresh frames per second before the existing 60 FPS warning closed the frame.
The harness had already declared startup success, so startup, simulation, visual,
input, and performance gates must be separated.

The earlier low-FPS investigation concluded framebuffer bandwidth was not the only
cause after write-combining was
enabled. The first regression was the sandbox Doom presenter being throttled.
The second was Wasm-side RGBA conversion. The current host-side regression was
single-threaded TCG starving the interpreted Wasm pump under host load. The fix
uses a generic indexed8 frame-resource ABI, keeps palette expansion/scaling in
the host framebuffer path, runs the Doom frame pump at a 1 ms cadence, uses
Doom's original low-detail/view-size controls, lets the sandbox read
frame-resource metrics and adapt view size before the 60fps floor is crossed,
and launches TCG as
`tcg,thread=multi` to keep both visible and fresh frame rates over 60.

`fps` measures how often the OS presents the sandbox frame. `fresh_fps` measures
how often the Doom sandbox creates a new engine-rendered frame sequence. The
regression gate now requires both to remain comfortably above 60 after warmup
(`90` by default), and the validation script rejects thresholds below 60. The
frame-resource runtime logs warmup separately as `[FrameResourceWarmup]`, so
`[FrameResource]` samples are post-warmup floor-gated values. It also records
post-warmup minimum `fps`/`fresh_fps`
values and emits `[FrameResource][WARN]` whenever either value drops below 60,
then requests sandbox-frame close instead of leaving a below-floor session
running. Low-frame regressions are visible even outside the validation harness.
The old requirement for more than 60 fresh Doom frames is removed from the final
acceptance contract. Vanilla Doom uses a 35 Hz simulation clock. OpenRhiza must
prove correct 35 Hz gameplay timing and responsive presentation instead of
accelerating the simulation to inflate frame counters. WHPX
acceleration is not currently a viable workaround on
this machine because the boot reaches Doom initialization much more slowly than
TCG and times out before the runnable state. Binaryen `wasm-opt` is also not
enabled by default because optimized artifacts failed before runnable Doom on
this kernel loader path.

## Data Requirement

Doom source code is open, but Doom game data is not bundled here. A full run
requires one of:

- a legally owned Doom IWAD, supplied by the user as `DOOM.WAD`
- the Doom shareware IWAD, supplied through a separately reviewed user flow
- FreeDoom or another compatible freely redistributable IWAD

The public OpenRhiza.com demo package must use a pinned, signed FreeDoom content
package. It must not redistribute commercial Doom data or download shareware data
from untrusted mirrors. A user-owned Doom IWAD is an optional local import.

## Playable Completion Gate

The authoritative completion criteria are in `DOOM_MODULE_ACCEPTANCE.md`. In
particular, engine initialization, a serial log line, static pixels, or high
presentation counters alone do not qualify as a playable result.

## Full Port Boundary

The full port needs these sandbox host ABIs:

- bounded WAD file handle from filesystem skills
- 320x200 framebuffer blit or GUI canvas object (`os_gui_blit_rgba_frame`
  exists for bounded RGBA frames)
- input event poll from sandbox input drivers
- optional audio submit path
- savegame object read/write with restore validation

`skill_doom_port_v1` now runs the doomgeneric WAD-backed
`doomgeneric_Tick`/indexed-frame path. The next quality step is not another
Doom-specific host blit. It is a generic sandbox execution upgrade:
AOT/native sandbox workers or an equivalent compiled sandbox backend, plus a
sandbox-owned GPU/compositor driver. That upgrade is required to keep larger
viewports/high-detail rendering above 60 fresh frames without moving Doom,
virtio-gpu, or any other specific driver into core.
