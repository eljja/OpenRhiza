# Security & Isolation Policy

Since OpenRhiza's core philosophy involves executing dynamically generated, unverified machine code (written by AI) on bare-metal hardware, traditional security and stability concepts take on a new meaning.

## The Layer 0 Sandbox
The most critical security feature of OpenRhiza is the **Isolated Sandbox** in Layer 0.
- When the AI writes a hardware driver (e.g., for a GPU or Network card), it is guaranteed to make mistakes (e.g., invalid memory access, division by zero).
- The Sandbox uses strict Interrupt Descriptor Table (IDT) configuration and Ring 3 isolation (or WebAssembly/JIT sandboxing) to trap these hardware faults.
- **Goal:** A fatal error in AI-generated code must *never* cause a Kernel Panic that halts the system. Instead, the fault is caught, and a detailed crash report (address, register state) is fed back to the AI for learning.

## DMA and Hardware Isolation (IOMMU)
When the AI learns to program devices that use Direct Memory Access (DMA), such as Network Interface Cards (NICs) or GPUs, a single bad memory address could overwrite the core Layer 0 memory, causing a true system crash.
- To prevent this, OpenRhiza leverages **IOMMU (VT-d / AMD-Vi)** technologies.
- The Layer 0 Seed configures the IOMMU to restrict the specific hardware device's DMA access strictly to a pre-allocated "AI Trial Buffer". Any out-of-bounds DMA attempt by the device will be blocked by the hardware, generating an IOMMU Fault which is then safely routed back to the AI as learning feedback.

## Reporting Vulnerabilities
While the AI writes its own dynamic code, the Layer 0 Seed must remain immutable and secure.
If you find a vulnerability in the core `Seed` architecture that allows a Sandbox escape or causes a hard system crash:

1. Please do not open a public issue.
2. Send a detailed report with reproduction steps targeting the current codebase.
3. Include your suggested fixes if possible.

*Contact information will be updated as the project matures.*