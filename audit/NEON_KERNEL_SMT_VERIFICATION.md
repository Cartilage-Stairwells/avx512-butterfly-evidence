# NEON Engine-Op SMT Verification — the 4-lane production schedule

**Status:** proofs executed 2026-10-23 (Z3 5.3.1, x86_64 host); silicon
premises PINNED on native Apple Silicon — GitHub Actions run
`39923096483` (`neon-m1-native`, macos-15 runner, aarch64-apple-darwin
fail-closed check, host `Apple M2 Pro (Virtual)`): lib tests 79/79,
`neon_intrinsic_semantics` 4/4, full run success. Log archived as
`NEON_CI_RUN_39923096483_silicon_log.txt` (sha256 cbbbdc9173fb3446acab
9a26e17801c21d4c7b953fcb67fb99d911adcb3e6e91). This document, the
scripts in this directory, the generator, and the silicon-pinning test
together extend the AVX-512 evidence set (see
`AVX512_KERNEL_SMT_VERIFICATION.md`) to the NEON engine — both engines'
production lane arithmetic is now formally verified as written.

## What is verified (universal, machine-checked)

Every production lane operation of `NeonVectorEngine`
(`src/vector/neon.rs`, lifted verbatim from `examples/butterfly_neon.rs`
per audit D5) is proven bit-exact **as written**, for ALL canonical inputs,
in SMT — 4 lanes per script, each lane independently symbolic:

| Op | Script | Claim proven |
|---|---|---|
| `add` | `neon_add_op.smt2` | out = (a+b) mod P, canonical, for all a,b ∈ [0,P) — the vaddq/vsubq/vcgeq/vbslq conditional-correction schedule is correct including the sum == P boundary |
| `sub` | `neon_sub_op.smt2` | out = (a−b) mod P, canonical, for all a,b ∈ [0,P) — the vbsl selects diff_plus_p exactly when a <u b |
| `mul` (`neon_mont_mul_vec` + `neon_reduce_2`) | `neon_mont_mul_vec.smt2` | for every lane: out·R + (ge ? P·R : 0) = prod + m·P with out < P — the vmull/vmovn/vmul/vmlal/vshrn/vcge/vand/vsub schedule computes a·b·R⁻¹ mod P exactly |

P = 0x78000001 (BabyBear), R = 2^32, NEG_INV = 0x77FFFFFF = −P⁻¹ mod 2^32
(P·NEG_INV ≡ 0xffffffff mod 2^32, checked at generator import).

**Why the mul claim pins the result:** the m and ge terms are both
multiplied by P, so the exact 64-bit identity forces out·R ≡ prod (mod P),
and out < P makes out the unique canonical Montgomery reduction
a·b·R⁻¹ mod P. The identity itself is t·R = prod + m·P with t = out + ge·P,
which holds exactly when the schedule's constants satisfy the REDC
invariant (lo32(prod + m·P) = 0).

**There is NO fused-butterfly script for NEON** — the engine has no fused
specialization (recorded decision in `src/vector/neon.rs`: it delegates to
the generic kernel; no fused claim exists to verify).

## Mutation controls (each must answer `sat`)

| Control | Expected | What it breaks |
|---|---|---|
| `mutation_add_vcgt.smt2` | sat | vcgt instead of vcge (misses sum == P) |
| `mutation_sub_bsl_swapped.smt2` | sat | swaps the vbsl operands in sub |
| `mutation_redc_wrong_neg_inv.smt2` | sat | NEG_INV off by one |
| `mutation_redc_wrong_p.smt2` | **unsat** | P off by one in the vmlal accumulate — see the equivalence note below |
| `mutation_redc_wrong_p_final.smt2` | sat | P off by one in the FINAL conditional subtraction (the observable P site) |
| `mutation_redc_no_cond_sub.smt2` | sat | drops the final conditional subtraction |
| `mutation_redc_m_zero.smt2` | sat | forces m = 0 (skips the vmul_u32 step) |

### Equivalence note — `mutation_redc_wrong_p.smt2` is expected UNSAT

During development, this mutation's sat-search found nothing (3,000,000
random inputs, no counterexample). Analysis shows why: changing the vmlal
constant P → P+1 alters the accumulated product by exactly m < 2^32,
which lands entirely in the LOW word of mp. The schedule only reads bits
[63:32] of mp (vshrn), so the mutation is invisible to the output. The
observable P control moved to `mutation_redc_wrong_p_final.smt2` (the
final conditional subtraction), which returns `sat` as a control should.

**Solver-checked derivation (sealed 2026-10-23).** The equivalence is not
left at prose: it is decomposed into two machine-checked lemmas plus the
sealed main proof:

1. `mutation_redc_wrong_p_lemma1.smt2` (unsat, seconds) — the REDC
   low-word invariant: for all a, b ∈ [0, P), lo32(a·b + m·P) = 0 where
   m = lo32(lo32(a·b)·NEG_INV).
2. `mutation_redc_wrong_p_lemma2.smt2` (unsat, trivial) — the carry
   lemma: adding any m < 2^32 to a 64-bit quantity with zero low word
   cannot change bits [63:32].

Together with mp2 = mp + m (exact integers), these imply t2 = t,
ge2 = ge, out2 = out for every canonical input — the mutated schedule
computes byte-identical outputs to the production schedule, so its
negated-claim query answers exactly like the sealed
`neon_mont_mul_vec.smt2` (unsat). The direct script
(`mutation_redc_wrong_p.smt2`) did not conclude within the sealed rerun's
3600 s per-script window (exit 124, recorded in
`seal_rerun_neon_2026-10-23.txt`); a 14400 s direct confirmation run was
launched afterward and its result, whenever it lands, can only confirm
what the lemmas already establish. A schedule-vs-schedule equivalence
script `mutation_redc_wrong_p_equiv.smt2` is committed for the record.
The fact that a "mutation" can be provably invisible to a checker is
itself a recorded finding about what mutation controls do and do not
see — and why each control's expected verdict is stated explicitly.

## Audit note — the controls caught two bugs in the generator itself

During development, the discipline of brute-force checking + mutation
controls caught two real bugs in the generator's encoding before anything
was sealed (mirroring the AVX-512 fused-strengthening episode):

1. The `sub` script's vbsl branch order was inverted, so the "swapped"
   mutation script was actually the correct schedule (its result was
   `unsat` — a mutation returning the wrong verdict is a flag, not a
   pass). The brute-force model confirmed the correct branch order; the
   generator was fixed and both scripts re-proven.
2. The `mul` script encoded `vshrn_n_u64(mp, 32)` as `bvlshr mp R64` —
   shifting a 64-bit value by the 64-bit constant 2^32 is a shift of
   4294967296 positions, i.e. always zero. The counterexample model
   (z3's `eval` of each lane under the model assignment) exposed that
   t was identically 0. Fixed to `((_ extract 63 32) mp)`, matching the
   sealed AVX-512 scripts' pattern.

An SMT solver proves the property encoded in its formula, not the one you
intended. Both times, the check discipline — not the intent — decided.

## The three-link chain and its boundaries

1. **Production → model.** `audit/gen_smt_neon.py` encodes
   `src/vector/neon.rs` instruction-for-instruction (constants P,
   NEG_INV; vbsl = (m & a) | (~m & b); vcge includes equality; vclt is
   strict; vmull = exact widening product; vmul = low 32 bits; vmlal =
   64-bit accumulate; vmovn = low 32; vshrn(v,32) = bits [63:32]).
   Checked line-by-line against the source on 2026-10-23.
2. **Model → property.** Z3 5.3.1 proves each negated claim unsat (all
   inputs in [0, P)); each mutation control is sat.
3. **Property → claim.** Precisely covered: the lane-arithmetic schedules
   of the NEON engine ops under the documented precondition, with the
   pinned instruction semantics. NOT covered: loop/control flow, the
   kernel-layer tail, compiler code generation, and any Lean binding —
   the differential parity suite (qemu + native Apple Silicon runs)
   covers behavior beyond the model, per the existing evidence chain.

## Silicon premise pinning

`tests/neon_intrinsic_semantics.rs` (4 tests) asserts every intrinsic
semantic the models rely on with exact bit patterns on native Apple
Silicon (aarch64 — NEON is baseline ISA there; the workflow aborts unless
the runner is genuinely aarch64), then runs the production op schedules
verbatim against the scalar Montgomery oracle on adversarial lanes
(0, P−1, P−2, 0x40000001, wrap boundaries). Executed via
`.github/workflows/neon-m1-native.yml` (macOS runner); the suite output is
SHA256-sealed in the run's evidence artifact.

## Runtime note

`neon_mont_mul_vec.smt2` (4 lanes × 3 multipliers) takes roughly 15–20
minutes of solver time on this class of host. All other scripts return in
seconds to a few minutes. Mutation sat-searches vary from seconds to tens
of minutes; each was allowed up to 1500 s.
