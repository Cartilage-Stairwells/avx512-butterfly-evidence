#!/bin/bash
# Clean-environment sealed rerun of the AVX-512 kernel SMT evidence.
# Stage C of the 2026-10-22 audit: runs every canonical script directly from
# audit/ with no stale processes, recording solver version, invocation,
# SHA-256 before and after each run, exit status, and result.
# Output: audit/seal_rerun_2026-10-22.txt
set -u
cd "$(dirname "$0")"
OUT=seal_rerun_2026-10-22.txt

{
echo "=== Sealed rerun of AVX-512 kernel SMT evidence ==="
echo "date_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "host: $(uname -srm)"
echo "z3: $(z3 --version)"
echo "invocation: z3 <script> (no options; timeout 1800s per script)"
echo "expected: unsat for 19 main scripts, sat for 6 mutations"
echo
} > "$OUT"

run_one() {
    local f="$1" expect="$2"
    local h_before h_after res code
    h_before=$(sha256sum "$f" | cut -d' ' -f1)
    res=$(timeout 1800 z3 "$f" 2>&1)
    code=$?
    h_after=$(sha256sum "$f" | cut -d' ' -f1)
    echo "script: $f  (expect $expect)" >> "$OUT"
    echo "  sha256_before: $h_before" >> "$OUT"
    echo "  result: $res" >> "$OUT"
    echo "  exit_status: $code" >> "$OUT"
    echo "  sha256_after:  $h_after" >> "$OUT"
}

MAIN="avx512_add_op.smt2 avx512_sub_op.smt2
mont_mul_16_pair0.smt2 mont_mul_16_pair1.smt2 mont_mul_16_pair2.smt2
mont_mul_16_pair3.smt2 mont_mul_16_pair4.smt2 mont_mul_16_pair5.smt2
mont_mul_16_pair6.smt2 mont_mul_16_pair7.smt2
mont_mul_16_pair2_even.smt2 mont_mul_16_pair2_odd.smt2
avx512_fused_butterfly_pair0.smt2 avx512_fused_butterfly_pair1.smt2
avx512_fused_butterfly_pair2.smt2 avx512_fused_butterfly_pair3.smt2
avx512_fused_butterfly_pair4.smt2 avx512_fused_butterfly_pair5.smt2
avx512_fused_butterfly_pair6.smt2 avx512_fused_butterfly_pair7.smt2"
MUT="mutation_wrong_mu_32lane.smt2 mutation_wrong_p_32lane.smt2
mutation_no_underflow_32lane.smt2 mutation_sub_blend_swapped.smt2
mutation_add_cmpgt.smt2 mutation_fused_blend_swapped.smt2"

for f in $MAIN; do run_one "$f" "unsat"; done
for f in $MUT;  do run_one "$f" "sat";   done

echo >> "$OUT"
echo "=== rerun complete: $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" >> "$OUT"
