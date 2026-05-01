# OpenRhiza Public Readiness Analysis + NIC Strategy

> Historical note:
> This readiness analysis is preserved for historical reference.
> It does not override the current authoritative runtime documents.
> Current reality has changed: the OS now has multiple named Wasm modules with bounded polling, direct TLS-backed API calls, a high-resolution bootstrap GUI, and fixed-slot FAT16 writes.
> Gaps listed below are useful as historical risk framing, not as the present task list.

---

## 1. Current Completion Tier Evaluation

### 🟢 Fully Addressed & Operational

| Item | State |
|---|---|
| x86_64 Bare-metal Boot Sequence | ✅ |
| IDT / APIC / Interrupts | ✅ |
| Async Executor (Timer-driven) | ✅ |
| Wasm Sandbox + MMIO Host Fns | ✅ |
| Native xHCI USB + HID Handlers | ✅ |
| Native `e1000` → `smoltcp` bridging | ✅ |
| Ed25519 payload verifier | ✅ |
| PCI Enumeration & DMA blocks | ✅ |
| VGA CLI Bottom-bar UI | ✅ |

### 🔴 Critical PR-blocking Gaps (Stale Features vs Reality)

#### Historical Gap-1: Sole Wasm Driver Limit
- **Historical Symptom**: Only one Wasm driver operated concurrently.
- **Current Status**: Multiple named Wasm modules with bounded polling now exist, but per-module quotas, isolation metrics, and lifecycle accounting remain bootstrap-grade.
- **Action Needed**: Harden multi-capability registries and resource accounting onto `OpenRhizaSeed`.

#### Historical Gap-2: TLS 1.3 Was Offline
- **Historical Reality Check**: The repository had a full software TLS 1.3 stack, but the live fetch path did not use it.
- **Current Status**: The OpenRhiza/Gemini API path uses in-repo TLS. Production-grade certificate and hostname validation still need hardening before public release.
- **Action Needed**: Unify all service fetches through the generic TLS/API response path.

#### Historical Gap-3: No Persistent Storage Writes (ATA/NVMe)
- **Historical Reality Check**: The bootloader could read the cache to find payloads, but the kernel could not write them.
- **Current Status**: ATA sector writes, verification, cache flush, and fixed-slot FAT16 file updates exist. General dynamic filesystem writes do not.
- **Action Needed**: Move FAT32/exFAT/NTFS/ext behavior behind filesystem bridge skills and validate through image-backed harnesses.

#### Gap-4: Basic Hardcoding Leftovers
- The right-shift key is still dropping on the QEMU Windows driver path.
- Legacy placeholders (`WasmEthernetDevice`) vs actual native endpoints need cleanup.

---

## 2. Priority Dispatch 

```
Priority A (Blocks Realistic OS Operation)
├── [ ] Wasm Registry Overhaul: Support simultaneous execution of NIC + Storage Wasm instances.
├── [ ] Activate `tls.rs` to handle HTTPS traffic instead of legacy HTTP stubs.
└── [ ] Implement Disk Writing so downloaded capability packages remain persistent.

Priority B (Refinement Quality)
├── [ ] Support generic `virtio-net` out-of-the-box for cloud agnostic boots.
├── [ ] Obvious verbose NIC discovery syslogs.
└── [ ] Fix the Right-Shift Windows QEMU bug.
```
