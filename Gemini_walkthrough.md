# NIC/Hardware Driver Migration — State Check

> Historical note:
> This file records an earlier migration checkpoint.
> It is useful as historical context, but it is not the authoritative statement of the current runtime path.

## Pipeline Serialization Results

> [!NOTE]
> The heavy QEMU hardware drivers were successfully removed from the kernel core. They have been rewritten as decoupled Wasm implementations and processed through our Python test pipeline (`register_nic_drivers.py`). 

The pipeline successfully generated serialized `.was` payloads for the following hardware targets, assigning them their respective cloud match IDs:

| Hardware | Extracted Payload ID |
|---|---|
| NVMe Storage | `drv_generated_pci_01_08_02` |
| AHCI SATA | `drv_generated_pci_01_06_01` |
| Virtio-Blk | `drv_generated_pci_1af4_1001` |
| Virtio-GPU | `drv_generated_pci_1af4_1050` |
| Virtio-Input | `drv_generated_pci_1af4_1052` |
| Intel HDA | `drv_generated_pci_8086_2668` |

### Clarification on "Live Registration" and "OS Execution"

While the python service effectively models the ingestion of these drivers into `openrhiza.com`, the **actual operating system kernel currently cannot automatically fetch and run these payloads.**

**The Reality of the Codebase:**
1. **Fetch Limitations:** The OS's current `https.rs` and PCI enumeration loops do not yet make explicit HTTP calls for `drv_generated_pci_01_08_02`, etc.
2. **Execution Limitations:** The kernel (`OpenRhizaSeed`) presently only supports activating one Wasm sandbox container at a time. If we load a storage driver, the NIC driver unloads. True multi-device hot-swapping remains a pending architectural task.

## The Role of Software Simulation

Since the OS isn't concurrently running these drivers yet, we rely heavily on our `nic_drivers/tests/sim_common.py` mock ecosystem to prove the Wasm code *will* work when the OS is ready. 

The simulator uses a Python `bytearray` to identically mock register responses (e.g., verifying that the NVMe driver correctly writes to the SQ/CQ doorbells). At this stage, our QEMU drivers pass the theoretical simulation.
