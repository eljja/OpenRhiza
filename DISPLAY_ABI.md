# OpenRhiza Display ABI v1

OpenRhiza display expansion must follow the same rule as drivers, skills, and workflows:

- The core keeps only the minimum display survival path.
- Console expansion, framebuffer transition, compositor startup, and GUI policy live outside the core.
- New display behavior must be delivered as sandboxed `skill` / `workflow` components through OpenRhiza.com.
- GUI surfaces, panels, input targets, and interactive elements should be represented as isolated objects with explicit bounds, state, and request paths.
- A broken or replaced display object must not implicitly corrupt unrelated display objects.

## Core responsibilities

The core may keep only these display responsibilities:

1. A guaranteed recovery display path.
2. A minimal text shell that remains usable when every sandbox component fails.
3. Safe host imports / ABI for sandbox display requests.
4. Safe rollback to the last stable recovery display path.
5. A small runtime state block that records:
   - current active display mode
   - requested next mode
   - GUI session phase
   - object-local display mutation state needed for validation and rollback

The core must not become the long-term home of:

- high-level compositor logic
- device-specific accelerated rendering stacks
- policy about when to switch to GUI
- rich text layout
- window manager behavior
- non-essential display drivers

## Sandbox-side responsibilities

Sandbox display skills and workflows are expected to handle:

- mode negotiation requests
- framebuffer text-console expansion
- GUI bootstrap orchestration
- compositor session startup
- validation and rollback policy before promotion
- object-oriented GUI composition, hit-testing, focus, and per-object redraw behavior

## Current host imports

The current display handoff imports exposed to sandbox Wasm are:

- `os_request_display_mode(backend, text_cols, text_rows, pixel_width, pixel_height)`
- `os_set_gui_session_state(state)`
- `os_set_display_session_target(target)`
- `os_set_display_validation_state(state)`
- `os_gui_select_session(session)`
- `os_gui_focus_object(target)`
- `os_gui_set_object_label(handle, ptr, len)`
- `os_gui_set_object_style(handle, style)`
- `os_gui_set_object_bounds(handle, x, y, width, height)`
- `os_gui_set_object_interaction(handle, interaction)`
- `os_gui_reset_object_mutations(handle)`

These imports are intentionally narrow. They let a sandbox component request a transition without forcing the core to embed the full implementation.

The GUI mutation imports exist so sandbox skills can treat the GUI as an object graph instead of mutating global display state. The core only forwards the request, validates object handles and enum codes, and triggers object-local redraw.

The core should not interpret these imports as permission to own GUI policy. It should only provide the narrowest possible validation and handoff layer.

## State model

### Display backend

- `0` = `vga-text`
- `1` = `framebuffer-text`
- `2` = `gui`

### GUI session phase

- `0` = `text-shell`
- `1` = `bootstrap-requested`
- `2` = `sandbox-session`

### Display session target

- `0` = `recovery-shell`
- `1` = `wide-console`
- `2` = `gui-session`

### Display validation state

- `0` = `none`
- `1` = `requested`
- `2` = `testing`
- `3` = `ready`
- `4` = `promoted`

### GUI session selector

- `1` = `openrhiza`
- `2` = `sandbox`
- `3` = `wide-console`
- `4` = `recovery-shell`

### GUI focus target

- `0` = `none`
- `1` = `conversation`
- `2` = `composer`

### GUI style selector

- `0` = `chrome`
- `1` = `sidebar`
- `2` = `sidebar-idle`
- `3` = `sidebar-hover`
- `4` = `sidebar-active`
- `5` = `conversation`
- `6` = `assistant`
- `7` = `user`
- `8` = `composer`
- `9` = `footer`
- `10` = `plain`
- `11` = `accent`

### GUI interaction selector

- `0` = `idle`
- `1` = `hovered`
- `2` = `focused`
- `3` = `active`
- `4` = `disabled`

## Required runtime behavior

When a sandbox component requests a wider console or GUI transition:

1. Record the request in the display runtime state.
2. Keep the current recovery console alive until a validated handoff path exists.
3. Validate the new path in sandbox first.
4. Promote only after success criteria are met.
5. Roll back immediately if input, display, or recovery visibility is lost.

## Architecture note

If a temporary reference implementation is added in the core to unblock bring-up, it must be treated as a bootstrap-only path and moved back behind the display ABI as soon as possible.

## Object discipline

- A display object should have a stable identity, explicit rectangle or surface bounds, and an isolated update path.
- Pointer routing should resolve against objects, not against implicit screen-global side effects.
- Focus, hover, selection, and activation should be tracked per object.
- Rendering should prefer object-local invalidation over full-scene mutation whenever possible.
