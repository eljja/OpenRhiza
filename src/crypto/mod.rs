// src/crypto/mod.rs
// Bare-metal cryptographic primitives (pure software, no SIMD)
// LLVM support for SIMD on x86_64-unknown-none is limited here,
// so all cryptography is implemented directly with scalar Rust code.

pub mod sha256;
pub mod aes;
pub mod bignum;
pub mod p256;
pub mod random;
