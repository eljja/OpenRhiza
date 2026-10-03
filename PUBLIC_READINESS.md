# OpenRhiza Public Readiness

This checklist records the current public-open gate for the command-first,
AI-native desktop work.

## Desktop Scope

- Boot default remains the command shell.
- `/desktop` launches `skill_desktop_compositor_v1`.
- `/gui` still launches the Workbench shell.
- `/recovery` clears GUI mutations and returns to the recovery shell.
- Desktop windows are sandbox-owned GUI objects in handle range `500..799`.

## Completed Gates

- Boot UX: command shell, `/desktop`, `/gui`, `/recovery`, `/desktop-status`.
- Desktop compositor: background, dock, and sandbox-owned windows.
- Window protocol: fixed handle layout, move/focus/close/restore selftests.
- Default apps: `Terminal + LLM`, `System Monitor`, `Skill Store`,
  `VFS Explorer`.
- Desktop actions: bounded `/desktop-action <request>` route with rejection for
  unbounded requests.

## KDE/GNOME Decision

KDE and GNOME should not be imported directly into the current bare-metal
OpenRhiza core. They assume a mature Linux userspace, process model, graphics
stack, D-Bus/session services, and filesystem conventions. The current public
path is to keep OpenRhiza's recovery core small and expose a sandbox-owned
desktop protocol first.

Future compatibility paths:

- Run selected Linux desktop components through a Linux/POSIX compatibility
  layer.
- Render remote Linux desktop sessions as sandboxed OpenRhiza windows.
- Port individual app concepts into OpenRhiza desktop skills.

## Release Smoke

The final smoke must pass:

- `cargo build`
- `build_sandbox_skills.ps1`
- `cargo bootimage`
- QEMU final desktop smoke:
  `/status`, `/desktop-status`, `/desktop`, `/desktop-apps-selftest`,
  `/desktop-action focus vfs explorer`, `/gui-scene`,
  `/desktop-action delete every file`, `/recovery`

Expected evidence:

- All four default app titles appear in `/gui-scene`.
- `VFS Explorer` becomes focused after the bounded action route.
- The unbounded desktop action is rejected.
- `/recovery` remains available after desktop work.
