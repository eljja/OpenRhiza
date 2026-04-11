# OpenRhiza 프로젝트 진행 기록 (Gemini)
# 세부사항에 대해서는 Gemini_stepbystep#1.md를 참고. #1 이후 숫자가 증가된 추가파일이 있을 수 있음.
# 주요 사항에 대해서는 간결하게 Gemini.md 파일에 지속적으로 기록하고,
# 향후 다시 시작하는데 완벽하게 문제없도록, 세부 사항에 대해서는 Gemini_stepbystep#1.md 등에 지속적으로 기록.

## 프로젝트 핵심 정체성
- AI가 하드웨어 제어 코드를 스스로 생성하고, 격리된 샌드박스(Layer 0)에서 실행하며 학습하는 기반 시스템(Seed).
- 단순한 OS가 아닌, **AI 자체가 운영체제가 되는 유기적 시스템**.
- **동적 생성:** 사용자가 요구하면 앱과 인터페이스를 구매/설치하는 것이 아니라 AI가 즉석에서 생성(Generative Apps).
- **AI 경제 생태계:** 자체 발견한 문제 해결법(드라이버, 코드)을 외부 정보나 타 AI 인스턴스와 교류하며 디지털 코인/신뢰도(좋아요) 기반으로 거래하는 P2P 네트워크 지향.

## 5단계 아키텍처 구조 (Evolutionary Architecture)
- **Layer 0 (Seed & HAL):** 부트로더, 예외 처리, **격리 샌드박스(핵심)**, 초기 시리얼/텍스트 출력(Framebuffer). OS가 죽지 않게 방어하는 최소한의 생명 유지 장치.
  - *Cross-platform:* x86_64, ARM(aarch64), RISC-V 등 범용 물리 하드웨어를 지원하기 위한 하드웨어 추상화 계층(HAL) 포함.
- **Layer 1 (Senses):** AI가 스스로 생성한 초기 I/O (마우스, 키보드) 및 **LAN 통신**.
- **Layer 2 (Advanced):** 고성능 연산을 위해 생성된 GPU, NPU, 파일시스템 드라이버.
- **Layer 3 (AI Brain):** 코드를 생성하는 엔진. (초기: 외부 LLM API 의존 -> 후기: 로컬 LLM 완전 독립)
- **Layer 4 (Generative Space):** 사용자의 의도에 따라 즉석에서 렌더링되고 소멸하는 생성형 UI/UX 및 외부 P2P 네트워크.

## 개발 경계선 (Bootstrap Boundary: Non-LLM vs LLM)
- 프로젝트의 목적은 AI가 스스로 OS를 구축하는 것이지만, 최소한의 '물리 법칙'은 인간이 하드코딩해야 함.
- **Non-LLM (Layer 0 최소 구현):** 부트로더, 예외 처리기(IDT), 샌드박스(IOMMU/Ring 3), 시리얼 통신, 기본 픽셀 출력(Framebuffer) 등 시스템 생존에 직결된 기능.
- **중간 영역 (Primitives):** AI가 하드웨어를 탐색할 수 있도록 제공하는 기본 API (예: PCI 스캔 함수, 단순 메모리 할당).
- **LLM/AI 필수 영역 (Layer 1+):** 장치 드라이버(USB, LAN), 네트워크 스택(TCP/IP), 파일 시스템, 고급 메모리 페이징 등 복잡한 제어 논리 전반.

## 현재 단계
- **Level 1**: 하드웨어 환경 스캔 및 Bootimage (부트 이미지) 제작
  - `cargo bootimage`를 사용하여 베어메탈에서 동작할 바이너리 생성 완료.
  - `discovery.rs`: CPU 코어 수, 메모리 등 초기 하드웨어 정보 탐색 로직 작성 중.
  - `seed.rs`: AI 생성 코드 모의 실행 및 피드백 루프(`OpenRhizaSeed`) 뼈대 구축.
  - **[최근 완료]** QEMU 자동 실행 환경(`cargo run`) 구성 및 VGA Text Buffer 문자 출력 성공.
  - **[최근 완료]** IDT(Interrupt Descriptor Table) 기반 예외 처리 방어막 구축 (커널 패닉 방지).
  - **[최근 완료]** PIC 설정 및 키보드 인터럽트(IRQ 1) 수신, 큐(Queue) 구조 구현.
  - **[최근 완료]** Dual-Brain 통신(UART 탯줄): QEMU 시리얼 통신을 통한 파이썬 호스트 스크립트(`host_brain.py`) 연동.
  - **[최근 완료]** 동적 드라이버 적재: Gemini API를 통해 전체 키보드 배열을 추론하고 OS 런타임에 128바이트 바이너리 데이터로 주입하여 키보드 활성화 완료.
  - **[최근 완료]** AI 환각(Hallucination) 방지를 위한 RAG(매뉴얼 주입) 및 Key-Value 파싱 도입. (파이썬 스크립트 모델 선택 기능 추가)
  - **[최근 완료]** VGA 텍스트 버퍼 고도화: 2차원 좌표계(X, Y) 도입, 특수 키(Enter, Backspace, Space) 처리 및 화면 스크롤(Scroll) 기능 구현.
  - **[최근 완료]** 키보드 상태 머신(State Machine) 확장: Shift, Ctrl, Alt, E0 확장키 처리 및 256바이트(Normal/Shifted) 매핑 구조 도입.
  - **[최근 완료]** 동적 캘리브레이션(Generative Calibration): "Hi.OpenRhiza!" 문자열 타이핑 패턴을 AI가 분석하여, 사용자 키보드 배열(QWERTY, Dvorak 등)을 자동 추론 및 적용하는 시스템 완성.
  - **[최근 완료]** Layer 0 샌드박스 진화: 베어메탈 커널 내 WebAssembly(Wasm) 런타임(`wasmi`) 통합 및 동작 성공. (AI 코드를 격리 실행할 궁극의 안전망 확보)
  - **[최근 완료]** 부트로더 메모리 안정화: 스택 오버플로우로 인한 E820 메모리 맵(BIOS) 손상 문제 및 `stage_3.s`의 Identity Mapping(페이징) 누락으로 인한 트리플 폴트(Triple Fault) 현상 완벽 해결. 커널 안착 성공.
  - **[최근 완료]** 자율 드라이버 생성 및 동적 WebAssembly 인젝션: 커널 힙 메모리를 32MB로 확장하여 `wasmi` 런타임 메모리 부족 현상(커널 패닉)을 해결. UART 통신 동기화 및 드레이닝(draining) 최적화를 통해 Intel e1000 네트워크 드라이버를 AI가 실시간으로 코딩, Wasm 바이너리로 컴파일하여 베어메탈 커널 내부에 주입하고 구동하는 전체 파이프라인 완벽 달성.
## 목표 및 환경
- **타겟 플랫폼**: 범용 Bare-metal CPU (x86_64, ARM, RISC-V) / 초기 개발 및 테스트용으로 VMware 사용
- **결과물 형태**: `cargo bootimage`로 빌드된 `.bin` / `.vmdk` 디스크 이미지
- **개발 환경**: Windows 11, VS Code
- **설치된 개발 도구**: Rust (rustup, cargo), `cargo-bootimage` 크레이트, Visual Studio Build Tools