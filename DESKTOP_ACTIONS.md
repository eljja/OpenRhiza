# OpenRhiza Desktop Actions

Milestone 5 adds a bounded route for LLM-native desktop actions. The route is
intentionally narrow: natural-language-like text is accepted only when it maps
to a known desktop object and a known operation.

## CLI Contract

- `/desktop-action focus terminal`
- `/desktop-action focus system monitor`
- `/desktop-action focus skill store`
- `/desktop-action focus vfs explorer`
- `/desktop-action restore apps`
- `/desktop-action move monitor`
- `/desktop-action-selftest`

Unknown or broad requests are rejected instead of being passed into arbitrary
Wasm calls.

## Sandbox Exports

The desktop compositor exposes zero-argument exports for current validation:

- `desktop_action_focus_terminal`
- `desktop_action_focus_system_monitor`
- `desktop_action_focus_skill_store`
- `desktop_action_focus_vfs_explorer`
- `desktop_action_route_selftest`

This keeps the action path compatible with the current `/skill-invoke` ABI while
still giving the LLM a stable object-scoped command surface.

## Safety

- Actions target known windows only.
- No raw coordinates, handles, or arbitrary export names are accepted from the
  natural-language route.
- Recovery remains `/recovery`, which clears GUI mutations and returns control
  to the command shell.
