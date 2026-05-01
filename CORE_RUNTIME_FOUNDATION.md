# OpenRhiza Core Runtime Foundation

This document records the current minimum core substrate for the OpenRhiza runtime after the
May 2026 stabilization pass.

## Purpose

The core must stay small.

It exists to provide:

- boot and recovery
- minimal display/input survival path
- sandbox execution boundary
- bounded storage/network host ABIs
- scheduler/runtime substrate
- rollback and validation gates

It must **not** absorb higher-level filesystem logic, GUI logic, compatibility runtimes, or
policy engines that can live in skills/workflows.

The current module-by-module boundary tags are maintained in
[`CORE_BOUNDARY_AUDIT.md`](CORE_BOUNDARY_AUDIT.md). Treat that file as the working checklist for
shrinking bootstrap fallbacks.

## Current Foundation

### 1. Multi-capability runtime

- `src/core/seed.rs`
- Named Wasm modules can coexist in `wasm_modules: Vec<LoadedWasmModule>`.
- Polling has been shifted toward bounded round-robin multiplexing so capability count can grow
  without forcing every module to be polled every tick.
- Runtime health is exposed through `/wasm-status`.
- The status snapshot records each loaded module key, byte size, poll count, trap count, last
  active tick, and last error.
- This is accounting only. Capability policy, replacement decisions, and higher-level recovery
  remain skill/workflow responsibilities.

### 2. Scheduler substrate

- `src/task/executor.rs`
- Single-runner cooperative executor remains the active model.
- Added:
  - bounded batch budget
  - wake-drop accounting
  - dropped-wake rescan recovery
  - queue-depth metrics
  - completed/poll counters

This is not a final SMP scheduler. It is a safer bootstrap floor.

### 3. SMP substrate

- `src/smp.rs`
- Tracks:
  - discovered logical cores
  - boot-core APIC id
  - runtime heartbeats
  - whether AP bring-up is still stubbed

This is groundwork only. OpenRhiza does not yet run a real multi-core executor.

### 4. Persistent write floor

- `src/storage.rs`
- FAT16 bootstrap writes now use:
  - bounded preallocated targets
  - sector write-back verification
  - cache flush after data and directory updates

This improves trust in bootstrap persistence without expanding the core into a general filesystem.

### 5. Active TLS transport

- `src/https.rs`
- `ApiClient` and Gemini/OpenRhiza HTTPS calls use the in-repo TLS stack from `src/tls.rs`.
- HTTP headers are preserved in `ApiResponse` so signature-bearing or policy-bearing flows can be
  unified later.

Nexus payload fetch still uses a dedicated client path and remains a follow-up cleanup target.

## What still must stay outside the core

The following belong in skills/workflows/compat layers unless there is an unavoidable survival
reason:

- FAT32/exFAT/NTFS/ext filesystem logic
- semantic graph indexing logic
- GUI scene mutation policy
- program compatibility runtimes
- autonomy planner behavior
- desktop/session feature logic

## Immediate Next Steps

1. Follow the migration order in `CORE_BOUNDARY_AUDIT.md`: e1000, xHCI/HID, GUI shell, filesystem bridge, autonomy workflow.
2. QEMU regression test the latest autonomy, scheduler, UTF-8, and bounded polling changes.
3. Move filesystem family parsing onto storage host ABI driven skills.
4. Expand SMP from heartbeat tracking to actual AP bring-up.
5. Add stronger per-capability quotas for CPU time, memory pages, and host ABI call rates.
6. Unify Nexus fetch on the same HTTPS/TLS response path as service APIs.
7. Keep the recovery console and bootstrap GUI independent so sandbox failures cannot kill the
   survival path.
