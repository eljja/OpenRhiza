import socket
import os

HOST = '127.0.0.1' 
PORT = 4443

def start_server():
    # Note: QEMU 10.0.2.2 forwards to host 127.0.0.1
    # For user-mode networking, QEMU acts as a gateway and translates 10.0.2.2 to the Host PC loopback.
    
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind((HOST, PORT))
        s.listen()
        print(f"Mock Nexus Server listening on {HOST}:{PORT}")
        
        while True:
            conn, addr = s.accept()
            with conn:
                print(f"[Nexus] Connected by {addr}")
                data = conn.recv(1024)
                if not data:
                    continue
                
                request = data.decode('utf-8', errors='ignore')
                print(f"[Nexus] Request:\n{request.strip()}")
                
                # Check if it's the expected driver
                if "GET /api/nexus/0x0C_0x03.wasm" in request:
                    # Serve the e1000 Wasm payload as a mock
                    cache_path = "nexus_cache/8086_100E.wasm"
                    if os.path.exists(cache_path):
                        with open(cache_path, "rb") as f:
                            wasm_content = f.read()
                            
                        response_headers = (
                            "HTTP/1.1 200 OK\r\n"
                            "Content-Type: application/wasm\r\n"
                            f"Content-Length: {len(wasm_content)}\r\n"
                            "X-Nexus-Signature: aee36161d01d14950fe89413819c54172ffb016f35eadbec5fee021371cb8b754f64712b38f0e1e128c26b1a1b02d4f9cb7f64d8dd61c13053a1f2a720ebfd0b\r\n"
                            "Connection: close\r\n"
                            "\r\n"
                        )
                        conn.sendall(response_headers.encode('utf-8'))
                        conn.sendall(wasm_content)
                        print(f"[Nexus] Sent {len(wasm_content)} bytes of Wasm payload.")
                    else:
                        print("[Nexus] Cache file not found. Have you successfully run host_brain.py first?")
                        conn.sendall(b"HTTP/1.1 404 Not Found\r\n\r\n")
                else:
                    conn.sendall(b"HTTP/1.1 404 Not Found\r\n\r\n")

if __name__ == "__main__":
    start_server()
