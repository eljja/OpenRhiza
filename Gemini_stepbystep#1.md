# OpenRhiza 상세 진행 기록 - Step 1

## 0. 궁극적 비전 및 아키텍처 방향성 (The Grand Vision)
- **AI as an OS:** AI는 단순한 에이전트가 아니라 OS 자체여야 함.
- **자가 진화 및 생성형 인터페이스:** 사용자가 프로그램을 구입하는 대신, AI가 사용자의 의도를 파악하여 즉석에서 인터페이스와 앱을 코딩하고 제공(JIT App Generation).
- **생태계 및 거래 (Nexus):** AI OS들이 서로 통신(Channel)하며 Trial & Error로 얻은 문제 해결법(예: 특정 디바이스 드라이버)을 교환. 이때 디지털 코인이나 "좋아요" 기반의 가치 거래 시스템 구축.
- **부트스트랩 아키텍처 (Layered Architecture):**
  - `Layer 0 (Seed)`: 기존 PC/VM 부팅 가능. 최소한의 생명 유지 및 샌드박스. 잘못된 코드에도 OS가 죽지 않도록 방어하는 Exception Handling 필수. 초기 외부 통신용 Serial Port 포함.
  - `Layer 1 (Senses)`: 사용자와의 1차 인터페이스(USB 컨트롤러 및 PS/2) 및 외부 세상과의 연결(LAN). AI가 스스로 PCI를 스캔하여 활성화함.
  - `Layer 2 (Advanced Drivers)`: Local LLM 구동을 위해 필수적인 고성능 하드웨어(GPU, NPU) 및 스토리지 제어 계층.
  - `Layer 3 (AI Brain)`: 두뇌 역할. LAN 확보 전엔 호스트 의존 -> LAN 확보 후 외부 LLM API 사용 -> GPU 확보 후 Local LLM으로 완전 독립.
  - `Layer 4 (Generative & Nexus)`: 실시간 생성형 사용자 인터페이스 및 AI 상호간 코드/가치 거래 네트워크.

- **진화 시나리오 (범용 하드웨어 타겟, 테스트용 VMware):**
  1. **Phase 1:** 구형 CPU 환경 부팅 -> Layer 0 동작 (Sandbox 활성화) -> 시리얼 포트를 통한 외부 LLM(호스트) 피드백 시작.
  2. **Phase 2:** 외부 LLM이 PCI 버스를 스캔하여 **USB 컨트롤러(마우스/키보드)**와 **LAN (e.g., e1000)** 활성화 성공.
  3. **Phase 3:** LAN을 통해 직접 외부 LLM API와 연결. 시리얼 포트 독립. (필요시 초경량 CPU 전용 LLM 다운로드 기반 마련)
  4. **Phase 4:** 외부 API 지식 기반으로 GPU/NPU 드라이버 자가 구축. (CPU 내장형 NPU와 외장형 PCIe NPU 토폴로지 구분 대응)
  5. **Phase 5:** 로컬 환경에 LLM 적재(GPU/NPU 또는 CPU). 완전 자립형 AI OS 완성.

## 1. 현재 프로젝트 아키텍처 및 현황 분석
- **핵심 구조**: 
  - 운영체제 부팅 직후의 하드웨어와 맞닿은 상태(Bare-metal)에서 AI가 직접 하드웨어를 제어하도록 설계됨.
  - `OpenRhizaSeed` 엔진을 통해 AI의 명령(`&str`)을 `unsafe` 샌드박스에서 실행.
  - 실행 중 발생하는 하드웨어 예외/성공 결과를 `ExecutionResult`로 반환받아 AI가 다시 학습하는 피드백 루프 존재.

- **작업 파일 현황**:
  - `src/arch/x86_64/discovery.rs` (또는 `src/arch/discovery.rs`): 부팅 직후 시스템 스펙 파악 로직. (CPU, 메모리 스캔 기능 구현 필요)
  - `src/core/seed.rs` (또는 `src/arch/core_logic/seed.rs`): AI 상호작용 및 로그 피드백을 위한 `OpenRhizaSeed` 구현. (동적 할당 없는 고정 버퍼 설계 확인 완료)

## 2. 왜 `cargo bootimage`인가?
이 프로젝트는 일반 응용 프로그램이 아니므로, OS의 간섭 없이 순수 하드웨어에서 동작해야 합니다. 따라서 `cargo bootimage`를 사용해 부트로더와 Rust 커널 코드를 하나로 묶은 `.bin` 형식의 디스크 이미지를 만들고 있습니다.

## 3. 최근 개발 성과 (Achievements)
- **VGA 터미널 구현:** 2D 좌표(X,Y) 기반 텍스트 출력, 특수키(Enter, Backspace), 화면 스크롤.
- **Dual-Brain 탯줄 통신:** QEMU의 시리얼(UART) 포트를 TCP 소켓으로 연결하여 파이썬(`host_brain.py`)과 실시간 통신 환경 구축.
- **상태 머신 및 256바이트 매핑:** Shift, Ctrl, Alt, 확장키(E0) 등을 기억하는 State Machine 커널 적용.
- **동적 캘리브레이션 (Generative Calibration):** OS 화면에 `Hi.OpenRhiza!`를 입력하도록 유도하고, 파이썬이 스캔코드 시퀀스를 모아 Gemini LLM에 전달. AI가 QWERTY/Dvorak 등 사용자 환경을 완벽히 유추하여 256바이트 바이너리 드라이버를 OS 런타임에 주입(Injection)하는 아키텍처 완성.
- **Wasm 샌드박스 통합 (Ultimate Sandbox):** `alloc` 힙 메모리 할당자를 기반으로, 베어메탈 커널 내부에 `wasmi` WebAssembly 런타임을 통합. AI가 작성한 코드를 커널 패닉의 위험 없이 완전히 격리된 환경에서 실행(JIT 대체)할 수 있는 엄청난 아키텍처적 기반 마련.
- **부트로더 메모리 패닉 & 트리플 폴트(Triple Fault) 완전 해결:** 
  - 과거 발생하던 프레임 할당자(Frame Allocator) 패닉은 Bootloader의 스택(0x7C00에서 아래로 자람)이 `0x5000`에 위치한 E820 메모리 맵(BIOS) 버퍼 영역을 침범하여 발생한 데이터 오염이 원인이었음.
  - 이를 해결하기 위해 링커 스크립트(`linker.ld`) 상의 페이지 테이블 위치를 부트로더 코드 뒤쪽(`0x16000`)으로 안전하게 밀어내어 스택 공간을 약 27KB로 넓힘.
  - 그러나 변경된 레이아웃으로 인해 부트로더 어셈블리(`stage_3.s`) 내부에 하드코딩 되어 있던 페이징(Identity Mapping) 루프의 시작-끝 범위 조건(`__page_table_start` < `__bootloader_end`)이 깨져버리면서, 정작 부트로더 자기 자신의 코드가 물리 메모리에 맵핑되지 않은 상태로 보호 모드에 진입, 그 즉시 Page Fault -> Triple Fault가 연쇄 발생하여 무한 QEMU 멈춤 현상 발생. 
  - 페이징 루프를 1번 페이지(`0x1000`)부터 확장된 `__page_table_end`까지 포괄적으로 맵핑하도록 어셈블리를 재작성함으로써 길었던 부트로더 커널 진입 불가 이슈를 완벽 정복함.

## 4. 현재 직면한 과제 (Next Actions)
다음에 바로 시작할 수 있도록 남겨두는 TODO 리스트입니다.

1. **Wasm Host Functions 구현 (MMIO 브릿지)**
   - 샌드박스 안의 Wasm 코드가 외부(물리 하드웨어)와 소통할 수 있도록, OS 커널이 제공하는 `read_mmio`, `write_mmio` 같은 호스트 함수를 Wasm 엔진에 연결(Link)해야 함.
2. **e1000 랜카드 드라이버 (Wasm 버전) 주입 및 실행**
   - 파이썬 AI가 작성한 e1000 초기화(Reset) 로직을 Wasm 바이너리로 컴파일한 후, 시리얼 포트를 통해 OS로 전송.
   - OS는 이 Wasm을 샌드박스에서 실행하여, 실제로 물리적 랜카드가 리셋되는지 확인!

> **Note**: 본 파일은 작업 내용이 길어지면 `#2`, `#3` 파일로 분리하여 순차적으로 저장할 예정입니다.