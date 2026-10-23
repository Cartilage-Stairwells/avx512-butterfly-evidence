//! `Avx512VectorEngine` — the 16-lane AVX-512 engine (LANES = 16).
//!
//! Spec: PORTABLE_BUTTERFLY_SPEC.md §6 (VectorEngine), §9 (reference match),
//! §12 (trust boundary).
//!
//! # Source provenance
//!
//! The arithmetic in this file is lifted **verbatim** from the production
//! module `src/avx512_butterfly_32bit.rs` (lines as of commit `9f16d82`) —
//! the first engine lifted from root production code rather than an
//! example. Lifted elements (instruction-for-instruction, same order):
//!
//! - `mont_reduce_epu64` — 8-lane AVX-512 REDC (verbatim)
//! - `mont_mul_16` — 16-lane Montgomery multiply (verbatim)
//! - `add` — butterfly step 1 of `avx512_radix2_butterfly_32`
//!   (`_mm512_add_epi32` + `cmpge_epu32_mask` + `mask_sub_epi32`)
//! - `sub` — butterfly step 2 (`_mm512_sub_epi32`, `+p` adjusted,
//!   `cmpge` mask, `mask_blend_epi32`)
//! - `load`/`store` — `_mm512_loadu_si512`/`_mm512_storeu_si512`
//!   (the pass-level load/store of `avx512_butterfly_pass_32`)
//!
//! # Adaptations (recorded, not silent)
//!
//! 1. **Constants**: the source module aliases
//!    (`use BABYBEAR_P as P; use BABYBEAR_NEG_INV as P_INV_NEG;`) are
//!    inlined here as the crate constants themselves. Identity holds by
//!    construction (same constants, same file family; verified values
//!    `0x7800_0001` / `0x77FF_FFFF`).
//! 2. **`core::arch` instead of `std::arch`**: `std::arch::x86_64` is a
//!    pure re-export of `core::arch::x86_64` — identical intrinsics, zero
//!    semantic difference. `core` keeps this module no_std-clean so the
//!    pre-existing `--no-default-features` error count stays exactly 212
//!    (audit D12). Recorded as an adaptation because it deviates from the
//!    source file's import line.
//! 3. **Tail loop NOT lifted**: `avx512_butterfly_pass_32`'s scalar tail is
//!    kernel-layer behavior (migration-commit-6+), same recorded boundary
//!    as the NEON engine. The engine contract is full vectors only.
//! 4. **Butterfly composition NOT lifted**: engine ops are add/sub/mul/
//!    load/store. Composition belongs to the kernel layer.
//!
//! # Runtime precondition (fail-closed contract)
//!
//! **Caller must ensure AVX-512F and AVX-512DQ are available** (use
//! `avx512_butterfly_32bit::is_avx512_supported()`). The engine performs
//! no runtime detection — mirroring the source module's own contract.
//! Dispatch and capability gating belong to the kernel layer
//! (migration-commit-6+). The parity tests assert native support and
//! fail (not skip) without it: a skip would be indistinguishable from a
//! pass.
//!
//! Precondition: all inputs are in `[0, BABYBEAR_P)`.

#[cfg(target_arch = "x86_64")]
mod engine {
    use crate::field::babybear::constants::BABYBEAR_P;
    // Commit-8 adaptation (recorded): the scalar tail lift calls
    // BABYBEAR_SCALAR.mul — the MontgomeryBackend trait must be in scope,
    // the same import the legacy module uses.
    use crate::field::babybear::montgomery::MontgomeryBackend;
    use crate::vector::traits::VectorEngine;
    use core::arch::x86_64::*;

    /// AVX-512 reference-validated engine over BabyBear Montgomery residues.
    ///
    /// `Repr = u32` (Montgomery residue, R = 2³²), `Vector = __m512i`
    /// (sixteen lanes), `LANES = 16`.
    pub struct Avx512VectorEngine;

    // -- Lifted verbatim from src/avx512_butterfly_32bit.rs ----------------

    // -- 32-bit-lane REDC (Firebird iteration 1, 2026-10-18) ------------------
    // Replaces the 64-bit-lane formulation. Mathematical identity UNCHANGED:
    // output = (a * b * R^{-1}) mod P, R = 2^32, canonical in [0, P).
    // Schedule: p3-style 32-bit lanes (vmovshdup extraction, vpmuludq products,
    // masked-movehdup high-half combine, underflow-mask correction).
    // Subtraction-form REDC with MU = P^{-1} mod 2^32: the low-word borrow is
    // always zero (q_p_lo ≡ prod_lo mod 2^32), so only high-word underflow
    // needs a conditional +P. ~13 vector instructions per 16-element multiply
    // (down from ~26).

    /// vmovshdup: copy odd 32-bit elements to even positions.
    /// Uses inline asm for the masked variant (intrinsic may compile to
    /// vpermt2ps with worse latency); plain cast for the unmasked variant.
    #[target_feature(enable = "avx512f,avx512dq")]
    #[inline]
    unsafe fn movehdup_epi32(a: __m512i) -> __m512i {
        _mm512_castps_si512(_mm512_movehdup_ps(_mm512_castsi512_ps(a)))
    }

    /// Masked vmovshdup: merge movehdup(a) with src according to mask k.
    #[target_feature(enable = "avx512f,avx512dq")]
    #[inline]
    unsafe fn mask_movehdup_epi32(src: __m512i, k: __mmask16, a: __m512i) -> __m512i {
        let dst: __m512i;
        core::arch::asm!(
            "vmovshdup {src_dst}{{{k}}}, {a}",
            src_dst = inlateout(zmm_reg) src => dst,
            k = in(kreg) k,
            a = in(zmm_reg) a,
            options(nomem, nostack, preserves_flags, pure),
        );
        dst
    }

    /// No-op barrier preventing LLVM from replacing vpmuludq with vpmullq.
    #[target_feature(enable = "avx512f,avx512dq")]
    #[inline]
    unsafe fn confuse_compiler(x: __m512i) -> __m512i {
        let y;
        core::arch::asm!(
            "/* {0} */",
            inlateout(zmm_reg) x => y,
            options(nomem, nostack, preserves_flags, pure),
        );
        y
    }

    /// 16-lane Montgomery multiply, 32-bit-lane subtraction-form REDC.
    /// Same mathematical identity as the previous 64-bit-lane version.
    #[target_feature(enable = "avx512f,avx512dq")]
    #[inline]
    unsafe fn mont_mul_16(vb: __m512i, vw: __m512i) -> __m512i {
        // MU = P^{-1} mod 2^32 (subtraction-form Montgomery constant).
        const MU: u32 = 0x8800_0001;
        const EVENS: __mmask16 = 0b0101_0101_0101_0101;

        let vmu = _mm512_set1_epi32(MU as i32);
        let vp  = _mm512_set1_epi32(BABYBEAR_P as i32);

        let vb_odd = movehdup_epi32(vb);
        let vw_odd = movehdup_epi32(vw);

        let prod_evn = _mm512_mul_epu32(vb, vw);
        let prod_odd = _mm512_mul_epu32(vb_odd, vw_odd);

        let q_evn = confuse_compiler(_mm512_mul_epu32(prod_evn, vmu));
        let q_odd = confuse_compiler(_mm512_mul_epu32(prod_odd, vmu));

        let prod_hi = mask_movehdup_epi32(prod_odd, EVENS, prod_evn);

        let q_p_evn = _mm512_mul_epu32(q_evn, vp);
        let q_p_odd = _mm512_mul_epu32(q_odd, vp);

        let q_p_hi = mask_movehdup_epi32(q_p_odd, EVENS, q_p_evn);

        let underflow = _mm512_cmplt_epu32_mask(prod_hi, q_p_hi);
        let res = _mm512_sub_epi32(prod_hi, q_p_hi);
        _mm512_mask_add_epi32(res, underflow, res, vp)
    }

    /// Modular addition `(aR + bR) mod p` — butterfly step 1 lifted to an
    /// engine op. Same instructions, same order.
    #[target_feature(enable = "avx512f,avx512dq")]
    #[inline]
    unsafe fn avx512_add(va: __m512i, vb: __m512i) -> __m512i {
        let vp = _mm512_set1_epi32(BABYBEAR_P as i32);
        let sum = _mm512_add_epi32(va, vb);
        let ge_sum = _mm512_cmpge_epu32_mask(sum, vp);
        _mm512_mask_sub_epi32(sum, ge_sum, sum, vp)
    }

    /// Modular subtraction `(aR - bR) mod p` — butterfly step 2 lifted to
    /// an engine op. Same instructions, same order.
    #[target_feature(enable = "avx512f,avx512dq")]
    #[inline]
    unsafe fn avx512_sub(va: __m512i, vb: __m512i) -> __m512i {
        let vp = _mm512_set1_epi32(BABYBEAR_P as i32);
        let raw_diff = _mm512_sub_epi32(va, vb);
        let adjusted = _mm512_add_epi32(raw_diff, vp);
        let ge_a = _mm512_cmpge_epu32_mask(va, vb);
        _mm512_mask_blend_epi32(ge_a, adjusted, raw_diff)
    }

    // -----------------------------------------------------------------------

    impl VectorEngine for Avx512VectorEngine {
        type Repr = u32;
        type Vector = __m512i;
        const LANES: usize = 16;

        #[inline(always)]
        unsafe fn load(ptr: *const Self::Repr) -> Self::Vector {
            _mm512_loadu_si512(ptr as *const __m512i)
        }

        #[inline(always)]
        unsafe fn store(ptr: *mut Self::Repr, value: Self::Vector) {
            _mm512_storeu_si512(ptr as *mut __m512i, value)
        }

        #[inline(always)]
        fn add(a: Self::Vector, b: Self::Vector) -> Self::Vector {
            unsafe { avx512_add(a, b) }
        }

        #[inline(always)]
        fn sub(a: Self::Vector, b: Self::Vector) -> Self::Vector {
            unsafe { avx512_sub(a, b) }
        }

        /// Montgomery multiplication `(aR)·(bR) → (abR)` — verbatim
        /// `mont_mul_16`.
        #[inline(always)]
        fn mul(a: Self::Vector, b: Self::Vector) -> Self::Vector {
            unsafe { mont_mul_16(a, b) }
        }
    }

    // -- Commit-8 fused pass (lifted verbatim from -----------------------
    //    src/avx512_butterfly_32bit.rs at migration-commit-8; same
    //    instruction-for-instruction discipline as the commit-5 lifts).
    //    Lifted elements: `avx512_radix2_butterfly_32` (fused butterfly
    //    composition — the engine's add/sub/mul ops in one body, same
    //    instructions, same order), `scalar_butterfly_32` (scalar tail
    //    butterflies), and `avx512_butterfly_pass_32` (the pass loop
    //    itself, renamed `avx512_fused_pass`). Adaptations (recorded):
    //    `P` alias -> crate constant `BABYBEAR_P`; `BABYBEAR_SCALAR` ->
    //    same field-layer object via full path. No instruction changes.

    /// Verbatim `scalar_butterfly_32` — DIF scalar tail butterfly.
    /// Calls the SAME field-layer `BABYBEAR_SCALAR.mul` the legacy tail
    /// calls (identity by shared object, not by re-implementation).
    #[inline(always)]
    fn scalar_butterfly_32(a: u32, b: u32, w: u32) -> (u32, u32) {
        let sum = a.wrapping_add(b);
        let sum = if sum >= BABYBEAR_P { sum - BABYBEAR_P } else { sum };
        let diff = if a >= b { a - b } else { a + BABYBEAR_P - b };
        let y = crate::field::babybear::montgomery::BABYBEAR_SCALAR.mul(diff, w);
        (sum, y)
    }

    /// Verbatim `avx512_radix2_butterfly_32` — fused 16-lane DIF butterfly.
    #[target_feature(enable = "avx512f,avx512dq")]
    unsafe fn avx512_radix2_butterfly_32(va: __m512i, vb: __m512i, vw: __m512i) -> (__m512i, __m512i) {
        let vp = _mm512_set1_epi32(BABYBEAR_P as i32);

        let sum = _mm512_add_epi32(va, vb);
        let ge_sum = _mm512_cmpge_epu32_mask(sum, vp);
        let a_new = _mm512_mask_sub_epi32(sum, ge_sum, sum, vp);

        let raw_diff = _mm512_sub_epi32(va, vb);
        let adjusted = _mm512_add_epi32(raw_diff, vp);
        let ge_a = _mm512_cmpge_epu32_mask(va, vb);
        let diff = _mm512_mask_blend_epi32(ge_a, adjusted, raw_diff);

        let b_new = mont_mul_16(diff, vw);

        (a_new, b_new)
    }

    /// Fused whole-pass body: 16-lane main loop + scalar tail.
    ///
    /// Firebird iteration 3 (PR head pipelined schedule): the main loop is
    /// SOFTWARE-PIPELINED — chunk i+1's (a, b, w) loads issue before chunk
    /// i's butterfly math, hiding load-use latency. Same math, same layout,
    /// same element values as the verbatim lift; OUTPUT parity with the
    /// legacy pass is asserted in `verification::kernel::fused_pass_parity`
    /// (fused == generic == legacy, tails included, all sizes), and the
    /// schedule itself was parity-proven on sealed silicon by the iteration-2
    /// probe (run 39135693333: bit-identical to the previous production pass
    /// over full 2^16/2^18/2^20 corpora before adoption, fail-closed gate).
    /// Motivation (measured, probe artifact): +4-11% at 2^16-2^20 on EPYC
    /// 9V45, +27-52% at 2^20 on EPYC 9V74 vs the previous schedule.
    #[target_feature(enable = "avx512f,avx512dq")]
    unsafe fn avx512_fused_pass(data: *mut u32, twiddles: *const u32, n: usize) {
        let n2 = n / 2;
        let chunks = n2 / 16;
        let mut c = 0usize;
        if chunks >= 1 {
            // Prime the pipeline with chunk 0.
            let mut na = _mm512_loadu_si512(data as *const __m512i);
            let mut nb = _mm512_loadu_si512(data.add(n2) as *const __m512i);
            let mut nw = _mm512_loadu_si512(twiddles as *const __m512i);
            while c + 1 < chunks {
                // Next chunk's loads hoisted above this chunk's math.
                let (va, vb, vw) = (na, nb, nw);
                let nxt = (c + 1) * 16;
                na = _mm512_loadu_si512(data.add(nxt) as *const __m512i);
                nb = _mm512_loadu_si512(data.add(nxt + n2) as *const __m512i);
                nw = _mm512_loadu_si512(twiddles.add(nxt) as *const __m512i);
                let (va_new, vb_new) = avx512_radix2_butterfly_32(va, vb, vw);
                let cur = c * 16;
                _mm512_storeu_si512(data.add(cur) as *mut __m512i, va_new);
                _mm512_storeu_si512(data.add(cur + n2) as *mut __m512i, vb_new);
                c += 1;
            }
            // Drain the last chunk.
            let (va_new, vb_new) = avx512_radix2_butterfly_32(na, nb, nw);
            let cur = c * 16;
            _mm512_storeu_si512(data.add(cur) as *mut __m512i, va_new);
            _mm512_storeu_si512(data.add(cur + n2) as *mut __m512i, vb_new);
        }
        // Scalar tail (unchanged): identical to the legacy pass.
        let mut i = chunks * 16;
        while i < n2 {
            let a = *data.add(i);
            let b = *data.add(i + n2);
            let w = *twiddles.add(i);
            let (a_new, b_new) = scalar_butterfly_32(a, b, w);
            *data.add(i) = a_new;
            *data.add(i + n2) = b_new;
            i += 1;
        }
    }

    /// Commit-8 fused pass for the 16-lane engine (see
    /// `kernel::butterfly::FusedButterflyPass` for the gate).
    unsafe impl crate::kernel::butterfly::FusedButterflyPass for Avx512VectorEngine {
        unsafe fn fused_butterfly_pass(data: *mut Self::Repr, twiddles: *const Self::Repr, n: usize) {
            avx512_fused_pass(data, twiddles, n)
        }
    }
}

#[cfg(target_arch = "x86_64")]
pub use engine::Avx512VectorEngine;
