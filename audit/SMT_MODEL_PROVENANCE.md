# Provenance binding for the sealed SMT models

## What this is, in plain terms

An inspector signs off on a building using a set of drawings. The drawings are
safe in a vault, sealed and numbered. But nothing on the drawings says which
building they describe. If someone renovates the building and leaves the vault
closed, the seal still checks, the drawings still match themselves, and the
inspection still reports clean, while describing a building that no longer
exists.

That was the state of this directory. The 42 `.smt2` models are sealed and their
digests verify. `gen_smt_avx512.py` builds them from Python constants and reads
neither the implementation nor the test that pins the intrinsics, so nothing tied
a proved model to a particular edition of `src/vector/avx512.rs`.

`SMT_MODEL_BINDINGS.json` records, for each sealed model, its digest, together
with the digests of the two source files the premise chain names. In technical
terms it is a **content-addressed manifest binding proof artifacts to the
implementation revision they describe**. `verify_smt_bindings.py` re-checks both
and reports three outcomes separately, never merged:

| line | meaning |
|---|---|
| `SCRIPT_INTEGRITY` | the models are the bytes that were bound |
| `SOURCE_BINDING` | `src/vector/avx512.rs` and the semantics test are the bytes that were bound |
| `RESULT` | `PASS`, `FAIL`, or `INCOMPLETE` |

Exit 0 is PASS, 1 is FAIL, 2 is INCOMPLETE. A missing source file reports
INCOMPLETE rather than PASS, because an absent comparison is not a successful
comparison.

## What this establishes, and what it does not

**Established.** Editing `src/vector/avx512.rs` without regenerating the proofs
and rebinding is now detectable, and the command that detects it ships here.
Measured: a one-byte change to a model gives `SCRIPT_INTEGRITY: FAIL` and names
the file; a source edit gives `SOURCE_BINDING: FAIL` and names the file; deleting
the source gives `INCOMPLETE` and exit 2.

**Not established, and not claimable.** This binds the models to the source as of
the record's `bound_at_utc`, which is after the models were generated. It does not
prove that the October scripts were generated from these exact bytes. That
question cannot be answered retroactively, and the record states so in its
`generation_provenance` field rather than implying otherwise.

## The domain qualification

Every production script guards its inputs with `bvult` against `#x78000001`, so
the universal claims range over the **reduced domain** only: inputs already below
the BabyBear prime. Whether the kernel's contract admits unreduced inputs is not
established by any artifact in this directory. If it does, these proofs do not
cover that case and the verification document should say so explicitly.

## Running it

    python3 audit/verify_smt_bindings.py
    python3 audit/verify_smt_bindings.py --source-root /path/to/full/checkout

From the repository root the source files are present and the whole check runs.
