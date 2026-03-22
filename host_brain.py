import socket
import time
import os
import argparse

try:
    from google import genai
except ImportError:
    print("[-] google-genai 패키지가 없습니다. 'pip install google-genai'를 실행하세요.")
    exit(1)

HOST = '127.0.0.1'
PORT = 4444

# --- Gemini API 설정 ---
# 환경 변수에서 API 키를 읽어옵니다. 절대 코드에 직접 적지 마세요!
api_key = os.environ.get("GEMINI_API_KEY")
if not api_key:
    print("[-] GEMINI_API_KEY 환경 변수가 설정되지 않았습니다.")
    print("[-] 터미널에서 'set GEMINI_API_KEY=당신의_실제_키' 를 실행한 후 다시 켜주세요.")
    exit(1)

# 새로운 google-genai 클라이언트 초기화
client = genai.Client(api_key=api_key)

# 커맨드라인 인자로 모델을 선택할 수 있도록 설정합니다. (기본값: gemini-2.5-flash-lite)
parser = argparse.ArgumentParser(description="OpenRhiza Host AI Brain")
parser.add_argument('--model', type=str, default='gemini-2.5-flash-lite', help='사용할 LLM 모델 ID (예: gemini-2.5-pro)')
args = parser.parse_args()
MODEL_ID = args.model
print(f"[*] Activated AI Model: {MODEL_ID}")
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

def generate_and_inject_keymap(s):
    """Gemini를 이용해 전체 키보드 매핑 테이블을 생성하고 OS로 전송(주입)합니다."""
    prompt = f"""
    당신은 지금 베어메탈 운영체제(OpenRhiza)를 스스로 코딩하는 AI 두뇌입니다.
    다음은 시스템에서 발견된 하드웨어 매뉴얼 발췌본입니다. 이것을 읽고 매핑 테이블을 구성하세요.

    [Hardware Reference Manual - PS/2 Scancode Set 1]
    Q=0x10, W=0x11, E=0x12, R=0x13, T=0x14, Y=0x15, U=0x16, I=0x17, O=0x18, P=0x19
    A=0x1E, S=0x1F, D=0x20, F=0x21, G=0x22, H=0x23, J=0x24, K=0x25, L=0x26
    Z=0x2C, X=0x2D, C=0x2E, V=0x2F, B=0x30, N=0x31, M=0x32
    1=0x02, 2=0x03, 3=0x04, 4=0x05, 5=0x06, 6=0x07, 7=0x08, 8=0x09, 9=0x0A, 0=0x0B
    Enter=0x1C, Space=0x39, Backspace=0x0E

    명령: 위 매뉴얼을 바탕으로, 각 키에 대해 반드시 '스캔코드:아스키코드(16진수)' 형태로 한 줄에 하나씩 출력하세요.
    규칙: 
    - 알파벳은 소문자 아스키코드(예: a는 0x61)를 사용하세요.
    - Enter(0x1C)는 0x0A, Backspace(0x0E)는 0x08, Space(0x39)는 0x20으로 매핑하세요.
    - 부가 설명, 마크다운(```) 등은 절대 금지합니다. 오직 매핑 데이터만 출력하세요.

    출력 예시:
    0x1E:0x61
    0x30:0x62
    0x39:0x20
    """
    print(f"\n[AI Brain] 전체 키보드 배열을 예측 중입니다 (Gemini)...")
    try:
        response = client.models.generate_content(
            model=MODEL_ID,
            contents=prompt
        )
        print("-" * 50)
        print(f"[Gemini의 답변]\n{response.text}")
        print("-" * 50)

        # 1. 128바이트 배열을 모두 '?'(0x3F)로 안전하게 초기화합니다.
        payload_array = [0x3F] * 128
        
        # 2. AI가 응답한 '키:값' 쌍을 파싱하여 정확한 주소에 꽂아 넣습니다.
        lines = response.text.replace(' ', '').strip().split('\n')
        for line in lines:
            if ':' in line:
                try:
                    scan_str, ascii_str = line.split(':')
                    scancode = int(scan_str, 16)
                    ascii_val = int(ascii_str, 16)
                    if scancode < 128:
                        payload_array[scancode] = ascii_val
                except Exception:
                    continue # 파싱에 실패한 줄은 무시합니다.
                    
        payload = bytes(payload_array)
        print(f"[AI Brain] 딕셔너리 매핑 완료. 128바이트 드라이버 주입을 시작합니다...")
        
        # OS 폴링 주기를 고려하여 데이터 유실을 막기 위해 아주 약간의 딜레이를 주며 전송합니다.
        for b in payload:
            s.sendall(bytes([b]))
            time.sleep(0.005)
        print("[AI Brain] 드라이버 주입 완료! 이제 QEMU에서 자유롭게 타이핑하세요.")
        return True

    except Exception as e:
        print(f"[!] Gemini API 호출 중 에러 발생: {e}")
        print("[AI Brain] 드라이버 생성/주입 실패. 다음 입력 시 다시 시도합니다...")
        return False

def listen_and_think(s):
    """OS에서 오는 로그를 수신하고 AI가 파싱하는 뼈대 함수입니다."""
    buffer = b""
    is_driver_injected = False # 드라이버 주입 여부 상태
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
                print(f"[AI Brain] 감각 데이터 수신됨: {decoded_line}")
                
                # 첫 키보드 입력이 감지되면, 전체 드라이버를 생성하여 주입합니다.
                if not is_driver_injected:
                    # 성공적으로 주입되었을 때만 상태를 변경하여, 실패 시 끝없이 재시도(Trial & Error) 하도록 함
                    is_driver_injected = generate_and_inject_keymap(s)

            elif decoded_line:
                print(f"[OS System] {decoded_line}")

if __name__ == "__main__":
    conn = connect_to_umbilical_cord()
    try:
        listen_and_think(conn)
    except KeyboardInterrupt:
        print("\n[*] Host AI shutdown.")
        conn.close()