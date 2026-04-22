# OpenRhiza Public Readiness Analysis + NIC Strategy

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

#### Gap-1: Sole Wasm Driver Limit
- **Symptom**: Only one Wasm driver operates concurrently.
- **Impact**: Breaking the core vision declaration of "Native AI controlling diverse hardware simultaneously." Our newly separated Storage and Video Wasm drivers cannot be run alongside the NIC.
- **Action Needed**: Introduce multi-capability driver registries onto `OpenRhizaSeed`.

#### Gap-2: TLS 1.3 is Offline
- **Reality Check**: We have a full software TLS 1.3 stack in `src/tls.rs` alongside software cryptography (`src/crypto/*`). However, the live `src/https.rs` Nexus fetch path is not yet utilizing it.
- **Action Needed**: Replace or wrap `https.rs` with the `tls.rs` client before public launch.

#### Gap-3: No Persistent Storage Writes (ATA/NVMe)
- **Reality Check**: The bootloader can read the cache to find payloads, but we do not have an ATA or NVMe write path implemented in the kernel.
- **Impact**: AI-generated responses and downloaded Wasm blobs disappear upon reboot.
- **Action Needed**: Establish a write function to persist fetched payloads locally.

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
