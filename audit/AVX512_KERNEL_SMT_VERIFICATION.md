# AVX-512 Engine-Op SMT Verification — the 32-bit-lane production schedule

**Status:** executed and sealed 2026-10-22 on this seat (Z3 4.x via /usr/local/bin/z3,
see REPRODUCTION update for the exact version and hashes). This document, the
scripts in this directory, the generator, and the silicon-pinning test
`tests/avx512_intrinsic_semantics.rs` together close the gap between the public
outreach claim ("formally verified AVX-512 butterfly/NTT kernels") and the
technical reality, by making the reality match the claim.

## What is now formally verified (universal, machine-checked)

Every production lane operation of `Avx512VectorEngine`
(`src/vector/avx512.rs` — the ops the butterfly kernel composes) is proven
bit-exact **as written**, for ALL canonical inputs, in SMT:

| Op | Script(s) | Claim proven |
|---|---|---|
| `avx512_add` | `avx512_add_op.smt2` (16 lanes) | result = (a+b) mod P, canonical, for all a,b ∈ [0,P) — the cmpge/mask_sub schedule is correct including the sum == P boundary |
| `avx512_sub` | `avx512_sub_op.smt2` (16 lanes) | result = (a−b) mod P, canonical, for all a,b ∈ [0,P) — the blend-picks-raw-when-a≥b schedule is correct |
| `mont_mul_16` (relaned 32-bit REDC) | `mont_mul_16_pair{0..7}.smt2` | for every output lane: res·R + q·P = T + (underflow ? P·R : 0) with res < P — the vpmuludq/vmovshdup/masked-merge schedule computes a·b·R⁻¹ mod P exactly |
| `avx512_radix2_butterfly_32` (fused DIF butterfly) | `avx512_fused_butterfly_pair{0..7}.smt2` | per lane, all three asserted together: out_a = (a+b) mod P (canonical, exact); diff = (a−b) mod P (canonical, exact — explicit sub claim); out_b·R + q·P = diff·w + (u ? P·R : 0) with out_b < P (REDC). Conjunction = the full DIF identity (a+b, (a−b)·w·R⁻¹) |

P = 0x78000001 (BabyBear), R = 2^32, MU = 0x88000001 = P⁻¹ mod 2^32
(subtraction-form REDC; P·MU ≡ 1 mod 2^32 checked numerically).

**Composition argument (why pair scripts cover all 16 lanes):** the engine's
ops are lane-independent, and the REDC's even/odd product structure pairs lane
(2i, 2i+1) with butterfly (2i, 2i+1). Each pair script states the SAME closed
formula instantiated on that pair's own symbols; there are no cross-pair
constraints. Z3 proving the template unsat on each of the 8 pairs proves it
for all 16 lanes. The 16-lane monolithic scripts (all claims in one check-sat)
were also generated; they do not terminate in practical time (48 bit-blasted
multipliers in one query) and are not part of the sealed evidence. The pair
files are the fast, independently re-runnable form.

**What this does NOT cover (boundaries, stated plainly):**
- The scalar tail of the fused pass and all loop/control flow: covered by the
  differential parity suite and the 10k-case proptest, not SMT.
- The Lean NTT model remains a separate artifact (tscp-anchor); no theorem
  here binds Lean to Rust beyond what the claims statement already says.
- Memory ops (load/store) and the masked-shuffle *semantics* are premises,
  not conclusions: they are pinned empirically by
  `tests/avx512_intrinsic_semantics.rs` (7 tests, exact bit patterns, on real
  AVX-512F+DQ silicon) rather than derived.

## The premise chain (why the models are the silicon)

1. `tests/avx512_intrinsic_semantics.rs` asserts, on real hardware, the exact
   semantics of every intrinsic the schedule uses (blend picks the SECOND arg
   where the mask is set; mask_add/sub apply the operation where set, keep src
   otherwise; mul_epu32 multiplies even 32-bit lanes into 64-bit products;
   movehdup duplicates odd lanes; masked vmovshdup merges shuffled vs src by
   mask; cmpge includes equality). It also runs the production op sequences
   verbatim against the scalar Montgomery oracle on adversarial lanes
   (0, 1, P−1, P−2, wrap boundaries, 0x40000000+). Fails (not skips) without
   AVX-512F+DQ.
2. `audit/gen_smt_avx512.py` generates the SMT models using ONLY those pinned
   semantics, instruction-for-instruction from `src/vector/avx512.rs`.
3. Z3 proves each schedule's identity universally (unsat), and every mutation
   control is caught (sat).

If a compiler or hardware change ever invalidated a premise, the test fails
before any SMT evidence goes stale.

## Mutation controls (each must answer `sat`)

| Control | What it breaks |
|---|---|
| `mutation_wrong_mu_32lane.smt2` | MU → MU+1 in the REDC |
| `mutation_wrong_p_32lane.smt2` | schedule's P → P+2 (claims keep real P) |
| `mutation_no_underflow_32lane.smt2` | drops the underflow correction add |
| `mutation_sub_blend_swapped.smt2` | swaps the blend operands in sub (the classic "adjusted where a≥b" bug) |
| `mutation_add_cmpgt.smt2` | cmpgt instead of cmpge (misses sum == P) |
| `mutation_fused_blend_swapped.smt2` | the same blend bug in the fused butterfly |

All six returned `sat` on this seat (a counterexample exists for each broken
variant), while the corresponding production scripts returned `unsat`.

## Reproduction

```sh
z3 audit/avx512_add_op.smt2            # unsat
z3 audit/avx512_sub_op.smt2            # unsat
for i in 0 1 2 3 4 5 6 7; do z3 audit/mont_mul_16_pair$i.smt2; done        # unsat ×8
for i in 0 1 2 3 4 5 6 7; do z3 audit/avx512_fused_butterfly_pair$i.smt2; done  # unsat ×8
z3 audit/mutation_wrong_mu_32lane.smt2        # sat
z3 audit/mutation_wrong_p_32lane.smt2          # sat
z3 audit/mutation_no_underflow_32lane.smt2     # sat
z3 audit/mutation_sub_blend_swapped.smt2       # sat
z3 audit/mutation_add_cmpgt.smt2              # sat
z3 audit/mutation_fused_blend_swapped.smt2    # sat
RUSTFLAGS="-C target-feature=+avx512f,+avx512dq" cargo test --test avx512_intrinsic_semantics --release  # 7 passed
```

Exact hashes, the Z3 version, and the seat's captured outputs are recorded in
the REPRODUCTION update appended 2026-10-22 (`audit/REPRODUCTION.md`).

## Relationship to the previous sealed evidence

The earlier `mont_mul_16_subtraction_form.smt2` (sha256 f7ea08c4…) verified
the OLD 64-bit-lane REDC formulation's per-lane identity. The production
engine was relaned to the 32-bit schedule on 2026-10-18 (Firebird iteration 1,
commit cac1c8a) — the old script's lane identity is mathematically the same,
but the *schedule* (even/odd products, masked merges) was never formally
verified as written. This evidence set closes exactly that gap: the
production instruction sequence is now what is proven, not just its ancestor's
math. The old script and its record remain untouched.
