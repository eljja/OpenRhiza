# OpenRhiza.com Driver Registry - Payload Manifest

이 문서는 OpenRhiza.com 클라우드 레지스트리(Nexus)에 업로드되고 관리되는 Wasm 드라이버 페이로드와 매치 ID(Match Key)를 추적합니다.
*(해당 문서는 외부 PR 및 OpenRhiza.com 드라이버/스킬 담당 AI에 의해 관리됩니다.)*

---

## Generated Wasm Driver Payloads (클라우드 등록 현황)

아래 하드웨어 타겟들은 Wasm(`.was`) 페이로드로 성공적으로 직렬화되어, OpenRhiza.com의 클라우드 매치 ID에 매핑되어 있습니다. 내부 OS가 해당 ID를 요청하면 이 페이로드들이 제공됩니다.

| 하드웨어 (Hardware) | 클라우드 매치 ID (Extracted Payload ID) |
|---|---|
| NVMe Storage | `drv_generated_pci_01_08_02` |
| AHCI SATA | `drv_generated_pci_01_06_01` |
| Virtio-Blk | `drv_generated_pci_1af4_1001` |
| Virtio-GPU | `drv_generated_pci_1af4_1050` |
| Virtio-Input | `drv_generated_pci_1af4_1052` |
| Intel HDA | `drv_generated_pci_8086_2668` |

> **Note:** 본 담당 AI는 이 드라이버 페이로드들을 OpenRhiza.com에 최적화하고 관리하는 역할만 수행합니다. OS 커널 내부에서 여러 Wasm 모듈을 동시에 실행(Multi-Wasm)하거나 다운로드하는 로직은 내부 OS 코어팀의 소관입니다.
