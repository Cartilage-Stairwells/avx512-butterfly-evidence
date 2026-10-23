//! Boundary corpus v1 — pinned razor-edge fixtures for Montgomery reduction
//! and the DIF butterfly. See docs/boundary_corpus.md for the boundary
//! DEFINITION, the constructive derivations, and the generator/gate
//! separation principle.
//!
//! Provenance chain per fixture (constants -> derivation -> inputs ->
//! expected boundary state -> golden oracle -> implementation result):
//!   - constants: P, NEG_INV pinned in docs/field_authority.md
//!   - derivation: constructive congruences (docs/boundary_corpus.md §2)
//!   - golden oracle values: computed independently in raw u64/u128
//!     modular arithmetic (tools/boundary_discovery/generate_corpus.py,
//!     2026-09-29, two independent methods agreeing) and pinned here
//!   - implementation: ScalarBackend::mul_raw / typed butterfly
//!
//! Self-validation is structural, not documentation: every fixture's
//! boundary predicate is recomputed HERE from the constants by two
//! independent arithmetic paths, the expected side is asserted, and the
//! corpus-level test pins the presence of both firing and non-firing
//! razor sides plus both subtraction directions.

use avx512_butterfly::avx512_butterfly_32bit::butterfly;
use avx512_butterfly::field::babybear::canonical::MontgomeryBabyBear;
use avx512_butterfly::field::babybear::constants::{BABYBEAR_NEG_INV, BABYBEAR_P, BABYBEAR_R_MOD_P};
use avx512_butterfly::field::babybear::montgomery::{ScalarBackend, BABYBEAR_MONTY};
use avx512_butterfly::field::babybear::reference::{babybear_mul_reference, butterfly_reference};

const P: u32 = BABYBEAR_P; // 0x7800_0001

/// Montgomery reduction intermediate, path 1: exact transcription of the
/// steps inside `ScalarBackend::mul_raw` (raw u64 arithmetic).
fn reduction_intermediate(a: u32, b: u32) -> u32 {
    let t: u64 = (a as u64) * (b as u64);
    let m: u32 = (t as u32).wrapping_mul(BABYBEAR_NEG_INV);
    ((t + (m as u64) * (P as u64)) >> 32) as u32
}

/// Independent recomputation, path 2: m = t * NEG_INV (mod 2^32) built by
/// 2-adic shift-add accumulation — no shared operation sequence with path 1.
fn reduction_intermediate_independent(a: u32, b: u32) -> u32 {
    let mut t: u64 = (a as u64) * (b as u64);
    let mut m: u64 = 0;
    let mut acc: u64 = BABYBEAR_NEG_INV as u64;
    while t > 0 {
        if t & 1 == 1 {
            m = (m + acc) & 0xFFFF_FFFF;
        }
        t >>= 1;
        acc = (acc * 2) & 0xFFFF_FFFF;
    }
    (((a as u64) * (b as u64) + m * P as u64) >> 32) as u32
}

struct MulCase {
    a: u32,
    b: u32,
    expect_u: u32,
    fires: bool,
    golden: u32,
    why: &'static str,
}

struct BfCase {
    a: u32,
    b: u32,
    w: u32,
    sum_fires: bool,
    borrow: bool,
    d: u32,
    expect_u: u32,
    golden: (u32, u32),
    why: &'static str,
}

/// Pinned mul fixtures. Razor cases carry their constructive provenance
/// in `why`; golden values are the independent oracle result.
#[rustfmt::skip]
const MUL_CASES: &[MulCase] = &[
    MulCase { a: 0x0000000B, b: 0x222E8BA3, expect_u: 0x78000002, fires: true,  golden: 0x00000001, why: "razor firing: a*b = 1*P + 1*2^32 -> u = P+1, result 1" },
    MulCase { a: 0x0000000B, b: 0x445D1746, expect_u: 0x78000003, fires: true,  golden: 0x00000002, why: "razor firing: a*b = 2*P + 2*2^32 -> u = P+2, result 2" },
    MulCase { a: 0x0000000B, b: 0x668BA2E9, expect_u: 0x78000004, fires: true,  golden: 0x00000003, why: "razor firing: a*b = 3*P + 3*2^32 -> u = P+3, result 3" },
    MulCase { a: 0x0000000D, b: 0x73B13B14, expect_u: 0x78000005, fires: true,  golden: 0x00000004, why: "razor firing: a*b = 4*P + 4*2^32 -> u = P+4, result 4" },
    MulCase { a: 0x00000047, b: 0x0176FC65, expect_u: 0x78000000, fires: false, golden: 0x78000000, why: "razor non-firing: a*b = 3*P - 2^32 -> u = P-1, no subtract" },
    MulCase { a: 0x78000000, b: 0x0FFFFFFE, expect_u: 0x78000000, fires: false, golden: 0x78000000, why: "razor non-firing: identity mul (P-1)*R_mod_p -> P-1 (largest no-subtract result)" },
    MulCase { a: 0x00000000, b: 0x00000000, expect_u: 0x00000000, fires: false, golden: 0x00000000, why: "zero annihilation control" },
    MulCase { a: 0x00000001, b: 0x00000001, expect_u: 0x38400000, fires: false, golden: 0x38400000, why: "identity interior control" },
    MulCase { a: 0x78000000, b: 0x78000000, expect_u: 0x38400000, fires: false, golden: 0x38400000, why: "max^2 control: (P-1)^2 = P^2-2P+1 -> oracle R^-1 mod P" },
    MulCase { a: 0x0000002A, b: 0x00000039, expect_u: 0x167FFB9E, fires: false, golden: 0x167FFB9E, why: "arbitrary interior control" },
];

/// Pinned DIF butterfly fixtures: (a+b, (a-b)*w). Every boundary state
/// (add subtract, borrow, mul reduction) is asserted, not implied.
#[rustfmt::skip]
const BF_CASES: &[BfCase] = &[
    BfCase { a: 0x3C000006, b: 0x3BFFFFFB, w: 0x222E8BA3, sum_fires: true,  borrow: false, d: 0x0000000B, expect_u: 0x78000002, golden: (0x00000000, 0x00000001), why: "dual razor: sum = P (add subtracts to 0); d=11, u = P+1 firing" },
    BfCase { a: 0x3C000006, b: 0x3BFFFFFB, w: 0x445D1746, sum_fires: true,  borrow: false, d: 0x0000000B, expect_u: 0x78000003, golden: (0x00000000, 0x00000002), why: "dual razor: sum = P; d=11, u = P+2 firing" },
    BfCase { a: 0x00000000, b: 0x00000001, w: 0x0FFFFFFE, sum_fires: false, borrow: true,  d: 0x78000000, expect_u: 0x78000000, golden: (0x00000001, 0x78000000), why: "borrow direction; d = P-1; identity twiddle: u = P-1 razor non-firing; sum = 1" },
    BfCase { a: 0x00000001, b: 0x78000000, w: 0x0FFFFFFE, sum_fires: true,  borrow: true,  d: 0x00000002, expect_u: 0x00000002, golden: (0x00000000, 0x00000002), why: "borrow taken; sum = P exactly (add razor); d = 2 identity twiddle" },
    BfCase { a: 0x78000000, b: 0x00000001, w: 0x0FFFFFFE, sum_fires: true,  borrow: false, d: 0x77FFFFFF, expect_u: 0x77FFFFFF, golden: (0x00000000, 0x77FFFFFF), why: "no borrow; sum = P exactly (add razor); d = P-2 identity twiddle" },
];

#[test]
fn mul_boundary_states_recompute_from_constants() {
    for c in MUL_CASES {
        let u1 = reduction_intermediate(c.a, c.b);
        let u2 = reduction_intermediate_independent(c.a, c.b);
        assert_eq!(u1, c.expect_u, "path-1 recompute mismatch for ({:X},{:X}): {}", c.a, c.b, c.why);
        assert_eq!(u2, c.expect_u, "path-2 (independent) recompute mismatch for ({:X},{:X})", c.a, c.b);
        assert_eq!(u1 >= P, c.fires, "expected side mismatch for ({:X},{:X}): {}", c.a, c.b, c.why);

        // implementation result == golden oracle (pinned independently)
        let impl_result = ScalarBackend::mul_raw(c.a, c.b, BABYBEAR_MONTY);
        assert_eq!(impl_result, c.golden, "mul_raw != golden for ({:X},{:X}): {}", c.a, c.b, c.why);
        // reference oracle == golden (independent check of both)
        let oracle = babybear_mul_reference(c.a, c.b);
        assert_eq!(oracle, c.golden, "reference oracle != golden for ({:X},{:X}): {}", c.a, c.b, c.why);
        // boundary-state consistency: impl == u - P iff subtract fired.
        // This is the assertion the fixed extreme-value corpus could never
        // make: on firing cases a missing-final-subtract regression MUST fail.
        let expect_from_state = if u1 >= P { u1 - P } else { u1 };
        assert_eq!(impl_result, expect_from_state, "impl != boundary-state derivation for ({:X},{:X})", c.a, c.b);
    }
}

#[test]
fn corpus_covers_both_sides_and_both_directions() {
    // Corpus-level invariants of the boundary definition (docs §1).
    assert!(MUL_CASES.iter().any(|c| c.fires && c.expect_u == P + 1),
        "corpus must pin a firing razor case at u = P+1");
    assert!(MUL_CASES.iter().any(|c| !c.fires && c.expect_u == P - 1),
        "corpus must pin a non-firing razor case at u = P-1");
    assert!(MUL_CASES.iter().any(|c| c.expect_u.abs_diff(P) > 1 << 20),
        "corpus must pin interior controls far from the boundary");
    assert!(BF_CASES.iter().any(|c| c.sum_fires && c.a + c.b == P),
        "butterfly corpus must pin an add razor case with sum = P exactly");
    assert!(BF_CASES.iter().any(|c| !c.sum_fires),
        "butterfly corpus must pin the add non-firing side");
    assert!(BF_CASES.iter().any(|c| c.borrow),
        "butterfly corpus must pin the borrow subtraction direction");
    assert!(BF_CASES.iter().any(|c| !c.borrow),
        "butterfly corpus must pin the no-borrow subtraction direction");
    assert!(BF_CASES.iter().any(|c| c.expect_u >= P),
        "butterfly corpus must pin a mul-razor firing case");
    assert!(BF_CASES.iter().any(|c| c.expect_u == P - 1),
        "butterfly corpus must pin a mul-razor non-firing case");
}

#[test]
fn butterfly_boundary_states_and_golden() {
    for c in BF_CASES {
        // add boundary
        assert_eq!(c.a + c.b >= P, c.sum_fires, "add side mismatch: {}", c.why);
        // sub direction
        assert_eq!(c.a < c.b, c.borrow, "borrow direction mismatch: {}", c.why);
        let d = (c.a.wrapping_sub(c.b)).wrapping_add(if c.a < c.b { P } else { 0 });
        assert_eq!(d, c.d, "difference mismatch: {}", c.why);
        // mul boundary: two independent recomputes
        let u1 = reduction_intermediate(d, c.w);
        let u2 = reduction_intermediate_independent(d, c.w);
        assert_eq!(u1, c.expect_u, "bf path-1 u mismatch: {}", c.why);
        assert_eq!(u2, c.expect_u, "bf path-2 u mismatch: {}", c.why);
        assert_eq!(u1 >= P, c.expect_u >= P, "bf mul side mismatch: {}", c.why);
        // oracle == golden
        let oracle = butterfly_reference(c.a, c.b, c.w);
        assert_eq!(oracle, c.golden, "butterfly_reference != golden: {}", c.why);
        // typed implementation == golden
        let (x, y) = butterfly(
            MontgomeryBabyBear::new(c.a),
            MontgomeryBabyBear::new(c.b),
            MontgomeryBabyBear::new(c.w),
        );
        assert_eq!((x.inner(), y.inner()), c.golden, "typed butterfly != golden: {}", c.why);
        // implementation mul result consistency for the y leg
        assert_eq!(ScalarBackend::mul_raw(d, c.w, BABYBEAR_MONTY), c.golden.1,
            "mul_raw(d,w) != golden y: {}", c.why);
    }
}

#[test]
fn constants_are_the_pinned_authority() {
    // The corpus is only correct for the pinned constants. If these change,
    // the corpus MUST be regenerated (docs/boundary_corpus.md §3).
    assert_eq!(P, 0x7800_0001, "BABYBEAR_P changed: regenerate the boundary corpus");
    assert_eq!(BABYBEAR_NEG_INV, 0x77FF_FFFF, "BABYBEAR_NEG_INV changed: regenerate the boundary corpus");
    assert_eq!(BABYBEAR_R_MOD_P, 0x0FFF_FFFE, "BABYBEAR_R_MOD_P changed: regenerate the boundary corpus");
    // neg_inv invariant (mirrors the compile-time check in constants.rs)
    assert_eq!(P.wrapping_mul(BABYBEAR_NEG_INV).wrapping_add(1), 0);
}
