//! Field Authority Record — executable provenance for the BabyBear constants.
//!
//! Companion to docs/field_authority.md (the evidence record) and the
//! P0-frozen generator decision recorded in `src/field/babybear/constants.rs`.
//!
//! Scope: this file pins, as executable fact, the full derivation chain
//! from the multiplicative structure of F_p down to the frozen two-adic
//! generator:
//!
//!   p prime, p - 1 = 2^27 × 3 × 5
//!       → 31 is a full-group primitive root (order exactly p - 1)
//!       → 31 is the smallest such root (nothing in 2..=30 qualifies)
//!       → BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL == 31^15 mod p
//!       → its order is exactly 2^27 (15 = (p-1)/2^27, the cofactor)
//!       → size-n roots from the frozen constant agree with roots derived
//!         independently from 31 for every power-of-two n <= 2^27.
//!
//! Why this exists: the doc comment on `BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL`
//! states the constant as `31^15 mod p` (P0-frozen decision, 2026-08-13).
//! Prior to this file that statement was documentation, not executable — the
//! constant was guarded by its order (2^27) and by custody hashes, but no
//! test tied it to the base 31 or established what 31 is. This file makes
//! the provenance line an assertion: an edit that breaks any link in the
//! chain fails here.
//!
//! Independence: all arithmetic in this file is raw u64 modular
//! exponentiation (same discipline as the §13 oracle independence rule and
//! `tests/ntt_equivalence.rs`). Production constants are imported as DATA
//! only (`BABYBEAR_P`, `BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL`); no
//! production Montgomery, twiddle, butterfly, or stage code is used or
//! trusted here. The recorded check values were computed independently
//! (Python, raw modular arithmetic) before being pinned below; they are
//! re-derived and asserted at run time.
//!
//! Evidence classification (Unified Provenance Framework): ESTABLISHED —
//! every claim below is executable in this file and reproduces on every CI
//! draw (no AVX-512 capability required; this file is scalar-only).
//!
//! Orthogonality note: this file adds a new evidence object in the
//! field-layer provenance class. It does not touch `src/`, does not
//! modify any existing test, and is outside the Step-10 determinism
//! hold's scope (that hold governs scheduler/concurrency evidence and
//! forbids patches until a reproduced failure is root-caused).

use avx512_butterfly::field::babybear::constants::{
    BABYBEAR_P, BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL,
};

// ---------------------------------------------------------------------------
// Helpers: raw u64 modular arithmetic (no production code).
// ---------------------------------------------------------------------------

fn mod_pow(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    let mut result = 1u64;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = result * base % modulus;
        }
        exp >>= 1;
        base = base * base % modulus;
    }
    result
}

/// Deterministic Miller-Rabin. The witness set {2, 3, 5, 7, 11, 13, 17}
/// is exact (no false positives) for all n < 3,317,044,064,679,887,385,
/// which covers u32-range moduli with certainty.
fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for q in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if n == q {
            return true;
        }
        if n % q == 0 {
            return false;
        }
    }
    let mut d = n - 1;
    let mut r = 0u32;
    while d % 2 == 0 {
        d /= 2;
        r += 1;
    }
    'witness: for a in [2u64, 3, 5, 7, 11, 13, 17] {
        let mut x = mod_pow(a, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 0..r {
            x = x * x % n;
            if x == n - 1 {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

/// Full-order test: g has multiplicative order exactly p - 1 iff
/// g^(p-1) == 1 and g^((p-1)/q) != 1 for every distinct prime factor q
/// of p - 1. For BabyBear the factor set is {2, 3, 5}.
fn has_full_order(g: u64, p: u64, factors: &[u64]) -> bool {
    if mod_pow(g, p - 1, p) != 1 {
        return false;
    }
    factors.iter().all(|&q| mod_pow(g, (p - 1) / q, p) != 1)
}

// ---------------------------------------------------------------------------
// Authority values. These literals are the evidence; the crate constants
// are checked AGAINST them. Recorded 2026-09-28 (independent Python
// computation, raw modular arithmetic — see docs/field_authority.md).
// ---------------------------------------------------------------------------

const AUTHORITY_BASE: u64 = 31; // smallest full-group primitive root of F_p*
const TWO_ADICITY: u32 = 27;

#[test]
fn babybear_p_is_prime() {
    assert_eq!(BABYBEAR_P, 0x7800_0001, "modulus must be the pinned BabyBear prime");
    assert!(is_prime(BABYBEAR_P as u64), "BABYBEAR_P must be prime");
}

#[test]
fn group_order_factorization_is_two_adic_27() {
    let p = BABYBEAR_P as u64;
    let pm1 = p - 1;
    assert_eq!(pm1, 2u64.pow(27) * 3 * 5, "p - 1 must equal 2^27 * 3 * 5");
    let mut v = 0u32;
    let mut m = pm1;
    while m % 2 == 0 {
        m /= 2;
        v += 1;
    }
    assert_eq!(v, TWO_ADICITY, "two-adicity must be exactly 27");
}

#[test]
fn base31_is_full_group_primitive_root() {
    let p = BABYBEAR_P as u64;
    let factors = [2u64, 3, 5];
    assert!(
        has_full_order(AUTHORITY_BASE, p, &factors),
        "31 must have multiplicative order exactly p - 1"
    );
    // Exact recorded check values (regression pins) — computed
    // independently and re-derived here:
    assert_eq!(mod_pow(31, p - 1, p), 1);
    assert_eq!(mod_pow(31, (p - 1) / 2, p), 2013265920); // ≡ -1
    assert_eq!(mod_pow(31, (p - 1) / 3, p), 1314723123);
    assert_eq!(mod_pow(31, (p - 1) / 5, p), 645581151);
}

#[test]
fn base31_is_smallest_primitive_root() {
    let p = BABYBEAR_P as u64;
    let factors = [2u64, 3, 5];
    for g in 2..AUTHORITY_BASE {
        assert!(
            !has_full_order(g, p, &factors),
            "g = {} also has full order — the pinned authority base is not minimal; \
             docs/field_authority.md is wrong",
            g
        );
    }
}

/// THE executable provenance assertion: the frozen two-adic generator is
/// 31^15 mod p, where 15 = (p-1)/2^27 is the cofactor of the 2-Sylow
/// subgroup. This makes the doc comment on the constant a test.
#[test]
fn frozen_generator_provenance_is_base31_pow_15() {
    let p = BABYBEAR_P as u64;
    let cofactor = (p - 1) >> TWO_ADICITY; // = 15
    assert_eq!(cofactor, 15);
    assert_eq!(
        mod_pow(AUTHORITY_BASE, cofactor, p),
        BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL as u64,
        "BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL must equal 31^15 mod p (P0-frozen provenance)"
    );
}

/// The frozen constant's order is exactly 2^27 — by direct computation
/// (independent of the existing in-repo re-verifications) and by group
/// theory: 31 has order 2^27·3·5, so 31^15 has order exactly 2^27.
#[test]
fn frozen_generator_order_is_exactly_2_pow_27() {
    let p = BABYBEAR_P as u64;
    let g2 = BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL as u64;
    assert_eq!(mod_pow(g2, 1u64 << TWO_ADICITY, p), 1, "g^(2^27) must be 1");
    assert_ne!(mod_pow(g2, 1u64 << (TWO_ADICITY - 1), p), 1, "g^(2^26) must not be 1");
}

/// Two independent derivation routes to the size-n primitive root must
/// agree for every power-of-two n <= 2^27:
///   route A: omega_n = 31^((p-1)/n)          (full-group generator route)
///   route B: omega_n = g2^(2^(27-log2 n))    (frozen two-adic generator route)
/// Route A is the route taken by the §13-independent test oracle in
/// tests/ntt_equivalence.rs (whose from-scratch root finder lands on 31);
/// route B is the production route. Golden values pinned for n in {4,8,16,32}.
#[test]
fn root_derivation_routes_agree_all_sizes() {
    let p = BABYBEAR_P as u64;
    let g2 = BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL as u64;
    let mut golden_checked = 0;
    for e in 1..=TWO_ADICITY {
        let n = 1u64 << e;
        let route_a = mod_pow(AUTHORITY_BASE, (p - 1) / n, p);
        let route_b = mod_pow(g2, 1u64 << (TWO_ADICITY - e), p);
        assert_eq!(
            route_a, route_b,
            "n = 2^{e}: full-group route and frozen-generator route disagree"
        );
        assert_eq!(mod_pow(route_a, n, p), 1, "omega_n^n != 1 for n = 2^{e}");
        assert_ne!(mod_pow(route_a, n / 2, p), 1, "omega_n^(n/2) == 1 for n = 2^{e} — order not exact");
        golden_checked += 1;
    }
    assert_eq!(golden_checked, 27, "all power-of-two sizes 2^1..2^27 checked");
}

/// Golden root pins — recorded independently (Python) 2026-09-28, guarded
/// here so any drift in the constants or in this file's arithmetic fails CI.
#[test]
fn golden_root_values_match_authority_record() {
    let p = BABYBEAR_P as u64;
    assert_eq!(mod_pow(31, (p - 1) / 4, p), 1_728_404_513); // omega_4
    assert_eq!(mod_pow(31, (p - 1) / 8, p), 1_592_366_214); // omega_8
    assert_eq!(mod_pow(31, (p - 1) / 16, p), 196_396_260); // omega_16
    assert_eq!(mod_pow(31, (p - 1) / 32, p), 760_005_850); // omega_32
}
