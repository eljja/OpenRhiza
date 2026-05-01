# Security & Isolation Policy

Since OpenRhiza's core philosophy involves executing dynamically generated or registry-provided capability code on bare-metal hardware, traditional security and stability concepts take on a new meaning.

## The Layer 0 Sandbox
The most critical security feature of OpenRhiza is the **Isolated Sandbox** in Layer 0.

- When an LLM writes a hardware driver, GUI mutator, filesystem adapter, or workflow helper, it must be assumed to contain mistakes.
- The active bootstrap boundary is the Wasm sandbox plus narrow host imports, explicit object scopes, and rollback-capable runtime bindings.
- The core may keep survival fallbacks, but it must not silently absorb large capability logic.
- **Goal:** A fatal error in generated capability code must not cause a kernel panic that halts the system. The error should be bounded to the object or sandbox module, reported, and fed back into evaluation and improvement.

Future isolation layers may include ring separation and stronger hardware-backed containment, but the current engineering rule is simpler: keep host imports narrow, keep capabilities object-scoped, and never promote without validation.

## DMA and Hardware Isolation (IOMMU)
When the AI learns to program devices that use Direct Memory Access (DMA), such as Network Interface Cards (NICs) or GPUs, a single bad memory address could overwrite the core Layer 0 memory, causing a true system crash.
- To prevent this, OpenRhiza leverages **IOMMU (VT-d / AMD-Vi)** technologies.
- The Layer 0 Seed configures the IOMMU to restrict the specific hardware device's DMA access strictly to a pre-allocated "AI Trial Buffer". Any out-of-bounds DMA attempt by the device will be blocked by the hardware, generating an IOMMU Fault which is then safely routed back to the AI as learning feedback.

Current repository status: IOMMU policy is a target requirement, not a fully enforced runtime boundary.
Until then, native DMA-capable paths must stay conservative and generated drivers must be validated through sandbox and host-ABI constrained paths first.

## Autonomy Safety

Autonomous OS behavior is opt-in and bounded.

- Autonomy defaults off.
- The user controls mode and interval.
- Autonomy proposals must not directly execute machine-action JSON through the interactive prompt path.
- Council-style decisions are advisory unless the OS can prove the action is within an allowed safe boundary.
- Expensive autonomy loops need stale-cycle timeout and evidence limits before public release.

## Reporting Vulnerabilities
While the AI writes its own dynamic code, the Layer 0 Seed must remain immutable and secure.
If you find a vulnerability in the core `Seed` architecture that allows a Sandbox escape or causes a hard system crash:

1. Please do not open a public issue.
2. Send a detailed report with reproduction steps targeting the current codebase.
3. Include your suggested fixes if possible.

*Contact information will be updated as the project matures.*
