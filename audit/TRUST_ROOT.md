# Trust root for the SMT model binding

## In plain English

A notary stamps a document with a date and keeps a record that cannot be quietly
edited afterwards. Comparing two copies tells you they match. Only the date
record tells you which copy came first, and that is the part that stops someone
from rewriting history.

The binding files in this directory say which source files the proofs describe.
They are ordinary files, so anyone can edit them. Editing one is not the real
problem. The problem is editing the source *and* the binding together so the
pair still looks consistent. Two mutating parties keeping each other honest is
not honest, it is merely tidy.

So the answer that a matching pair cannot give is a date. This directory records
a digest of the binding, and that digest is timestamped outside this repository,
in Bitcoin, through OpenTimestamps. A later reader can then tell whether the
binding they hold is the one that existed before, or a fresh one dressed as old.

## Technically

Chain, deliberately acyclic so no file contains its own digest:

    M  = audit/SHA256SUMS_2026-11-07_SMT_PROVENANCE   (seals the binding artifacts)
    D  = audit/SMT_PROVENANCE_DIGEST.txt              (records sha256 of M; D is NOT in M)
    P  = audit/SMT_PROVENANCE_DIGEST.txt.ots          (OpenTimestamps proof over D; P is NOT in M)

A reader verifies in three steps:

    1. sha256sum -c audit/SHA256SUMS_2026-11-07_SMT_PROVENANCE   -> artifacts are the sealed ones
    2. sha256sum audit/SHA256SUMS_2026-11-07_SMT_PROVENANCE      -> equals the value in D
    3. ots verify audit/SMT_PROVENANCE_DIGEST.txt.ots            -> D existed at the recorded time

## What this does and does not establish

ESTABLISHED: the binding existed, unchanged, no later than the timestamped time.
A re-cut binding therefore cannot claim to predate this one, because Bitcoin's
block ordering fixes the earlier claim.

NOT ESTABLISHED: that the binding is *correct*. A timestamp proves a document is
old, never that it is true. If the recorded digests never matched the source in
the first place, this chain dates the error faithfully. Correctness rests on
running verify_smt_bindings.py against the source, which is a separate check.

ALSO NOT ESTABLISHED: that a reader fetched D from an honest channel. An
attacker who controls every copy the reader can see can withhold this record.
The defence is that the record is cheap to mirror, so distributing it widely is
the mitigation, not a stronger file format.
