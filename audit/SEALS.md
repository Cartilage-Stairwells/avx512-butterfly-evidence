# Seals in this directory

Every file here is a `sha256sum` manifest. Run any of them from the repository root.

| Manifest | Covers | Entries | Status |
|---|---|---|---|
| `SHA256SUMS_2026-11-07_AVX512_KERNEL` | the AVX-512 kernel evidence, current | 32 | verifies |
| `SHA256SUMS_2026-11-07_NEON_LEMMAS` | the supplementary NEON lemma rerun, its runner and log | 3 | verifies |
| `SHA256SUMS_2026-10-23_NEON_KERNEL` | the NEON kernel evidence, unchanged since that date | 20 | verifies |
| `SHA256SUMS_2026-10-22_AVX512_KERNEL` | the AVX-512 evidence as sealed on that date | 32 | verifies |

## The one edit to a sealed entry, disclosed

On 2026-11-07 the status header of `AVX512_KERNEL_SMT_VERIFICATION.md` was corrected from
"Z3 4.x" to "Z3 5.3.1", which both sealed rerun logs support. That changed a file covered by
the 2026-10-22 seal, so its single entry was re-pointed to the corrected bytes.

The manifest carries the original digest in a comment above the entry, so the change can be
read rather than guessed at. The alternative, leaving the entry stale, would have made the
seal that the README tells a reader to run fail with no explanation, which reads as tampering
even when nothing is wrong. Coverage and all other entries are unchanged.

This page is not itself sealed, so it can be corrected without invalidating a manifest.
