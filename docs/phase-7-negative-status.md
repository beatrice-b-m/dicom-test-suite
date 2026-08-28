# Phase 7 Negative Corpus Status

Status date: 2026-08-28

## Implemented slice

The `negative` profile generates all 15 registered deterministic mutation
cases. Each artifact is derived from a private, known-good source that is
removed before promotion. The manifest records the source identity and hash,
ordered mutation steps, exact parameters and changed byte ranges, the hash
chain, expected failure layers, and bounded acceptable and unacceptable
outcomes.

Expected-invalid files branch before the normal valid-DICOM reopen path.
They are excluded from valid source/reference closure and from the ordinary
coverage matrix. JSON and Markdown reports place them only in negative
coverage and label the built-in bounded classifier as `same_project`, never as
independent evidence. `negative` remains excluded from `all`.

Two seed-7 roots were byte-for-byte identical. A fresh root contained 15 files
and passed strict validation with zero failures. No private source artifacts
were present in either promoted root.

## Independent bounded tool exercise

All 15 generated payloads were exercised with a 10-second per-process wall
timeout through:

- DCMTK `dcmdump` 3.7.0, executable SHA-256
  `d2261944ea1ceb6743df9866f2237014b284fa39119c8a5eee226ae922ead45f`;
- dicom3tools `dciodvfy` snapshot `1.00.snapshot.20260803085716`, executable
  SHA-256
  `1aeb75d6ccd3f193e3b322b6da77742cdce2e0604868eaf2a2669c786cbc27e5`.

No invocation timed out, crashed, or terminated by signal. `dcmdump` rejected
8 payloads and completed with bounded diagnostic or parsed output for 7.
`dciodvfy` rejected 14 payloads; it completed successfully for the broken
Extended Offset Table case.

The latter behavior is an important qualification result, not a reason to
weaken the mutation. DCMTK `dcmdrle` 3.7.0 (SHA-256
`d63743af7ec1dc8f0af0dc7562e2c502e81c3af9f38a7b51de30e822de7c8daf`)
also decoded that object by ignoring the corrupt Extended Offset Table and
using the Fragment stream. The file remains deterministically invalid because
the first Extended Offset Table value is `2^64 - 1`; the result demonstrates
that consumers may safely ignore optional random-access metadata. The suite
therefore reports its checked mutation contract and same-project classification
separately from these external tool outcomes.

## Remaining Phase 7 work

The deterministic mutation milestone is complete. The remaining Phase 7 slice
is the opt-in `fuzz` profile: bounded seed descriptions, reproducible candidate
generation, deterministic minimization, distinct crash/hang/timeout reporting,
and promotion of valuable minimized inputs into named negative recipes without
committing fuzz-generated DICOM payloads.
