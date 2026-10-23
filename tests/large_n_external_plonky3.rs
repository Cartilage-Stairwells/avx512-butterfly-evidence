//! §14 Large Tier — external comparison against Plonky3 (spec §14).
//!
//! Oracle: `p3-dft` 0.1.0 `Radix2Dit<BabyBear>::dft` — a trusted external
//! transform from a SEPARATE codebase (tier-independence rule: an oracle
//! derived from this repository's production code would be a consistency
//! check, not an oracle). Root convention alignment, established before
//! the comparison and asserted here: Plonky3's
//! `two_adic_generator(27) = 0x1A427A41 = 440,564,289` equals this
//! repository's frozen `BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL`; both sides
//! derive the size-n root as `g^(2^(27-log_n))`. Their `dft` is
//! natural-order-in → natural-order-out, matching our DIF + bit_reverse.
//!
//! Both sides convert through canonical representation (their
//! `from_canonical_u32`/`as_canonical_u32`, our
//! `to_montgomery_reference`/`from_montgomery_reference`), so equality is
//! byte-for-byte on canonical residues — no representation shortcut.
//!
//! Failure classes probed at 1M+ (spec §14): index arithmetic overflow,
//! allocator behavior, scheduler correctness, real working-set pressure.
//!
//! Epistemic note: the LARGE-tier portable test runs on CI and qemu; the
//! AVX-512 module name contains `avx512_parity` so CI's known-silicon
//! filter (audit D17) excludes it on AVX-512-absent runners — it is
//! fail-closed native-only, never silently skipped in place.

#[cfg(target_arch = "x86_64")]
use avx512_butterfly::avx512_butterfly_32bit::is_avx512_supported;
use avx512_butterfly::field::babybear::constants::BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL;
use avx512_butterfly::field::babybear::reference::{from_montgomery_reference, to_montgomery_reference};
use avx512_butterfly::field::babybear::twiddles::dif_twiddle_table;
use avx512_butterfly::kernel::ntt::{bit_reverse, dif_ntt, dit_ntt};
use avx512_butterfly::scheduler::serial::SerialScheduler;
use avx512_butterfly::vector::reference::ScalarVectorEngine;
#[cfg(target_arch = "x86_64")]
use avx512_butterfly::vector::avx512::Avx512VectorEngine;

use p3_baby_bear::BabyBear;
use p3_dft::{Radix2Dit, TwoAdicSubgroupDft};
use p3_field::{AbstractField as _, PrimeField32 as _, TwoAdicField as _};

#[cfg(target_arch = "x86_64")]
use sha2::{Digest, Sha256};

/// Deterministic LCG corpus — same generator and seed as the commit-8
/// benchmark surface (reproducible across runs and revisions).
fn lcg_corpus(n: usize) -> Vec<u32> {
    let mut s = 0x243F_6A88u64;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 33) as u32 % BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL % 0x7800_0001
        })
        .collect()
}

/// Our composed transform, canonical-u32 end to end, Scalar engine (portable).
fn our_ntt_scalar_canonical(input: &[u32]) -> Vec<u32> {
    let log_n = input.len().trailing_zeros() as usize;
    let table = dif_twiddle_table(log_n);
    let tw: Vec<&[u32]> = table.iter().map(|v| v.as_slice()).collect();
    let mut mont: Vec<u32> = input.iter().map(|&v| to_montgomery_reference(v)).collect();
    dif_ntt::<ScalarVectorEngine, _>(&mut mont, &tw, &SerialScheduler);
    bit_reverse(&mut mont);
    mont.iter().map(|&x| from_montgomery_reference(x)).collect()
}

/// Our composed transform, canonical-u32 end to end, AVX-512 engine (x86_64).
#[cfg(target_arch = "x86_64")]
fn our_ntt_avx512_canonical(input: &[u32]) -> Vec<u32> {
    let log_n = input.len().trailing_zeros() as usize;
    let table = dif_twiddle_table(log_n);
    let tw: Vec<&[u32]> = table.iter().map(|v| v.as_slice()).collect();
    let mut mont: Vec<u32> = input.iter().map(|&v| to_montgomery_reference(v)).collect();
    dif_ntt::<Avx512VectorEngine, _>(&mut mont, &tw, &SerialScheduler);
    bit_reverse(&mut mont);
    mont.iter().map(|&x| from_montgomery_reference(x)).collect()
}

/// Plonky3 external oracle: Radix2Dit DFT, canonical-u32 end to end.
fn plonky3_dft_canonical(input: &[u32]) -> Vec<u32> {
    let vec: Vec<BabyBear> = input
        .iter()
        .map(|&v| BabyBear::from_canonical_u32(v))
        .collect();
    let out = Radix2Dit::<BabyBear>::default().dft(vec);
    out.iter().map(|x| x.as_canonical_u32()).collect()
}

/// Root-convention alignment, asserted as a precondition of the comparison
/// (recorded, not assumed: both sides' size-n root is g^(2^(27-log_n))).
#[test]
fn root_conventions_align() {
    let theirs = BabyBear::two_adic_generator(27).as_canonical_u32();
    assert_eq!(
        theirs, BABYBEAR_TWO_ADIC_GENERATOR_CANONICAL,
        "external root convention diverged: comparison boundary invalid"
    );
}

/// Sanity ladder before the large sizes: small-tier alignment with the
/// external codebase (portable — runs on CI and qemu).
#[test]
fn small_sizes_match_plonky3_scalar() {
    for log_n in [4usize, 8, 12] {
        let input = lcg_corpus(1usize << log_n);
        let ours = our_ntt_scalar_canonical(&input);
        let theirs = plonky3_dft_canonical(&input);
        assert_eq!(ours, theirs, "log_n={log_n}: composed(Scalar) diverged from Plonky3");
    }
}

/// LARGE tier, portable route: composed Scalar vs Plonky3 at 1M elements.
/// Runs on CI and qemu (no AVX-512 required).
#[test]
fn large_1m_matches_plonky3_scalar() {
    let input = lcg_corpus(1usize << 20);
    let ours = our_ntt_scalar_canonical(&input);
    let theirs = plonky3_dft_canonical(&input);
    assert_eq!(ours, theirs, "n=2^20: composed(Scalar) diverged from Plonky3");
}

/// Negative controls: the comparison must DETECT divergence, not just pass.
/// (a) one corrupted oracle element is caught; (b) a coset DFT — a
/// different transform on the same input — is caught. Both demonstrate the
/// byte-identity assertion is not vacuous.
#[test]
fn negative_controls_detect_divergence() {
    let input = lcg_corpus(1usize << 10);
    let ours = our_ntt_scalar_canonical(&input);

    // (a) corruption detection
    let mut corrupted = plonky3_dft_canonical(&input);
    corrupted[3] ^= 1;
    assert_ne!(ours, corrupted, "corrupted Plonky3 output NOT detected");

    // (b) different-transform discrimination (coset shift = 2)
    let vec: Vec<BabyBear> = input.iter().map(|&v| BabyBear::from_canonical_u32(v)).collect();
    let coset = Radix2Dit::<BabyBear>::default().coset_dft(vec, BabyBear::from_canonical_u32(2));
    let coset_u32: Vec<u32> = coset.iter().map(|x| x.as_canonical_u32()).collect();
    assert_ne!(ours, coset_u32, "coset DFT conflated with subgroup DFT");
}

/// LARGE tier, native route (name contains `avx512_parity` — CI filter D17):
/// composed AVX-512 vs Plonky3 at 1M..16M elements. Fail-closed: refuses
/// to run without AVX-512F+DQ (a skip would be indistinguishable from a
/// pass). Records SHA-256 of both outputs per size (printed; sealed into
/// the audit record via --nocapture run).
#[cfg(target_arch = "x86_64")]
#[test]
fn large_1m_to_16m_matches_plonky3_avx512_parity() {
    assert!(is_avx512_supported(), "AVX-512F+DQ required — refusing to skip");
    for log_n in [20usize, 21, 22, 23, 24] {
        let input = lcg_corpus(1usize << log_n);
        let ours = our_ntt_avx512_canonical(&input);
        let theirs = plonky3_dft_canonical(&input);

        let h = |v: &[u32]| {
            let mut hasher = Sha256::new();
            for &x in v {
                hasher.update(x.to_le_bytes());
            }
            format!("{:x}", hasher.finalize())
        };
        println!("log_n={log_n} ours={}", h(&ours));
        println!("log_n={log_n} p3  ={}", h(&theirs));

        assert_eq!(
            ours, theirs,
            "n=2^{log_n}: composed(AVX-512) diverged from Plonky3 at large n"
        );
    }
}

// ---------------------------------------------------------------------------
// DIT route (migration-commit-10): Plonky3's Radix2Dit IS a DIT transform —
// the external oracle directly validates the executable DIT boundary at
// large n. Route: natural input -> bit_reverse -> our dit_ntt -> natural
// output, compared against their dft(natural input).
// ---------------------------------------------------------------------------

/// Our DIT transform, canonical-u32 end to end, Scalar engine (portable).
fn our_dit_scalar_canonical(input: &[u32]) -> Vec<u32> {
    let log_n = input.len().trailing_zeros() as usize;
    let table = dif_twiddle_table(log_n);
    let tw: Vec<&[u32]> = table.iter().map(|v| v.as_slice()).collect();
    let mut mont: Vec<u32> = input.iter().map(|&v| to_montgomery_reference(v)).collect();
    bit_reverse(&mut mont);
    dit_ntt::<ScalarVectorEngine, _>(&mut mont, &tw, &SerialScheduler);
    mont.iter().map(|&x| from_montgomery_reference(x)).collect()
}

/// Our DIT transform, canonical-u32 end to end, AVX-512 engine (x86_64).
#[cfg(target_arch = "x86_64")]
fn our_dit_avx512_canonical(input: &[u32]) -> Vec<u32> {
    let log_n = input.len().trailing_zeros() as usize;
    let table = dif_twiddle_table(log_n);
    let tw: Vec<&[u32]> = table.iter().map(|v| v.as_slice()).collect();
    let mut mont: Vec<u32> = input.iter().map(|&v| to_montgomery_reference(v)).collect();
    bit_reverse(&mut mont);
    dit_ntt::<Avx512VectorEngine, _>(&mut mont, &tw, &SerialScheduler);
    mont.iter().map(|&x| from_montgomery_reference(x)).collect()
}

/// Sanity ladder for the DIT route against the external oracle.
#[test]
fn small_sizes_match_plonky3_dit_scalar() {
    for log_n in [4usize, 8, 12] {
        let input = lcg_corpus(1usize << log_n);
        let ours = our_dit_scalar_canonical(&input);
        let theirs = plonky3_dft_canonical(&input);
        assert_eq!(ours, theirs, "log_n={log_n}: composed DIT(Scalar) diverged from Plonky3");
    }
}

/// LARGE tier, portable DIT route: composed Scalar vs Plonky3 at 1M.
#[test]
fn large_1m_matches_plonky3_dit_scalar() {
    let input = lcg_corpus(1usize << 20);
    let ours = our_dit_scalar_canonical(&input);
    let theirs = plonky3_dft_canonical(&input);
    assert_eq!(ours, theirs, "n=2^20: composed DIT(Scalar) diverged from Plonky3");
}

/// LARGE tier, native DIT route (name contains `avx512_parity` — D17
/// filter): composed AVX-512 DIT vs Plonky3's own DIT at 1M..16M.
#[cfg(target_arch = "x86_64")]
#[test]
fn large_1m_to_16m_matches_plonky3_dit_avx512_parity() {
    assert!(is_avx512_supported(), "AVX-512F+DQ required — refusing to skip");
    for log_n in [20usize, 21, 22, 23, 24] {
        let input = lcg_corpus(1usize << log_n);
        let ours = our_dit_avx512_canonical(&input);
        let theirs = plonky3_dft_canonical(&input);
        assert_eq!(
            ours, theirs,
            "n=2^{log_n}: composed DIT(AVX-512) diverged from Plonky3 at large n"
        );
    }
}
