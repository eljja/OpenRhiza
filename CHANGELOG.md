# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog,
and this project adheres to Semantic Versioning.

## [Unreleased]
### Added
### Added
- Object-scoped bootstrap GUI runtime with sidebar, conversation surface, composer, footer, and per-object focus/hover/selection tracking.
- Shared GUI scene contract plus LVGL-style bridge scaffold so native and LVGL-like renderers can target the same scene model.
- Sandbox GUI mutation imports:
  - `os_gui_select_session`
  - `os_gui_focus_object`
  - `os_gui_set_object_label`
  - `os_gui_set_object_style`
  - `os_gui_set_object_bounds`
  - `os_gui_set_object_interaction`
  - `os_gui_reset_object_mutations`
- `1920x1080` display-session bootstrap through sandbox display skills and workflows.
- Boot autorun support that can load and run local seed skills after reaching `input>`.
- Local GUI inspection and mutation console commands such as `/gui-scene`, `/gui-focus`, `/gui-session`, `/gui-label`, `/gui-style`, `/gui-bounds`, `/gui-interaction`, and `/gui-reset`.
- `skill_gui_scene_mutator_v1` bootstrap skill for self-hosted GUI mutation testing.
- Noto Sans KR based GUI font pipeline and asset generation for a more modern bootstrap UI.
- Python/PowerShell serial log viewer improvements and direct log-based debugging support.

### Changed
- Reframed OpenRhiza around explicit minimal-core and sandbox-first rules across the main documentation set.
- Moved GUI ownership further toward sandbox skills and object-scoped mutation instead of core-global layout edits.
- Switched seed skill loading to fixed slot files (`SK000.WAS` through `SK005.WAS`) for more reliable QEMU FAT driver-disk boot flows.
- Tightened display redraw behavior so GUI hover and pointer changes no longer force large layout redraws.
- Simplified GUI status rendering to reduce redraw-triggered flicker.
- Refined the bootstrap pointer shape and size.

### Fixed
- Fixed a GUI/input deadlock caused by re-entering the VGA writer lock during GUI pointer interaction updates.
- Fixed boot autorun stalls caused by FAT root lookup returning early and by padded Wasm slot files being passed directly to the Wasm parser.
- Fixed multiple GUI flicker paths by separating layout changes from interaction-state changes and reducing redraw scope.
- Fixed GUI bootstrap regressions where seed skills were looked up by unstable direct filenames instead of stable slot files.
- Restored stable GUI input after removing the pointer-motion deadlock path.
