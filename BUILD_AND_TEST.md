# OpenRhiza Build & Test Guide
# 빌드, 실행, 테스트, 검증 방법

---

## 전제 조건 (Prerequisites)

### 필수 도구
| 도구 | 설치 확인 | 설치 방법 |
|------|-----------|-----------|
| Rust (nightly) | `rustup show` | `rustup default nightly` |
| x86_64-unknown-none 타겟 | `rustup target list --installed` | `rustup target add x86_64-unknown-none` |
| wasm32-unknown-unknown 타겟 | (host_brain.py용) | `rustup target add wasm32-unknown-unknown` |
| cargo-bootimage | `cargo bootimage --help` | `cargo install bootimage` |
| QEMU x86_64 | `qemu-system-x86_64 --version` | [공식 사이트](https://www.qemu.org) |
| Python 3.10+ | `python --version` | (host_brain.py용) |
| google-genai | `pip show google-genai` | `pip install google-genai` |

### 환경 변수
```bash
# .env 파일 또는 시스템 환경 변수
GEMINI_API_KEY=your_api_key_here
```

---

## 빌드 (Build)

### 커널만 빌드 (가장 빠름, 수정 후 항상 실행)
```bash
cargo build
```
- 타겟: `x86_64-unknown-none` (.cargo/config.toml에 지정)
- 결과물: `target/x86_64-unknown-none/debug/OpenRhiza`

### 부트 이미지 생성
```bash
cargo bootimage
```
- 결과물: `target/x86_64-unknown-none/debug/bootimage-OpenRhiza.bin`

### 릴리즈 빌드
```bash
cargo build --release
cargo bootimage --release
```

---

## 실행 (Run)

### 방법 1: cargo run (권장)
```bash
cargo run
```
- 자동으로 bootimage를 빌드하고 QEMU를 실행합니다.
- QEMU 옵션은 `Cargo.toml`의 `[package.metadata.bootimage] run-command`에 정의됨.

### 현재 QEMU 실행 명령 (자동 생성됨)
```
qemu-system-x86_64.exe \
  -drive format=raw,file={bootimage} \
  -drive file=fat:rw:rhiza_drivers,format=raw,index=2 \
  -serial tcp:127.0.0.1:4444,server \
  -netdev user,id=n1 \
  -device e1000,netdev=n1
```

**QEMU 옵션 해설:**
| 옵션 | 의미 |
|------|------|
| `-drive format=raw,file={}` | 커널 부트 이미지 |
| `-drive file=fat:rw:rhiza_drivers,...` | `rhiza_drivers/` 폴더를 FAT16 가상 디스크로 마운트 (Secondary IDE) |
| `-serial tcp:127.0.0.1:4444,server` | 시리얼 포트를 TCP 서버로 노출 (host_brain.py 연결용) |
| `-netdev user,id=n1` | 사용자 모드 네트워킹 (NAT, 게이트웨이 10.0.2.2) |
| `-device e1000,netdev=n1` | Intel e1000 가상 NIC 장착 |

### 방법 2: 호스트 AI 뇌 연결 (별도 터미널)
```bash
python host_brain.py
python host_brain.py --model gemini-2.5-flash
python host_brain.py --model gemini-2.5-pro
```

---

## 검증 (Verification)

### 단계 1: 빌드 성공 확인
```bash
cargo build 2>&1
# 경고는 허용, 에러는 불허
# 허용되는 경고: unused import, static_mut_refs (Rust 2024 호환성)
```

### 단계 2: 부팅 확인
QEMU 실행 후 다음 시리얼 출력이 나와야 정상:
```
OpenRhiza Seed (Layer 0) Booting... Serial Connected!
Heap Allocator initialized!
Total Usable Memory: XXXXXXXX Bytes
Hardware Discovery Complete.
Found N PCI devices:
  Bus 0 Device X: Vendor 0x8086, Device 0x100E, BAR0: 0xFEBXXXXX
```

### 단계 3: 호스트 연결 확인
host_brain.py 실행 후:
```
[+] Successfully connected to OpenRhiza Umbilical Cord (Serial)!
[AI Brain] Injecting default QWERTY driver for instant boot...
[AI Brain] QWERTY driver injected. Scanning OS hardware logs...
```

### 단계 4: e1000 드라이버 자동 생성 확인
PCI 스캔에서 e1000 감지 시:
```
[AI Brain] === Intel e1000 NIC Detected! BAR0: 0xFEBXXXXX ===
[AI Brain] Generating initialization driver for Intel e1000 (Network Stack Bridge)...
```

---

## 문제 해결 (Troubleshooting)

### "error: linker `rust-lld` not found"
```bash
rustup component add llvm-tools-preview
```

### QEMU Triple Fault (무한 재부팅)
1. VGA 출력 대신 시리얼만으로 디버깅: `vga.rs`의 `_print()`에서 VGA 부분 주석 처리
2. QEMU에 `-d int -no-reboot` 추가하여 인터럽트 로그 확인

### "Kernel Panic: out of memory"
`allocator.rs`의 `HEAP_SIZE`를 늘리세요. 현재: 1 MiB.

### cargo run 시 QEMU 경로 오류
`Cargo.toml`의 `run-command`에서 QEMU 경로를 시스템에 맞게 수정.

### host_brain.py "ConnectionRefusedError"
QEMU가 먼저 실행되어야 합니다. `cargo run` 후 시리얼 TCP 포트(4444)가 열릴 때까지 기다립니다.
