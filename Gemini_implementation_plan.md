# OpenRhiza Architecture Correction: True Wasm Microkernel

> Historical note:
> This plan reflects an older checkpoint in the OpenRhiza migration process.
> Use the current architecture and OS baseline documents for the active direction.
> The current baseline has moved beyond the single-Wasm barrier with multiple named Wasm modules and bounded polling.
> Storage is still not a general filesystem stack, but fixed-slot FAT16 writes and cache flush verification now exist.

We briefly lost sight of our core philosophy! We are reverting to the foundational concept of OpenRhiza: **"The OS kernel only acts as a minimal bootstrapper meant to establish outbound networks, while all subsequent hardware operations are dynamically retrieved as Wasm modules and executed in the sandbox."**

## Proposed Changes (Partially Implemented)

### 1. Kernel Diet (Rollback) - **[DONE]**
We deleted the heavily hardcoded drivers appended inside `src/main.rs` and removed the files from the kernel source tree (`src/ps2.rs`, `src/nvme.rs`, etc.). The kernel is now restored to its bare-metal, lightweight state.

### 2. Standalone Wasm Driver Refactoring - **[DONE]**
We rewrote the expunged code into separate `.rs` files located inside `nic_drivers/` (acting as our Wasm driver repo). They have been mapped to PCI Match Keys (VID/DID) via the `driver_manifest.json`.

### 3. Pipeline Registration - **[DONE]**
We validated the drivers through our mock Python pipeline (`register_nic_drivers.py`), which successfully packaged them as Wasm payloads ready for the cloud.

## Historical Future Engineering Work From This Checkpoint

While the theory and the driver payloads are prepared, the actual **Live OS Integration** is currently blocked by several kernel limitations that we must implement next:

> [!WARNING]
> **1. Multi-Wasm Capability Barrier**
> At this checkpoint, the kernel's `OpenRhizaSeed` restricted operations to a single Wasm instance at a time. The current baseline has moved to multiple named Wasm modules with bounded polling, but still needs stronger per-module quotas and lifecycle accounting.
> 
> **2. The Fetch & Bind Routine**
> The OS needs the explicit logic to send the `pci:XXXX:XXXX` keys to Nexus, fetch the correct serialized `.was`, and map its host functions without crashing.
>
> **3. Persistent Storage**
> At this checkpoint, the OS lacked ATA write support. The current baseline has verified fixed-slot FAT16 writes, but still lacks a general writable filesystem stack.
