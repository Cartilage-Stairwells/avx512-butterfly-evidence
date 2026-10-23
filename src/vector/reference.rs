//! `ScalarVectorEngine` — the scalar reference engine (LANES = 1).
//!
//! Spec: PORTABLE_BUTTERFLY_SPEC.md §6 (reference engine), §9 (reference
//! correctness), §12 (trust boundary).
//!
//! This engine is the **trust root** for all optimized engines: every other
//! `VectorEngine` must produce byte-identical results on identical inputs
//! (parity requirement, §9). It is deliberately the slowest correct thing
//! the system can do.
//!
//! # Delegation map (migration-commit-3)
//!
//! | Engine method | Delegates to | Status |
//! |---|---|---|
//! | `mul` | `BABYBEAR_SCALAR.mul` (`field::babybear::montgomery`) — the single CIOS implementation, oracle-verified by `tests/babybear_montgomery.rs::oracle_agreement` | **pure delegation, zero new arithmetic** |
//! | `add` | — | **NEW arithmetic** (see caveat) |
//! | `sub` | — | **NEW arithmetic** (see caveat) |
//! | `load`/`store` | trivial pointer deref | no arithmetic |
//!
//! # add/sub caveat (stated plainly, not hidden)
//!
//! The repository's existing oracle-verified scalar path covers
//! multiplication (`ScalarBackend::mul_raw`, CIOS) and reduction only.
//! There is **no pre-existing scalar add/sub for Montgomery residues** to
//! delegate to (verified during Step-3 planning; `canonical.rs` provides
//! only domain conversions). Montgomery-form residues satisfy
//! `aR + bR ≡ (a+b)R (mod p)`, so addition/subtraction act directly on
//! residues with a single conditional correction. These two one-liners are
//! the **only new arithmetic** in this commit. They carry their own
//! differential obligation: `src/verification/vector/mod.rs` proves them
//! against an independent formulation (canonical-domain round-trip through
//! the existing, mul_raw-tested conversion machinery). The proof burden is
//! on that differential test, not on this file's assertion of triviality.
//!
//! No optimized engine (NEON, AVX-512) is touched by this commit.

use crate::field::babybear::constants::BABYBEAR_P;
use crate::field::babybear::montgomery::{BABYBEAR_SCALAR, MontgomeryBackend};
use crate::vector::traits::VectorEngine;

/// Scalar reference engine over BabyBear Montgomery residues.
///
/// `Repr = u32` (Montgomery residue, R = 2³²), `Vector = u32` (one lane),
/// `LANES = 1`.
///
/// Precondition: all inputs are in `[0, BABYBEAR_P)`. Out-of-domain input
/// behavior is documented (not guaranteed) by the sentinel test in
/// `src/verification/vector/mod.rs`.
pub struct ScalarVectorEngine;

impl VectorEngine for ScalarVectorEngine {
    type Repr = u32;
    type Vector = u32;
    const LANES: usize = 1;

    /// Trivial pointer read (unsafe per the §6 contract).
    #[inline(always)]
    unsafe fn load(ptr: *const Self::Repr) -> Self::Vector {
        *ptr
    }

    /// Trivial pointer write (unsafe per the §6 contract).
    #[inline(always)]
    unsafe fn store(ptr: *mut Self::Repr, value: Self::Vector) {
        *ptr = value;
    }

    /// Montgomery addition `(aR + bR) mod p`.
    ///
    /// **NEW ARITHMETIC** — see the add/sub caveat in the module docs.
    #[inline(always)]
    fn add(a: Self::Vector, b: Self::Vector) -> Self::Vector {
        // NEW ARITHMETIC — see the add/sub caveat in the module docs.
        // Direct residue addition with a single conditional correction:
        // aR + bR = (a+b)R (mod p); both operands are < p, so a+b < 2p.
        let s = (a as u64) + (b as u64);
        if s >= BABYBEAR_P as u64 {
            (s - BABYBEAR_P as u64) as u32
        } else {
            s as u32
        }
    }

    /// Montgomery subtraction `(aR - bR) mod p`.
    ///
    /// **NEW ARITHMETIC** — see the add/sub caveat in the module docs.
    #[inline(always)]
    fn sub(a: Self::Vector, b: Self::Vector) -> Self::Vector {
        // NEW ARITHMETIC — direct residue subtraction with a single
        // conditional correction: a < p and b < p, so a + p - b is exact.
        if a >= b {
            a - b
        } else {
            (a as u64 + BABYBEAR_P as u64 - b as u64) as u32
        }
    }

    /// Montgomery multiplication `(aR)·(bR) → (abR)`.
    ///
    /// **Pure delegation** to the single CIOS implementation
    /// (`BABYBEAR_SCALAR.mul`). Do not re-implement REDC here; the
    /// parity test fails if this method stops delegating.
    #[inline(always)]
    fn mul(a: Self::Vector, b: Self::Vector) -> Self::Vector {
        BABYBEAR_SCALAR.mul(a, b)
    }
}

/// Commit-8: no fused specialization for the scalar engine — LANES = 1 has
/// no per-op vector overhead to fuse, so it delegates to the generic kernel
/// (recorded decision; the parity suite asserts fused == generic anyway).
unsafe impl crate::kernel::butterfly::FusedButterflyPass for ScalarVectorEngine {
    unsafe fn fused_butterfly_pass(data: *mut Self::Repr, twiddles: *const Self::Repr, n: usize) {
        unsafe { crate::kernel::butterfly::butterfly_pass::<Self>(data, twiddles, n) }
    }
}
