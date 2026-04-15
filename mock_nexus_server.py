import socket
import os
import ssl
from cryptography import x509
from cryptography.x509.oid import NameOID
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives import serialization
import datetime

HOST = '127.0.0.1' 
PORT = 4443

def generate_self_signed_cert(cert_path="nexus_cert.pem", key_path="nexus_key.pem"):
    if os.path.exists(cert_path) and os.path.exists(key_path):
        print("[Nexus] Using existing certificate.")
        return

    print("[Nexus] Generating new secp256r1 self-signed certificate for TLS 1.3 testing...")
    # Generate secp256r1 private key
    private_key = ec.generate_private_key(ec.SECP256R1())

    # Generate certificate
    subject = issuer = x509.Name([
        x509.NameAttribute(NameOID.COMMON_NAME, u"openrhiza.com"),
    ])
    cert = x509.CertificateBuilder().subject_name(
        subject
    ).issuer_name(
        issuer
    ).public_key(
        private_key.public_key()
    ).serial_number(
        x509.random_serial_number()
    ).not_valid_before(
        datetime.datetime.utcnow()
    ).not_valid_after(
        # Valid for 30 days
        datetime.datetime.utcnow() + datetime.timedelta(days=30)
    ).sign(private_key, hashes.SHA256())

    # Write private key
    with open(key_path, "wb") as f:
        f.write(private_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.TraditionalOpenSSL,
            encryption_algorithm=serialization.NoEncryption(),
        ))

    # Write certificate
    with open(cert_path, "wb") as f:
        f.write(cert.public_bytes(serialization.Encoding.PEM))

    print(f"[Nexus] Certificate generated at {cert_path} and {key_path}")

def start_server():
    generate_self_signed_cert()

    # Note: QEMU 10.0.2.2 forwards to host 127.0.0.1
    # For user-mode networking, QEMU acts as a gateway and translates 10.0.2.2 to the Host PC loopback.
    
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain("nexus_cert.pem", "nexus_key.pem")
    # Restrict to TLS 1.3
    context.options |= ssl.OP_NO_TLSv1 | ssl.OP_NO_TLSv1_1 | ssl.OP_NO_TLSv1_2
    
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind((HOST, PORT))
        s.listen()
        print(f"Mock Nexus Server listening on {HOST}:{PORT} (TLS 1.3)")
        
        with context.wrap_socket(s, server_side=True) as tls_s:
            while True:
                try:
                    conn, addr = tls_s.accept()
                except ssl.SSLError as e:
                    print(f"[Nexus] TLS Error: {e}")
                    continue

                with conn:
                    print(f"[Nexus] TLS Connected by {addr}")
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
