import socket
import time
import os
import argparse
import subprocess
import tempfile
import re

try:
    from google import genai
except ImportError:
    print("[-] google-genai package is not installed. Run 'pip install google-genai'.")
    exit(1)

HOST = '127.0.0.1'
PORT = 4444

# --- Gemini API 설정 ---
# Read the API key from the environment variable. Never hardcode it!
api_key = os.environ.get("GEMINI_API_KEY")
if not api_key:
    print("[-] GEMINI_API_KEY environment variable is not set.")
    print("[-] Please set the GEMINI_API_KEY environment variable and restart the terminal.")
    exit(1)

# 새로운 google-genai 클라이언트 초기화
client = genai.Client(api_key=api_key)

# Configure command-line argument for model selection (Default: gemini-2.5-flash-lite)
parser = argparse.ArgumentParser(description="OpenRhiza Host AI Brain")
parser.add_argument('--model', type=str, default='gemini-2.5-flash-lite', help='Primary LLM Model ID (e.g., gemini-2.5-pro)')
args = parser.parse_args()
PRIMARY_MODEL = args.model

# API 에러 시 순차적으로 시도할 Fallback 모델 라인업
FALLBACK_MODELS = [
    'gemini-2.5-flash-lite-preview-09-2025', # $0.10 / 0.40
    'gemini-2.5-flash-lite', # $0.10 / 0.40
    'gemini-3.1-flash-lite-preview', # $0.25 / 1.50
    'gemini-2.5-flash', # $0.30 / 2.50
    'gemini-3-flash-preview' # $0.50 / 3.00
#    'gemini-2.5-pro' # $1.25 / 10.00
#    'gemini-3.1-pro-preview' # $2.00 / 12.00
]

# Primary를 맨 앞에 두고, 중복을 제거하여 최종 시도할 모델 체인(순서) 생성
MODELS_TO_TRY = [PRIMARY_MODEL] + [m for m in FALLBACK_MODELS if m != PRIMARY_MODEL]
print(f"[*] AI Models sequence (Primary -> Fallback): {MODELS_TO_TRY}")
# -----------------------

def connect_to_umbilical_cord():
    """QEMU의 시리얼 포트(TCP 4444)에 접속을 시도합니다."""
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    print(f"[*] Waiting for OpenRhiza OS to boot on {HOST}:{PORT}...")
    while True:
        try:
            s.connect((HOST, PORT))
            print("[+] Successfully connected to OpenRhiza Umbilical Cord (Serial)!")
            return s
        except ConnectionRefusedError:
            # OS가 켜질 때까지 1초마다 재시도합니다.
            time.sleep(1)

# --- 기본 QWERTY 드라이버 데이터 (빠른 부팅을 위한 하드코딩) ---
DEFAULT_QWERTY = [0x3F] * 256
qwerty_lower = {
    0x10:b'q', 0x11:b'w', 0x12:b'e', 0x13:b'r', 0x14:b't', 0x15:b'y', 0x16:b'u', 0x17:b'i', 0x18:b'o', 0x19:b'p',
    0x1E:b'a', 0x1F:b's', 0x20:b'd', 0x21:b'f', 0x22:b'g', 0x23:b'h', 0x24:b'j', 0x25:b'k', 0x26:b'l',
    0x2C:b'z', 0x2D:b'x', 0x2E:b'c', 0x2F:b'v', 0x30:b'b', 0x31:b'n', 0x32:b'm',
    0x02:b'1', 0x03:b'2', 0x04:b'3', 0x05:b'4', 0x06:b'5', 0x07:b'6', 0x08:b'7', 0x09:b'8', 0x0A:b'9', 0x0B:b'0',
    0x1C:0x0A, 0x39:0x20, 0x0E:0x08, 0x28:b"'", 0x27:b';', 0x33:b',', 0x34:b'.', 0x35:b'/', 0x0C:b'-', 0x0D:b'='
}
qwerty_upper = {
    0x10:b'Q', 0x11:b'W', 0x12:b'E', 0x13:b'R', 0x14:b'T', 0x15:b'Y', 0x16:b'U', 0x17:b'I', 0x18:b'O', 0x19:b'P',
    0x1E:b'A', 0x1F:b'S', 0x20:b'D', 0x21:b'F', 0x22:b'G', 0x23:b'H', 0x24:b'J', 0x25:b'K', 0x26:b'L',
    0x2C:b'Z', 0x2D:b'X', 0x2E:b'C', 0x2F:b'V', 0x30:b'B', 0x31:b'N', 0x32:b'M',
    0x02:b'!', 0x03:b'@', 0x04:b'#', 0x05:b'$', 0x06:b'%', 0x07:b'^', 0x08:b'&', 0x09:b'*', 0x0A:b'(', 0x0B:b')',
    0x1C:0x0A, 0x39:0x20, 0x0E:0x08, 0x28:b'"', 0x27:b':', 0x33:b'<', 0x34:b'>', 0x35:b'?', 0x0C:b'_', 0x0D:b'+'
}
for k, v in qwerty_lower.items(): DEFAULT_QWERTY[k] = ord(v) if isinstance(v, bytes) else v
for k, v in qwerty_upper.items(): DEFAULT_QWERTY[k + 0x80] = ord(v) if isinstance(v, bytes) else v
# -------------------------------------------------------------

def generate_and_inject_keymap(s, calibration_sequence):
    """Predicts the layout based on the calibration sequence and injects the driver."""
    prompt = f"""
    You are an AI brain autonomously coding a bare-metal operating system (OpenRhiza).

    [Calibration Data]
    The user was instructed to type "hi!" and press Enter.
    Received scancode sequence: {', '.join(calibration_sequence)}
    
    Instruction: Infer the user's keyboard layout (QWERTY, Dvorak, etc.) based on the scancode sequence above.
    Then, create a mapping table for that layout using the manual below (for physical location reference).
    
    [Physical Hardware Reference - QWERTY Scancode Set 1 (For reference)]
    Q=0x10, W=0x11, E=0x12, R=0x13, T=0x14, Y=0x15, U=0x16, I=0x17, O=0x18, P=0x19
    A=0x1E, S=0x1F, D=0x20, F=0x21, G=0x22, H=0x23, J=0x24, K=0x25, L=0x26
    Z=0x2C, X=0x2D, C=0x2E, V=0x2F, B=0x30, N=0x31, M=0x32
    1=0x02, 2=0x03, 3=0x04, 4=0x05, 5=0x06, 6=0x07, 7=0x08, 8=0x09, 9=0x0A, 0=0x0B
    Shift+1='!', 2='@', 3='#', 4='$', 5='%', 6='^', 7='&', 8='*', 9='(', 0=')'
    Enter=0x1C, Space=0x39, Backspace=0x0E

    Mapping Rules: Map both Normal keys and Shifted keys.
    Rules: 
    - Use lowercase ASCII codes for Normal alphabets, and uppercase ASCII codes for Shifted alphabets.
    - [Important] The Index of a Normal key must be written exactly as its scancode (e.g., 0x1E).
    - [Important] The Index of a Shifted key must be calculated as 'scancode + 0x80' (e.g., Shift+A is 0x1E + 0x80 = 0x9E).
    - Map Enter(0x1C) to 0x0A, Backspace(0x0E) to 0x08, and Space(0x39) to 0x20.
    - Absolutely no greetings, markdown (```), or additional explanations are allowed. Output ONLY the mapping data.

    Output Example:
    0x1E:0x61
    0x9E:0x41
    0x30:0x62
    0x02:0x31
    0x82:0x21
    0x39:0x20
    """
    print(f"\n[AI Brain] Predicting keyboard layout by analyzing sequence ({len(calibration_sequence)} items)...")
    
    response = None
    used_model = None
    
    # 1. API 호출 및 Fallback (우회) 로직
    for model_id in MODELS_TO_TRY:
        try:
            response = client.models.generate_content(
                model=model_id,
                contents=prompt
            )
            used_model = model_id
            break  # 성공적으로 답변을 받았으므로 루프 탈출
        except Exception as e:
            print(f"[!] {model_id} call failed (Trying fallback): {e}")
            continue # 에러 발생 시 다음 모델 시도

    if not response:
        print("[AI Brain] All AI models failed. Will retry on next input...")
        s.sendall(bytes([0xFE])) # 0xFE: 실패 시그널 전송
        return False

    print("-" * 50)
    print(f"[Gemini's Response ({used_model})]\n{response.text}")
    print("-" * 50)

    # 2. 결과 파싱 및 데이터 주입 로직
    try:
        # 1. 256바이트 배열을 모두 '?'(0x3F)로 안전하게 초기화합니다.
        payload_array = [0x3F] * 256
        
        # 2. AI가 응답한 '키:값' 쌍을 파싱하여 정확한 주소에 꽂아 넣습니다.
        lines = response.text.replace(' ', '').strip().split('\n')
        for line in lines:
            if ':' in line:
                try:
                    scan_str, ascii_str = line.split(':')
                    scancode = int(scan_str, 16)
                    ascii_val = int(ascii_str, 16)
                    if scancode < 256:
                        payload_array[scancode] = ascii_val
                except Exception:
                    continue # 파싱에 실패한 줄은 무시합니다.
                    
        payload = bytes(payload_array)
        print(f"[AI Brain] Dictionary mapping complete. Starting 256-byte driver injection...")
        
        # OS 버퍼 초기화 시그널(0xFD) 전송 (TCP 쓰레기 데이터 방지)
        s.sendall(bytes([0xFD]))
        time.sleep(0.05)

        # OS 폴링 주기를 고려하여 데이터 유실을 막기 위해 아주 약간의 딜레이를 주며 전송합니다.
        for b in payload:
            s.sendall(bytes([b]))
            time.sleep(0.005)
        print("[AI Brain] Driver injection complete! You can now type freely in QEMU.")
        return True

    except Exception as e:
        print(f"[!] Error occurred during data parsing and injection: {e}")
        print("[AI Brain] Driver injection failed. Will retry on next input...")
        s.sendall(bytes([0xFE])) # 0xFE: 실패 시그널 전송
        return False

def extract_rust_code(text):
    """LLM의 응답에서 마크다운을 제거하고 순수 Rust 코드만 추출합니다."""
    match = re.search(r'```(?:rust)?\s*(.*?)\s*```', text, re.DOTALL)
    if match:
        return match.group(1)
    return text.strip()

def compile_rust_to_wasm(rust_code):
    """임시 Rust 프로젝트를 생성하여 Wasm 바이너리로 컴파일합니다."""
    print("\n[AI Brain] Compiling generated Rust code to WebAssembly...")
    
    # Wasm 베어메탈 컴파일을 위한 필수 보일러플레이트 자동 주입
    if "#![no_std]" not in rust_code:
        rust_code = "#![no_std]\n" + rust_code
    if "#[panic_handler]" not in rust_code:
        rust_code += "\n#[cfg(not(test))]\n#[panic_handler]\nfn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }\n"

    with tempfile.TemporaryDirectory() as temp_dir:
        project_dir = os.path.join(temp_dir, "wasm_driver")
        
        try:
            # 1. 라이브러리 프로젝트 생성
            subprocess.run(["cargo", "new", "--lib", "wasm_driver"], cwd=temp_dir, check=True, capture_output=True)
            
            # 2. Cargo.toml 수정 (cdylib 설정 및 사이즈 최적화)
            cargo_toml_path = os.path.join(project_dir, "Cargo.toml")
            with open(cargo_toml_path, "a") as f:
                f.write("\n[lib]\ncrate-type = [\"cdylib\"]\n")
                f.write("\n[profile.release]\nopt-level = \"z\"\nlto = true\n")
                
            # 3. 소스 코드 작성
            lib_rs_path = os.path.join(project_dir, "src", "lib.rs")
            with open(lib_rs_path, "w", encoding="utf-8") as f:
                f.write(rust_code)
                
            # 4. Wasm 타겟으로 빌드 실행
            print("[AI Brain] Running 'cargo build --target wasm32-unknown-unknown --release'...")
            subprocess.run(
                ["cargo", "build", "--target", "wasm32-unknown-unknown", "--release"],
                cwd=project_dir, check=True, capture_output=True, text=True
            )
            
            # 5. 생성된 Wasm 바이너리 읽기
            wasm_path = os.path.join(project_dir, "target", "wasm32-unknown-unknown", "release", "wasm_driver.wasm")
            with open(wasm_path, "rb") as f:
                wasm_bytes = f.read()
            print(f"[AI Brain] Wasm compilation successful! Binary size: {len(wasm_bytes)} bytes.")
            return wasm_bytes, None
        except subprocess.CalledProcessError as e:
            print(f"[!] Wasm Compilation Failed:\n{e.stderr}")
            print("[!] Hint: Make sure you have run 'rustup target add wasm32-unknown-unknown'")
            return None, e.stderr

def generate_e1000_driver(s, bar0_address):
    """e1000 랜카드의 BAR0 주소를 바탕으로 초기화 드라이버 코드를 생성합니다."""
    initial_prompt = f"""
    You are an AI brain autonomously coding a bare-metal operating system (OpenRhiza).

    [Hardware Discovery]
    Found an Intel e1000 Network Interface Card (Vendor: 0x8086, Device: 0x100E).
    The Memory-Mapped I/O (MMIO) Base Address (BAR0) is {bar0_address}.

    Instruction: Write a Rust code snippet (that will be compiled to WebAssembly) to perform a Global Reset on this e1000 controller.
    - You must use the following imported Host Functions to access MMIO instead of raw pointers:
      `extern "C" {{ fn read_mmio(addr: u32) -> u32; fn write_mmio(addr: u32, val: u32); }}`
    - The Device Control Register (CTRL) is at offset 0x0000 from BAR0.
    - The Device Reset (RST) bit is bit 26.
    - Read the current CTRL value, set bit 26, and write it back using the host functions.
    - Ensure your code is wrapped in a public function exported for WebAssembly: `#[no_mangle] pub extern "C" fn init_e1000()`

    Output ONLY the Rust code snippet. Do not include markdown formatting or explanations.
    """
    print(f"\n[AI Brain] Intel e1000 detected at {bar0_address}! Generating initialization driver...")
    
    # OS 화면에 "드라이버 생성 중..." 상태를 띄우기 위해 0xFB 시그널 전송
    s.sendall(bytes([0xFB]))

    MAX_RETRIES = 3 # 코드가 컴파일될 때까지 LLM에게 다시 기회를 주는 최대 횟수

    for model_id in MODELS_TO_TRY:
        print(f"\n[AI Brain] Trying model: {model_id}")
        current_prompt = initial_prompt
        
        for attempt in range(MAX_RETRIES):
            try:
                print(f"[AI Brain] Attempt {attempt + 1}/{MAX_RETRIES}...")
                response = client.models.generate_content(
                    model=model_id,
                    contents=current_prompt
                )
                
                rust_code = extract_rust_code(response.text)
                print("-" * 50)
                print(f"[AI Generated e1000 Driver ({model_id} - Attempt {attempt+1})]\n{rust_code}")
                print("-" * 50)
                
                # 추출 및 컴파일 시도
                wasm_bytes, compile_err = compile_rust_to_wasm(rust_code)
                
                if wasm_bytes:
                    print(f"[AI Brain] Injecting Wasm binary ({len(wasm_bytes)} bytes) into OS...")
                    s.sendall(bytes([0xFC])) # Wasm 전송 시작 시그널
                    s.sendall(len(wasm_bytes).to_bytes(4, byteorder='little')) # 크기(4바이트) 전송
                    for b in wasm_bytes:
                        s.sendall(bytes([b]))
                        time.sleep(0.002) # 시리얼 포트 버퍼 오버플로우 방지
                    print("[AI Brain] Wasm injection complete!")
                    return True
                else:
                    print(f"[!] Compilation failed on attempt {attempt + 1}.")
                    if attempt < MAX_RETRIES - 1:
                        print("[AI Brain] Feeding compiler errors back to the LLM to fix the code...")
                        current_prompt = f"""
Your previous code failed to compile. 

[Previous Code]
```rust
{rust_code}
```

[Compiler Error]
```
{compile_err}
```

Instruction: Fix the errors based on the compiler output above and provide the complete, corrected Rust code.
- Output ONLY the fixed Rust code snippet. Do not include markdown formatting or explanations.
"""
                    else:
                        print(f"[!] Max retries reached for model {model_id}.")
            except Exception as e:
                print(f"[!] {model_id} API failed during attempt {attempt + 1}: {e}")
                break # 현재 모델의 API 호출 에러 시, 재시도를 중단하고 다음 Fallback 모델로 넘어감
            
    print("[AI Brain] Failed to generate e1000 driver.")
    
    # OS 화면에 "생성 실패" 상태를 띄우기 위해 0xFA 시그널 전송
    s.sendall(bytes([0xFA]))
    return False

def listen_and_think(s):
    """Receives logs from the OS and parses them for the AI."""
    buffer = b""
    calibration_done = False
    calibration_sequence = [] # 캘리브레이션용 스캔코드 수집 리스트
    e1000_driver_generated = False # e1000 드라이버 생성 여부
    
    # 연결 즉시 기본 QWERTY 드라이버를 선제적으로 주입합니다.
    print("[AI Brain] Injecting default QWERTY driver for instant boot...")
    s.sendall(bytes([0xFD]))
    time.sleep(0.05)
    for b in DEFAULT_QWERTY:
        s.sendall(bytes([b]))
        time.sleep(0.002) # 시리얼 포트 버퍼 오버플로우 방지
    print("[AI Brain] QWERTY driver injected. Waiting for user verification...")

    while True:
        data = s.recv(1024)
        if not data:
            print("[-] Connection closed by QEMU.")
            break
        
        buffer += data
        while b'\n' in buffer:
            line, buffer = buffer.split(b'\n', 1)
            decoded_line = line.decode('utf-8', errors='ignore').strip()
            
            # 'QEMU_LOG'라는 태그가 붙은 하드웨어 이벤트만 골라냅니다.
            if "QEMU_LOG:" in decoded_line:
                print(f"[AI Brain] Sensory data received: {decoded_line}")
                
                if not calibration_done:
                    # 스캔코드를 추출하여 리스트에 수집
                    parts = decoded_line.split("->")
                    if len(parts) == 2:
                        scancode_str = parts[1].strip()
                        calibration_sequence.append(scancode_str)
                        
                        # Enter 키(0x1C)가 입력되면 검증 시작
                        if int(scancode_str, 16) == 0x1C:
                            # 순수하게 누른 키(Make Code) 중 Shift(0x2A, 0x36)를 제외하고 추출
                            make_codes = [int(x, 16) for x in calibration_sequence if int(x, 16) < 0x80 and int(x, 16) not in [0x2A, 0x36]]
                            
                            # QWERTY 배열에서 'h'는 0x23, 'i'는 0x17. 이 코드가 포함되어 있다면 QWERTY가 맞음!
                            if 0x23 in make_codes and 0x17 in make_codes:
                                print("[AI Brain] Standard QWERTY layout verified! Skipping AI calibration.")
                                calibration_done = True
                            else:
                                print("\n[AI Brain] Alternative keyboard layout detected! Engaging AI...")
                                if generate_and_inject_keymap(s, calibration_sequence):
                                    calibration_done = True
                                else:
                                    calibration_sequence.clear()

            elif decoded_line:
                print(f"[OS System] {decoded_line}")
                
                # OS 로그 중 Intel e1000 랜카드가 발견되면 드라이버 코드를 생성합니다.
                if not e1000_driver_generated and "Vendor 0x8086" in decoded_line and "Device 0x100E" in decoded_line:
                    parts = decoded_line.split("BAR0: ")
                    if len(parts) == 2:
                        bar0_address = parts[1].strip()
                        e1000_driver_generated = generate_e1000_driver(s, bar0_address)

if __name__ == "__main__":
    conn = connect_to_umbilical_cord()
    try:
        listen_and_think(conn)
    except KeyboardInterrupt:
        print("\n[*] Host AI shutdown.")
        conn.close()