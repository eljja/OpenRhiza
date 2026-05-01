# Contributing to OpenRhiza

We are building an AI-native operating system with one hard architectural rule:

- keep only the minimum survival path in the core
- push everything else toward sandboxed skills, workflows, drivers, programs, and object-capabilities

## How You Can Help
Because of the evolutionary nature of this project, contributions are best understood by boundary:

### 1. Developing the Core Seed
- **Rust Bare-metal (`no_std`):** We need robust, crash-resistant code for boot, recovery input, recovery display, interrupts, storage bootstrap, networking bootstrap, and the Wasm sandbox boundary.
- **Minimalism:** Do not let convenience logic, GUI policy, or heavy device-specific behavior creep into the core.

### 2. Sandbox Capability Work
- Build and refine drivers, skills, workflows, GUI scene mutators, and other runtime capabilities as isolated sandbox components first.
- Prefer object-scoped mutation and rollback boundaries over global shared-state hacks.

## Contribution Guidelines
1. **Fork the repository** and create your branch from `main`.
2. **Keep the Seed minimal:** If a feature can be expressed as a sandbox capability with a stable host ABI, it does *not* belong in the core.
3. **Test in VMware/QEMU:** Ensure any core changes build successfully via `cargo bootimage` and boot in standard `x86_64` virtualization environments.
4. **Update docs when architecture changes:** Especially `README.md`, `OS.md`, `ROADMAP.md`, `Codex.md`, `DISPLAY_ABI.md`, `GUI_DEVELOPMENT.md`, `MODULE_MAP.md`, and `KNOWN_ISSUES.md`.
5. **Open a Pull Request:** Describe the intent behind your changes clearly.

## Code Style
- Follow standard Rust formatting guidelines (`rustfmt`).
- Use `#![no_std]` strictly for all core OS development.
- Document unsafe blocks. `unsafe` is necessary for bare-metal hardware access, but it must be clearly commented explaining *why* it is used and how memory safety is conditionally guaranteed.

The project should become more usable by making the core smaller, the sandbox boundaries clearer, and the user-facing prompt path more capable.
