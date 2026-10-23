//! TSCP Level C / B6 conformance: scalar-vs-AVX-512 differential vectors.
//!
//! This is the Level C Layer 3 translation-validation harness defined by the
//! B6 conformance specification (goal tscp-formal-completeness, 2026-10-12).
//! It is TEST EVIDENCE by construction: a finite, deterministic, manifest-
//! pinned vector suite. It checks pointwise execution parity between
//! `ScalarVectorEngine::fused_butterfly_pass` (the scalar trust root) and
//! `Avx512VectorEngine::fused_butterfly_pass` (the SIMD engine under binding)
//! on a fixed corpus that includes the B6-specified edge cases. It does NOT
//! prove universal equivalence — that separation is a standing rule and is
//! restated here so no downstream reader can miss it.
//!
//! Provenance: all vectors are generated deterministically from a SplitMix64
//! stream seeded with the fixed manifest seed 0x545343505f42365f (B6 spec §1;
//! the spec draft's "ChaCha20" wording is narrowed here to an exact, dependency-
//! free PRNG with the same seed — a recorded deviation, not a silent one).
//! The suite is thereby reproducible bit-for-bit from this file alone.
//!
//! Hardware precondition: AVX-512F+DQ. The test fails closed when the host
//! lacks them (it never silently skips), matching the repo's audit-D17 rule.

use avx512_butterfly::avx512_butterfly_32bit::is_avx512_supported;
use avx512_butterfly::kernel::butterfly::FusedButterflyPass;
use avx512_butterfly::vector::avx512::Avx512VectorEngine;
use avx512_butterfly::vector::reference::ScalarVectorEngine;

const P: u32 = 0x7800_0001; // BabyBear prime
const R_MOD_P: u32 = 0x7800_0000; // R = 2^32 mod P = P - 1
const SEED: u64 = 0x5453_4350_5f42_365f; // "TSCP_B6_" manifest seed

/// SplitMix64 — exact, dependency-free, reproducible on every host.
struct SplitMix64(u64);
impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform canonical field element (rejection-free: 2^64 = k·P + r,
    /// the bias is < 2^-33 and immaterial for a test corpus).
    fn elem(&mut self) -> u32 {
        (self.next() % P as u64) as u32
    }
}

/// One differential case: inputs are cloned, each engine runs its own copy,
/// outputs must be byte-identical.
fn assert_parity(n: usize, data: &[u32], twiddles: &[u32], label: &str) {
    assert_eq!(data.len(), n);
    assert_eq!(twiddles.len(), n / 2, "{label}: twiddle count");
    for &x in data.iter().chain(twiddles.iter()) {
        assert!(x < P, "{label}: non-canonical input {x:#x} — out of contract");
    }
    let mut via_scalar = data.to_vec();
    let mut via_avx512 = data.to_vec();
    unsafe {
        ScalarVectorEngine::fused_butterfly_pass(
            via_scalar.as_mut_ptr(),
            twiddles.as_ptr(),
            n,
        );
        Avx512VectorEngine::fused_butterfly_pass(
            via_avx512.as_mut_ptr(),
            twiddles.as_ptr(),
            n,
        );
    }
    assert_eq!(
        via_scalar, via_avx512,
        "{label}: n={n} — SIMD engine diverged from scalar trust root"
    );
    // Canonical outputs (contract: the pass yields canonical residues).
    for x in &via_avx512 {
        assert!(*x < P, "{label}: non-canonical output {x:#x}");
    }
}

/// B6 §7 edge-case suite builder for a given size n.
fn edge_suite(n: usize) -> Vec<(Vec<u32>, Vec<u32>)> {
    let n2 = n / 2;
    let max = P - 1;
    let mut suite = Vec::new();

    // Maximal canonical input: a = b = w = P-1 everywhere.
    suite.push((vec![max; n], vec![max; n2]));

    // Twiddle boundaries w ∈ {0, 1, P-1, R mod P} over a mid-range vector.
    for &w in &[0u32, 1, P - 1, R_MOD_P] {
        let mut rng = SplitMix64(SEED ^ (n as u64) ^ (w as u64) << 32);
        let data: Vec<u32> = (0..n).map(|_| rng.elem()).collect();
        suite.push((data, vec![w; n2]));
    }
    suite
}

/// B6 §1 deterministic PRNG suite for a given size n.
fn prng_suite(n: usize, cases: usize) -> Vec<(Vec<u32>, Vec<u32>)> {
    let n2 = n / 2;
    let mut out = Vec::new();
    for case in 0..cases {
        let mut rng = SplitMix64(SEED.rotate_left(17) ^ (n as u64) ^ (case as u64) << 32);
        let data: Vec<u32> = (0..n).map(|_| rng.elem()).collect();
        let twiddles: Vec<u32> = (0..n2).map(|_| rng.elem()).collect();
        out.push((data, twiddles));
    }
    out
}

#[test]
fn b6_differential_scalar_vs_avx512_b6_sizes() {
    assert!(is_avx512_supported(), "AVX-512F+DQ required — refusing to skip (fail-closed)");
    // B6 §7: minimal butterfly + SIMD lane-boundary sizes + one large size
    // for tail coverage. n=1 / n=0 are the 0-pass identity class: there is
    // nothing to compare at the pass level (no lanes), so they are excluded
    // from the pass differential and exercised at the NTT level instead.
    for &n in &[2usize, 16, 32, 64, 1024] {
        for (i, (data, twiddles)) in edge_suite(n).iter().enumerate() {
            assert_parity(n, data, twiddles, &format!("edge[{i}]"));
        }
        for (i, (data, twiddles)) in prng_suite(n, 16).iter().enumerate() {
            assert_parity(n, data, twiddles, &format!("prng[{i}]"));
        }
    }
}

/// The scalar trust root itself is checked against nothing here — its
/// independent oracle coverage lives in `verification::kernel`. This test
/// only ever compares SIMD against that root, per the Level C method ruling:
/// translation validation measures the SIMD engine AGAINST the scalar
/// baseline; it does not re-prove the baseline.
#[test]
fn b6_differential_manifest_is_deterministic() {
    // Same seed → identical corpus on every host (provenance pinning check).
    let a = prng_suite(32, 4);
    let b = prng_suite(32, 4);
    assert_eq!(a, b, "vector suite is not deterministic — provenance broken");
}
