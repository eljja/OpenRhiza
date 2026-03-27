# OpenRhiza: Level 1 Development Blueprint

본 문서는 OpenRhiza의 초기 단계인 **Level 1 (Phase 1 & 2: Seed & Senses)** 구현을 위한 마스터 지침서입니다. 개발자와(향후 코드를 작성할) AI 엔진이 OS의 기반을 다질 때 반드시 준수해야 할 목적, 구조, 해결 방법을 정의합니다.

---

## 1. 목적 (Objective)
Level 1의 궁극적인 목표는 **"AI가 하드웨어를 스스로 탐구하고, 실수해도 죽지 않으며, 외부 세상(사용자/네트워크)과 최초로 연결되는 것"**입니다.
- 인간 개발자는 복잡한 USB 드라이버나 LAN 드라이버를 하드코딩하지 않습니다.
- 개발자는 단지 AI가 하드웨어(PCI 버스, I/O 포트)를 건드릴 수 있는 **'원시적인 손(Primitives)'**과, 잘못 건드렸을 때 시스템 패닉을 막아주는 **'안전망(Sandbox)'**만을 제공합니다.

---

## 2. 하이라키 및 구조 (Hierarchy & Structure)

```text
[ AI Brain Engine (외부 LLM API -> 향후 로컬 탑재) ]
    ▲ (명령 하달: "xHCI 레지스터를 읽어라")
    │
    ▼ (결과/예외 반환: "Page Fault 발생: 주소 0xDEADBEEF")
========================================================================
[ Layer 1: Senses (AI가 동적으로 샌드박스에 적재/실행하는 영역) ]
  ├─ Human Input : USB Host Controller (xHCI/eHCI) 제어 및 PS/2 Fallback
  └─ Networking  : PCI Bus 스캔 -> NIC (e.g., e1000, Realtek) 패킷 I/O
------------------------------------------------------------------------
[ Layer 0: OpenRhiza Seed (인간이 Rust로 작성하는 불변의 핵) ]
  ├─ Communication: 초기 통신 채널 (가상머신용 UART 또는 물리머신용 USB xHCI DbC)
  ├─ Sandbox      : WebAssembly(wasmi) 런타임 및 Exception Handler (IDT)
  ├─ UI Base      : Linear Framebuffer (해상도 독립적 텍스트 렌더링)
  └─ HAL          : x86_64, ARM, RISC-V 하드웨어 추상화
========================================================================
[ Physical Hardware (범용 CPU, Motherboard, 버스 시스템) ]
```

---

## 3. 핵심 모듈별 구현 지침 및 해결법 (Guidelines)

### A. The Sandbox & Exception Handling (생명 유지 장치)
- **의미:** AI는 매뉴얼을 완벽히 이해하지 못한 상태에서 코드를 짭니다. 잘못된 메모리 접근이나 0으로 나누기 등의 하드웨어 Fault는 필연적입니다.
- **해결법:**
  - x86_64 기준 IDT(Interrupt Descriptor Table)를 엄격하게 구성합니다.
  - 특히 `Page Fault (#PF)`와 `General Protection Fault (#GP)` 발생 시, 기존 OS처럼 커널 패닉(블루스크린)을 띄우고 멈추는 것이 아니라, 오류가 발생한 명령어 주소(RIP)와 레지스터 상태를 캡처하여 **AI에게 문자열(Log)로 반환**해야 합니다.
  - **WebAssembly 샌드박스:** AI가 작성한 드라이버 코드를 커널의 네이티브 기계어로 직접 실행하지 않고, 커널 내부에 탑재된 초경량 `wasmi` Wasm 런타임 위에서 실행합니다. 이로써 AI가 잘못된 메모리를 참조하더라도 커널 패닉 대신 Wasm Trap(안전한 에러)으로 방어할 수 있습니다.

### B. Input Devices: USB over PS/2 (감각 기관 확보)
- **의미:** 현대 컴퓨터는 PS/2 포트가 없습니다. USB 마우스/키보드 연결이 필수입니다.
- **해결법:**
  - 아주 단순한 레거시 PS/2는 Fallback(대비책)으로만 남겨둡니다.
  - 핵심 과제는 **PCI Bus Enumeration**입니다. AI가 PCI 버스를 스캔하여 메인보드에 장착된 **USB 호스트 컨트롤러(xHCI/EHCI/UHCI)**를 찾아내고, 해당 컨트롤러의 메모리 맵(MMIO)을 조작하여 USB 장치와 통신하는 코드를 스스로 작성하게 유도합니다.

### C. Display: Linear Framebuffer (시각 기관)
- **의미:** 다양한 해상도와 모니터 환경에서 복잡한 GPU 드라이버 없이 즉각적인 텍스트 및 UI 출력이 필요합니다.
- **해결법:**
  - 부트로더(예: UEFI/coreboot)가 초기화해 준 Linear Framebuffer의 메모리 주소를 넘겨받습니다.
  - 커널 내부에 초경량 비트맵 폰트(8x8 또는 8x16)를 하드코딩하여, 특정 픽셀 위치에 점을 찍어 글자를 렌더링하는 `Writer` 모듈을 Layer 0에 구현합니다.

---

## 4. Phase 3+ 를 위한 아키텍처적 대비 (Forward Compatibility)

Level 1 구현 시, 향후 AI가 로컬로 완전히 독립하는 단계(Phase 3~5)를 위해 다음 사항을 구조적으로 고려해야 합니다.

### A. CPU-Only LLM Fallback (초경량 모델)
- **문제:** 모든 사용자가 고성능 GPU나 NPU를 가지는 것은 아닙니다.
- **설계 대비:** AI가 반드시 GPU/NPU 드라이버를 완성해야만 자립할 수 있는 구조를 지양합니다. 시스템은 `ggml`/`llama.cpp`와 유사하게 **순수 CPU 연산만으로 동작하는 양자화된 초경량 LLM(Quantized Lightweight Model)**을 시스템 메인 메모리(RAM)에 적재하고 실행할 수 있는 메모리 구조를 확보해야 합니다.

### B. NPU Hardware Topology (NPU 연결 구조의 다양성)
- **문제:** NPU 하드웨어는 메인보드와 결합되는 방식이 다양합니다.
- **설계 대비:** AI가 하드웨어를 탐색할 때 토폴로지를 인지할 수 있도록 정보를 제공해야 합니다.
  1. **Integrated NPU (직렬/내장형):** CPU 다이 내부에 존재하여 메인 RAM을 직접 공유(UMA)하는 경우. (예: Apple Neural Engine, 최신 Intel/AMD 모바일 칩)
  2. **Discrete NPU (병렬/외장형):** 메인보드의 PCIe 버스를 거쳐 연결되며 별도의 VRAM을 가지는 경우. DMA(Direct Memory Access) 셋업이 필수적임.

---

## 5. 완전 자율화 피드백 루프 (Zero-Human Intervention)
초기 OS는 외부 세계의 물리적 자극(키보드 등)이 무엇을 의미하는지 알지 못합니다. 이때 인간 개발자가 정답을 알려주는 방식(Human-in-the-loop)을 배제하고, AI가 스스로 학습하도록 다음 두 가지 방식을 혼합하여 사용합니다.

### A. 매뉴얼 기반 지식 획득 (Document-Driven)
- AI 두뇌(Host LLM)가 인터넷이나 사전 지식을 활용하여 하드웨어 스펙 문서(예: PS/2 Scancode Set 1, xHCI 매뉴얼)를 직접 읽고, 이를 바탕으로 드라이버 코드를 선제적으로 생성하여 샌드박스에 주입합니다.

### B. 가상의 손 (Virtual Hand) & 역공학 (Trial & Error)
- 에뮬레이터(QEMU/VMware)의 기계 제어 API(예: QEMU QMP)를 사용하여, 파이썬 스크립트가 인간을 대신해 물리적 신호(예: 'A' 키 입력)를 OS에 주입합니다.
- OS(Layer 0)는 감지된 날것의 하드웨어 값(예: `0x1E`)을 **시리얼 포트(UART)**를 통해 Host LLM으로 전송합니다.
- Host LLM은 "내가 'A'를 찔러 넣었을 때 `0x1E`가 반환되는구나"라는 원인과 결과의 쌍을 스스로 터득하고, 이를 매핑하는 드라이버를 즉석에서 코딩하여 OS로 전송합니다. 
- 인간의 개입 없이 완벽하게 독립적인 하드웨어 장악 루프가 완성됩니다.

---

## 6. 현실 세계의 물리적 통신 브릿지 (Modern 'Umbilical Cord')
과거의 DB9 시리얼 포트가 없는 현대 하드웨어에서 베어메탈 디버깅(탯줄)을 연결하기 위해 다음의 기술을 활용합니다.
- **가상 환경 (QEMU/VMware):** 100% 에뮬레이션되는 레거시 UART(COM1) 포트 또는 `Virtio-serial`을 사용하여 호스트 PC와 통신합니다.
- **현대 물리 하드웨어 (xHCI Debug Capability):** 현대 인텔/AMD 메인보드는 **USB DbC (Debug Capability)**라는 하드웨어 기능을 지원합니다. 복잡한 USB 소프트웨어 스택 없이도, 전용 'USB 크로스오버 케이블'을 꽂으면 하드웨어가 USB 포트를 원시 시리얼 포트처럼 변환하여 외부 호스트 PC로 로그를 전송합니다. Layer 0는 물리 환경 배포 시 이 DbC 초기화 코드를 포함할 수 있습니다.

---

## 7. Level 1 개발 절대 원칙 (Strict Rules)
1. **최소주의 (Minimalism):** Layer 0에 인간이 편하자고 방대한 외부 크레이트(라이브러리)를 포함시키지 마십시오.
2. **No Standard Library:** Rust의 `#![no_std]` 환경을 엄격히 유지합니다. 동적 메모리 할당(Heap)조차 초기에는 없이 고정 크기 배열로 버퍼링합니다.
3. **Everything is a file/memory:** 모든 하드웨어 장치는 메모리 주소(MMIO) 또는 포트(I/O Port)로 환원됩니다. AI에게는 이 주소에 대한 읽기/쓰기 권한만 샌드박스 내에서 허용합니다.