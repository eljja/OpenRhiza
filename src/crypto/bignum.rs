// src/crypto/bignum.rs
// 256비트 정수 연산 (P256 ECDH에 필요)
// 순수 소프트웨어, SIMD 없음

/// 256비트 부호 없는 정수 (리틀엔디언 u64 x4)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct U256(pub [u64; 4]); // [0]=low, [3]=high

impl U256 {
    pub const ZERO: U256 = U256([0, 0, 0, 0]);
    pub const ONE: U256 = U256([1, 0, 0, 0]);

    /// 빅엔디언 32바이트 배열에서 생성
    pub fn from_be_bytes(bytes: &[u8; 32]) -> Self {
        let mut limbs = [0u64; 4];
        for i in 0..4 {
            let offset = (3 - i) * 8;
            limbs[i] = u64::from_be_bytes([
                bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3],
                bytes[offset+4], bytes[offset+5], bytes[offset+6], bytes[offset+7],
            ]);
        }
        U256(limbs)
    }

    /// 빅엔디언 32바이트 배열로 변환
    pub fn to_be_bytes(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..4 {
            let offset = (3 - i) * 8;
            out[offset..offset+8].copy_from_slice(&self.0[i].to_be_bytes());
        }
        out
    }

    /// self가 0인지 확인
    pub fn is_zero(&self) -> bool {
        self.0[0] | self.0[1] | self.0[2] | self.0[3] == 0
    }

    /// self >= other
    pub fn gte(&self, other: &Self) -> bool {
        for i in (0..4).rev() {
            if self.0[i] > other.0[i] { return true; }
            if self.0[i] < other.0[i] { return false; }
        }
        true // equal
    }

    /// self + other, carry 반환
    pub fn add(&self, other: &Self) -> (Self, bool) {
        let mut result = [0u64; 4];
        let mut carry = 0u64;
        for i in 0..4 {
            let (s1, c1) = self.0[i].overflowing_add(other.0[i]);
            let (s2, c2) = s1.overflowing_add(carry);
            result[i] = s2;
            carry = (c1 as u64) + (c2 as u64);
        }
        (U256(result), carry > 0)
    }

    /// self - other, borrow 반환
    pub fn sub(&self, other: &Self) -> (Self, bool) {
        let mut result = [0u64; 4];
        let mut borrow = 0u64;
        for i in 0..4 {
            let (s1, b1) = self.0[i].overflowing_sub(other.0[i]);
            let (s2, b2) = s1.overflowing_sub(borrow);
            result[i] = s2;
            borrow = (b1 as u64) + (b2 as u64);
        }
        (U256(result), borrow > 0)
    }

    /// 비트 i 가져오기 (0 = LSB)
    pub fn bit(&self, i: usize) -> u64 {
        let limb = i / 64;
        let bit = i % 64;
        if limb >= 4 { return 0; }
        (self.0[limb] >> bit) & 1
    }

    /// 1비트 왼쪽 시프트
    pub fn shl1(&self) -> Self {
        let mut result = [0u64; 4];
        result[0] = self.0[0] << 1;
        for i in 1..4 {
            result[i] = (self.0[i] << 1) | (self.0[i-1] >> 63);
        }
        U256(result)
    }

    /// 1비트 오른쪽 시프트
    pub fn shr1(&self) -> Self {
        let mut result = [0u64; 4];
        result[3] = self.0[3] >> 1;
        for i in (0..3).rev() {
            result[i] = (self.0[i] >> 1) | (self.0[i+1] << 63);
        }
        U256(result)
    }
}

// ========================================================================
// P-256 소수체(Prime Field) 연산
// p = 2^256 - 2^224 + 2^192 + 2^96 - 1
// ========================================================================

/// P-256 소수
pub const P256_P: U256 = U256([
    0xFFFFFFFF_FFFFFFFF, // limb 0
    0x00000000_FFFFFFFF, // limb 1
    0x00000000_00000000, // limb 2
    0xFFFFFFFF_00000001, // limb 3
]);

/// P-256 곡선 차수 n
pub const P256_N: U256 = U256([
    0xF3B9CAC2_FC632551,
    0xBCE6FAAD_A7179E84,
    0xFFFFFFFF_FFFFFFFF,
    0xFFFFFFFF_00000000,
]);

/// 모듈러 덧셈: (a + b) mod p
pub fn mod_add(a: &U256, b: &U256, p: &U256) -> U256 {
    let (sum, carry) = a.add(b);
    if carry || sum.gte(p) {
        let (result, _) = sum.sub(p);
        result
    } else {
        sum
    }
}

/// 모듈러 뺄셈: (a - b) mod p
pub fn mod_sub(a: &U256, b: &U256, p: &U256) -> U256 {
    let (diff, borrow) = a.sub(b);
    if borrow {
        let (result, _) = diff.add(p);
        result
    } else {
        diff
    }
}

/// 모듈러 곱셈: (a * b) mod p (schoolbook + Barrett reduction 대신 단순 방법)
#[inline(never)]
pub fn mod_mul(a: &U256, b: &U256, p: &U256) -> U256 {
    // 512비트 곱셈 후 모듈러 환원
    let product = mul_wide(a, b);
    mod_reduce_512(&product, p)
}

/// 256x256 → 512비트 곱셈
#[inline(never)]
fn mul_wide(a: &U256, b: &U256) -> [u64; 8] {
    let mut result = [0u64; 8];

    for i in 0..4 {
        let mut carry = 0u128;
        for j in 0..4 {
            let prod = (a.0[i] as u128) * (b.0[j] as u128) + (result[i + j] as u128) + carry;
            result[i + j] = prod as u64;
            carry = prod >> 64;
        }
        result[i + 4] = carry as u64;
    }
    result
}

/// 512비트 → 256비트 모듈러 환원 (반복 뺄셈, P-256 특화 가능하지만 범용으로 구현)
fn mod_reduce_512(product: &[u64; 8], _p: &U256) -> U256 {
    p256_reduce(product)
}

/// P-256 전용 빠른 환원 (NIST 방법)
/// 512비트 정수를 P-256 소수로 환원
#[inline(never)]
fn p256_reduce(c: &[u64; 8]) -> U256 {
    let low = U256([c[0], c[1], c[2], c[3]]);
    let high = U256([c[4], c[5], c[6], c[7]]);

    // 2^256 ≡ 2^224 - 2^192 - 2^96 + 1 (mod p)
    // result = low + high * r_mod_p (mod p)

    let r_mod_p = U256([
        0x00000000_00000001,
        0xFFFFFFFF_00000000,
        0xFFFFFFFF_FFFFFFFF,
        0x00000000_FFFFFFFE,
    ]);

    // result = low + high * r_mod_p (mod p)
    // 이 곱셈도 512비트로 오버플로우할 수 있으므로 조심
    let hr = mul_wide(&high, &r_mod_p);
    // hr + low
    let mut sum = [0u64; 8];
    let mut carry = 0u128;
    for i in 0..4 {
        let s = (hr[i] as u128) + (low.0[i] as u128) + carry;
        sum[i] = s as u64;
        carry = s >> 64;
    }
    for i in 4..8 {
        let s = (hr[i] as u128) + carry;
        sum[i] = s as u64;
        carry = s >> 64;
    }

    // 아직 512비트일 수 있으므로 다시 한번 환원 (재귀 대신 최대 2~3회 뺄셈)
    let mut result = U256([sum[0], sum[1], sum[2], sum[3]]);
    let high2 = U256([sum[4], sum[5], sum[6], sum[7]]);

    if !high2.is_zero() {
        // 한 번 더 환원
        let hr2 = mul_wide(&high2, &r_mod_p);
        let mut carry2 = 0u128;
        for i in 0..4 {
            let s = (result.0[i] as u128) + (hr2[i] as u128) + carry2;
            result.0[i] = s as u64;
            carry2 = s >> 64;
        }
    }

    // 최종 조건 뺄셈: result >= p이면 result -= p
    while result.gte(&P256_P) {
        let (r, _) = result.sub(&P256_P);
        result = r;
    }

    result
}

/// 모듈러 역원: a^(-1) mod p (페르마 소정리: a^(p-2) mod p)
pub fn mod_inv(a: &U256, p: &U256) -> U256 {
    let (exp, _) = p.sub(&U256([2, 0, 0, 0])); // p - 2
    mod_pow(a, &exp, p)
}

/// 모듈러 거듭제곱: base^exp mod p (제곱-곱셈 방법)
pub fn mod_pow(base: &U256, exp: &U256, p: &U256) -> U256 {
    let mut result = U256::ONE;
    let mut b = *base;

    for i in 0..256 {
        if exp.bit(i) == 1 {
            result = mod_mul(&result, &b, p);
        }
        b = mod_mul(&b, &b, p);
    }
    result
}
