#!/bin/bash
# Clean-environment sealed rerun of the NEON kernel SMT evidence.
# Mirrors rerun_sealed.sh (AVX-512 set). Runs every canonical NEON script
# directly from audit/ with no stale processes, recording solver version,
# invocation, SHA-256 before and after each run, exit status, and result.
# Output: audit/seal_rerun_neon_2026-10-23.txt
set -u
cd "$(dirname "$0")"
OUT=seal_rerun_neon_2026-10-23.txt

{
echo "=== Sealed rerun of NEON kernel SMT evidence ==="
echo "date_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "host: $(uname -srm)"
echo "z3: $(z3 --version)"
echo "invocation: z3 <script> (no options; timeout 3600s per script)"
echo "expected: unsat for 3 main scripts + mutation_redc_wrong_p (output-"
echo "  equivalent by analysis); sat for the 6 other mutations"
echo
} > "$OUT"

run_one() {
    local f="$1" expect="$2"
    local h_before h_after res code
    h_before=$(sha256sum "$f" | cut -d' ' -f1)
    res=$(timeout 3600 z3 "$f" 2>&1)
    code=$?
    h_after=$(sha256sum "$f" | cut -d' ' -f1)
    echo "script: $f  (expect $expect)" >> "$OUT"
    echo "  sha256_before: $h_before" >> "$OUT"
    echo "  result: $res" >> "$OUT"
    echo "  exit_status: $code" >> "$OUT"
    echo "  sha256_after:  $h_after" >> "$OUT"
}

MAIN="neon_add_op.smt2 neon_sub_op.smt2 neon_mont_mul_vec.smt2
mutation_redc_wrong_p.smt2"
MUT="mutation_add_vcgt.smt2 mutation_sub_bsl_swapped.smt2
mutation_redc_wrong_neg_inv.smt2 mutation_redc_wrong_p_final.smt2
mutation_redc_no_cond_sub.smt2 mutation_redc_m_zero.smt2"

for f in $MAIN; do run_one "$f" "unsat"; done
for f in $MUT;  do run_one "$f" "sat";   done

echo >> "$OUT"
echo "=== rerun complete: $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" >> "$OUT"
