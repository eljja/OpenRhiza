# OpenRhiza Display ABI v1

OpenRhiza display expansion must follow the same rule as drivers, skills, and workflows:

- The core keeps only the minimum display survival path.
- Console expansion, framebuffer transition, compositor startup, and GUI policy live outside the core.
- New display behavior must be delivered as sandboxed `skill` / `workflow` components through OpenRhiza.com.

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

## Current host imports

The current display handoff imports exposed to sandbox Wasm are:

- `os_request_display_mode(backend, text_cols, text_rows, pixel_width, pixel_height)`
- `os_set_gui_session_state(state)`
- `os_set_display_session_target(target)`
- `os_set_display_validation_state(state)`

These imports are intentionally narrow. They let a sandbox component request a transition without forcing the core to embed the full implementation.

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

## Required runtime behavior

When a sandbox component requests a wider console or GUI transition:

1. Record the request in the display runtime state.
2. Keep the current recovery console alive until a validated handoff path exists.
3. Validate the new path in sandbox first.
4. Promote only after success criteria are met.
5. Roll back immediately if input, display, or recovery visibility is lost.

## Architecture note

If a temporary reference implementation is added in the core to unblock bring-up, it must be treated as a bootstrap-only path and moved back behind the display ABI as soon as possible.
