# OpenRhiza

**OpenRhiza** is not just an operating system; it is an organic, self-evolving environment where **Artificial Intelligence becomes the OS itself**.

Traditional operating systems are static collections of rules created by humans. OpenRhiza flips this paradigm: it starts as a minimalistic bare-metal "Seed" and uses AI to dynamically write its own hardware drivers, manage memory, and eventually generate applications on the fly based on user intent.

## Core Philosophy
- **AI as an OS:** The AI doesn't run *on* the OS; the AI *is* the OS.
- **Generative Interfaces (JIT Apps):** Users do not buy or install apps. You state your need, and the AI generates the application and interface instantly.
- **Trial & Error Learning:** The system learns to control hardware by generating code, running it in an isolated Layer 0 Sandbox, and learning from hardware faults (panics/exceptions).
- **P2P AI Economy (Nexus):** OpenRhiza instances communicate with each other to trade successful driver implementations and problem-solving logic using a digital coin or reputation (likes) system.
- **Capability Registry:** `OpenRhiza.com` is not an app store. It is a shared registry of drivers, programs, skills, workflows, policies, models, nodes, and evaluations that the OS can query and update.

## Current Status
- **Phase 1 (Bootstrap):** Building the minimal Layer 0 `Seed` using Rust (`no_std`). 
- Targeting **VMware** as the initial test environment.
- Successfully integrated a **WebAssembly (`wasmi`) runtime** directly into the bare-metal kernel to act as the ultimate isolated Sandbox for executing AI-generated drivers safely.

## Getting Started
Currently in the early bootstrapping phase. You need Rust nightly, `cargo-bootimage`, and QEMU/VMware to run the initial bare-metal image.

```bash
cargo bootimage
```

## Documentation
- VISION.md: The ultimate goal and grand vision.
- ARCHITECTURE.md: The 5-layer system and evolutionary bootstrap phases.
- SECURITY.md: Sandbox and fault isolation concepts.
- CONTRIBUTING.md: How to contribute to the future of AI OS.
- OPENRHIZA_COM_PLAN.md: The current service and integration roadmap for `openrhiza.com`.
- OPENRHIZA_API_V1.md: The first draft of the machine-oriented OS and service API contract.
- CAPABILITY_REGISTRY.md: Why OpenRhiza.com is a capability registry rather than a traditional package catalog.
- PERSISTENT_DRIVER_CACHE.md: The target local driver persistence and later-boot reuse model.
- HOT_SWAP_RUNTIME.md: The reboot-free, sandbox-first runtime activation direction.
- OS.md: The baseline operating policy injected into Gemini requests for registry-first, validate-then-activate behavior.
