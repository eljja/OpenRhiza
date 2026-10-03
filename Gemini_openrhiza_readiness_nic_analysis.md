# OpenRhiza Public Readiness Analysis & NIC Strategy (PR Perspective)

이 문서는 OpenRhiza의 공식 외부 공개(Public Launch)를 앞두고, **외부 PR, 마케팅, 그리고 대외 기술 요구사항(External Tech Requirements)** 관점에서 시스템의 준비도와 치명적인 블로커(Blocker)들을 정리한 문서입니다.
*(해당 문서는 외부 PR 및 OpenRhiza.com 드라이버/스킬 담당 AI에 의해 관리됩니다.)*

---

## 1. 대외 홍보 시 강조할 핵심 무기 (Launch Strengths)
기술 커뮤니티(Hacker News, Reddit)에 즉각적으로 어필할 수 있는 현재의 강점입니다.
* **x86_64 Bare-metal Boot:** 단순한 파이썬 스크립트 기반의 에이전트가 아닌 진짜 OS 커널이라는 점.
* **Wasm Sandbox Isolation:** LLM이 생성한 코드가 커널을 죽이지 않는다는 안전성의 증명.
* **Ed25519 Payload Verifier:** OpenRhiza.com(Nexus)과 통신 시 보안이 보장되는 서명 검증 체계.

## 2. 외부 공개 전 반드시 해결해야 할 핵심 과제 (PR-Blocking Gaps)
외부 개발자들의 냉혹한 평가를 통과하기 위해, 내부 OS 커널 개발팀이 반드시 해결해주어야 하는 대외 리스크입니다.

### 🔴 Risk 1: The Network Bootstrap Paradox (NIC 전략 부재)
* **외부의 지적:** "드라이버를 인터넷에서 받아오려면, 처음에 인터넷은 어떻게 연결하나요?"
* **OS팀 대응 요구사항:** 모든 랜카드 드라이버를 커널에 넣는 것은 OS 철학에 어긋납니다. 대신 가장 널리 쓰이는 가상머신 드라이버(`virtio-net`, `e1000`)와 **USB 스마트폰 테더링(CDC-ECM / RNDIS)**을 기본 생존팩(Bootstrap Skills)으로 내장하여 이 논리적 모순을 완벽히 방어해야 합니다.

### 🔴 Risk 2: Hardware Bricking Risk (안전성 논란)
* **외부의 지적:** "AI가 환각(Hallucination)으로 잘못된 MMIO 주소에 전압을 쏘거나 레지스터를 망가뜨리면 하드웨어가 물리적으로 고장 나는 것 아닌가요?"
* **OS팀 대응 요구사항:** 외부 홍보 시 "안전하다"고 자신 있게 말할 수 있도록, Wasm Host ABI 수준에서 IOMMU 및 포트 접근을 강력하게 제한하는 '하드웨어 화이트리스트' 샌드박스 정책을 구현해야 합니다.

### 🔴 Risk 3: OpenRhiza.com 통신 안정성
* **외부의 지적:** "레지스트리가 죽으면 OS도 죽는 반쪽짜리 OS 아닌가요?"
* **OS팀 대응 요구사항:** TLS API 통신이 실패하거나 다운로드한 Wasm 파일이 깨져 있을 경우, 메인 시스템이 다운되지 않고 우아하게 복구 모드(Recovery Shell)로 돌아가는 롤백(Rollback) 및 로컬 캐시(FAT16) 로딩 메커니즘을 증명해야 합니다.
