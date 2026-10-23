# avx512-butterfly-evidence

Public evidence bundle for the SMT verification of the AVX-512 and NEON
BabyBear (P = 0x78000001) butterfly kernel lane-arithmetic schedules.

## Try it in one command — don't take our word for it

```
curl -fsSL https://raw.githubusercontent.com/Cartilage-Stairwells/avx512-butterfly-evidence/main/demo/try.sh | bash
```

Seconds to run. You will watch a solver PROVE a real production kernel
schedule correct for every possible input, watch the same checker CATCH a
deliberately broken variant, and verify the files you ran against the
published SHA256 seal. Needs only `z3` (the script tries to install it).
The demo states plainly what it establishes and what it does not.

**What this repo is.** A curated, public copy of the verification evidence
from the private development repository `Cartilage-Stairwells/avx512-butterfly`
at commit `78f7a8833da66e68b8a339d8a50ef5fe9262fbd0` (2026-10-23). The
development repo also contains private working material (notes, strategy,
archives) that is intentionally NOT published; everything verification-
relevant is here.

**Provenance and integrity.** The files here are byte-identical copies.
Verify against the seals inside `audit/`:

- `audit/SHA256SUMS_2026-10-22_AVX512_KERNEL` — AVX-512 seal (20+ entries)
- `audit/SHA256SUMS_2026-10-23_NEON_KERNEL` — NEON seal (20 entries)

Run `sha256sum -c audit/SHA256SUMS_2026-10-23_NEON_KERNEL` from the repo
root (paths differ only by the `audit/` prefix the seals use — the sealed
scripts, logs, and docs are all inside `audit/`). The sealed rerun logs
record per-script hashes, solver version, and exit statuses.

**Start here.**

- `audit/EVIDENCE_INDEX.md` — every proof obligation: source → model →
  claim → input domain → solver → sealed result → what stays outside the
  proof.
- `audit/AVX512_KERNEL_SMT_VERIFICATION.md` and
  `audit/NEON_KERNEL_SMT_VERIFICATION.md` — method, claims, boundaries.
- `audit/REPRODUCTION.md` — how to reproduce everything, with honest
  runtimes (seconds for the standard path; tens of minutes for the
  largest proofs; one recorded timeout).
- `audit/demo/demo.sh` — runnable end-to-end demonstration: production
  schedule proves (unsat) → a one-character defect is introduced → the
  pipeline rejects it (sat) → intrinsic-semantics premises pinned on the
  host silicon (fail-closed) → evidence package + explicit boundary.

**Reproduce.** With Z3 installed: `z3 audit/neon_add_op.smt2` (unsat, in
seconds). With Rust: run the intrinsic-semantics pinning suites in
`tests/`. The full evidence set for both engines is sealed in
`audit/seal_rerun_2026-10-22.txt` (26 scripts) and
`audit/seal_rerun_neon_2026-10-23.txt` (10 scripts).

**Boundaries (stated plainly).** The SMT proofs cover the lane-arithmetic
schedules of the engine ops under canonical-input preconditions with
intrinsics semantics pinned on real silicon. They do not cover loop/control
flow, scalar tails, compiler code generation, or any Lean↔Rust binding;
those are covered by the differential parity suites, not by the proofs.
Negative results (one 3600 s solver timeout, one provable output-
equivalence mutation finding) are recorded, not hidden.

Solver: Z3 5.3.1. Language: Rust. Field: BabyBear, P = 0x78000001, R = 2^32.

## License

Copyright 2026 Sean Christopher Southwick.

This repository is licensed under the Apache License 2.0. See [LICENSE](LICENSE).

The license applies to the repository's contents, including the kernel source files (byte-identical to avx512-butterfly), the audit artifacts, and the demo scripts. It governs reuse; it does not certify verification results beyond the documented evidence scope, which is governed by the audit records.
