#!/usr/bin/env bash
# Supplementary sealed rerun: the NEON wrong_p lemma scripts (and, optionally,
# the equivalence script).
#
# Why this exists: NEON_KERNEL_SMT_VERIFICATION.md argues that
# mutation_redc_wrong_p is output-equivalent to production via two lemmas, but
# the original sealed rerun (seal_rerun_neon_2026-10-23.txt) never executed the
# lemma or equivalence scripts. This script runs them under the SEALED solver
# version and writes a log in the style of the existing rerun logs. It never
# modifies an existing sealed file and refuses to overwrite an existing log.
#
# Place at audit/rerun_sealed_neon_lemmas.sh and run from anywhere.
#
# What counts as a result:
#   REQUIRED  lemma1 and lemma2 must return "unsat" (documented, seconds each).
#   OPTIONAL  RUN_EQUIV=1 also runs mutation_redc_wrong_p_equiv.smt2. The docs
#             give it NO expected verdict ("committed for the record"), and it
#             did not conclude in a 300 s local trial under z3 5.3.2. (The
#             separate DIRECT script mutation_redc_wrong_p.smt2 is the one
#             that timed out in the original 3600 s window; the equivalence
#             script was not in that rerun at all.) A timeout (exit 124) or
#             "unknown" is INCONCLUSIVE:
#             it is never a pass. A definite sat/unsat is recorded without
#             judgment.
#
# Environment:
#   LEMMA_TIMEOUT=300    per-lemma timeout, seconds
#   EQUIV_TIMEOUT=3600   timeout for the equivalence script (RUN_EQUIV=1 only)
#   RUN_EQUIV=0|1        include the equivalence script (default 0)
#   ALLOW_OTHER_Z3=1     run under a z3 other than the sealed version; the
#                        deviation is recorded and the log is NOT a sealed-
#                        solver rerun
#
# The z3 binary is hashed before the version check, again after it, and before and
# after every solve; any difference is an integrity failure (exit 1, no seal
# commands). A hostile binary can still lie or swap and restore within a solve,
# and the recorded hash only means something when compared with a known-good one.
#
# Exit status:
#   4  verdicts complete but under a non-sealed solver; not sealable
#   0  both lemmas unsat, hashes match the manifest, nothing changed mid-run,
#      and (if requested) the equivalence script gave a definite verdict
#   1  a lemma was not unsat, a hash mismatched, or a script changed mid-run
#   2  environment problem (z3 missing or wrong version, file missing,
#      log already exists)
#   3  lemmas passed but the requested equivalence run was INCONCLUSIVE
set -uo pipefail

cd "$(dirname "$0")/.." || exit 2          # repository root
SEALED_Z3="5.3.1"
LEMMA_TIMEOUT="${LEMMA_TIMEOUT:-300}"
EQUIV_TIMEOUT="${EQUIV_TIMEOUT:-3600}"
RUN_EQUIV="${RUN_EQUIV:-0}"
MANIFEST="audit/SHA256SUMS_2026-10-23_NEON_KERNEL"
LEMMA1="audit/mutation_redc_wrong_p_lemma1.smt2"
LEMMA2="audit/mutation_redc_wrong_p_lemma2.smt2"
EQUIV="audit/mutation_redc_wrong_p_equiv.smt2"
DATE_TAG="$(date -u +%F)"
OUT="audit/seal_rerun_neon_lemmas_${DATE_TAG}.txt"

if [ -e "$OUT" ] && grep -qx "outcome: PASS" "$OUT" 2>/dev/null; then
  echo "refusing to overwrite $OUT, which records a passing run" >&2; exit 2
fi
for f in "$MANIFEST" "$LEMMA1" "$LEMMA2"; do
  [ -f "$f" ] || { echo "missing $f" >&2; exit 2; }
done
[ "$RUN_EQUIV" = "1" ] && { [ -f "$EQUIV" ] || { echo "missing $EQUIV" >&2; exit 2; }; }

# Resolve ONE real executable file and use it for the version check, the hash and
# every solve. type -P ignores shell functions and aliases, so an exported
# function named z3 cannot answer --version while a different binary runs the
# scripts (timeout and other external commands never see shell functions).
Z3_BIN="$(type -P z3)"
if [ -z "$Z3_BIN" ] || [ ! -f "$Z3_BIN" ] || [ ! -x "$Z3_BIN" ]; then
  echo "no z3 executable file found on PATH (shell functions and aliases are not accepted)" >&2; exit 2
fi
Z3_SHA="$(sha256sum "$Z3_BIN" 2>/dev/null | cut -d' ' -f1)"
[ -n "$Z3_SHA" ] || { echo "cannot hash ${Z3_BIN}" >&2; exit 2; }
Z3_VER="$("$Z3_BIN" --version)"
[ "$(sha256sum "$Z3_BIN" 2>/dev/null | cut -d' ' -f1)" = "$Z3_SHA" ] || { echo "${Z3_BIN} changed while its version was being read" >&2; exit 2; }
Z3_NOTE=""
if ! printf '%s' "$Z3_VER" | grep -qF "version ${SEALED_Z3} "; then
  if [ "${ALLOW_OTHER_Z3:-0}" != "1" ]; then
    echo "z3 is '${Z3_VER}', sealed runs used ${SEALED_Z3}." >&2
    echo "Install ${SEALED_Z3}, or set ALLOW_OTHER_Z3=1 to run anyway (deviation is recorded)." >&2
    exit 2
  fi
  Z3_NOTE="NOTE: run under a different solver than the sealed ${SEALED_Z3} runs (ALLOW_OTHER_Z3=1); not a sealed-solver rerun"
fi

# Preflight has passed: only now set aside a prior non-passing log for today, so
# a failed preflight never leaves the operator with no current log.
if [ -e "$OUT" ]; then
  mv "$OUT" "${OUT%.txt}.superseded_$(date -u +%H%M%S).txt"
fi

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

RESULT=""; RC=0; INTEGRITY_OK=1

# run_one <path> <timeout> <expectation text>
# Prints a log block; sets RESULT, RC, INTEGRITY_OK (globals).
run_one() {
  local f="$1" to="$2" expect="$3" before sealed after out zb za errf first
  before="$(sha256sum "$f" | cut -d' ' -f1)"
  sealed="$(awk -v p="$f" '$2==p {print $1}' "$MANIFEST")"
  INTEGRITY_OK=1
  echo "script: $(basename "$f")  (${expect})"
  echo "  sha256_before: ${before}"
  if [ "$before" != "$sealed" ]; then
    echo "  manifest_check: MISMATCH (manifest has ${sealed:-nothing})"
    INTEGRITY_OK=0
  else
    echo "  manifest_check: matches ${MANIFEST}"
  fi
  zb="$(sha256sum "$Z3_BIN" 2>/dev/null | cut -d' ' -f1)"
  errf="$(mktemp)"
  out="$(timeout "$to" "$Z3_BIN" "$f" 2>"$errf")"; RC=$?
  za="$(sha256sum "$Z3_BIN" 2>/dev/null | cut -d' ' -f1)"
  # RESULT is always one word: a solver verdict, "error", or empty for a timeout.
  # Anything else (shell or solver diagnostics) goes to the separate stderr line.
  first="$(printf '%s\n' "$out" | head -n1)"
  case "$first" in
    sat|unsat|unknown) RESULT="$first" ;;
    *) RESULT="error" ;;
  esac
  [ "$RC" -eq 124 ] && RESULT=""
  [ "$RC" -ne 0 ] && [ "$RC" -ne 124 ] && RESULT="error"
  echo "  result: ${RESULT}"
  echo "  exit_status: ${RC}"
  if [ "$RESULT" = "error" ] && [ -n "$first" ]; then echo "  stdout_first_line: $(printf '%s' "$first" | tr -cd '[:print:]' | cut -c1-200)"; fi
  if [ -s "$errf" ]; then echo "  stderr_first_line: $(head -n1 "$errf" | tr -cd '[:print:]' | cut -c1-200)"; fi
  rm -f "$errf"
  after="$(sha256sum "$f" | cut -d' ' -f1)"
  echo "  sha256_after:  ${after}"
  if [ "$before" != "$after" ]; then
    echo "  WARNING: script changed during run"
    INTEGRITY_OK=0
  fi
  echo "  z3_sha256_before: ${zb}"
  echo "  z3_sha256_after:  ${za}"
  if [ "$zb" != "$Z3_SHA" ] || [ "$za" != "$Z3_SHA" ]; then
    echo "  WARNING: z3 binary differs from the preflight hash (${Z3_SHA})"
    INTEGRITY_OK=0
  fi
}

fail=0; inconclusive=0; SUMMARY=""; FAILED_IDS=""; EQUIV_STATE="not run"
{
  echo "=== Supplementary sealed rerun of NEON wrong_p lemma scripts ==="
  echo "date_utc: $(date -u +%FT%TZ)"
  echo "host: $(uname -srm)"
  echo "z3: ${Z3_VER}"
  # Recorded so a later reader can compare against a known-good solver binary.
  # This does not prevent substitution (a stub can print any version); it makes
  # substitution detectable by anyone who holds the real binary's hash.
  echo "z3_binary: ${Z3_BIN} sha256 ${Z3_SHA}"
  [ -n "$Z3_NOTE" ] && echo "$Z3_NOTE"
  echo "invocation: z3 <script> (no options; timeout ${LEMMA_TIMEOUT}s per lemma, ${EQUIV_TIMEOUT}s for equivalence)"
  echo "required: lemma1 unsat, lemma2 unsat (NEON_KERNEL_SMT_VERIFICATION.md)"
  echo "optional: equivalence script, no documented expected verdict; timeout/unknown = INCONCLUSIVE, never a pass"
  echo "scope: supplements seal_rerun_neon_2026-10-23.txt, which did not execute these scripts"
  echo

  for f in "$LEMMA1" "$LEMMA2"; do
    run_one "$f" "$LEMMA_TIMEOUT" "expect unsat"
    if [ "$INTEGRITY_OK" = "1" ]; then SUMMARY+="$(basename "$f" .smt2)=${RESULT:-none}(hash-ok,rc${RC}) "
else SUMMARY+="$(basename "$f" .smt2)=${RESULT:-none}(INTEGRITY-FAIL,rc${RC}) "; fi
    if [ "$INTEGRITY_OK" != "1" ]; then fail=1; FAILED_IDS+="$(basename "$f")(integrity: script or z3 hash) "
elif [ "$RESULT" != "unsat" ]; then fail=1; FAILED_IDS+="$(basename "$f")(result=${RESULT:-none}) "; fi
  done

  if [ "$RUN_EQUIV" = "1" ]; then
    run_one "$EQUIV" "$EQUIV_TIMEOUT" "no documented expected verdict; recorded for the record"
    if [ "$INTEGRITY_OK" != "1" ]; then
      fail=1; FAILED_IDS+="$(basename "$EQUIV")(integrity: script or z3 hash) "; EQUIV_STATE="INTEGRITY FAILURE"
    elif [ "$RESULT" = "sat" ] || [ "$RESULT" = "unsat" ]; then
      EQUIV_STATE="verdict ${RESULT} (recorded; no documented expectation, not counted as a check)"
    else
      inconclusive=1; EQUIV_STATE="INCONCLUSIVE (exit ${RC}) - not a pass"
    fi
  fi

  echo
  echo "summary: ${SUMMARY}"
  echo "equivalence: ${EQUIV_STATE}"
  if [ "$fail" -ne 0 ]; then echo "outcome: FAIL"
  elif [ "$inconclusive" -ne 0 ]; then echo "outcome: INCONCLUSIVE"
  elif [ -n "$Z3_NOTE" ]; then echo "outcome: DEVIATION"
  else echo "outcome: PASS"; fi
  echo "=== rerun complete: $(date -u +%FT%TZ) ==="
} >"$TMP"

mv "$TMP" "$OUT"
echo "wrote ${OUT}"
echo "summary: ${SUMMARY}"
echo "equivalence: ${EQUIV_STATE}"
if [ "$fail" -ne 0 ]; then
  echo "FAILED (${FAILED_IDS% }). Do not seal this log." >&2
  exit 1
fi
if [ "$inconclusive" -ne 0 ]; then
  echo "Lemmas passed, but the equivalence run was INCONCLUSIVE. Do not describe it as verified." >&2
  exit 3
fi
if [ -n "$Z3_NOTE" ]; then
  echo "NOT SEALABLE: this run used a non-sealed solver; the log records the deviation, and no seal commands are offered." >&2
  exit 4
fi
echo "OK. To seal (from the repo root):"
echo "  sha256sum audit/rerun_sealed_neon_lemmas.sh ${OUT} audit/REPRODUCTION.md > audit/SHA256SUMS_${DATE_TAG}_NEON_LEMMAS"
echo "  sha256sum -c audit/SHA256SUMS_${DATE_TAG}_NEON_LEMMAS"
