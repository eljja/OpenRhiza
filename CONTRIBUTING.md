# Contributing to OpenRhiza

First off, thank you for considering contributing to OpenRhiza! 
We are building something unprecedented: an operating system where Artificial Intelligence replaces static human code as the core driving force.

## How You Can Help
Because of the unique evolutionary nature of this project, contributions are categorized by our architectural phases:

### 1. Developing the Core Seed (Layer 0)
- **Rust Bare-metal (`no_std`):** We need robust, crash-proof code for the minimal bootloader, hardware discovery (CPUID, ACPI), and most importantly, the Exception Handler (IDT) and Sandbox.
- **Serial Communication:** Establishing reliable UART/Serial port communication for the initial "Umbilical Cord" to the Host AI.

### 2. AI Prompting & Learning Pipelines
- Developing the Host-side scripts (Python/Rust) that interact with the Core Seed via Serial, feed the hardware logs to an LLM, and parse the LLM's generated code back into machine instructions.

## Contribution Guidelines
1. **Fork the repository** and create your branch from `main`.
2. **Keep the Seed minimal:** Do not add massive libraries to Layer 0. If a feature can be learned and dynamically loaded by the AI in Phase 2 or 3, it does *not* belong in Layer 0.
3. **Test in VMware/QEMU:** Ensure any core changes build successfully via `cargo bootimage` and boot in standard x86_64 virtualization environments.
4. **Open a Pull Request:** Describe the intent behind your changes clearly.

## Code Style
- Follow standard Rust formatting guidelines (`rustfmt`).
- Use `#![no_std]` strictly for all core OS development.
- Document unsafe blocks. `unsafe` is necessary for bare-metal hardware access, but it must be clearly commented explaining *why* it is used and how memory safety is conditionally guaranteed.

We are excited to build the future of AI Operating Systems with you!