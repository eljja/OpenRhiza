# Architecture & Evolution

OpenRhiza follows a strict layered architecture and a phased bootstrap scenario to safely evolve from a primitive bare-metal state to an advanced AI OS.

## 5-Layer Architecture

OpenRhiza must preserve one non-negotiable rule:

- the core stays minimal
- device-specific capability lives outside the core whenever possible
- the default expansion path is sandbox component loading directed by the LLM and the capability registry
- native graduation is optional and rare, not the default development model

| Layer | Name | Description |
|-------|------|-------------|
| **Layer 4** | **Generative Space** | Prompt-driven user space. Session UIs, tools, workflows, and capability scenes are produced or adapted by the AI. |
| **Layer 3** | **AI Brain Engine** | The reasoning layer that queries the registry, plans work, generates missing capabilities, and asks the sandbox to validate them. |
| **Layer 2** | **Sandbox Capability Runtime** | Drivers, skills, workflows, display sessions, and GUI scene mutators. These should stay isolated, hot-swappable, and object-scoped by default. |
| **Layer 1** | **Bootstrap Senses (I/O & Net)** | The narrow path that keeps input, display recovery, storage bootstrap, and networking reachable long enough for sandbox capabilities to take over. |
| **Layer 0** | **OpenRhiza Seed** | The minimal immutable core (`no_std` Rust): boot, interrupts, Wasm sandbox host, rollback boundaries, and the recovery shell. |

---

## The 5 Phases of Evolution (Generic Hardware & VMware)

While targeting generic physical CPUs (x86, ARM, RISC-V), we use VMware/QEMU for rapid testing.

### Phase 1: The Seed & Basic Vision
- Boot in a standard `x86_64` environment.
- Keep a reliable recovery shell alive.
- Bring up the sandbox and exception handling early enough that capability experiments cannot take down the machine.

### Phase 2: Opening the Senses
- Probe hardware.
- Prefer raw hardware handoff into sandbox-owned drivers and parsers.
- Use the core only as the survival bridge when the sandbox path is not ready yet.

### Phase 3: Declaration of Independence
- Replace host-umbilical dependency with direct network/API access.
- Query the capability registry before generating missing work.
- Keep local fallback paths when network is down.

### Phase 4: Muscle Building
- Build richer runtime objects on top of the sandbox:
  - driver binding
  - workflow execution
  - display expansion
  - GUI scene mutation
- Prefer live activation and rollback over reboot-first promotion.
- Treat native graduation as exceptional, not routine.

### Phase 5: The Ultimate Form
- A local or remote LLM can inspect and evolve the machine from inside the machine itself.
- The OS can improve drivers, workflows, programs, and its own GUI as isolated capability objects.
- The user sees one AI-native operating environment; internally the system still preserves recovery and rollback stages.
