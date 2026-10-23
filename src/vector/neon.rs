//! `NeonVectorEngine` — the 4-lane NEON engine (LANES = 4).
//!
//! Spec: PORTABLE_BUTTERFLY_SPEC.md §6 (VectorEngine), §9 (reference match),
//! §12 (trust boundary).
//!
//! # Source provenance (audit D5)
//!
//! The arithmetic in this file is lifted **verbatim** from
//! `examples/butterfly_neon.rs` — the sole real NEON implementation in
//! the repository (there is no root NEON placeholder; audit D5 recorded
//! the example as the implementation of record). The example file itself
//! is historical evidence and remains untouched by this commit.
//!
//! Lifted elements (instruction-for-instruction, same order):
//! - `neon_reduce_2`  — vectorized REDC (2 lanes) → this file
//! - `neon_mont_mul_vec` — 4-lane Montgomery multiply → this file
//! - add/sub conditional-correction pattern from the example's
//!   `neon_butterfly` (branchless `vbslq` sum/diff) → engine ops `add`/`sub`
//! - `load`/`store` — `vld1q_u32`/`vst1q_u32`
//!
//! # Adaptations (recorded, not silent)
//!
//! 1. **Constants**: the example defines local `P`/`P_INV_NEG`. This file
//!    uses the crate constants `BABYBEAR_P`/`BABYBEAR_NEG_INV`. Identity
//!    verified before lifting: `0x7800_0001` and `0x77FF_FFFF` in both
//!    sources (compile-time constants; provably identical).
//! 2. **Safety wrapper**: the example wraps NEON helpers in
//!    `unsafe fn` + `#[target_feature(enable = "neon")]`. The helper
//!    functions here keep that exact shape for fidelity; the trait
//!    methods are the safe surface. NEON is part of the baseline aarch64
//!    ISA, so the feature gate is a no-op on any conforming target.
//! 3. **Scope**: the example's scalar tail loop is NOT lifted. The engine
//!    contract is full vectors only; tail handling belongs to the kernel
//!    layer (migration-commit-6+), where the example's tail logic will
//!    be revisited at its scheduled boundary.
//!
//! # What this engine must satisfy (§9)
//!
//! Lane-for-lane byte-identical results with `ScalarVectorEngine` on
//! identical inputs. Proven in `src/verification/vector/mod.rs`
//! (`neon_parity`, executed under qemu-aarch64 emulation — the only
//! aarch64 execution surface available; stated plainly, not native
//! silicon).
//!
//! Precondition: all inputs are in `[0, BABYBEAR_P)`.

#[cfg(target_arch = "aarch64")]
mod engine {
    use crate::field::babybear::constants::{BABYBEAR_NEG_INV, BABYBEAR_P};
    use crate::vector::traits::VectorEngine;
    use std::arch::aarch64::*;

    /// NEON reference-validated engine over BabyBear Montgomery residues.
    ///
    /// `Repr = u32` (Montgomery residue, R = 2³²), `Vector = uint32x4_t`
    /// (four lanes), `LANES = 4`.
    pub struct NeonVectorEngine;

    // -- Lifted verbatim from examples/butterfly_neon.rs -------------------

    /// Vectorized Montgomery REDC (2 lanes) — verbatim `neon_reduce_2`.
    #[inline(always)]
    unsafe fn neon_reduce_2(prod: uint64x2_t) -> uint32x2_t {
        let prod_low32 = vmovn_u64(prod);
        let m = vmul_u32(prod_low32, vdup_n_u32(BABYBEAR_NEG_INV));
        let mp = vmlal_u32(prod, m, vdup_n_u32(BABYBEAR_P));
        let t = vshrn_n_u64(mp, 32);
        let ge = vcge_u32(t, vdup_n_u32(BABYBEAR_P));
        let sub = vand_u32(ge, vdup_n_u32(BABYBEAR_P));
        vsub_u32(t, sub)
    }

    /// 4-lane Montgomery multiply — verbatim `neon_mont_mul_vec`.
    #[inline(always)]
    unsafe fn neon_mont_mul_vec(a: uint32x4_t, b: uint32x4_t) -> uint32x4_t {
        let a_lo = vget_low_u32(a);
        let a_hi = vget_high_u32(a);
        let b_lo = vget_low_u32(b);
        let b_hi = vget_high_u32(b);
        let prod_lo = vmull_u32(a_lo, b_lo);
        let prod_hi = vmull_u32(a_hi, b_hi);
        let res_lo = neon_reduce_2(prod_lo);
        let res_hi = neon_reduce_2(prod_hi);
        vcombine_u32(res_lo, res_hi)
    }

    // -----------------------------------------------------------------------

    impl VectorEngine for NeonVectorEngine {
        type Repr = u32;
        type Vector = uint32x4_t;
        const LANES: usize = 4;

        #[inline(always)]
        unsafe fn load(ptr: *const Self::Repr) -> Self::Vector {
            vld1q_u32(ptr)
        }

        #[inline(always)]
        unsafe fn store(ptr: *mut Self::Repr, value: Self::Vector) {
            vst1q_u32(ptr, value)
        }

        /// Montgomery addition `(aR + bR) mod p` — the example's
        /// branchless conditional-correction pattern lifted to an engine op.
        #[inline(always)]
        fn add(a: Self::Vector, b: Self::Vector) -> Self::Vector {
            let sum = unsafe { vaddq_u32(a, b) };
            let sum_minus_p = unsafe { vsubq_u32(sum, vdupq_n_u32(BABYBEAR_P)) };
            let ge = unsafe { vcgeq_u32(sum, vdupq_n_u32(BABYBEAR_P)) };
            unsafe { vbslq_u32(ge, sum_minus_p, sum) }
        }

        /// Montgomery subtraction `(aR - bR) mod p` — the example's
        /// branchless conditional-correction pattern lifted to an engine op.
        #[inline(always)]
        fn sub(a: Self::Vector, b: Self::Vector) -> Self::Vector {
            let diff = unsafe { vsubq_u32(a, b) };
            let diff_plus_p = unsafe { vaddq_u32(diff, vdupq_n_u32(BABYBEAR_P)) };
            let lt = unsafe { vcltq_u32(a, b) };
            unsafe { vbslq_u32(lt, diff_plus_p, diff) }
        }

        /// Montgomery multiplication `(aR)·(bR) → (abR)` — verbatim
        /// `neon_mont_mul_vec`.
        #[inline(always)]
        fn mul(a: Self::Vector, b: Self::Vector) -> Self::Vector {
            unsafe { neon_mont_mul_vec(a, b) }
        }
    }

    /// Commit-8: no fused specialization for the 4-lane engine — it
    /// delegates to the generic kernel (recorded decision: no native
    /// NEON silicon in the measurement environment, so no fused claim is
    /// possible; fused == generic parity is still asserted under qemu).
    unsafe impl crate::kernel::butterfly::FusedButterflyPass for NeonVectorEngine {
        unsafe fn fused_butterfly_pass(data: *mut Self::Repr, twiddles: *const Self::Repr, n: usize) {
            unsafe { crate::kernel::butterfly::butterfly_pass::<Self>(data, twiddles, n) }
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub use engine::NeonVectorEngine;
