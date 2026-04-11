# OpenRhiza Known Issues & Constraints
# 알려진 버그, 제약사항, 해결 상태

> **코드를 수정하기 전에 이 문서를 확인하여 이미 알려진 문제를 중복 수정하거나,
> 의도된 제약을 위반하지 않도록 하세요.**

---

## 🔴 치명적 (Critical)

### KI-001: ~~uptime_ms가 실제 시간이 아님~~ → **해결됨 (KI-R06)**

### KI-002: ~~네이티브 e1000 드라이버 부재~~ → **해결됨 (KI-R08)**

---

## 🟡 중요 (High)

### KI-003: 단일 Wasm 인스턴스만 지원
- **위치:** `core/seed.rs` - `wasm_state: Option<WasmState>`
- **증상:** 새 Wasm 드라이버 주입 시 이전 드라이버가 덮어씌워짐
- **영향:** e1000 + xHCI 동시 실행 불가
- **해결:** `Vec<WasmState>` 또는 이름 기반 드라이버 레지스트리
- **상태:** ⏳ 대기

### KI-004: ATA PIO 쓰기 미구현
- **위치:** `storage.rs`
- **증상:** FAT16 디스크 읽기만 가능, 쓰기 불가
- **영향:** 성공한 Wasm 드라이버를 wasm_cache.img에 저장할 수 없음 (재부팅 시 소실)
- **해결:** `write_sector_ata_secondary()` 함수 구현
- **상태:** ⏳ 대기

### KI-005: VGA WRITER lazy_static 초기화 시점
- **위치:** `vga.rs` L15-19
- **증상:** `WRITER` static이 초기화될 때 `PHYS_MEM_OFFSET`이 아직 0이면 주소 0xB8000을 참조하여 Triple Fault
- **현재 완화:** `_print()`에서 `PHYS_MEM_OFFSET != 0` 체크 추가
- **근본 해결:** lazy_static 대신 `OnceCell` 패턴으로 명시적 초기화
- **상태:** ⚠️ 완화됨 (근본 해결 필요)

---

## 🟢 미미 (Low)

### KI-006: 레거시 core_logic 디렉토리 잔존
- **위치:** `src/arch/core_logic/`
- **증상:** `seed.rs`가 `src/core/seed.rs`로 이동되었으나, 이전 버전이 아직 존재
- **영향:** 혼란 유발 가능
- **해결:** 삭제 또는 명시적 deprecated 표기
- **상태:** ⏳ 대기

### KI-007: `unused import` 경고
- **위치:** `storage.rs` L3 (`core::cmp::min`)
- **증상:** 사용되지 않는 import 경고
- **상태:** ⏳ 대기

### KI-008: static_mut_refs 경고 (Rust 2024 호환성)
- **위치:** 다수 (`DMA_BASE`, `DMA_OFFSET`, `PHYS_MEM_OFFSET`)
- **증상:** Rust 2024 edition에서 `static mut` 직접 참조가 unsafe 블록 필요
- **해결:** `AtomicU32` / `AtomicU64` 또는 `unsafe {}` 블록으로 감싸기
- **상태:** ⏳ 대기 (현재 Rust 2021 edition이므로 경고만)

---

## ✅ 해결됨 (Resolved)

### KI-R01: VGA 출력 비활성화 (2026-04-02 해결)
- `vga.rs`의 `_print()`에서 VGA 쓰기가 주석 처리되어 시리얼로만 출력되던 문제
- **해결:** Triple Fault 방지 가드(`PHYS_MEM_OFFSET != 0`)와 함께 VGA+시리얼 이중 출력 복원

### KI-R02: 힙 메모리 부족 100KB (2026-04-02 해결)
- wasmi + smoltcp 동시 사용 시 OOM 가능
- **해결:** 100KB → 1MB로 확대

### KI-R03: host_brain.py 드라이버 트리거 데드코드 (2026-04-02 해결)
- `generate_e1000_driver()`, `generate_xhci_driver()` 함수가 정의만 되고 호출 안 됨
- **해결:** `listen_and_think()`에서 PCI 로그 파싱하여 자동 트리거

### KI-R04: DMA 풀 경계 체크 없음 (2026-04-02 해결)
- `alloc_dma_page`가 무한 할당 가능
- **해결:** `DMA_POOL_SIZE` (4MB) 상한 추가

### KI-R05: `net.rs` `queue_rx_packet()` raw pointer 위험 (2026-04-02 해결)
- 사용되지 않는 함수가 raw pointer로 잘못된 메모리 접근 패턴을 가짐
- **해결:** 함수 제거 (실제 로직은 seed.rs linker에서 안전하게 처리)

### KI-R06: uptime_ms가 실제 시간이 아님 (2026-04-02 해결)
- 루프 회전 수를 ms로 취급하여 smoltcp 타이밍이 완전히 잘못되던 문제
- **해결:** PIT Channel 0 ~1000Hz 초기화 + AtomicI64 글로벌 카운터 + `interrupts::uptime_ms()` API

### KI-R07: 키보드가 시리얼 주입에 의존 (2026-04-02 해결)
- host_brain.py에서 256바이트 키맵을 시리얼로 주입해야만 키보드가 동작하던 문제
- **해결:** `keyboard.rs` 네이티브 QWERTY 모듈 구현. 모든 표준 키 지원 (Caps Lock, Num Lock, F1-F12, 방향키, Numpad 등)

### KI-R08: 네이티브 e1000 드라이버 부재 (2026-04-02 해결)
- 네트워크가 Wasm 드라이버에만 의존하여 host_brain.py 없이는 사용 불가능하던 문제
- **해결:** `e1000.rs` 네이티브 드라이버. NIC 리셋, EEPROM MAC 읽기, RX/TX Descriptor Ring, PCI Bus Mastering, smoltcp 연동 완료

### KI-R09: e1000 DMA 버퍼 물리 주소 변환 오버플로우 (2026-04-02 해결)
- 정적 변수(.bss)의 가상 주소에서 PHYS_MEM_OFFSET를 빼면 오버플로우 발생 (bootloader가 커널을 높은 가상 주소에 매핑)
- **해결:** DMA_BASE 물리 메모리 풀에서 할당. PHYS_MEM_OFFSET+물리주소로 가상 접근. QEMU 테스트 완료.
