# Task Checklist: "Wasm Microkernel Migration"

> Historical note:
> This checklist captures an older migration phase.
> Some items remain relevant, but this file should be treated as historical planning rather than current architecture truth.
> The current baseline has multiple named Wasm modules and verified fixed-slot FAT16 writes; the remaining work is hardening quotas, lifecycle accounting, and general filesystem bridge support.

- `[x]` **Phase 1: Rollback Kernel Modifications**
  - `[x]` Remove hardcoded QEMU driver modules from `src/main.rs`
  - `[x]` Delete original driver files from `src/` (`nvme.rs`, `ahci.rs`, etc.)
- `[x]` **Phase 2: Create Standalone Wasm Drivers**
  - `[x]` Write `nic_drivers/nvme.rs` (Standalone Wasm version)
  - `[x]` Write `nic_drivers/ahci.rs` (Standalone Wasm version)
  - `[x]` Write `nic_drivers/storage_virtio.rs` (Standalone Wasm version)
  - `[x]` Write `nic_drivers/virtio_gpu.rs` (Standalone Wasm version)
  - `[x]` Write `nic_drivers/virtio_input.rs` (Standalone Wasm version)
  - `[x]` Write `nic_drivers/intel_hda.rs` (Standalone Wasm version)
  - `[x]` Write `nic_drivers/ps2.rs` (Standalone Wasm version)
- `[x]` **Phase 3: Simulation and Upload Pipeline**
  - `[x]` Update `driver_manifest.json` with new driver metadata
  - `[x]` Validate compilation and serialize payloads using `register_nic_drivers.py`

## Next Steps / Pending Work
- `[~]` **Phase 4: Kernel Multi-Wasm Execution Blockers (Gap-1)**
  - `[x]` Add multiple named Wasm module slots with bounded polling.
  - `[ ]` Add stronger per-module quotas, fault isolation metrics, and lifecycle accounting.
- `[~]` **Phase 5: Live OS Integration**
  - `[ ]` Unify generated driver fetch/download through structured OpenRhiza API calls.
  - `[x]` Add verified fixed-slot FAT16 writes for local cache artifacts.
  - `[ ]` Add general filesystem bridge support for dynamic file creation and richer filesystems.
