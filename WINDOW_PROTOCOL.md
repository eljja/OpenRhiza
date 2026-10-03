# OpenRhiza Window Protocol

OpenRhiza boots into the command shell first. The desktop is a sandbox skill
(`skill_desktop_compositor_v1`) that owns desktop window state and renders GUI
objects through the existing host GUI mutation ABI.

## Current Contract

- Desktop entry: `/desktop`
- Recovery entry: `/recovery`
- Status entry: `/desktop-status`
- Desktop skill id: `skill_desktop_compositor_v1`
- Desktop object handle range: `500..799`
- The host validates and displays GUI objects. The sandbox desktop skill owns
  window placement, focus, close state, and title/content labels.

## Window Handle Layout

Each window slot uses a fixed 20-handle block.

- `base = 510 + slot * 20`
- `base + 0`: window surface
- `base + 1`: title bar
- `base + 2`: close button
- `base + 3`: primary content line
- `base + 4`: secondary content line

Reserved handles:

- `500`: desktop background
- `790`: dock surface
- `791`: dock label

## Window Operations

The desktop skill exposes these low-level operations internally:

- `compositor_handle_click(x, y) -> u32`
- `agent_cmd_move_window(win_id, x, y) -> u32`
- `agent_cmd_close_window(win_id) -> u32`

The generic CLI `/skill-invoke` path currently supports zero-argument exports,
so QEMU-verifiable protocol checks use zero-argument selftest exports:

- `desktop_window_protocol_selftest() -> i32`
- `desktop_window_close_selftest() -> i32`
- `desktop_window_restore_selftest() -> i32`
- `desktop_default_apps_selftest() -> i32`

CLI wrappers:

- `/desktop-window-selftest`
- `/desktop-window-close`
- `/desktop-window-restore`
- `/desktop-apps-selftest`

## Safety Rules

- The command shell remains the boot default.
- `/recovery` closes the sandbox frame, clears GUI mutations, and returns to the
  recovery shell.
- Desktop objects stay inside `500..799`; dynamic shell objects stay inside the
  existing `120..159` range.
- A closed window slot is cleared by writing empty labels and zero bounds to its
  handles, so stale scene nodes do not survive close/restore tests.

## Next ABI Step

Milestone 3 keeps the public test contract stable without extending the generic
Wasm invocation ABI. A later action layer can add typed argument dispatch for
bounded object-scoped commands once the desktop action router is ready.
