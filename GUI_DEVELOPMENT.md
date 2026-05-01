# OpenRhiza GUI Development Plan

This document defines how OpenRhiza should evolve from the current bootstrap GUI into a usable AI-native desktop without violating the core OpenRhiza rules.

## Non-negotiable rules

- Keep only the minimum survival path in the core.
- Build everything else as sandboxed skills, workflows, drivers, and object-capabilities whenever possible.
- Treat every GUI item as an isolated object with identity, bounds, state, events, and rollback boundaries.
- A broken GUI object must not silently corrupt unrelated GUI objects.
- Pointer routing, focus, selection, scrolling, text input, and redraw should be resolved per object.
- Native bootstrap code may exist temporarily, but it must remain a small handoff path and not become the long-term GUI engine.

## Current state

OpenRhiza already has:

- a recovery text shell
- a bootstrap 1920x1080 GUI surface
- sandbox display skills that request `wide-console` and `gui-session`
- a first retained object runtime for sidebar, conversation area, and composer hit-testing
- object-scoped GUI mutation imports that sandbox skills and LLM machine actions can use
- Korean-capable GUI text rendering path
- UTF-8-safe context extraction for autonomy and GUI conversation summaries
- reduced pointer-motion redraw churn after object-local redraw fixes

OpenRhiza does not yet have:

- a full object scene graph shared between skills and the renderer
- per-object scroll handling
- per-object text editing and selection
- stable object-local invalidation
- a rich session/chat model
- a production-quality GUI toolkit path
- a fully stable compositor-seed stage after the bootstrap GUI handoff
- a fully self-hosted font import and atlas validation workflow inside the guest runtime
- long-running conversation history and scroll persistence that are fully regression-tested

## Two-track strategy

OpenRhiza should pursue two GUI tracks in parallel.

### Track A: Native OpenRhiza Object GUI

This is the primary path and the one most aligned with the OS philosophy.

Goals:

1. Define a stable object contract for GUI scenes.
2. Build a retained object runtime that supports:
   - object registration
   - hit-testing
   - hover/focus/active state
   - object-local redraw
   - object references instead of global mutation
3. Support a Codex-like shell:
   - left session list
   - main conversation surface
   - dynamic composer
   - assistant messages using the full content width
   - user prompts rendered as indented objects
4. Make the GUI itself editable by the OpenRhiza AI through scene or mutation requests.

Why this path matters:

- It keeps OpenRhiza in control of its own runtime model.
- It matches the object-isolation rule.
- It gives the AI a clean way to inspect and mutate the UI as data.
- It is much lighter than embedding a web stack.

Typography rule:

- font parsing and atlas generation should not expand the core
- the core should consume validated atlas assets
- importing TTF, OTF, TTC, OTC, WOFF, and WOFF2 should happen through a host-side or sandbox-owned font skill/workflow

### Track B: LVGL-style Bridge

This is the parallel reference path.

Goals:

1. Map OpenRhiza GUI objects onto an LVGL-like widget vocabulary.
2. Learn from LVGL-style retained widgets:
   - container
   - list
   - button
   - label
   - textarea
3. Keep LVGL-like behavior behind a scene adapter, not inside the core.
4. Use the same OpenRhiza scene contract so native and LVGL-style renderers can coexist.

Why this path matters:

- LVGL provides a practical retained widget model suitable for embedded systems.
- It can guide styling, widget behavior, and scene decomposition.
- It may later become a sandbox renderer or compatibility layer.

Why it must stay secondary:

- OpenRhiza should not become dependent on a heavy external toolkit in the core.
- Any LVGL-style path must remain replaceable and sandbox-owned.

### Track C: Sandbox-Owned Modern Shell Skill

OpenRhiza must not carry a permanent toolkit-specific GUI session in the core.

The modern shell path is now represented as `skill_gui_modern_shell_v1`, a sandbox skill that mutates existing GUI objects through the `os_gui_*` host ABI.

This preserves the core boundary:

- the core owns only the recovery presenter, display handoff, object handles, validation, and rollback gates
- the skill owns layout intent, labels, focus hints, object style selection, and session-specific behavior
- the skill can be replaced, rolled back, or regenerated without adding GUI policy to the kernel
- future egui-like, LVGL-like, or custom OpenRhiza GUI renderers must enter as skills or renderer capabilities, not as core branches

The rule is strict: if a GUI feature can run as a skill, it must not be implemented as hardcoded core UI behavior.

## Not recommended right now: full web GUI

A browser-style HTML/CSS/JS stack is too heavy for the current OpenRhiza stage.

Problems:

- parser complexity
- layout engine complexity
- scripting runtime
- much larger memory and rendering footprint
- more difficult rollback and object isolation

Long-term possibility:

- Use web-like declarative layout ideas
- But keep the runtime object model native and sandbox-owned

## Scene contract

The common contract should be the same for both tracks.

Each GUI object should carry:

- stable object handle
- parent handle
- object kind
- bounds
- style class
- interaction state
- object-scoped label or content
- optional object reference to a capability or session

Each scene should carry:

- scene id
- backend preference
- object list
- object policy

Each mutation should be object-scoped:

- move object
- resize object
- change style
- change interaction state
- change content

## Required object types for the next milestone

- root
- sidebar
- session list
- session item
- conversation surface
- message
- composer
- text input
- footer
- label
- scroll area

## Event model

The next stable event model should support:

- pointer move
- pointer down
- pointer up
- scroll
- focus
- blur
- text input
- activate

## Short-term milestones

### Milestone 1

- object registry exists
- sidebar/session items are hit-tested
- composer is focusable
- conversation area is an object

### Milestone 2

- scene contract shared in code
- native renderer consumes object scene data
- per-object redraw replaces larger panel redraw
- user prompt and assistant reply are both object-based messages

### Milestone 3

- LVGL-style mapping layer uses the same scene contract
- widget mapping exists for session list, chat surface, composer, and footer
- renderer choice can be treated as a sandbox capability

### Milestone 4

- OpenRhiza AI can inspect the current scene
- OpenRhiza AI can request object mutations from the console
- GUI improvements can be developed from inside OpenRhiza itself

## Self-hosted GUI development from the OpenRhiza console

Yes, the OS should eventually be able to develop its own GUI from inside the OpenRhiza console.

That requires:

1. stable object model
2. stable event model
3. stable sandbox skill execution
4. scene inspection
5. scene mutation requests
6. object-local rollback

Target workflow:

1. user requests GUI change from the OpenRhiza console
2. AI inspects current scene and current object graph
3. AI queries OpenRhiza.com for relevant GUI skills, workflows, and policies
4. AI generates or updates a sandbox GUI skill if needed
5. sandbox skill proposes scene/object mutations
6. OS validates the result without risking the recovery shell
7. successful mutation becomes the active GUI scene

## Current implementation anchors

- core display handoff rules: [DISPLAY_ABI.md](D:/python/github/OpenRhiza/OpenRhiza/DISPLAY_ABI.md)
- OS baseline rules: [OS.md](D:/python/github/OpenRhiza/OpenRhiza/OS.md)
- font import workflow: [FONT_SKILL.md](D:/python/github/OpenRhiza/OpenRhiza/FONT_SKILL.md)
- scene/object contract code: [src/gui_contract.rs](D:/python/github/OpenRhiza/OpenRhiza/src/gui_contract.rs)
- LVGL-style scene bridge scaffold: [src/gui_lvgl_bridge.rs](D:/python/github/OpenRhiza/OpenRhiza/src/gui_lvgl_bridge.rs)
- current bootstrap renderer and object hit-test path: [src/display.rs](D:/python/github/OpenRhiza/OpenRhiza/src/display.rs)

## Current self-hosted inspection and mutation path

OpenRhiza can now inspect and mutate the bootstrap GUI scene from inside the OS itself.

- `/gui-scene`
  - print the current scene id, backend preference, node list, and LVGL-style widget translation count
- `/gui-mutations`
  - print active object-local scene overrides
- `/gui-session <openrhiza|sandbox|wide|recovery>`
  - switch the selected session object
- `/gui-focus <conversation|composer|none>`
  - move focus between key GUI objects
- `/gui-label <handle> <text>`
  - override an object label through an object-scoped mutation
- `/gui-style <handle> <style>`
  - override an object style class through an object-scoped mutation
- `/gui-reset <handle|all>`
  - clear object mutations without touching unrelated objects

There is also a local bootstrap skill for self-hosted GUI mutation testing:

- `skill_gui_scene_mutator_v1`
  - a sandbox skill that mutates conversation/composer/footer objects through the GUI host imports
  - intended to validate that object bounds, interaction, style, and label changes can be owned by a skill instead of by the core
- `skill_gui_modern_shell_v1`
  - a sandbox-owned modern shell seed
  - applies the current polished GUI layout through object-scoped mutations
  - deliberately avoids toolkit-specific branches in `src/display.rs`

This is still a bootstrap path, but it is the correct direction: GUI changes are becoming object-scoped scene mutations rather than global layout edits.

The next step is that these same operations must be available to sandbox skills through host imports, not only to local CLI commands. A GUI skill should be able to:

- select the active session object
- move focus to conversation or composer
- update object labels
- update object styles
- clear only its own object mutations or reset the whole scene layer when re-bootstrap is required
- mutate object bounds and interaction state without requiring global scene replacement

That keeps GUI ownership in sandbox code while the core stays limited to object validation, redraw scheduling, and rollback.

OpenRhiza also now exposes a matching Gemini machine-action surface so a prompt can eventually propose object-scoped GUI changes:

- `{"action":"gui_select_session","session":"openrhiza|sandbox|wide|recovery"}`
- `{"action":"gui_focus","target":"conversation|composer|none"}`
- `{"action":"gui_set_label","handle":40,"text":"..."}`
- `{"action":"gui_set_style","handle":30,"style":"composer|conversation|sidebar-active|plain|accent"}`
- `{"action":"gui_set_bounds","handle":30,"x":304,"y":916,"width":1592,"height":78}`
- `{"action":"gui_set_interaction","handle":30,"interaction":"idle|hovered|focused|active|disabled"}`
- `{"action":"gui_reset","handle":0}`

This action path must remain object-scoped. It should never become a back door for uncontrolled global layout mutation.

## Parallel work plan

OpenRhiza should keep both tracks alive at the same time:

### Native object track

- make the current bootstrap GUI consume a richer shared scene contract
- move ad-hoc layout logic toward scene/object-driven rendering
- add per-object focus, scroll, selection, text-editing, and invalidation
- make assistant and user messages first-class message objects

### LVGL-style bridge track

- keep LVGL-style behavior outside the core
- expand the bridge so the same scene can map onto LVGL-like widgets
- compare the native renderer and LVGL-style renderer from the same scene data
- use the bridge as a sandbox capability, not as a kernel dependency

## Immediate next steps

1. Regression test pointer, keyboard, Korean input, Gemini responses, and scroll after the latest runtime changes.
2. Stabilize conversation history retention and scroll persistence during long sessions.
3. Expand the LVGL-style bridge to cover the current bootstrap scene with the same object contract.
4. Move more GUI mutation ownership into sandbox skills instead of core helpers.
5. Let OpenRhiza console prompts and sandbox skills mutate the GUI scene through the same object-scoped path.
6. Replace temporary external edits with self-hosted GUI improvement loops from inside OpenRhiza.
