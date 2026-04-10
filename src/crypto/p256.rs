// src/crypto/p256.rs
// P-256 (secp256r1) 타원 곡선 디피-헬만 (ECDH) 순수 소프트웨어 구현
// 야코비안 좌표(Jacobian coordinates)를 사용하여 mod_inv 호출을 최소화합니다.
// y² = x³ - 3x + b (mod p)

use super::bignum::*;

/// P-256 기저점 G의 x 좌표
const G_X: U256 = U256([
    0xF4A13945_D898C296,
    0x77037D81_2DEB33A0,
    0xF8BCE6E5_63A440F2,
    0x6B17D1F2_E12C4247,
]);

/// P-256 기저점 G의 y 좌표
const G_Y: U256 = U256([
    0xCBB64068_37BF51F5,
    0x2BCE3357_6B315ECE,
    0x8EE7EB4A_7C0F9E16,
    0x4FE342E2_FE1A7F9B,
]);

/// 야코비안 좌표 점: (X : Y : Z), 아핀 좌표는 (X/Z², Y/Z³)
#[derive(Clone, Copy)]
struct JPoint {
    x: U256,
    y: U256,
    z: U256,
}

impl JPoint {
    fn identity() -> Self {
        JPoint { x: U256::ONE, y: U256::ONE, z: U256::ZERO }
    }

    fn from_affine(x: U256, y: U256) -> Self {
        JPoint { x, y, z: U256::ONE }
    }

    fn is_identity(&self) -> bool {
        self.z.is_zero()
    }
}

/// 야코비안 점 두 배: 2P (mod_inv 불필요)
/// 참고: https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-3.html#doubling-dbl-2001-b
#[inline(never)]
fn point_double_j(p: &JPoint) -> JPoint {
    if p.is_identity() { return *p; }

    let prime = &P256_P;

    // delta = Z1²
    let delta = mod_mul(&p.z, &p.z, prime);
    // gamma = Y1²
    let gamma = mod_mul(&p.y, &p.y, prime);
    // beta = X1 * gamma
    let beta = mod_mul(&p.x, &gamma, prime);

    // alpha = 3*(X1 - delta)*(X1 + delta)  [P-256에서 a=-3 이므로]
    let xmd = mod_sub(&p.x, &delta, prime);
    let xpd = mod_add(&p.x, &delta, prime);
    let alpha3 = mod_mul(&xmd, &xpd, prime);
    let three = U256([3, 0, 0, 0]);
    let alpha = mod_mul(&three, &alpha3, prime);

    // X3 = alpha² - 8*beta
    let alpha_sq = mod_mul(&alpha, &alpha, prime);
    let four_beta = mod_add(&beta, &beta, prime);
    let four_beta = mod_add(&four_beta, &four_beta, prime);
    let eight_beta = mod_add(&four_beta, &four_beta, prime);
    let x3 = mod_sub(&alpha_sq, &eight_beta, prime);

    // Z3 = (Y1 + Z1)² - gamma - delta
    let yz = mod_add(&p.y, &p.z, prime);
    let yz_sq = mod_mul(&yz, &yz, prime);
    let z3 = mod_sub(&mod_sub(&yz_sq, &gamma, prime), &delta, prime);

    // Y3 = alpha*(4*beta - X3) - 8*gamma²
    let fb_mx = mod_sub(&four_beta, &x3, prime);
    let gamma_sq = mod_mul(&gamma, &gamma, prime);
    let eight_gamma_sq = mod_add(&gamma_sq, &gamma_sq, prime);
    let eight_gamma_sq = mod_add(&eight_gamma_sq, &eight_gamma_sq, prime);
    let eight_gamma_sq = mod_add(&eight_gamma_sq, &eight_gamma_sq, prime);
    let y3 = mod_sub(&mod_mul(&alpha, &fb_mx, prime), &eight_gamma_sq, prime);

    JPoint { x: x3, y: y3, z: z3 }
}

/// 야코비안 점 덧셈: P + Q (혼합 덧셈, Q는 아핀 — Z=1)
/// 참고: https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-3.html#addition-madd-2004-hmv
#[inline(never)]
fn point_add_mixed(p: &JPoint, qx: &U256, qy: &U256) -> JPoint {
    if p.is_identity() {
        return JPoint::from_affine(*qx, *qy);
    }

    let prime = &P256_P;

    // U2 = X2 * Z1²
    let z1_sq = mod_mul(&p.z, &p.z, prime);
    let u2 = mod_mul(qx, &z1_sq, prime);

    // S2 = Y2 * Z1³
    let z1_cu = mod_mul(&z1_sq, &p.z, prime);
    let s2 = mod_mul(qy, &z1_cu, prime);

    // H = U2 - X1
    let h = mod_sub(&u2, &p.x, prime);
    // R = S2 - Y1
    let r = mod_sub(&s2, &p.y, prime);

    if h.is_zero() {
        if r.is_zero() {
            return point_double_j(p);
        } else {
            return JPoint::identity();
        }
    }

    let hh = mod_mul(&h, &h, prime);
    let hhh = mod_mul(&hh, &h, prime);
    let v = mod_mul(&p.x, &hh, prime);

    // X3 = R² - HHH - 2*V
    let r_sq = mod_mul(&r, &r, prime);
    let two_v = mod_add(&v, &v, prime);
    let x3 = mod_sub(&mod_sub(&r_sq, &hhh, prime), &two_v, prime);

    // Y3 = R*(V - X3) - Y1*HHH
    let v_mx = mod_sub(&v, &x3, prime);
    let y1_hhh = mod_mul(&p.y, &hhh, prime);
    let y3 = mod_sub(&mod_mul(&r, &v_mx, prime), &y1_hhh, prime);

    // Z3 = Z1 * H
    let z3 = mod_mul(&p.z, &h, prime);

    JPoint { x: x3, y: y3, z: z3 }
}

/// 스칼라 곱셈: k * G (왼쪽→오른쪽 이진법, 야코비안 좌표)
fn scalar_mul_g(k: &U256) -> JPoint {
    let mut result = JPoint::identity();

    for i in (0..256).rev() {
        result = point_double_j(&result);
        if k.bit(i) == 1 {
            result = point_add_mixed(&result, &G_X, &G_Y);
        }
    }
    result
}

/// 임의의 점에 대한 스칼라 곱셈: k * P
fn scalar_mul_point(k: &U256, px: &U256, py: &U256) -> JPoint {
    let mut result = JPoint::identity();

    for i in (0..256).rev() {
        result = point_double_j(&result);
        if k.bit(i) == 1 {
            result = point_add_mixed(&result, px, py);
        }
    }
    result
}

/// 야코비안 → 아핀 변환 (최종 1회만 mod_inv 호출)
fn to_affine(p: &JPoint) -> (U256, U256) {
    if p.is_identity() {
        return (U256::ZERO, U256::ZERO);
    }

    let prime = &P256_P;
    let z_inv = mod_inv(&p.z, prime);
    let z_inv2 = mod_mul(&z_inv, &z_inv, prime);
    let z_inv3 = mod_mul(&z_inv2, &z_inv, prime);

    let x = mod_mul(&p.x, &z_inv2, prime);
    let y = mod_mul(&p.y, &z_inv3, prime);
    (x, y)
}

/// ECDH 키 교환
/// 개인키(32바이트 랜덤) → 공개키(65바이트 비압축: 04 || x || y)
pub fn ecdh_public_key(private_key: &[u8; 32]) -> [u8; 65] {
    let k = U256::from_be_bytes(private_key);
    let jp = scalar_mul_g(&k);
    let (px, py) = to_affine(&jp);

    let mut out = [0u8; 65];
    out[0] = 0x04;
    out[1..33].copy_from_slice(&px.to_be_bytes());
    out[33..65].copy_from_slice(&py.to_be_bytes());
    out
}

/// ECDH 공유 비밀 계산
/// (내 개인키, 상대방 공개키) → 32바이트 공유 비밀 (x 좌표)
pub fn ecdh_shared_secret(private_key: &[u8; 32], peer_public: &[u8]) -> Option<[u8; 32]> {
    if peer_public.len() < 65 || peer_public[0] != 0x04 {
        return None;
    }

    let mut x_bytes = [0u8; 32];
    let mut y_bytes = [0u8; 32];
    x_bytes.copy_from_slice(&peer_public[1..33]);
    y_bytes.copy_from_slice(&peer_public[33..65]);

    let peer_x = U256::from_be_bytes(&x_bytes);
    let peer_y = U256::from_be_bytes(&y_bytes);

    let k = U256::from_be_bytes(private_key);
    let jp = scalar_mul_point(&k, &peer_x, &peer_y);

    if jp.is_identity() {
        return None;
    }

    let (sx, _) = to_affine(&jp);
    Some(sx.to_be_bytes())
}
