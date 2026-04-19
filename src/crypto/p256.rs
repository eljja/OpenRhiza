// src/crypto/p256.rs
// Wrapper around the audited RustCrypto P-256 implementation for TLS ECDH.

use ::p256::elliptic_curve::ecdh::diffie_hellman;
use ::p256::{EncodedPoint, PublicKey, SecretKey};

/// ECDH public-key derivation.
/// Converts a 32-byte private key into an uncompressed 65-byte SEC1 public key (`04 || x || y`).
pub fn ecdh_public_key(private_key: &[u8; 32]) -> [u8; 65] {
    let secret = match SecretKey::from_slice(private_key) {
        Ok(secret) => secret,
        Err(_) => return [0u8; 65],
    };

    let public = secret.public_key();
    let encoded = EncodedPoint::from(public);

    let mut out = [0u8; 65];
    if encoded.len() == out.len() {
        out.copy_from_slice(encoded.as_bytes());
    }
    out
}

/// Compute the ECDH shared secret.
/// Takes `(my_private_key, peer_public_key)` and returns the 32-byte x-coordinate secret.
pub fn ecdh_shared_secret(private_key: &[u8; 32], peer_public: &[u8]) -> Option<[u8; 32]> {
    let secret = SecretKey::from_slice(private_key).ok()?;
    let public = PublicKey::from_sec1_bytes(peer_public).ok()?;
    let shared = diffie_hellman(secret.to_nonzero_scalar(), public.as_affine());

    let mut out = [0u8; 32];
    out.copy_from_slice(shared.raw_secret_bytes().as_ref());
    Some(out)
}
