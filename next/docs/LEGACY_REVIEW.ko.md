# OpenRhiza 기존 구현 조사

[영문 원문](LEGACY_REVIEW.md) · [한국어 목차](../README.ko.md)

목적: 새 구현 전에 원래 의도와 공학적 교훈을 복원합니다. 기존 소스·바이너리·로컬 변경·이력은 보존합니다.
주제별 저장소 검토와 재현 가능한 메타데이터 조사이며, 모든 파일·외부 의존성을 줄 단위로 보안 인증한 것은 아닙니다.

## 조사 범위

프로젝트 Markdown, 루트 빌드·호스트 도구, 커널 소스, 샌드박스 스킬·입력 드라이버, 장치 후보·시뮬레이터,
ARM 복구·증명 소스, 플랫폼·타깃 데이터, 자체 Nexus 소스·배포 메타데이터, 검증 도구를 목록화합니다.
정확한 참고 상태를 식별하도록 경로·분류·행 수·SHA-256을 `legacy-inventory.json`에 기록합니다.

재시작 목록은 Markdown 76개를 포함한 프로젝트 소스·설정·문서 364개입니다.
그 범위의 목록·해시를 기록했으며, 아래 코드 관찰은 주요 경로를 주제별로 검토한 결과입니다.
모든 구현의 모든 행을 감사했다는 주장은 아닙니다. 참고 파일이 바뀌면 목록을 다시 생성합니다.
검증 도구가 기록된 해시와의 차이를 검출합니다.

생성된 target 디렉터리, `.next`, `node_modules`, `.fslab`, 디스크·Wasm·WAD·목적 파일,
이미지·폰트, 사용자 로그, 비밀정보, PEM·개인키는 요구사항 출처로 삼지 않습니다.
외부 라이브러리·부트로더는 향후 도입 시 별도 검토하며, 존재 자체가 사용자 기능 요구를 뜻하지 않습니다.
자격증명을 출력하거나 참고 자료를 업로드하지 않습니다.
선별한 과거 로그는 인용만 하며 신규 시험이나 재인증 결과로 계산하지 않습니다.

## 주제별 문서 대조

| 주제 | 참고 자료 | 보존한 결정 |
| --- | --- | --- |
| 정체성·목표 | README, VISION, OS, ARCHITECTURE, LEVEL1_BLUEPRINT, HARNESS, ROADMAP | 작은 코어와 확장되는 샌드박스 기능을 가진 독립 AI 기반 OS |
| 코어·런타임·보안 | CORE_RUNTIME_FOUNDATION, CORE_BOUNDARY_AUDIT, MODULE_MAP, SECURITY, OPENRHIZA_MODULE_ARCHITECTURE, 이전 재구축 계획 | 기능 확대 전에 강한 격리; 과거 영구 코어 분류를 승계하지 않음 |
| 패키지·런타임 | CAPABILITY_REGISTRY, HOT_SWAP_RUNTIME, PERSISTENT_DRIVER_CACHE, SANDBOX_MIGRATION_STATUS, QEMU_DRIVER_SET, BOOT_AUTORUN | 캐시·레지스트리·생성 순서, 실행·영속 상태 분리, 상태 확인·롤백 |
| 서비스 생태계 | OPENRHIZA_COM_PLAN, OPENRHIZA_API_V1, Nexus README/CI_CD/소스 | 공유 기능·근거 레지스트리와 별도 제작 환경; API 초안 미확정 |
| 데스크톱 | GUI_DEVELOPMENT, DISPLAY_ABI, WINDOW_PROTOCOL, DEFAULT_DESKTOP_APPS, DESKTOP_ACTIONS, OPENRHIZA_DESKTOP_COMPLETION | 명령 우선, 검사 가능한 GUI, 실제 다중창·앱; 기존 완료 표시 초기화 |
| 입력·텍스트 | SANDBOX_INPUT_DRIVER, USB_HID_HANDOFF, FONT_SKILL, VOICE_INPUT, 한글·폰트·음성 도구 | 장치·텍스트·음성 모듈, 포커스·개인정보, 한·영과 세션 사용성 |
| 저장소·지식 | FILESYSTEM_SKILLS, FILESYSTEM_VALIDATION_MATRIX, STORAGE_HOST_ABI, STORAGE_IMAGE_HARNESS, SKILL_FS_BRIDGE, 모든 파일시스템·의미 객체 문서 | 게스트 파일·내구성과 재생성 가능한 비공개 의미 계층; 호스트 실험을 게스트 지원으로 보지 않음 |
| 자율 동작·개발 | AUTONOMY_MODE, GUI_DEVELOPMENT, 컴파일러·브라우저·스토어·보안·포렌식 시제품 | 통제된 계획·자가 개발, 검사 가능한 실제 도구; 실행을 흉내 내는 스크립트로 대체하지 않음 |
| 앱·성능 | DOOM_SKILL, DOOM_MODULE_ACCEPTANCE, Doom 포트 기록, SANDBOX_EXECUTION_ACCELERATION | 실제 게임을 통합 통과 조건으로 사용; 게임 전용 코어·FPS를 위한 실행 기아 금지 |
| 플랫폼·호환성 | PLATFORM_EXPANSION, PLATFORM_PROGRESS, CROSS_PLATFORM_BRINGUP, 플랫폼 README·표·타깃, PROGRAM_COMPATIBILITY_GOALS | 아키텍처 독립 계약; 확장·호환성은 별도 검증 |
| 품질·공개 | BUILD_AND_TEST, VALIDATION_MATRIX, PUBLIC_ALPHA_CHECKLIST, PUBLIC_READINESS, KNOWN_ISSUES, CONTRIBUTING, CHANGELOG, PROTOCOL, DOC_INDEX, Codex | 정확한 실측 주장, 재현성, 실패·복구, 작업 시작·종료 보고 |
| 과거 제안 | Gemini 준비 상태·작업 기록, 샌드박스 분석 | 부트스트랩·물리 위험의 유용한 질문; 홍보성 주장·제품 전환 제안은 기준이 아님 |

## 새 설계를 바꾸는 코드 교훈

1. `src/main.rs`, `src/display.rs`, `src/task/keyboard.rs`, `src/core/seed.rs`에는 오케스트레이션,
   UI, 모델·API 작업, 장치 중개와 커널 실행이 섞여 있습니다. 이 파일을 리팩터링하는 대신 코어의 기구부터 새로 소유권을 정합니다.
   수백 개의 호스트 import는 최소 시스템 호출 ABI가 아닙니다.
2. `src/task/executor.rs`는 협력형이며 큐가 가득 차면 spawn이 패닉을 낼 수 있습니다.
   `src/arch/x86_64/interrupts.rs`의 페이지 폴트와 커널 패닉 경로는 루프에 머뭅니다.
   라운드로빈 poll 횟수는 선점·장애 격리의 증거가 아닙니다.
3. Wasmi는 커널 문맥에서 실행됩니다. 모듈이 준 패킷 길이로 게스트 메모리 검증 전에 커널 할당이 일어날 수 있습니다.
   전역 핸들·장치 상태, 넓은 호스트 import, DMA/MMIO 연산은 주체별 소유권이나 강제된 IOMMU 격리를 제공하지 않습니다.
   이 경로는 복사하지 않습니다.
4. 일반 드라이버 다운로드 경로는 내려받은 내용을 컴파일·적재해 실제 장치를 전환했음을 입증하지 않은 채
   활성·선호 연결과 페이로드 기록을 저장합니다. e1000/PIIX/xHCI 개별 실험은 유용하지만 일반 수명주기나 하드웨어 격리 체계의 증거는 아닙니다.
5. TLS는 상대 인증 단계를 건너뛰며, 서명 없는 스킬도 캐시·적재될 수 있습니다.
   모의 바이너리·헤더 서명 경로와 v1 JSON 전달은 별개입니다. 새 전송·매니페스트 허가·시험은 각각 실패 시 거부해야 합니다.
6. `build.rs`는 환경·로컬 `.env`의 모델 API 키를 읽어 `src/api_v1.rs`가 사용하는 컴파일 시 값으로 넣습니다.
   새 이미지·패키지에는 사용자 API 키를 넣지 않고 별도 보호된 서비스 객체로 관리합니다.
7. `src/main.rs`의 전경 프레임 분기는 일반 네트워크·드라이버·API·GUI 작업을 건너뜁니다.
   Doom의 시간 처리, 아무것도 하지 않는 쓰기 우회 함수, 오디오·입력 공백 때문에 표시 FPS만으로 플레이 가능성을 입증할 수 없습니다.
   새 스케줄링은 허가된 모든 예산을 보존하고 시뮬레이션 시간과 표시 시간을 분리합니다.
8. 데스크톱 창은 독립 앱이 아닌 정적 장면 객체입니다. 브라우저는 URL을 고정 HTML에 대응시키고 JS는 문자열을 검사합니다.
   앱·Nexus 스토어, VFS, 방화벽, 메모리 포렌식 시제품은 로컬·정적 모의 데이터에 의존합니다.
   UX 아이디어는 보존하되 가짜 설치·저장·필터링·프로세스 권한·커널 메모리 접근을 실제 기능으로 주장하지 않습니다.
9. `nic_drivers/driver_manifest.json`에는 후보 20개가 있습니다. Python 시험은 대체로 컴파일된 Rust 산출물 대신
   Python 모델을 시험하고, `test_qemu_hw.py`에는 무조건 참인 검사가 있습니다.
   점수·소스 파일만으로 게스트 장치 지원이 되지 않습니다. 실제 I/O와 독립 장애 시험이 필요합니다.
10. 파일시스템 계열 읽기·쓰기 시험은 WSL 파일시스템을 사용하며 게스트 탐침은 시그니처·임시 블록을 확인합니다.
    유용한 시험 자료지만 게스트 VFS 구현은 아닙니다. 호스트 의미 색인기도 게스트 색인·질의·권한별 문맥 조회를 입증하지 않습니다.
11. ARM에는 별도 복구 커널과 선택적 Wasmi·증명 도구가 있습니다. 큰 증명·카운터 경로는 x86 사용자 런타임을 ARM에 옮긴 것이 아닙니다.
    새 플랫폼 동등성은 공통 네이티브 ABI·모듈 모델과 새로운 플랫폼별 통과 근거로 확보합니다.
12. Nexus에는 실제 SQLite, 공개 게시판, 업로드·다운로드, 설정된 Gemini 프록시가 있지만 인증·불변성·출처·근거를
    연결한 완전한 릴리스 체계가 없습니다. 주장된 신원·TPM 신뢰, 자기 보고 점수·투표는 독립 근거가 아닙니다.
    배포 작업 흐름만으로 커널·모듈·보안 CI나 운영 롤백이 검증되지는 않습니다.

참고할 아이디어는 CPU 진입·직렬 진단, 장치 레지스터·디스크립터 관찰, 제한된 저장소 임시 시험 자료,
GUI 상호작용, 한글 조합 사례, 그래프 스키마, 게임 이식 경험, 명시적 실패 시험 요구입니다.
명세·계약에서 새로 구현하고 독립 검증해야 하며, 성공해 보이는 경로를 통째로 복사하지 않습니다.

## 해결한 문서 충돌

- 사용자의 새 출발 요청은 "현재 커널을 출발점으로 사용"하던 결정과 이전 중심 코어 분류보다 우선합니다.
- `OPENRHIZA_DESKTOP_COMPLETION.md`는 장면 이름·자가시험으로 과거 데스크톱 6단계를 모두 완료 처리했지만 새 OS의 통과 단계는 아닙니다.
- 과거 일부 통합 문서는 DNS가 없다고 했으나 `src/dns.rs`와 QEMU 로그에는 네이티브 A 질의가 있습니다.
  TLS가 휴면 상태라는 설명과 달리 API 경로가 사용했습니다. 실제 소스·실행 경로 근거가 오래된 설명보다 우선합니다.
- Doom 성능 기록과 이후 실패 로그가 다릅니다. 과거 최고 수치를 승계하지 않고 정확한 시간·입력·소리·영속 저장·백그라운드 동작으로 검증합니다.
- 복구를 이유로 큰 네트워크·저장소·Wasm·UI를 코어에 유지할 수 없습니다. 오프라인 서명 모듈 집합과 예약된 복구 실행을 제공합니다.
- 모델 협의체나 인기 레지스트리 산출물은 스스로 권한을 부여할 수 없습니다. 사용자 정책·독립 허가를 유지합니다.

## 재시작 당시 참고 검증

기존 브랜치는 `codex/sandbox-core-migration`입니다. 추적·미추적 변경이 많은 상태이며 이를 버리지 않았습니다.
관찰한 도구는 Rust `1.96.0-nightly`(2026-03-21 커밋), QEMU `10.2.0`입니다.
새 도구체인의 확정 버전은 아닙니다. 루트 Cargo와 수정 부트로더 설정은 기존 구현용입니다.

기존 x86_64 TCG 이미지에서 `/status`, `/scheduler-status`, `/wasm-status`, `/desktop-status`,
`/platform-status`, `/recovery`를 실행했습니다. 로그: `../../logs/x86-legacy-reference-reset-review-20261003.log`.
기존 부팅·CLI·복구 참고 결과이며 새 커널이나 악성 모듈 생존 근거는 아닙니다.
이 문서·재시작 작업을 위해 이미지를 다시 빌드하지 않았습니다.
시험 도구가 성공 종료했고 직렬 로그에는 명령 응답·복구 선택이 있으며 panic·page-fault·triple-fault 표식이 없었습니다.
이 짧은 시험으로 GUI 조작, 게임 플레이, 전원 손실 무결성, 동시 보호 앱, 적대적 모듈을 검증하지 않았습니다.

직전 생태계 검토는 공개 게시판·상태 확인 HTTP 200, 게스트 DNS·레지스트리 질의,
Nexus lint 오류 0개·경고 2개를 기록했습니다. 이는 과거 근거이며 이번에 다시 수행한 공급망·보안 시험이 아닙니다.
공개 업로드·배포, 유료 모델 작업, 키 교체, 기존 소스 삭제는 이 재시작 작업에 포함하지 않았습니다.
