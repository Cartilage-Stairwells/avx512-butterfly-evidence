# Evidence Index — proof → source → claim → seal

**Purpose.** For an independent reviewer: every SMT proof obligation, what
production code it covers, what it claims, over what input domain, with what
solver, where the sealed result lives, and what remains outside the proof.
Modeled after the collaborator review of 2026-10-23 ("make the proof boundary
explicit; make it difficult to confuse a proof of the model with a proof of
the production implementation").

**Global facts.** Solver: Z3 5.3.1 (64-bit), invoked `z3 <script>`, no
options. Field: BabyBear P = 0x78000001, R = 2^32. Input domain everywhere:
all lane values in [0, P) (the documented engine precondition; the SMT
scripts assert it per lane). Proof method: negated lane-identity conjunction,
`unsat` = proven for ALL inputs in the domain. Source revisions: the commits
that sealed each set (below) — the sealed rerun logs hash every script
executed, and the seals hash the scripts, method docs, and logs.

**The model↔implementation binding (applies to every row).** Each generator
(`audit/gen_smt_avx512.py`, `audit/gen_smt_neon.py`) encodes the production
source file (`src/vector/avx512.rs`, `src/vector/neon.rs`) instruction-for-
instruction: constants, operand order, lane mappings, mask semantics. The
intrinsic semantics the models rely on are pinned empirically by the silicon
suites (`tests/avx512_intrinsic_semantics.rs` 7/7 on an AVX-512F+DQ host;
`tests/neon_intrinsic_semantics.rs` 4/4 on Apple M2 Pro, Actions run
39923096483). OUTSIDE every proof: loop/control flow, kernel-layer scalar
tails, memory layout, compiler code generation (`confuse_compiler` is
modeled as identity), and any Lean↔Rust binding — covered instead by the
differential parity suites (179 passed / 0 failed, full suite, 2026-10-22).

## AVX-512 engine — sealed 2026-10-22 (commit 7710507; rerun log `seal_rerun_2026-10-22.txt`; seal `SHA256SUMS_2026-10-22_AVX512_KERNEL`)

| Proof script(s) | Production op (src/vector/avx512.rs) | Claim (all 16 lanes) | Result |
|---|---|---|---|
| `avx512_add_op.smt2` | `avx512_add` (add_epi32 + cmpge mask + mask_sub) | out = (a+b) mod P, canonical | unsat |
| `avx512_sub_op.smt2` | `avx512_sub` (sub + adjust + cmpge + blend) | out = (a−b) mod P, canonical | unsat |
| `mont_mul_16_pair{0..7}.smt2` (+ pair-2 split halves, same assertion) | `mont_mul_16` (32-bit-lane subtraction-form REDC: mul_epu32 products, MU=0x88000001, masked merges, underflow correction) | out = a·b·R⁻¹ mod P, canonical, via the exact identity out·R + q·P = prod + (u ? P·R : 0) | 8× unsat |
| `avx512_fused_butterfly_pair{0..7}.smt2` | composed butterfly op (add + sub/blend + mul on diff) | add claim ∧ explicit sub claim (diff = (a−b) mod P, canonical) ∧ REDC claim — the full DIF identity | 8× unsat |

Mutation controls (all sat — the checker rejects each defect): wrong MU,
wrong schedule P, dropped underflow correction, swapped blend (sub),
cmpgt-instead-of-cmpge (add), swapped blend (fused). The fused control
initially returned unsat and exposed a missing sub assertion — the scripts
were strengthened before sealing (documented in the method doc).

## NEON engine — sealed 2026-10-23 (commit 8e1ce66; rerun log `seal_rerun_neon_2026-10-23.txt`; seal `SHA256SUMS_2026-10-23_NEON_KERNEL`)

| Proof script(s) | Production op (src/vector/neon.rs) | Claim (all 4 lanes) | Result |
|---|---|---|---|
| `neon_add_op.smt2` | `add` (vaddq + vsubq + vcgeq + vbslq) | out = (a+b) mod P, canonical | unsat |
| `neon_sub_op.smt2` | `sub` (vsubq + vaddq + vcltq + vbslq) | out = (a−b) mod P, canonical | unsat |
| `neon_mont_mul_vec.smt2` | `mul` = `neon_mont_mul_vec` + `neon_reduce_2` (vmull + vmovn + vmul + vmlal + vshrn + conditional sub; NEG_INV=0x77FFFFFF) | out = a·b·R⁻¹ mod P, canonical, via out·R + (ge ? P·R : 0) = prod + m·P | unsat |

No fused NEON script exists: the engine has no fused op (recorded decision —
it delegates to the generic kernel). Mutation controls: 6 observable defects
all sat (vcgt edge, swapped vbsl, wrong NEG_INV, wrong P at the final
subtraction, dropped conditional subtraction, m=0). `mutation_redc_wrong_p`
(P+1 in vmlal) is output-EQUIVALENT — proven by lemmas 1–2 (both unsat); its
direct script timed out at 3600 s (exit 124, recorded in the rerun log), and
the equivalence lemmas carry the disposition.

## Run times (for reproduction planning)

Seconds: add/sub scripts (both engines), all 6+6 observable mutations, the
pair-2 conjunction halves, the equivalence lemmas, and `audit/demo/demo.sh`
(the end-to-end demonstration: proof → deliberate defect → rejection →
silicon pinning → evidence pointers; standard path only). Tens of minutes:
`mont_mul_16_pair2.smt2` (10–20 min; wide solver variance), the fused pairs
(~2–4 min each), `neon_mont_mul_vec.smt2` (15–20 min), and the
wrong_p direct confirmation (not concluded in 3600 s; supplementary to the
lemmas). Long experiments are recorded as such — never silently excluded.

## Access note (updated 2026-10-23)

This evidence is PUBLICLY accessible: the curated bundle
`Cartilage-Stairwells/avx512-butterfly-evidence` (public repository, pushed
from the sealed commit 78f7a88 of the private development repo — byte-
identical files, verify against the SHA256SUMS seals above). The public
repository `Cartilage-Stairwells/tscp-anchor` carries the Lean proofs and the
claims statement. The private development repository retains working
material that is intentionally unpublished (including privileged and
personal documents — the reason the full repo is not public and only this
curated evidence bundle is).
