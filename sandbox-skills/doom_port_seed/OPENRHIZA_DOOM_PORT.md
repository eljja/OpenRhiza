# skill_doom_port_v1

`skill_doom_port_v1` is the OpenRhiza boundary object for running Doom without
moving game, renderer, filesystem, input, audio, or save policy into the core.

## Source

- Engine source: `third_party/doom`
- Upstream: `https://github.com/id-Software/DOOM`
- License: GPL family release from id Software. Treat all linked or derived
  Doom skill code as GPL-compatible.

The original commercial WAD data is not included. Use a legally owned Doom WAD
or a free IWAD such as FreeDoom when the filesystem bridge can expose WAD files
to sandbox skills.

## Current Skill Stage

The current Wasm artifact is a runnable sandbox Doom runtime. It owns player
state, a fixed-point raycast maze renderer, HUD/weapon rendering, keyboard input
polling, and an automatic `poll_net` frame loop. The OpenRhiza core only exposes
bounded GUI frame and input imports.

Exports:

- `init_driver`
- `run_skill`
- `tick_skill`
- `poll_net`

Imports:

- `os_log`
- `os_gui_blit_rgba_frame`
- `os_fetch_hid_packet_for_kind`
- `os_gui_select_session`
- `os_gui_focus_object`
- `os_gui_set_object_label`
- `os_gui_set_object_style`
- `os_gui_set_object_bounds`
- `os_gui_set_object_interaction`

## Full Engine ABI Requirements

The full Doom engine port should depend only on sandbox host objects:

- `doom_wad_open(path_handle) -> wad_handle`
- `doom_wad_read(wad_handle, offset, len, out_ptr) -> bytes`
- `doom_framebuffer_acquire(width=320, height=200, format=pal8|rgba8888) -> fb_handle`
- `doom_framebuffer_blit(fb_handle, ptr, len, palette_ptr, palette_len) -> status`
- `doom_input_poll(out_ptr, out_len) -> events`
- `doom_audio_submit(ptr, len, sample_rate) -> status`
- `doom_save_read(slot, out_ptr, out_len) -> bytes`
- `doom_save_write(slot, ptr, len) -> status`

OpenRhiza core may provide bounded handles, validation, scheduling, and rollback
only. It must not parse WAD lumps, run Doom game ticks, mix audio, or own save
policy.

## Porting Order

1. Keep this launcher skill buildable and seed-cache visible.
2. Add a bounded WAD file handle over the filesystem bridge.
3. Port the Linux Doom platform layer (`i_*`) to OpenRhiza imports.
4. Compile the engine as a sandbox artifact or sidecar Wasm component.
5. Add a 320x200 framebuffer blit ABI with palette conversion outside core.
6. Add keyboard/mouse input event translation from sandbox input drivers.
7. Add muted mode first, then bounded audio.
8. Add savegame object handles with restore/rollback validation.
9. Run with FreeDoom IWAD as the default validation target.
