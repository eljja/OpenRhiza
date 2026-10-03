# OpenRhiza Default Desktop Apps

Milestone 4 makes the desktop launch as a useful multi-window workspace instead
of a single demo surface. The default app windows are owned by
`skill_desktop_compositor_v1` and rendered through the desktop window protocol.

## Default Windows

- `Terminal + LLM`: command shell and natural-language prompt entry point.
- `System Monitor`: runtime health, sandbox skill status, display/input state.
- `Skill Store`: local cache, registry lookup, install/promotion candidates.
- `VFS Explorer`: boot files, driver cache, documents, and bounded file actions.

## CLI

- `/desktop`: loads and runs the default desktop.
- `/desktop-status`: reports the desktop skill, handle range, and app list.
- `/desktop-apps-selftest`: re-renders the four-app layout for QEMU validation.
- `/recovery`: clears GUI mutations and returns to the command-first shell.

## Validation

The QEMU smoke for this milestone must show all four app titles in
`/gui-scene` and must still return cleanly through `/recovery`.
