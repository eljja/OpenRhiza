# OpenRhiza

**작은 독립 커널과 격리된 모듈을 기반으로 만드는 AI 운영체제.**

[English](README.en.md) · [한국어 문서](next/README.ko.md) · [English documents](next/README.md) · [개발 로드맵](next/docs/ROADMAP.ko.md)

## 현재 상태

OpenRhiza는 기존 시제품을 참고 자료로 보존하고, 새로운 구현으로 다시 시작했습니다.
현재는 **R0 방향 수립·요구사항 취합·Astra 계획 재검토 완료** 상태입니다.
**새 커널은 아직 부팅하지 않습니다.** 이 브랜치는 설치 가능한 OS나 완성된 제품이 아닙니다.

다음 작업은 **1.1단계 K 계약**입니다. 부팅 환경, 신뢰 경계, 메모리 소유권, 최소 복구 경로,
독립 시험 도구를 선정·검토한 뒤 커널 구현을 시작합니다. 계획 승인은 구현 통과가 아닙니다.

## 만들고 있는 것

- Linux 등 기존 OS 위에 올라가지 않는 독립 운영체제.
- CPU·메모리·권한·선점·제한된 IPC를 담당하는 작은 코어. 드라이버·저장소·GUI·앱·AI는 격리 모듈.
- 로컬 명령으로 시작하는 세션, 선택적으로 실행하는 다중창 데스크톱, 백그라운드 작업과 MDI.
- 정확한 명령은 로컬 처리하고, 나머지 의도는 교체 가능한 LLM이 해석·계획하는 공통 동작 경로.
- 키보드·마우스와 한·영 입력을 먼저 제공하고, 이후 사용자 동의가 있는 음성 입력을 추가.
- OpenRhiza.com/Nexus에서 재사용하거나 AI로 제작한 모듈을 독립 빌드·시험·승인 후 실행.
- 내려받아 실제로 플레이하고 저장·재실행할 수 있는 Doom/FreeDoom 모듈을 통합 시험으로 사용.

```text
사용자 명령 / 선택적 음성
          |
로컬 동작 + LLM 계획 -> 결정적 정책·승인
          |
격리 모듈: 앱 · 데스크톱 · 저장소 · 드라이버 · 모델
          |
Rhiza Core: 보호 · 자원 소유권 · 선점 · 제한된 통신
```

AI가 코어를 즉시 패치하거나 스스로 권한을 부여하지 않습니다. 모듈 서명은 출처와 허가의
근거이지 정확성의 증명이 아닙니다. 설치 기능의 실행·부팅·복구는 모델이나 네트워크 장애에도
유지되어야 합니다. 절대적인 무충돌을 약속하지 않고, 명시한 결함 모델 안에서 재부팅 없는
생존·복구·자원 회수를 검증합니다.

## 개발 순서

| 단계 | 목표 | 상태 |
| --- | --- | --- |
| R0 | 방향·요구사항·기존 구현 조사·계획 검토 | 완료, 구현 검증 아님 |
| 1 | 새로운 부팅과 재현 가능한 기반 | 1.1 K 계약 대기 |
| 2 | 보호된 네이티브 실행·복구·Wasm, 별도 SMP 관문 | 미구현 |
| 3 | 격리된 장치·저장소·네트워크·안전한 모듈 공급 | 미구현 |
| 4 | 실제 멀티태스킹 데스크톱과 네이티브 SDK | 미구현 |
| 5 | AI 작업·음성·지식 계층·플레이 가능한 게임 | 미구현 |
| 6 | 업데이트·복구·장기 시험을 통과한 공개 시스템 | 미구현 |

세부 순서와 예외는 [로드맵](next/docs/ROADMAP.ko.md), 진입·통과 조건은
[구현 기준](next/docs/IMPLEMENTATION_GATES.ko.md)을 따릅니다. 검증된 단일 vCPU 환경과
미검증 SMP를 구분합니다. 이전 시제품의 완료율·QEMU 결과를 새 구현에 승계하지 않습니다.

## 문서

| 문서 | 한국어 | English |
| --- | --- | --- |
| 전체 목차·현재 기준 | [읽기](next/README.ko.md) | [Read](next/README.md) |
| 제품·아키텍처 방향 | [읽기](next/docs/DIRECTION.ko.md) | [Read](next/docs/DIRECTION.md) |
| 요구사항 R01~R25 | [읽기](next/docs/REQUIREMENTS.ko.md) | [Read](next/docs/REQUIREMENTS.md) |
| 기존 구현 조사 | [읽기](next/docs/LEGACY_REVIEW.ko.md) | [Read](next/docs/LEGACY_REVIEW.md) |
| 6단계 로드맵 | [읽기](next/docs/ROADMAP.ko.md) | [Read](next/docs/ROADMAP.md) |
| 구현 착수·통과 기준 | [읽기](next/docs/IMPLEMENTATION_GATES.ko.md) | [Read](next/docs/IMPLEMENTATION_GATES.md) |
| Astra 초기·후속 검토 | [읽기](next/docs/ASTRA_PLAN_REVIEW.ko.md) | [Read](next/docs/ASTRA_PLAN_REVIEW.md) |
| 검토 반영 내역 | [읽기](next/docs/ASTRA_REVIEW_DISPOSITION.ko.md) | [Read](next/docs/ASTRA_REVIEW_DISPOSITION.md) |

## 저장소와 기록

- **새 개발:** `codex/openrhiza-next`. 현재 계획과 앞으로의 구현만 관리합니다.
- **기존 작업 보존:** [codex/legacy-snapshot-20261003](https://github.com/eljja/OpenRhiza/tree/codex/legacy-snapshot-20261003). 미커밋 변경을 포함한 소스·문서 스냅샷입니다.
- **기존 공개 이력:** `main` 및 기존 브랜치를 삭제하거나 강제로 변경하지 않았습니다.

이 브랜치의 `next/`에는 기존 커널·루트 Cargo 설정·배포 작업을 가져오지 않았습니다.
보존 범위, 제외한 로컬 자료, 과거 문서의 경로 해석은 [기록 안내](ARCHIVE.md)를 참고하세요.
기존 Nexus 배포 작업을 여기서 실행하지 않습니다. 웹사이트 운영 상태와 새 OS 준비도는 별개입니다.

## 문서 검증

PowerShell 7과 Git으로 저장소 루트에서 실행합니다.

```powershell
pwsh -NoProfile -File next/tools/verify-baseline.ps1
pwsh -NoProfile -File next/tools/test-baseline.ps1
git fetch --no-tags origin codex/legacy-snapshot-20261003
pwsh -NoProfile -File next/tools/verify-baseline.ps1 -LegacyRef FETCH_HEAD
```

기본 검사는 필수 문서·번역 구조·식별자·조사 목록의 무결성을 확인합니다. 마지막 검사는 고정한
보존 커밋의 실제 파일 바이트를 조사 목록과 대조합니다. **OS 빌드·QEMU·커널 안전성 검증은 아닙니다.**
새 이미지가 없는 현재 단계에서는 실행 명령이나 다운로드 가능한 OS 이미지를 제공하지 않습니다.

[기여 안내](CONTRIBUTING.md) · [보안 원칙](SECURITY.md) · [Apache-2.0 라이선스](LICENSE)
