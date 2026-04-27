# OpenRhiza Capability Registry

OpenRhiza.com is not an app store.

It is a capability registry for the OpenRhiza OS and its nodes.

The registry should hold:

- drivers
- programs
- skills
- workflows
- policies
- llm endpoints
- nodes
- evaluations
- comments
- votes
- artifacts

The registry should also preserve the object model of those capabilities where relevant:

- GUI scene capabilities should be modeled as object graphs or object mutations
- drivers should expose bounded activation targets
- workflows should expose explicit steps and rollback boundaries
- skills should describe narrow abilities rather than global behavior patches
- semantic index builders and graph refreshers should be explicit capability objects, not invisible background magic

## Why This Model Exists

OpenRhiza is prompt-first.

The user should not manually manage:

- driver installation
- program selection
- skill discovery
- workflow sequencing
- policy interpretation

The OS should inspect local state, query the registry, reuse known-good work, generate missing parts, validate them, activate them, and then report results back.

The registry is not only storage.
It is part of the planner context for the OS.

That means OpenRhiza should feed recent skill, workflow, and policy results back into the LLM planning surface before it decides what to do next.

## Meaning Of Each Category

### Driver

Hardware-facing execution components.

Examples:

- `e1000` network driver
- xHCI input driver
- storage controller driver

### Program

User-facing tools and applications.

Examples:

- terminal utilities
- diagnostic consoles
- generated text-first apps

### Skill

LLM-facing unit abilities.

Examples:

- web search
- registry lookup
- python sandbox test
- driver smoke test

Skills are not primarily for the user to run directly.
They are building blocks the OS can invoke while solving a user request.
Display expansion and GUI session behavior should also be modeled as skills and workflows before they are modeled as new core kernel features.

### Workflow

Reusable multi-step execution plans.

Examples:

- driver acquire -> validate -> activate -> persist
- program fetch -> run -> evaluate
- skill load -> execute -> report

### Policy

Operational rules that govern how OpenRhiza should behave.

Examples:

- registry-first lookup
- hot-swap before reboot
- read-only storage before write promotion
- rollback before unsafe persistence

### LLM

Remote or local model endpoints used by the OS.

Examples:

- Gemini direct access
- OpenRhiza-hosted remote models

### Node

A participating OpenRhiza machine with a trust tier, hardware profile, and current status.

### Evaluation

Observed results after trying a capability artifact.

Examples:

- driver stability score
- program usefulness note
- workflow success/failure report

## Expected OS Behavior

When the user asks for something, OpenRhiza should prefer:

1. local validated state
2. registry lookup
3. generation
4. sandbox validation
5. live activation
6. persistence
7. evaluation upload

This applies across drivers, skills, programs, and workflows.
It also applies to display, compositor, and GUI evolution: registry skill first, sandbox validation second, core promotion last.
It should later also apply to filesystem semantic graph layers: sidecar graph object first, bounded refresh second, raw full scan only when necessary.

The same principle applies to interface work:

1. inspect the current GUI scene
2. query for existing GUI skills or workflows
3. reuse a known-good scene mutation first
4. generate only the missing mutation or skill
5. validate object-local effects without risking recovery display
6. promote only after success

## UI And API Direction

The web UI should expose board-style views for all capability classes.

The machine API should let the OS query and update:

- drivers
- skills
- workflows
- policies
- software/programs
- models
- evaluations

This registry is a shared operational memory for an AI-native OS, not a traditional package catalog.
