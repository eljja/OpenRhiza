# OpenRhiza Level 1 Blueprint

This document captures the intended design of the early bootstrap layers.
It has been refreshed to acknowledge that several pieces once described as future work now exist in source.

## 1. Goal

Level 1 is about enabling the system to observe and influence the outside world without losing control of the kernel.

That means:

- safe execution boundaries
- early hardware discovery
- first input paths
- first network paths
- enough structure for AI-generated code to be tested without crashing the seed

## 2. Layer View

```text
[ AI Brain / External model / Future local model ]
        ^
        |
        v
========================================================
[ Layer 1: Senses ]
  - keyboard and pointer input
  - USB host controller access
  - initial networking
--------------------------------------------------------
[ Layer 0: Seed ]
  - boot and exception handling
  - sandbox runtime
  - serial diagnostics
  - early display
  - hardware primitives
========================================================
[ Physical hardware ]
```

## 3. Design Principles

### Sandbox first

AI-generated logic should not be treated as trusted kernel code by default.
The Wasm runtime remains the main execution boundary for exploratory logic.

### Layer 0 stays small

Layer 0 should provide:

- memory and MMIO primitives
- hardware discovery
- exception handling
- scheduling basics
- trusted verification hooks

It should not silently absorb large, unreviewed logic blobs.

### Native bootstrap is acceptable when it establishes the boundary

The original ideal was that Layer 1 drivers would be AI-generated as early as possible.
The current repository already includes native xHCI, a native `e1000` driver module, and software crypto/TLS code.
That means the practical design has shifted toward:

- human-written Layer 0 and bootstrap Layer 1 primitives
- AI-generated code still tested and promoted through the sandbox

## 4. Current Mapping to This Blueprint

### Already implemented in some form

- exception handling
- Wasm sandbox
- serial development bridge
- recovery text output and high-resolution bootstrap GUI handoff
- PCI enumeration
- APIC-based interrupt setup
- async executor
- native xHCI keyboard path
- native USB mouse path
- Ed25519 trust anchor for Nexus payloads
- native `e1000` network path into `smoltcp`
- in-repo TLS 1.3 API client path
- multiple named Wasm modules with bounded polling
- fixed-slot FAT16 cache reads and verified writes

### Implemented but still bootstrap-grade

- software crypto stack
- sandbox input driver handoff
- GUI scene object mutation imports
- autonomy mode and council prompt flow
- SMP discovery and heartbeat stubs

### Still missing or partial

- DHCP and DNS
- general writable filesystem support beyond fixed-slot FAT16 cache files
- per-module Wasm CPU/memory quota accounting
- production-grade certificate and hostname validation
- full AP startup and per-core scheduling
- stale-cycle recovery for autonomy council execution

## 5. Practical Guidance

If you work on Level 1, keep asking:

1. Is this path active in the current runtime, or only present in source?
2. Is the trust boundary explicit?
3. Does the code improve bootstrap autonomy, or just add dormant complexity?
