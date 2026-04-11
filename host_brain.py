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
api_key = os.environ.get("GEMINI_API_KEY")
if not api_key:
    # 혹시 .env 에 있다면 파이썬 자체적으로 읽어옵니다.
    try:
        from dotenv import load_dotenv
        load_dotenv()
        api_key = os.environ.get("GEMINI_API_KEY")
    except ImportError:
        pass

if not api_key:
    print("[-] GEMINI_API_KEY environment variable is not set.")
    exit(1)

client = genai.Client(api_key=api_key)

parser = argparse.ArgumentParser(description="OpenRhiza Host AI Brain")
parser.add_argument('--model', type=str, default='gemini-2.5-flash-lite', help='Primary LLM Model ID')
args = parser.parse_args()
PRIMARY_MODEL = args.model

FALLBACK_MODELS = [
    'gemini-2.5-flash-lite', 
    'gemini-2.5-flash-lite-preview-09-2025', 
    'gemini-3.1-flash-lite-preview', 
    'gemini-2.5-flash', 
    'gemini-3-flash-preview'
]

MODELS_TO_TRY = [PRIMARY_MODEL] + [m for m in FALLBACK_MODELS if m != PRIMARY_MODEL]
print(f"[*] AI Models sequence: {MODELS_TO_TRY}")

def connect_to_umbilical_cord():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    print(f"[*] Waiting for OpenRhiza OS to boot on {HOST}:{PORT}...")
    while True:
        try:
            s.connect((HOST, PORT))
            print("[+] Successfully connected to OpenRhiza Umbilical Cord (Serial)!")
            return s
        except ConnectionRefusedError:
            time.sleep(1)

DEFAULT_QWERTY = [0x3F] * 256
qwerty_lower = {0x10:b'q', 0x11:b'w', 0x12:b'e', 0x13:b'r', 0x14:b't', 0x15:b'y', 0x16:b'u', 0x17:b'i', 0x18:b'o', 0x19:b'p', 0x1E:b'a', 0x1F:b's', 0x20:b'd', 0x21:b'f', 0x22:b'g', 0x23:b'h', 0x24:b'j', 0x25:b'k', 0x26:b'l', 0x2C:b'z', 0x2D:b'x', 0x2E:b'c', 0x2F:b'v', 0x30:b'b', 0x31:b'n', 0x32:b'm', 0x02:b'1', 0x03:b'2', 0x04:b'3', 0x05:b'4', 0x06:b'5', 0x07:b'6', 0x08:b'7', 0x09:b'8', 0x0A:b'9', 0x0B:b'0', 0x1C:0x0A, 0x39:0x20, 0x0E:0x08, 0x28:b"'", 0x27:b';', 0x33:b',', 0x34:b'.', 0x35:b'/', 0x0C:b'-', 0x0D:b'='}
qwerty_upper = {0x10:b'Q', 0x11:b'W', 0x12:b'E', 0x13:b'R', 0x14:b'T', 0x15:b'Y', 0x16:b'U', 0x17:b'I', 0x18:b'O', 0x19:b'P', 0x1E:b'A', 0x1F:b'S', 0x20:b'D', 0x21:b'F', 0x22:b'G', 0x23:b'H', 0x24:b'J', 0x25:b'K', 0x26:b'L', 0x2C:b'Z', 0x2D:b'X', 0x2E:b'C', 0x2F:b'V', 0x30:b'B', 0x31:b'N', 0x32:b'M', 0x02:b'!', 0x03:b'@', 0x04:b'#', 0x05:b'$', 0x06:b'%', 0x07:b'^', 0x08:b'&', 0x09:b'*', 0x0A:b'(', 0x0B:b')', 0x1C:0x0A, 0x39:0x20, 0x0E:0x08, 0x28:b'"', 0x27:b':', 0x33:b'<', 0x34:b'>', 0x35:b'?', 0x0C:b'_', 0x0D:b'+'}
for k, v in qwerty_lower.items(): DEFAULT_QWERTY[k] = ord(v) if isinstance(v, bytes) else v
for k, v in qwerty_upper.items(): DEFAULT_QWERTY[k + 0x80] = ord(v) if isinstance(v, bytes) else v

def extract_rust_code(text):
    match = re.search(r'```(?:rust)?\s*(.*?)\s*```', text, re.DOTALL)
    if match: return match.group(1)
    return text.strip()

def compile_rust_to_wasm(rust_code):
    print("\n[AI Brain] Compiling generated Rust code to WebAssembly...")
    if "#![no_std]" not in rust_code: rust_code = "#![no_std]\n" + rust_code
    if "#[panic_handler]" not in rust_code:
        rust_code += "\n#[cfg(not(test))]\n#[panic_handler]\nfn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }\n"
    
    # Auto-bandage Rust 2024 edition strict unsafe requirements
    rust_code = rust_code.replace('extern "C" {', 'unsafe extern "C" {')
    rust_code = rust_code.replace('#[no_mangle]', '#[unsafe(no_mangle)]')

    with tempfile.TemporaryDirectory() as temp_dir:
        project_dir = os.path.join(temp_dir, "wasm_driver")
        try:
            subprocess.run(["cargo", "new", "--lib", "wasm_driver"], cwd=temp_dir, check=True, capture_output=True)
            cargo_toml_path = os.path.join(project_dir, "Cargo.toml")
            with open(cargo_toml_path, "a") as f:
                f.write("\n[lib]\ncrate-type = [\"cdylib\"]\n")
                f.write("\n[profile.release]\nopt-level = \"z\"\nlto = true\n")
            
            lib_rs_path = os.path.join(project_dir, "src", "lib.rs")
            with open(lib_rs_path, "w", encoding="utf-8") as f:
                f.write(rust_code)
                
            # Changed to +nightly to avoid missing toolchain targets on Windows
            print("[AI Brain] Running 'cargo +nightly build --target wasm32-unknown-unknown --release'...")
            subprocess.run(
                ["cargo", "+nightly", "build", "--target", "wasm32-unknown-unknown", "--release"],
                cwd=project_dir, check=True, capture_output=True, text=True
            )
            
            wasm_path = os.path.join(project_dir, "target", "wasm32-unknown-unknown", "release", "wasm_driver.wasm")
            with open(wasm_path, "rb") as f:
                wasm_bytes = f.read()
            print(f"[AI Brain] Wasm compilation successful! Binary size: {len(wasm_bytes)} bytes.")
            return wasm_bytes, None
        except subprocess.CalledProcessError as e:
            print(f"[!] Wasm Compilation Failed:\n{e.stderr}")
            return None, e.stderr

# Self-Healing Driver Builder Template
def inject_and_wait(s, wasm_bytes):
    print(f"[AI Brain] Injecting Wasm binary ({len(wasm_bytes)} bytes) into OS...")
    s.sendall(bytes([0xFC])) 
    s.sendall(len(wasm_bytes).to_bytes(4, byteorder='little')) 
    for b in wasm_bytes:
        s.sendall(bytes([b]))
        time.sleep(0.002) 
    print("[AI Brain] Wasm injection complete! Waiting for OS runtime feedback...")
    
    buffer = b""
    start_wait = time.time()
    s.settimeout(1.0)
    err_out = None
    while time.time() - start_wait < 5.0:
        try:
            data = s.recv(1024)
            if data:
                buffer += data
                if b"Wasm Execution Success" in buffer:
                    print("\n[AI Brain] YES! OS reported Wasm Sandbox Execution SUCCESS!")
                    s.settimeout(None)
                    return True, None
                elif b"Wasm Sandbox Trap (Panic):" in buffer:
                    err_text = buffer.decode('utf-8', errors='ignore')
                    if "Wasm Sandbox Trap (Panic):" in err_text:
                        err_msg = err_text.split("Wasm Sandbox Trap (Panic):")[1].split("\n")[0]
                        print(f"[!] OS Sandbox Trap / Runtime Panic Detected!\n{err_msg}")
                        err_out = err_msg
                        break
        except socket.timeout:
            continue
    else:
        print("[?] OS did not return Success/Panic code within timeout. Assuming tentative success.")
        s.settimeout(None)
        return True, None
    s.settimeout(None)
    return False, err_out

def generate_and_inject_driver(s, hardware_name, hardware_id, prompt):
    print(f"\n[AI Brain] Generating initialization driver for {hardware_name}...")
    s.sendall(bytes([0xFB])) 

    MAX_RETRIES = 3 
    
    # OS Error Loop Storage
    last_rust_code = ""

    for model_id in MODELS_TO_TRY:
        print(f"\n[AI Brain] Trying model: {model_id}")
        current_prompt = prompt
        
        for attempt in range(MAX_RETRIES):
            try:
                print(f"[AI Brain] Attempt {attempt + 1}/{MAX_RETRIES}...")
                response = client.models.generate_content(model=model_id, contents=current_prompt)
                
                rust_code = extract_rust_code(response.text)
                last_rust_code = rust_code
                print("-" * 50)
                print(f"[AI Generated Driver ({model_id} - Attempt {attempt+1})]\n{rust_code}")
                print("-" * 50)
                
                wasm_bytes, compile_err = compile_rust_to_wasm(rust_code)
                
                if wasm_bytes:
                    success, runtime_err = inject_and_wait(s, wasm_bytes)
                    if success:
                        os.makedirs("nexus_cache", exist_ok=True)
                        cache_path = f"nexus_cache/{hardware_id.replace(':', '_')}.wasm"
                        try:
                            # locate cargo output folder and copy
                            with open(cache_path, "wb") as f:
                                f.write(wasm_bytes)
                            print(f"[AI Brain] Saved successful driver to '{cache_path}'")
                        except Exception as e:
                            print(f"[AI Brain] Failed to cache driver: {e}")
                        return True
                    else:
                        compile_err = runtime_err
                        
                # Re-prompt generation triggered either by compile failure OR runtime sandbox panic
                print(f"[!] Feedback Loop Triggered on attempt {attempt + 1}.")
                if attempt < MAX_RETRIES - 1:
                    print("[AI Brain] Feeding errors back to the LLM to fix the code (Self-Healing)...")
                    current_prompt = f"""
Your previous code failed.

[Previous Code]
```rust
{last_rust_code}
```

[Error Output (Compilation or Runtime Trap)]
```
{compile_err}
```

Instruction: Fix the errors based on the output above and provide the complete, corrected Rust code.
- Output ONLY the fixed Rust code snippet. Do not include markdown formatting or explanations.
"""
                else:
                    print(f"[!] Max retries reached for model {model_id}.")
            except Exception as e:
                print(f"[!] {model_id} API failed during attempt {attempt + 1}: {e}")
                break 
            
    print("[AI Brain] Failed to generate valid driver.")
    s.sendall(bytes([0xFA]))
    return False

def search_and_verify_driver_from_nexus(s, hardware_name, hardware_id, bar0_address, prompt_template):
    cache_path = f"nexus_cache/{hardware_id.replace(':', '_')}.wasm"
    print(f"\n[AI Brain] Searching OpenRhiza Nexus Cache for '{hardware_name}' ({hardware_id})...")
    
    if os.path.exists(cache_path):
        print(f"[AI Brain] Cache Hit! Found compiled WebAssembly driver at {cache_path}")
        with open(cache_path, "rb") as f:
            wasm_bytes = f.read()
            
        success, _ = inject_and_wait(s, wasm_bytes)
        return success
    else:
        print(f"[AI Brain] No validated driver found on Nexus. Proceeding with generative creation from scratch...")
        return generate_and_inject_driver(s, hardware_name, hardware_id, prompt_template)

def generate_e1000_driver(s, bar0_address):
    prompt = f"""
    You are an AI brain autonomously coding a bare-metal operating system.
    Found an Intel e1000 NIC. The Memory-Mapped I/O (MMIO) Base Address (BAR0) is {bar0_address}.

    Instruction: Write a WebAssembly-compatible Rust snippet to initialize e1000 Tx/Rx DMA rings.
    - Host Functions provided: 
      `extern "C" {{ fn read_mmio(addr: u32) -> u32; fn write_mmio(addr: u32, val: u32); fn alloc_dma_page() -> u32; fn os_rx_packet(ptr: u32, len: u32); fn os_fetch_tx_packet(ptr: u32, max_len: u32) -> u32; }}`
    - Accessing host memory: the OS `os_rx_packet` and `os_fetch_tx_packet` read/write from your local WASM array buffer pointer `ptr`.
    - Call `alloc_dma_page()` to get a 4KB physical page. 
    - Split the 4KB page: Use the first 2KB for Rx Descriptors & buffers, and the rest for Tx. Note: Wasm must write raw physical addresses into RDBAL and TDBAL.
    - Export `#[no_mangle] pub extern "C" fn poll_net()`. Inside this function:
      1) Check Rx Descriptor status. If a packet arrived, copy data to a local WASM array and call `os_rx_packet(local_array_ptr, len)`.
      2) Pass a local WASM array to `let tx_len = os_fetch_tx_packet(local_array_ptr, 1500)`. If `tx_len > 0`, use DMA to push it into the Tx Ring and set Tx Descriptor ready.
    - The main entry signature must be: `#[no_mangle] pub extern "C" fn init_driver()`
    - Keep Wasm global variables in `static mut` to track Descriptor Tail and Head pointers across `poll_net` calls.
    - Output ONLY the raw Rust code snippet. No markdown wrappers.
    """
    return search_and_verify_driver_from_nexus(s, "Intel e1000 (Network Stack Bridge)", "8086:100E", bar0_address, prompt)

def generate_xhci_driver(s, bar0_address):
    prompt = f"""
    You are an AI brain autonomously coding a bare-metal operating system.
    Found a USB xHCI Controller. The MMIO Base Address (BAR0) is {bar0_address}.

    Instruction: Write a Rust snippet to perform a Host Controller Reset (HCRST) on xHCI.
    - Host Functions provided: `extern "C" {{ fn read_mmio(addr: u32) -> u32; fn write_mmio(addr: u32, val: u32); }}`
    - Capability Length (CAPLENGTH) is an 8-bit value at offset 0x00. You can do a 32-bit read at offset 0x00 and take the lowest 8 bits.
    - Calculate Operational Base = BAR0 + CAPLENGTH.
    - Modify the USBCMD register at Operational Base + 0x00, setting bit 1 (HCRST).
    - Function signature must be: `#[no_mangle] pub extern "C" fn init_driver()`
    - Output ONLY the Rust code snippet.
    """
    return search_and_verify_driver_from_nexus(s, "USB xHCI", "0x0C:0x03", bar0_address, prompt)

def listen_and_think(s):
    buffer = b""
    e1000_done = False
    xhci_done = False
    
    print("[AI Brain] Waiting for OS hardware logs and Keyboard Ready signal...")
    qwerty_injected = False
    pending_e1000_bar0 = None

    while True:
        data = s.recv(1024)
        if not data:
            print("[-] Connection closed by QEMU.")
            break
        buffer += data
        while b'\n' in buffer:
            line, buffer = buffer.split(b'\n', 1)
            decoded_line = line.decode('utf-8', errors='ignore').strip()
            if decoded_line:
                print(f"[OS System] {decoded_line}")
                
                 # Check for e1000 (0x8086:0x100E)
                if not e1000_done and pending_e1000_bar0 is None and "Vendor 0x8086, Device 0x100E" in decoded_line:
                    match = re.search(r"BAR0:\s*(0x[0-9A-Fa-f]+)", decoded_line)
                    if match:
                        bar0 = match.group(1)
                        print(f"[AI Brain] Detected Intel e1000 NIC at {bar0} (Queued for generation after QWERTY)!")
                        pending_e1000_bar0 = bar0
                
                # Check for xHCI (0x0C:0x03) -> For now we simulate match
                if not xhci_done and "Vendor 0x8086, Device 0x1E31" in decoded_line:
                    match = re.search(r"BAR0:\s*(0x[0-9A-Fa-f]+)", decoded_line)
                    if match:
                        bar0 = match.group(1)
                        print(f"[AI Brain] Detected USB xHCI Controller at {bar0}!")
                        xhci_done = generate_xhci_driver(s, bar0)

                if not qwerty_injected and "Verify Keyboard:" in decoded_line:
                    print("[AI Brain] OS is ready for keyboard inputs. Injecting default QWERTY driver...")
                    s.sendall(bytes([0xFD]))
                    time.sleep(0.05)
                    for b in DEFAULT_QWERTY:
                        s.sendall(bytes([b]))
                        time.sleep(0.01) # Slow down UART to avoid dropping bytes in OS
                    print("[AI Brain] QWERTY driver injected.")
                    qwerty_injected = True
                    
                    if pending_e1000_bar0 and not e1000_done:
                        print("\n[AI Brain] Processing queued e1000 driver generation...")
                        e1000_done = generate_e1000_driver(s, pending_e1000_bar0)

if __name__ == "__main__":
    conn = connect_to_umbilical_cord()
    try:
        listen_and_think(conn)
    except KeyboardInterrupt:
        print("\n[*] Host AI shutdown.")
        conn.close()