# OpenRhiza Module Map
# 모든 소스 파일의 역할, 인터페이스, 의존관계를 정의합니다.
# **코드를 수정하기 전에 반드시 이 문서를 확인하세요.**

> 최종 갱신: 2026-04-02 (Phase 1 Quick Fixes 반영)

---

## 디렉토리 구조

```
OpenRhiza/
├── .cargo/config.toml          # 빌드 타겟 & cargo run 설정
├── Cargo.toml                  # 프로젝트 의존성 (no_std 전용)
├── host_brain.py               # [개발/테스트 전용] 외부 AI 뇌 (파이썬)
├── rhiza_drivers/              # FAT16 가상 디스크용 드라이버 캐시 디렉토리
│   └── e1000.bin               # 테스트용 더미 파일
├── wasm_cache.img              # 1MB FAT16 디스크 이미지 (Wasm 캐시 저장용)
├── src/
│   ├── main.rs                 # 커널 엔트리 포인트 & 메인 루프
│   ├── allocator.rs            # 힙 메모리 할당자 (1MB)
│   ├── vga.rs                  # VGA 텍스트 모드 화면 출력
│   ├── net.rs                  # smoltcp 네트워크 스택 & Wasm 브릿지
│   ├── keyboard.rs             # 완전한 QWERTY 키보드 네이티브 구현 (PS/2 Scancode Set 1)
│   ├── storage.rs              # ATA PIO 디스크 읽기 & FAT16 파서
│   ├── core/
│   │   └── seed.rs             # OpenRhizaSeed - Wasm 샌드박스 엔진
│   └── arch/
│       ├── mod.rs              # 아키텍처 모듈 트리 (레거시, 현재 사용 안 함)
│       ├── core_logic/         # 레거시 seed (현재 사용 안 함, src/core/seed.rs로 이동됨)
│       └── x86_64/
│           ├── discovery.rs    # PCI 스캔, CPUID, 메모리 맵, DMA 풀
│           ├── interrupts.rs   # IDT, PIC, 키보드/타이머 인터럽트 핸들러
│           ├── port.rs         # I/O 포트 읽기/쓰기 래퍼
│           ├── serial.rs       # COM1 UART 시리얼 통신
│           └── linker.ld       # 링커 스크립트
└── 문서들 (HARNESS.md, MODULE_MAP.md, etc.)
```

---

## 모듈 상세

### `main.rs` — 커널 엔트리 포인트

**역할:** 부팅 시퀀스 제어, 메인 이벤트 루프

**부팅 시퀀스:**
```
1. IDT 초기화 (인터럽트 방어막)
2. PIC 초기화 (하드웨어 인터럽트 활성화)
3. PHYS_MEM_OFFSET 저장
4. 힙 메모리 초기화
5. 하드웨어 스캔 (CPUID + PCI Enumeration)
6. FAT16 Wasm 캐시 검사 (Secondary IDE)
7. OpenRhizaSeed 인스턴스 생성
8. smoltcp 네트워크 스택 초기화
9. 메인 루프 진입
```

**메인 루프 구조:**
```
loop {
    rhiza.poll_wasm_network()    // Wasm NIC 드라이버 poll
    net::poll(uptime_ms)         // smoltcp TCP/IP 폴링
    
    if serial_data_received {    // 시리얼 데이터 수신 처리
        // 0xFD: 키맵 초기화
        // 0xFE: 캘리브레이션 실패
        // 0~255: 키맵 바이트 적재
        // 0xFB: 드라이버 생성 중 알림
        // 0xFA: 드라이버 생성 실패
        // 0xFC: Wasm 바이너리 수신 시작
        // → 4바이트 크기 → N바이트 Wasm 바이너리
    }
    
    if keyboard_scancode {       // 키보드 입력 처리
        // E0 확장키, Shift/Ctrl/Alt 상태 머신
        // dynamic_keymap으로 문자 변환 → VGA 출력
    }
    
    hlt()  // CPU 대기
}
```

**의존하는 모듈:** `allocator`, `vga`, `net`, `storage`, `arch::x86_64::*`, `core::seed`

**외부에 노출하는 인터페이스:** 없음 (엔트리 포인트)

---

### `allocator.rs` — 힙 메모리 할당자

**역할:** `linked_list_allocator`를 사용한 글로벌 힙 메모리 제공

**핵심 상수:**
| 이름 | 값 | 설명 |
|------|-----|------|
| `HEAP_SIZE` | `1,048,576` (1 MiB) | 정적 배열 기반 힙 크기 |

**외부에 노출하는 인터페이스:**
```rust
pub fn init_heap()  // main.rs에서 부팅 시 1회 호출
```

**주의:** 정적 배열 `HEAP_MEM`으로 힙을 제공하므로, 크기 증가 시 커널 바이너리 크기도 증가.

---

### `vga.rs` — VGA 텍스트 모드 출력

**역할:** 0xB8000 VGA 텍스트 버퍼를 통한 80x25 화면 출력

**핵심 타입:**
- `VgaWriter` — 커서 위치 추적, 글자 출력, 스크롤, 백스페이스
- `Buffer` — 80x25 ScreenChar 배열

**외부에 노출하는 인터페이스:**
```rust
pub static ref WRITER: Mutex<VgaWriter>     // 전역 VGA Writer
pub fn _print(args: fmt::Arguments)          // 시리얼+VGA 이중 출력
// 매크로: print!(), println!()
```

**주의:**
- `_print()`는 `PHYS_MEM_OFFSET != 0`일 때만 VGA에 쓴다 (Triple Fault 방지).
- WRITER의 lazy_static 초기화 시점에서 `PHYS_MEM_OFFSET`이 아직 0이면 잘못된 주소 참조 발생 가능.

---

### `keyboard.rs` — 완전한 QWERTY 키보드 네이티브 구현

**역할:** PS/2 Scancode Set 1 기반 완전한 QWERTY 키보드 처리. 시리얼 주입 없이 부팅 즉시 동작.

**지원하는 키:**
- 영문 a-z / A-Z (Caps Lock + Shift XOR)
- 숫자 0-9 및 Shifted 기호 (!@#$%^&*())
- 모든 기호 키 (`~, []{}, \|, ;:, '", ,<.>/?)
- Enter, Backspace, Tab, Escape, Space
- 방향키 (↑↓←→), Home, End, Page Up/Down, Insert, Delete
- F1~F12 기능키
- Numpad (Num Lock 토글로 숫자/네비게이션 전환)
- Modifier: Shift, Ctrl, Alt, Caps Lock, Num Lock
- E0 확장 키 (Right Ctrl, Right Alt, Keypad Enter, Keypad /)

**핵심 타입:**
- `KeyEvent` — Char(u8), Enter, Backspace, Tab, Escape, Arrow*, Home, End, PageUp/Down, Insert, Delete, FunctionKey(u8), ModifierOnly
- `KeyboardState` — shift/ctrl/alt/caps_lock/num_lock/is_extended 상태 추적

**외부에 노출하는 인터페이스:**
```rust
pub enum KeyEvent { Char(u8), Enter, Backspace, Tab, Escape, ArrowUp, ... }
pub struct KeyboardState { shift_pressed, ctrl_pressed, alt_pressed, caps_lock, num_lock, ... }
impl KeyboardState {
    pub const fn new() -> Self
    pub fn process_scancode(&mut self, scancode: u8) -> Option<KeyEvent>
}
```

---

### `net.rs` — 네트워크 스택

**역할:** smoltcp TCP/IP 스택과 Wasm NIC 드라이버 사이의 브릿지

**핵심 구조:**
```
Wasm NIC Driver  →  RX_QUEUE  →  smoltcp (수신)
smoltcp          →  TX_QUEUE  →  Wasm NIC Driver (송신)
```

**외부에 노출하는 인터페이스:**
```rust
pub static ref RX_QUEUE: Mutex<Vec<Vec<u8>>>  // Wasm → smoltcp 수신 큐
pub static ref TX_QUEUE: Mutex<Vec<Vec<u8>>>  // smoltcp → Wasm 송신 큐
pub fn init_network()                          // 네트워크 스택 초기화 (IP: 10.0.2.15/24)
pub fn poll(timestamp_ms: i64)                 // smoltcp 주기적 폴링
```

**현재 상태:** 
- ICMP 소켓만 등록됨
- TCP 소켓 미등록 (향후 HTTP/HTTPS용 필요)
- MAC 주소: `52:54:00:12:34:56` (QEMU 기본)
- Gateway: `10.0.2.2` (QEMU 사용자 모드 네트워킹)

---

### `storage.rs` — 디스크 I/O

**역할:** Secondary IDE(0x170) ATA PIO 모드 디스크 읽기 + FAT16 파서

**외부에 노출하는 인터페이스:**
```rust
pub fn read_sector_ata_secondary(lba: u32, buffer: &mut [u8; 512])  // 512바이트 섹터 읽기
pub fn extract_payload() -> Option<[u8; 1024]>                       // FAT16에서 E1000.BIN 추출
```

**한계:**
- 읽기 전용 (쓰기 미구현)
- 최대 1024바이트(2섹터)만 읽음
- FAT16 체인 추적 미구현 (첫 번째 클러스터만)

---

### `core/seed.rs` — OpenRhizaSeed (Wasm 샌드박스 엔진)

**역할:** AI 코드 실행 엔진. Wasm 바이너리를 격리 실행하고 결과를 반환.

**핵심 타입:**
- `OpenRhizaSeed` — Wasm 엔진 + 로그 버퍼 + 시스템 ID 보유
- `WasmState` — 활성 Wasm 인스턴스 (Engine + Store + Instance)
- `ExecutionResult` — Success(String) | Panic(String)

**Wasm Host Functions (AI 코드가 호출 가능한 OS API):**

| 이름 | 시그니처 | 설명 |
|------|----------|------|
| `read_mmio` | `(addr: u32) -> u32` | 물리 주소에서 32비트 MMIO 읽기 |
| `write_mmio` | `(addr: u32, val: u32)` | 물리 주소에 32비트 MMIO 쓰기 |
| `alloc_dma_page` | `() -> u32` | 4KB DMA 물리 페이지 할당 (경계 체크 포함) |
| `os_rx_packet` | `(ptr: u32, len: u32)` | Wasm 메모리에서 패킷을 읽어 OS RX_QUEUE로 전달 |
| `os_fetch_tx_packet` | `(ptr: u32, max_len: u32) -> u32` | OS TX_QUEUE에서 패킷을 꺼내 Wasm 메모리에 쓰기 |

**외부에 노출하는 인터페이스:**
```rust
pub fn new(identity: SystemIdentity) -> Self
pub fn execute_wasm_sandbox(&mut self, wasm_bytes: &[u8]) -> ExecutionResult
pub fn poll_wasm_network(&mut self)         // Wasm의 poll_net() 호출
pub fn poll_hardware_event(&self) -> Option<u8>  // 키보드 큐에서 스캔코드 꺼내기
pub fn poll_host_data(&self) -> Option<u8>       // 시리얼에서 데이터 읽기
```

**한계:**
- `wasm_state: Option<WasmState>` — 단일 Wasm 인스턴스만 보유 가능

---

### `arch/x86_64/discovery.rs` — 하드웨어 탐색

**역할:** CPUID, 부트 메모리 맵, PCI 버스 스캔

**전역 변수:**
| 이름 | 타입 | 설명 |
|------|------|------|
| `DMA_BASE` | `u32` | DMA 물리 메모리 시작 주소 (4MB 이상 영역에서 자동 선택) |
| `DMA_OFFSET` | `u32` | 다음 할당할 DMA 오프셋 |
| `DMA_POOL_SIZE` | `u32` | DMA 풀 상한 (4 MiB) |
| `PHYS_MEM_OFFSET` | `u64` | 부트로더가 제공한 물리→가상 메모리 오프셋 |

**PCI 스캔 결과:**
```rust
pub struct PciDevice { bus, device, vendor_id, device_id, bar0 }
```

**xHCI 감지:** class=0x0C, subclass=0x03, prog_if=0x30 → 시리얼로 BAR0 출력

---

### `arch/x86_64/interrupts.rs` — 인터럽트 시스템

**역할:** IDT 설정, CPU 예외 핸들러, 하드웨어 인터럽트 핸들러

**등록된 핸들러:**
| 벡터 | 이름 | 동작 |
|------|------|------|
| #3 | Breakpoint | 시리얼 로그 출력 |
| #14 | Page Fault | 시리얼 에러 출력 + 무한루프 (TODO: 복구 가능하게) |
| 32 (IRQ0) | Timer | EOI만 전송 (현재 미사용) |
| 33 (IRQ1) | Keyboard | 스캔코드 → KEYBOARD_QUEUE push |

**외부에 노출하는 인터페이스:**
```rust
pub fn init_idt()
pub static PICS: Mutex<ChainedPics>
pub static KEYBOARD_QUEUE: Mutex<ScancodeQueue>  // 256바이트 Ring Buffer
```

---

### `arch/x86_64/serial.rs` — 시리얼 통신 (개발/테스트 전용)

**역할:** COM1 (0x3F8) UART 16550 시리얼 포트 드라이버

> ⚠️ 시리얼 통신은 QEMU 테스트 환경 전용입니다. 프로덕션에서는 키보드+모니터+LAN이 핵심 I/O입니다.

**외부에 노출하는 인터페이스:**
```rust
pub static ref SERIAL1: Mutex<SerialPort>
pub fn _print(args: Arguments)      // 포맷된 문자열 시리얼 출력
pub fn send_byte(data: u8)          // 원시 바이트 전송
pub fn poll_receive() -> Option<u8> // 비동기 바이트 수신
// 매크로: serial_print!(), serial_println!()
```

---

### `arch/x86_64/port.rs` — I/O 포트 래퍼

**역할:** x86 I/O 포트 접근을 안전하게 래핑

**외부에 노출하는 인터페이스:**
```rust
pub fn read_port_u8(port_addr: u16) -> u8
pub fn write_port_u8(port_addr: u16, value: u8)
pub fn read_port_u16(port_addr: u16) -> u16
pub fn write_port_u16(port_addr: u16, value: u16)
```

---

### `host_brain.py` — 외부 AI 뇌 (개발/테스트 전용)

**역할:** QEMU 테스트 환경에서 시리얼 포트를 통해 드라이버를 생성/주입하는 파이썬 스크립트. 프로덕션에서는 OS가 LAN을 통해 직접 LLM API에 접속하므로 이 스크립트는 불필요.

**핵심 함수:**
| 함수 | 설명 |
|------|------|
| `connect_to_umbilical_cord()` | TCP 127.0.0.1:4444로 QEMU 시리얼 연결 |
| `listen_and_think(s)` | OS 로그 수신, PCI 디바이스 감지, 자동 드라이버 생성 트리거 |
| `generate_and_inject_driver(s, name, prompt)` | LLM으로 Rust 코드 생성 → Wasm 컴파일 → 시리얼 주입 (Self-Healing 포함) |
| `compile_rust_to_wasm(rust_code)` | `cargo +nightly build --target wasm32-unknown-unknown` |
| `generate_e1000_driver(s, bar0)` | e1000 NIC init 프롬프트 생성 |
| `generate_xhci_driver(s, bar0)` | xHCI USB 리셋 프롬프트 생성 |

**자동 감지 트리거:**
- `"Vendor 0x8086, Device 0x100E"` → `generate_e1000_driver()`
- `"xHCI BAR:"` → `generate_xhci_driver()`

**LLM 폴백 체인:** Primary → gemini-2.5-flash-lite → 기타 모델 (각 3회 재시도)

---

## 빌드 의존성 (Cargo.toml)

| 크레이트 | 버전 | 용도 | no_std |
|----------|------|------|--------|
| `bootloader` | 0.9.34 | 부트로더 + 물리 메모리 매핑 | ✅ |
| `x86_64` | 0.14.2 | IDT, I/O 포트, CPU 명령어 | ✅ |
| `lazy_static` | 1.4.0 | 정적 변수 초기화 (spin lock) | ✅ |
| `pic8259` | 0.10.1 | PIC 인터럽트 컨트롤러 | ✅ |
| `spin` | 0.9.8 | Mutex (spinlock 기반) | ✅ |
| `uart_16550` | 0.2.0 | 시리얼 포트 UART 드라이버 | ✅ |
| `linked_list_allocator` | 0.10.5 | 힙 메모리 할당자 | ✅ |
| `wasmi` | 0.31.0 | WebAssembly 인터프리터 | ✅ |
| `smoltcp` | 0.11 | TCP/IP 네트워크 스택 | ✅ |
