# Task Checklist: "Wasm Microkernel Migration"

> Historical note:
> This checklist captures an older migration phase.
> Some items remain relevant, but this file should be treated as historical planning rather than current architecture truth.

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
- `[ ]` **Phase 4: Kernel Multi-Wasm Execution Blockers (Gap-1)**
  - `[ ]` Resolve the single-instance Wasm limitation in `OpenRhizaSeed`. Currently, loading a storage driver overrides the NIC driver. A multi-driver registry is needed.
- `[ ]` **Phase 5: Live OS Integration**
  - `[ ]` Implement actual fetching logic in the kernel to dynamically request these specific `drv_generated_pci_...` payloads during PCI enumeration.
  - `[ ]` Implement disk/ATA write support so these retrieved blobs can be persisted instead of re-downloaded every boot.
