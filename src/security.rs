use ed25519_compact::{PublicKey, Signature};

// The Master Root of Trust Public Key for OpenRhiza Nexus (Deterministic from our seed)
const NEXUS_PUBLIC_KEY: [u8; 32] = [
    0x1a, 0xb2, 0xa2, 0x01, 0x7a, 0x2f, 0x2b, 0xd0, 
    0x58, 0x38, 0x51, 0xcd, 0x55, 0x15, 0xe8, 0xc8, 
    0x3d, 0xcd, 0x94, 0x60, 0x80, 0x72, 0xbf, 0xfb, 
    0x7a, 0xba, 0xad, 0xfd, 0x8e, 0x87, 0xbf, 0x24
];

pub fn verify_nexus_signature(wasm_payload: &[u8], signature_bytes: &[u8; 64]) -> bool {
    let pk = match PublicKey::from_slice(&NEXUS_PUBLIC_KEY) {
        Ok(pk) => pk,
        Err(_) => return false,
    };
    
    let sig = match Signature::from_slice(signature_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };
    
    match pk.verify(wasm_payload, &sig) {
        Ok(_) => true,
        Err(_) => false,
    }
}
