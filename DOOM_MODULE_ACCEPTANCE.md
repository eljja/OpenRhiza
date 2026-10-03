# Doom Downloadable Module Acceptance

Status: required OpenRhiza public-demo gate proposed on 2026-08-14

## 1. Purpose

Doom/FreeDoom is the reference proof that OpenRhiza can acquire, verify, install,
run, supervise, update, and remove a complex third-party application entirely as
sandbox modules. It is not allowed to rely on Doom-specific kernel code.

The public demo consists of two independently signed packages:

- `org.openrhiza.doom.engine`: doomgeneric-derived GPL-2.0 engine module and
  corresponding source offer/build provenance;
- `org.freedoom.phase1.content`: pinned FreeDoom IWAD content module with license,
  credits, digest, and provenance.

Commercial Doom WADs are never bundled by OpenRhiza. A user may import a legally
owned WAD into a private content object after local digest and format validation.

## 2. Allowed capabilities

The engine receives only:

- one application surface and focused keyboard/pointer input;
- monotonic game clock and bounded timers;
- read-only access to the selected content object;
- private read/write save and configuration directory;
- one bounded audio playback stream;
- structured logging, lifecycle, and cancellation.

It receives no raw device, MMIO, DMA, global input, arbitrary filesystem, package
activation, registry administration, microphone, model, or unrestricted network
capability. Installation networking belongs to the package resolver, not the game.

## 3. User journeys

Both paths must work:

```text
/app install org.openrhiza.doom.demo
/app launch org.openrhiza.doom.engine
```

```text
"FreeDoom을 설치해서 실행해줘"
  -> LLM typed package.resolve/install/launch plan
  -> package and capability confirmation
  -> deterministic package services execute the plan
```

After installation the game launches offline from the desktop, app launcher, or
direct command. Uninstall removes package state but preserves saves only when the
user explicitly requests that behavior.

## 4. Functional acceptance

- The engine reaches the title/menu and starts the first FreeDoom level.
- Keyboard controls move, turn, strafe, fire, use, pause, open the menu, and exit.
- Relative mouse movement turns the player; pointer capture and release are visible
  and recover correctly when focus changes.
- Game simulation advances at Doom's 35 Hz timebase independent of compositor
  refresh and host/QEMU speed.
- The compositor may present at 60 Hz, but duplicated presentation is not counted as
  a new engine frame and the simulation is never accelerated to satisfy an FPS gate.
- Sound effects play through a generic audio stream. Music may be a separately
  declared post-alpha feature, but silent sound effects fail the public-demo gate.
- Save, quit, relaunch, load, reboot, and load again restore the same verified state.
- Minimize, restore, workspace switch, and application focus changes preserve the
  game. The default policy pauses or throttles simulation when fully backgrounded.
- Closing the window terminates or suspends the module according to explicit user
  policy and releases surface, input, audio, file, and memory resources.

## 5. Automated QEMU evidence

The gate uses a pinned FreeDoom fixture and deterministic QEMU machine. It records:

1. Registry query, manifest/digest/signature verification, download, install, and
   activation evidence.
2. A title/menu screenshot and pixel hash regions proving nonblank game output.
3. Synthetic key and relative-mouse events with before/after screenshots.
4. Game-state telemetry proving player position/angle, ammo, and level time changed
   as expected. Test telemetry is a declared debug interface unavailable to normal
   packages.
5. Simulation samples proving 35 Hz within a documented tolerance over at least 60
   seconds and no dependence on compositor presentation counters.
6. Focus loss, minimize, restore, pointer release, pause/throttle, and resumed input.
7. Save/load persistence across process restart and full guest reboot.
8. Deterministic audio-buffer hashes for known sound events plus no underrun storm.
9. Module memory, CPU, handles, input queue, frame queue, and audio queue high-water
   marks with no unbounded growth.
10. Forced Wasm trap, timeout, malformed WAD, missing content, and revoked signature;
    the desktop and recovery console remain available in every case.
11. Offline relaunch after successful installation.
12. Uninstall and reinstall without stale executable or capability state.

The test fails on frame-resource warnings, dropped input that changes gameplay,
incorrect game time, silent required effects, no-op saves, focus leakage, module
trap propagation, unsigned content, or a serial-only success assertion.

## 6. Performance target

For the declared x86_64 QEMU profile:

- 35 Hz simulation is sustained without catch-up spirals;
- at least 35 fresh engine frames per second during ordinary gameplay;
- 60 Hz compositor presentation where the QEMU display backend permits it;
- focused input-to-visible-response p95 is below 100 ms;
- audio remains continuous within the declared buffer budget;
- an eight-hour gameplay/demo loop has bounded memory and queue growth.

If interpreted Wasm cannot meet the target, the optimization order is shared-memory
frame/audio buffers, batching, Wasm AOT or JIT in an isolated worker, then a native
user-mode sandbox package. Doom code still does not move into Rhiza Core.

## 7. Distribution and license gate

- The Doom-derived engine package carries GPL-2.0 notices, corresponding source,
  build scripts, modifications, and reproducible provenance.
- The FreeDoom content package carries its copyright, BSD-style license, credits,
  source/content version, and exact digest.
- Engine and content remain separate packages and their licenses are shown before
  installation and in package details.
- OpenRhiza.com does not host commercial Doom IWADs or fetch shareware IWADs from
  arbitrary mirrors.
- A user-owned WAD import is local, private by default, and never uploaded for module
  generation, evaluation, deduplication, or telemetry.

## 8. Current baseline and gaps

The 2026-08-14 local QEMU run proves:

- `SKDOGEN.WAS` loads as `skill_doom_port_v1`;
- the doomgeneric engine initializes and opens `DOOM1.WAD`;
- the 320x200 indexed framebuffer path submits frames;
- the existing launcher reaches its runnable-state check.

It does not yet prove this acceptance gate. The same run recorded roughly 40
presented and 20 fresh frames per second followed by a floor warning and close
request. Input-driven state evidence is absent, game timing is coupled to the pump,
mouse and audio are absent, and save/config writes are currently no-ops.
