// src/crypto/aes.rs
// AES-128 순수 소프트웨어 구현 (FIPS 197)
// AES-NI/SIMD 없이 룩업 테이블 기반 스칼라 연산

// S-Box (SubBytes 변환 테이블)
const SBOX: [u8; 256] = [
    0x63,0x7c,0x77,0x7b,0xf2,0x6b,0x6f,0xc5,0x30,0x01,0x67,0x2b,0xfe,0xd7,0xab,0x76,
    0xca,0x82,0xc9,0x7d,0xfa,0x59,0x47,0xf0,0xad,0xd4,0xa2,0xaf,0x9c,0xa4,0x72,0xc0,
    0xb7,0xfd,0x93,0x26,0x36,0x3f,0xf7,0xcc,0x34,0xa5,0xe5,0xf1,0x71,0xd8,0x31,0x15,
    0x04,0xc7,0x23,0xc3,0x18,0x96,0x05,0x9a,0x07,0x12,0x80,0xe2,0xeb,0x27,0xb2,0x75,
    0x09,0x83,0x2c,0x1a,0x1b,0x6e,0x5a,0xa0,0x52,0x3b,0xd6,0xb3,0x29,0xe3,0x2f,0x84,
    0x53,0xd1,0x00,0xed,0x20,0xfc,0xb1,0x5b,0x6a,0xcb,0xbe,0x39,0x4a,0x4c,0x58,0xcf,
    0xd0,0xef,0xaa,0xfb,0x43,0x4d,0x33,0x85,0x45,0xf9,0x02,0x7f,0x50,0x3c,0x9f,0xa8,
    0x51,0xa3,0x40,0x8f,0x92,0x9d,0x38,0xf5,0xbc,0xb6,0xda,0x21,0x10,0xff,0xf3,0xd2,
    0xcd,0x0c,0x13,0xec,0x5f,0x97,0x44,0x17,0xc4,0xa7,0x7e,0x3d,0x64,0x5d,0x19,0x73,
    0x60,0x81,0x4f,0xdc,0x22,0x2a,0x90,0x88,0x46,0xee,0xb8,0x14,0xde,0x5e,0x0b,0xdb,
    0xe0,0x32,0x3a,0x0a,0x49,0x06,0x24,0x5c,0xc2,0xd3,0xac,0x62,0x91,0x95,0xe4,0x79,
    0xe7,0xc8,0x37,0x6d,0x8d,0xd5,0x4e,0xa9,0x6c,0x56,0xf4,0xea,0x65,0x7a,0xae,0x08,
    0xba,0x78,0x25,0x2e,0x1c,0xa6,0xb4,0xc6,0xe8,0xdd,0x74,0x1f,0x4b,0xbd,0x8b,0x8a,
    0x70,0x3e,0xb5,0x66,0x48,0x03,0xf6,0x0e,0x61,0x35,0x57,0xb9,0x86,0xc1,0x1d,0x9e,
    0xe1,0xf8,0x98,0x11,0x69,0xd9,0x8e,0x94,0x9b,0x1e,0x87,0xe9,0xce,0x55,0x28,0xdf,
    0x8c,0xa1,0x89,0x0d,0xbf,0xe6,0x42,0x68,0x41,0x99,0x2d,0x0f,0xb0,0x54,0xbb,0x16,
];

// Round Constants
const RCON: [u8; 10] = [0x01,0x02,0x04,0x08,0x10,0x20,0x40,0x80,0x1b,0x36];

/// AES-128 키 확장: 16바이트 키 → 11개 라운드 키 (176바이트)
pub fn key_expansion(key: &[u8; 16]) -> [u8; 176] {
    let mut w = [0u8; 176];
    w[..16].copy_from_slice(key);

    for i in 1..11 {
        let prev = i * 16 - 16;
        let curr = i * 16;

        // RotWord + SubWord + Rcon
        let mut temp = [
            SBOX[w[prev + 13] as usize] ^ RCON[i - 1],
            SBOX[w[prev + 14] as usize],
            SBOX[w[prev + 15] as usize],
            SBOX[w[prev + 12] as usize],
        ];

        for j in 0..4 {
            w[curr + j] = w[prev + j] ^ temp[j];
        }
        for j in 4..16 {
            w[curr + j] = w[prev + j] ^ w[curr + j - 4];
        }
    }
    w
}

/// AES-128 단일 블록 암호화 (16바이트 입력 → 16바이트 출력)
pub fn encrypt_block(block: &[u8; 16], round_keys: &[u8; 176]) -> [u8; 16] {
    let mut state = *block;

    // Initial AddRoundKey
    xor_block(&mut state, &round_keys[0..16]);

    // Rounds 1-9
    for round in 1..10 {
        sub_bytes(&mut state);
        shift_rows(&mut state);
        mix_columns(&mut state);
        xor_block(&mut state, &round_keys[round * 16..(round + 1) * 16]);
    }

    // Round 10 (no MixColumns)
    sub_bytes(&mut state);
    shift_rows(&mut state);
    xor_block(&mut state, &round_keys[160..176]);

    state
}

#[inline]
fn xor_block(state: &mut [u8; 16], key: &[u8]) {
    for i in 0..16 { state[i] ^= key[i]; }
}

fn sub_bytes(state: &mut [u8; 16]) {
    for i in 0..16 { state[i] = SBOX[state[i] as usize]; }
}

fn shift_rows(state: &mut [u8; 16]) {
    // Row 0: no shift
    // Row 1: shift left 1 (indices 1,5,9,13)
    let t = state[1];
    state[1] = state[5]; state[5] = state[9]; state[9] = state[13]; state[13] = t;
    // Row 2: shift left 2 (indices 2,6,10,14)
    let (t0, t1) = (state[2], state[6]);
    state[2] = state[10]; state[6] = state[14]; state[10] = t0; state[14] = t1;
    // Row 3: shift left 3 = shift right 1 (indices 3,7,11,15)
    let t = state[15];
    state[15] = state[11]; state[11] = state[7]; state[7] = state[3]; state[3] = t;
}

fn mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let i = c * 4;
        let (s0, s1, s2, s3) = (state[i], state[i+1], state[i+2], state[i+3]);
        state[i]   = gf_mul2(s0) ^ gf_mul3(s1) ^ s2 ^ s3;
        state[i+1] = s0 ^ gf_mul2(s1) ^ gf_mul3(s2) ^ s3;
        state[i+2] = s0 ^ s1 ^ gf_mul2(s2) ^ gf_mul3(s3);
        state[i+3] = gf_mul3(s0) ^ s1 ^ s2 ^ gf_mul2(s3);
    }
}

#[inline]
fn gf_mul2(x: u8) -> u8 {
    let mut r = (x as u16) << 1;
    if r & 0x100 != 0 { r ^= 0x11b; } // x^8 + x^4 + x^3 + x + 1
    r as u8
}

#[inline]
fn gf_mul3(x: u8) -> u8 { gf_mul2(x) ^ x }

// ========================================================================
// AES-128-GCM (Galois/Counter Mode)
// ========================================================================

/// GF(2^128) 곱셈 (GHASH용, 비트 단위 순수 구현)
fn gf128_mul(x: &[u8; 16], y: &[u8; 16]) -> [u8; 16] {
    let mut z = [0u8; 16];
    let mut v = *y;

    for i in 0..128 {
        let byte_idx = i / 8;
        let bit_idx = 7 - (i % 8);

        if (x[byte_idx] >> bit_idx) & 1 == 1 {
            for j in 0..16 { z[j] ^= v[j]; }
        }

        // v = v >> 1 in GF(2^128), with reduction polynomial
        let carry = v[15] & 1;
        for j in (1..16).rev() {
            v[j] = (v[j] >> 1) | (v[j-1] << 7);
        }
        v[0] >>= 1;
        if carry == 1 {
            v[0] ^= 0xe1; // Reduction: x^128 + x^7 + x^2 + x + 1
        }
    }
    z
}

/// GHASH: GCM의 인증 태그 생성용 해시
fn ghash(h: &[u8; 16], aad: &[u8], ciphertext: &[u8]) -> [u8; 16] {
    let mut tag = [0u8; 16];

    // Process AAD (Additional Authenticated Data)
    let mut offset = 0;
    while offset < aad.len() {
        let mut block = [0u8; 16];
        let end = core::cmp::min(offset + 16, aad.len());
        block[..end - offset].copy_from_slice(&aad[offset..end]);
        for i in 0..16 { tag[i] ^= block[i]; }
        tag = gf128_mul(&tag, h);
        offset += 16;
    }

    // Process Ciphertext
    offset = 0;
    while offset < ciphertext.len() {
        let mut block = [0u8; 16];
        let end = core::cmp::min(offset + 16, ciphertext.len());
        block[..end - offset].copy_from_slice(&ciphertext[offset..end]);
        for i in 0..16 { tag[i] ^= block[i]; }
        tag = gf128_mul(&tag, h);
        offset += 16;
    }

    // Length block: [AAD length in bits (64-bit BE)][Ciphertext length in bits (64-bit BE)]
    let mut len_block = [0u8; 16];
    let aad_bits = (aad.len() as u64) * 8;
    let ct_bits = (ciphertext.len() as u64) * 8;
    len_block[..8].copy_from_slice(&aad_bits.to_be_bytes());
    len_block[8..].copy_from_slice(&ct_bits.to_be_bytes());
    for i in 0..16 { tag[i] ^= len_block[i]; }
    tag = gf128_mul(&tag, h);

    tag
}

/// CTR 모드에서 카운터 블록 증가 (마지막 4바이트만 증가)
fn inc32(counter: &mut [u8; 16]) {
    for i in (12..16).rev() {
        counter[i] = counter[i].wrapping_add(1);
        if counter[i] != 0 { break; }
    }
}

/// AES-128-GCM 암호화
/// - key: 16바이트 AES 키
/// - iv: 12바이트 초기화 벡터 (nonce)
/// - aad: 추가 인증 데이터
/// - plaintext: 평문
/// 반환: (ciphertext, 16바이트 authentication tag)
pub fn aes_gcm_encrypt(
    key: &[u8; 16], iv: &[u8; 12], aad: &[u8], plaintext: &[u8]
) -> (alloc::vec::Vec<u8>, [u8; 16]) {
    let round_keys = key_expansion(key);

    // H = AES(K, 0^128) — GHASH 서브키
    let h = encrypt_block(&[0u8; 16], &round_keys);

    // J0 = IV || 0x00000001 (96비트 IV인 경우)
    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(iv);
    j0[15] = 1;

    // CTR 모드 암호화 (J0+1 부터 시작)
    let mut counter = j0;
    let mut ciphertext = alloc::vec![0u8; plaintext.len()];

    let mut offset = 0;
    while offset < plaintext.len() {
        inc32(&mut counter);
        let keystream = encrypt_block(&counter, &round_keys);
        let end = core::cmp::min(offset + 16, plaintext.len());
        for i in offset..end {
            ciphertext[i] = plaintext[i] ^ keystream[i - offset];
        }
        offset += 16;
    }

    // GHASH → 인증 태그
    let ghash_result = ghash(&h, aad, &ciphertext);

    // Tag = GHASH XOR AES(K, J0)
    let encrypted_j0 = encrypt_block(&j0, &round_keys);
    let mut tag = [0u8; 16];
    for i in 0..16 { tag[i] = ghash_result[i] ^ encrypted_j0[i]; }

    (ciphertext, tag)
}

/// AES-128-GCM 복호화 + 인증 검증
/// 태그 불일치 시 None 반환 (위조 감지)
pub fn aes_gcm_decrypt(
    key: &[u8; 16], iv: &[u8; 12], aad: &[u8], ciphertext: &[u8], tag: &[u8; 16]
) -> Option<alloc::vec::Vec<u8>> {
    let round_keys = key_expansion(key);
    let h = encrypt_block(&[0u8; 16], &round_keys);

    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(iv);
    j0[15] = 1;

    // 태그 검증
    let ghash_result = ghash(&h, aad, ciphertext);
    let encrypted_j0 = encrypt_block(&j0, &round_keys);
    let mut computed_tag = [0u8; 16];
    for i in 0..16 { computed_tag[i] = ghash_result[i] ^ encrypted_j0[i]; }

    // 상수 시간 비교 (타이밍 공격 방지)
    let mut diff = 0u8;
    for i in 0..16 { diff |= computed_tag[i] ^ tag[i]; }
    if diff != 0 { return None; }

    // CTR 모드 복호화
    let mut counter = j0;
    let mut plaintext = alloc::vec![0u8; ciphertext.len()];
    let mut offset = 0;
    while offset < ciphertext.len() {
        inc32(&mut counter);
        let keystream = encrypt_block(&counter, &round_keys);
        let end = core::cmp::min(offset + 16, ciphertext.len());
        for i in offset..end {
            plaintext[i] = ciphertext[i] ^ keystream[i - offset];
        }
        offset += 16;
    }

    Some(plaintext)
}
