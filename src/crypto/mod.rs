// src/crypto/mod.rs
// 베어메탈 암호화 프리미티브 (순수 소프트웨어, SIMD 없음)
// LLVM이 x86_64-unknown-none에서 SIMD 벡터 연산을 처리하지 못하므로
// 모든 암호화를 순수 Rust 스칼라 연산으로 직접 구현합니다.

pub mod sha256;
pub mod aes;
pub mod bignum;
pub mod p256;
pub mod random;
