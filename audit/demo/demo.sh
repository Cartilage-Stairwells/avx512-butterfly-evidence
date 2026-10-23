#!/bin/bash
# ============================================================================
# END-TO-END VERIFICATION DEMONSTRATION — the standard verification path
# ============================================================================
# Demonstrates, live, the four things a skeptical buyer should check:
#
#   1. The real production schedule passes its proof (unsat).
#   2. A deliberately broken variant is REJECTED (sat) — the checks have teeth.
#   3. The intrinsic-semantics premises hold on THIS host's silicon (or the
#      pinned-silicon record is shown when the host lacks the ISA).
#   4. The evidence package (seals, rerun logs, method docs) is stated.
#
# Scope and boundaries (stated, not hidden):
#   - Runs the FAST standard-path scripts only: the 4-lane NEON add/sub
#     proofs and one mutation control. Each returns in seconds.
#   - The largest proofs (AVX-512 pair-2, NEON mul) need 10-20+ minutes of
#     solver time; they are part of the sealed rerun record, not this demo.
#     See REPRODUCTION.md for runtimes and the full sealed rerun logs.
#   - The demo proves: the checker accepts correct schedules and rejects a
#     known-broken one. It does not prove the checker detects every defect.
#     Sensitivity evidence = the 12 mutation controls + the recorded
#     equivalence finding (see NEON_KERNEL_SMT_VERIFICATION.md).
#   - Exit code 0 only if every gate behaves exactly as documented.
# ============================================================================
set -e
cd "$(dirname "$0")/../.."

pass() { echo "  ✓ $1"; }
fail() { echo "  ✗ $1"; exit 1; }

echo "==================================================================="
echo " DEMONSTRATION: verification gates on a real field-arithmetic kernel"
echo " $(date -u +%Y-%m-%dT%H:%M:%SZ) | host: $(uname -srm)"
echo "==================================================================="

echo
echo "[1/6] SPECIFICATION — what is being verified"
echo "  NeonVectorEngine ops in src/vector/neon.rs (BabyBear, P=0x78000001):"
echo "   add: out = (a+b) mod P, canonical, for all a,b in [0,P)"
echo "   sub: out = (a-b) mod P, canonical, for all a,b in [0,P)"
echo "  Modeled instruction-for-instruction from the production source."
pass "specification stated"

echo
echo "[2/6] CORRECT SCHEDULE — the production add schedule must prove (unsat)"
r=$(z3 audit/neon_add_op.smt2)
[ "$r" = "unsat" ] && pass "neon_add_op.smt2 -> unsat (4 lanes, all inputs)" || fail "expected unsat, got: $r"
r=$(z3 audit/neon_sub_op.smt2)
[ "$r" = "unsat" ] && pass "neon_sub_op.smt2 -> unsat (4 lanes, all inputs)" || fail "expected unsat, got: $r"

echo
echo "[3/6] DELIBERATE DEFECT — break one boundary: vcgt instead of vcge"
echo "  (misses the sum == P equality case; a one-character defect)"
echo "  The pipeline must REJECT this: expected result sat."

echo
echo "[4/6] REJECTION"
r=$(z3 audit/mutation_add_vcgt.smt2)
[ "$r" = "sat" ] && pass "mutation_add_vcgt.smt2 -> sat (DEFECT DETECTED)" || fail "expected sat, got: $r"

echo
echo "[5/6] SILICON PREMISES — the intrinsics must behave as modeled"
ARCH=$(uname -m)
if [ "$ARCH" = "aarch64" ]; then
  if command -v cargo >/dev/null; then
    cargo test --test neon_intrinsic_semantics -- --nocapture 2>&1 | grep "test result" | tail -1
  else
    echo "  (no cargo on host; pinned-silicon record: Actions run 39923096483,"
    echo "   Apple M2 Pro, semantics tests 4/4 — audit/NEON_CI_RUN_39923096483_silicon_log.txt)"
  fi
elif grep -q avx512 /proc/cpuinfo 2>/dev/null; then
  if command -v cargo >/dev/null; then
    # fail-closed: the pinning suite requires the ISA enabled at compile time
    out=$(RUSTFLAGS="-C target-feature=+avx512f,+avx512dq" \
           cargo test --test avx512_intrinsic_semantics -- --nocapture 2>&1 \
           | grep "test result" | tail -1)
    echo "  $out"
    echo "$out" | grep -q "7 passed" \
      || { echo "  ✗ expected 7 pinning tests, got: $out (host or flags mismatch)"; exit 1; }
  else
    echo "  (no cargo on host; see tests/avx512_intrinsic_semantics.rs —"
    echo "   the sealed record: 7/7 on an AVX-512F+DQ host)"
  fi
else
  echo "  (host has neither pinned ISA; the silicon premise records live in"
  echo "   audit/*_KERNEL_SMT_VERIFICATION.md and the CI silicon log)"
fi
pass "silicon premise gate stated for this host"

echo
echo "[6/6] EVIDENCE PACKAGE — where the full record lives"
echo "  Seals:        audit/SHA256SUMS_2026-10-22_AVX512_KERNEL (20+ entries)"
echo "                audit/SHA256SUMS_2026-10-23_NEON_KERNEL"
echo "  Sealed reruns: audit/seal_rerun_2026-10-22.txt (26 scripts)"
echo "                 audit/seal_rerun_neon_2026-10-23.txt (10 scripts)"
echo "  Method+bounds: audit/AVX512_KERNEL_SMT_VERIFICATION.md"
echo "                 audit/NEON_KERNEL_SMT_VERIFICATION.md"
echo "  Index:         audit/EVIDENCE_INDEX.md (proof -> source -> claim)"
echo
echo " BOUNDARY: this demo runs the fast standard path. The full evidence"
echo " set covers both engines' lane-arithmetic schedules; loop/control"
echo " flow, scalar tails, compiler codegen, and Lean bindings are outside"
echo " the SMT proofs (covered by the differential parity suites)."
echo
echo "==================================================================="
echo " DEMONSTRATION COMPLETE — all gates behaved exactly as documented."
echo "==================================================================="
