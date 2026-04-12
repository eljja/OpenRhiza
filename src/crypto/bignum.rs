// src/crypto/bignum.rs
// 256-bit integer arithmetic for P-256 ECDH
// Pure software implementation with no SIMD requirements.

/// 256-bit unsigned integer stored as little-endian `u64 x4`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct U256(pub [u64; 4]); // [0]=low, [3]=high

impl U256 {
    pub const ZERO: U256 = U256([0, 0, 0, 0]);
    pub const ONE: U256 = U256([1, 0, 0, 0]);

    /// Construct from a big-endian 32-byte array.
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

    /// Convert into a big-endian 32-byte array.
    pub fn to_be_bytes(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..4 {
            let offset = (3 - i) * 8;
            out[offset..offset+8].copy_from_slice(&self.0[i].to_be_bytes());
        }
        out
    }

    /// Return true if the value is zero.
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

    /// Add `other`, returning the sum and carry flag.
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

    /// Subtract `other`, returning the difference and borrow flag.
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

    /// Return bit `i` (`0` = LSB).
    pub fn bit(&self, i: usize) -> u64 {
        let limb = i / 64;
        let bit = i % 64;
        if limb >= 4 { return 0; }
        (self.0[limb] >> bit) & 1
    }

    /// Shift left by one bit.
    pub fn shl1(&self) -> Self {
        let mut result = [0u64; 4];
        result[0] = self.0[0] << 1;
        for i in 1..4 {
            result[i] = (self.0[i] << 1) | (self.0[i-1] >> 63);
        }
        U256(result)
    }

    /// Shift right by one bit.
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
// P-256 prime-field arithmetic
// p = 2^256 - 2^224 + 2^192 + 2^96 - 1
// ========================================================================

/// P-256 prime
pub const P256_P: U256 = U256([
    0xFFFFFFFF_FFFFFFFF, // limb 0
    0x00000000_FFFFFFFF, // limb 1
    0x00000000_00000000, // limb 2
    0xFFFFFFFF_00000001, // limb 3
]);

/// P-256 curve order `n`
pub const P256_N: U256 = U256([
    0xF3B9CAC2_FC632551,
    0xBCE6FAAD_A7179E84,
    0xFFFFFFFF_FFFFFFFF,
    0xFFFFFFFF_00000000,
]);

/// Modular addition: `(a + b) mod p`
pub fn mod_add(a: &U256, b: &U256, p: &U256) -> U256 {
    let (sum, carry) = a.add(b);
    if carry || sum.gte(p) {
        let (result, _) = sum.sub(p);
        result
    } else {
        sum
    }
}

/// Modular subtraction: `(a - b) mod p`
pub fn mod_sub(a: &U256, b: &U256, p: &U256) -> U256 {
    let (diff, borrow) = a.sub(b);
    if borrow {
        let (result, _) = diff.add(p);
        result
    } else {
        diff
    }
}

/// Modular multiplication: `(a * b) mod p`
#[inline(never)]
pub fn mod_mul(a: &U256, b: &U256, p: &U256) -> U256 {
    // Multiply to 512 bits, then reduce modulo `p`.
    let product = mul_wide(a, b);
    mod_reduce_512(&product, p)
}

/// 256x256 -> 512-bit multiplication.
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

/// 512-bit -> 256-bit modular reduction.
fn mod_reduce_512(product: &[u64; 8], _p: &U256) -> U256 {
    p256_reduce(product)
}

/// Fast P-256-specific reduction using the NIST prime structure.
/// Reduces a 512-bit integer modulo the P-256 prime.
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
    // This multiplication can still exceed 256 bits, so keep the wide result.
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

    // The intermediate result can still be wide, so reduce one more time if needed.
    let mut result = U256([sum[0], sum[1], sum[2], sum[3]]);
    let high2 = U256([sum[4], sum[5], sum[6], sum[7]]);

    if !high2.is_zero() {
        // Reduce once more.
        let hr2 = mul_wide(&high2, &r_mod_p);
        let mut carry2 = 0u128;
        for i in 0..4 {
            let s = (result.0[i] as u128) + (hr2[i] as u128) + carry2;
            result.0[i] = s as u64;
            carry2 = s >> 64;
        }
    }

    // Final conditional subtraction: if result >= p, subtract p.
    while result.gte(&P256_P) {
        let (r, _) = result.sub(&P256_P);
        result = r;
    }

    result
}

/// Modular inverse: `a^(-1) mod p` using Fermat's little theorem.
pub fn mod_inv(a: &U256, p: &U256) -> U256 {
    let (exp, _) = p.sub(&U256([2, 0, 0, 0])); // p - 2
    mod_pow(a, &exp, p)
}

/// Modular exponentiation: `base^exp mod p` using square-and-multiply.
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
