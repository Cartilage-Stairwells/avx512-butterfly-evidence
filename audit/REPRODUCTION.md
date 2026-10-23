# C2 SMT reproduction record

This record binds the Montgomery REDC evidence to the implementation repository
state in which the scripts are present.

- Repository: `Cartilage-Stairwells/avx512-butterfly`
- Evidence revision: `ab38ee3e3cd8f6a011d6b6b9d2d3fabd114eb93f`
- Kernel script: `audit/mont_mul_16_subtraction_form.smt2`
- Mutation controls: `audit/mutation_wrong_mu.smt2`, `audit/mutation_wrong_p.smt2`
- Solver: Z3 5.1.0 or a compatible Z3 release

## Reproduction

From the repository root:

```sh
z3 audit/mont_mul_16_subtraction_form.smt2   # expected: unsat
z3 audit/mutation_wrong_mu.smt2              # expected: sat
z3 audit/mutation_wrong_p.smt2               # expected: sat
```

The proof script's `unsat` result establishes the encoded equivalence under its
stated assumptions. The mutation controls returning `sat` show that the check
rejects the selected deliberately broken variants. This is kernel-level SMT
evidence only. It is not a proof of the surrounding implementation or of the
formal Lean model's correspondence to the Rust engines.

The SHA-256 hashes of the scripts at this revision are:

```text
f7ea08c4ba43d0bb973ab2a4ef3644265ce998c4272cb90c0afd27f7f6661c40  audit/mont_mul_16_subtraction_form.smt2
92e7a40415db90bf932943a32b3a33e431e73645787bccc336895f9446ec62c0  audit/mutation_wrong_mu.smt2
ea99a9c1350702335eba2fd55cbe72f0bddf328c1fbe07b5d1b8fb19f739d965  audit/mutation_wrong_p.smt2
```

Replace the abbreviated mutation hash above with the complete value produced by
`sha256sum` before treating this record as sealed.

---

## UPDATE 2026-10-22 (second session): production 32-bit-lane AVX-512 schedule

**What this update adds.** The sealed records above cover the original
64-bit-lane REDC formulation. The production engine was since relaned
(Firebird iteration 1, commit cac1c8a) to a 32-bit-lane schedule
(vpmuludq products + vmovshdup extraction + masked merges). This update
seals the SMT verification of the schedules AS WRITTEN in
`src/vector/avx512.rs`: `avx512_add`, `avx512_sub`, `mont_mul_16`, and the
fused DIF butterfly op.

**Evidence layout.** See `AVX512_KERNEL_SMT_VERIFICATION.md` for the full
method (generator `gen_smt_avx512.py`; intrinsic-semantics premises pinned
on silicon by `tests/avx512_intrinsic_semantics.rs`, 7/7 tests, fail-closed
without AVX-512F+DQ). Scripts: `avx512_add_op.smt2`,
`avx512_sub_op.smt2`, `mont_mul_16_pair{0..7}.smt2` (+ pair-2 conjunction
split `mont_mul_16_pair2_even/odd.smt2`), `avx512_fused_butterfly_pair{0..7}.smt2`,
and six mutation controls `mutation_*.smt2`.

**Solver.** Z3 version 5.3.1 - 64 bit; invocation `z3 <script>` with no
options; per-script timeout 1800 s in the sealed rerun harness
`rerun_sealed.sh`.

**Expected and observed results.** Main scripts (20 incl. the pair-2 split
halves): `unsat` on the negated lane-identity claims. Mutations (6): `sat`
(wrong MU; wrong P in schedule; dropped underflow correction; swapped blend
operands in sub; cmpgt instead of cmpge in add; swapped blend operands in
the fused butterfly). The complete captured rerun with per-script
SHA-256 before/after, exit statuses, and timestamps is
`seal_rerun_2026-10-22.txt` (summarized below).

**Audit note recorded during verification.** The first fused-butterfly
scripts asserted only the add and REDC claims; the blend-swap mutation
control returned `unsat` (not caught) because REDC is correct for any
32-bit diff, so those claims did not constrain the sub schedule. The
fused scripts were strengthened to assert the sub output explicitly
(canonical diff = (a−b) mod P); the mutation then returned `sat` and all
eight strengthened pairs returned `unsat`. The strengthened scripts
supersede the first round entirely. The mutation control did its job:
it exposed a missing assertion before any seal was claimed.

---

## UPDATE 2026-10-23: NEON engine schedules (4-lane Apple Silicon engine)

**What this update adds.** SMT verification of the production NEON engine
ops in `src/vector/neon.rs` (lifted verbatim from `examples/butterfly_neon.rs`
per audit D5): `add`, `sub`, and `mul` (`neon_mont_mul_vec` + `neon_reduce_2`
REDC). There is no fused NEON op (recorded decision — the engine delegates
to the generic kernel), so no fused script exists.

**Evidence layout.** Method: `NEON_KERNEL_SMT_VERIFICATION.md`. Generator:
`gen_smt_neon.py`. Scripts: `neon_add_op.smt2`, `neon_sub_op.smt2`,
`neon_mont_mul_vec.smt2` (all 4 lanes, unsat expected), seven mutation
controls: `mutation_add_vcgt`, `mutation_sub_bsl_swapped`,
`mutation_redc_wrong_neg_inv`, `mutation_redc_wrong_p`,
`mutation_redc_wrong_p_final`, `mutation_redc_no_cond_sub`,
`mutation_redc_m_zero`. Note: `mutation_redc_wrong_p` (P+1 in the vmlal
accumulate) is expected **unsat** — the mutation is output-equivalent (the
difference lands in the low word of the accumulated product, which the
schedule never reads); the analysis and the random-search evidence are
recorded in the method doc. The observable P control is
`mutation_redc_wrong_p_final` (expected sat).

**Solver.** Z3 version 5.3.1 - 64 bit; invocation `z3 <script>` with no
options; sealed rerun harness `rerun_sealed_neon.sh` (3600 s per-script
timeout), captured in `seal_rerun_neon_2026-10-23.txt`.

**Silicon premises.** `tests/neon_intrinsic_semantics.rs` (4 tests) pins
every intrinsic semantic the models rely on, plus engine-op vs scalar
Montgomery oracle on adversarial lanes, on native Apple Silicon via the
`neon-m1-native` workflow: GitHub Actions run 39923096483, host
`Apple M2 Pro (Virtual)`, aarch64 fail-closed check passed, lib tests
79/79, semantics tests 4/4, run success. Log archived as
`NEON_CI_RUN_39923096483_silicon_log.txt`
(sha256 cbbbdc9173fb3446acab9a26e17801c21d4c7b953fcb67fb99d911adcb3e6e91).

**Development audit note.** The check discipline caught two generator bugs
before sealing (inverted vbsl branch order — the swapped mutation returned
the wrong verdict, unsat, which flagged it; and a shift-width encoding
error — `bvlshr` by the 64-bit constant 2^32 is always zero, exposed by
counterexample-model evaluation). Both fixed; all scripts regenerated and
re-proven from the final generator.
