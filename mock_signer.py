import nacl.signing
import nacl.encoding
import os

seed = b"OpenRhiza-Nexus-Root-Key-0000000" # 32 bytes

signing_key = nacl.signing.SigningKey(seed)
verify_key = signing_key.verify_key

print("Public Key (Hex):")
public_key_hex = verify_key.encode(encoder=nacl.encoding.HexEncoder).decode('utf-8')
print(public_key_hex)
# Format as Rust array
rust_pk = ", ".join([f"0x{public_key_hex[i:i+2]}" for i in range(0, len(public_key_hex), 2)])
print(f"[{rust_pk}]")

wasm_path = "nexus_cache/8086_100E.wasm"
with open(wasm_path, "rb") as f:
    wasm_bytes = f.read()

signed = signing_key.sign(wasm_bytes)
signature_hex = signed.signature.hex()

print("\nSignature (Hex):")
print(signature_hex)
rust_sig = ", ".join([f"0x{signature_hex[i:i+2]}" for i in range(0, len(signature_hex), 2)])
print(f"[{rust_sig}]")
