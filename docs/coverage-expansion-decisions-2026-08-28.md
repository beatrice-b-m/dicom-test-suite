# Coverage Expansion Decisions — 2026-08-28

These decisions resolve the explicit policy checkpoints in
`docs/coverage-expansion-plan.md`. They authorize implementation; they do not
waive any generation, independent-validation, determinism, or profile gate.

## Lossy Numeric Acceptance

All lossy comparisons use decoded stored samples in declared channel order and
bit-depth domain before display transforms. Every result records sample count,
per-channel maximum absolute error, per-channel RMSE, and overall RMSE.
Codestream bytes may vary only when the case declares semantic stability and
the executable fingerprint and fixed encoder options remain recorded.

Authorized initial thresholds:

| Case family | Source domain | Maximum absolute error | Aggregate threshold |
| --- | --- | ---: | ---: |
| JPEG-LS Near-Lossless | 8-bit monochrome, `NEAR=2` | 2 | overall RMSE ≤ 2 |
| JPEG XL lossy | 8-bit RGB | 8 per channel | overall RMSE ≤ 3 |
| HTJ2K lossy | 16-bit monochrome | 64 | overall RMSE ≤ 16 |

The diagnostic input is at least 32 by 32 and includes ramps, hard edges, and,
for RGB, color bars. Lossy metadata is required: Lossy Image Compression is
`01`; the ratio is uncompressed sample bytes divided by compressed codestream
bytes; and Lossy Image Compression Method matches the selected Transfer
Syntax. JPEG-LS remains explicitly unavailable until a decoder independent of
the CharLS generation family is proven. JPEG 2000 lossy and JPEG Extended
12-bit remain unavailable under their existing independence blockers.

## Stress Envelopes

Stress remains opt-in and excluded from `all` unless explicitly selected.

| Job class | Output ceiling | Peak RSS ceiling | Per-case wall time | Job wall time |
| --- | ---: | ---: | ---: | ---: |
| Reduced CI boundary | 256 MiB | 512 MiB | 2 minutes | 10 minutes |
| Scheduled/release full | 8 GiB | 2 GiB | 30 minutes | 2 hours |

Authorized requested scales:

- EOT/encapsulated: reduced 64 MiB Pixel Data, 256 Frames, 64 Fragments;
  full `4 GiB + 64 MiB`, at least two Frames, second Item offset greater than
  `0xFFFF_FFFF`, and 1024 Fragments.
- Enhanced CT: reduced 256 Frames at 64 by 64 unsigned 16-bit; full 8192 Frames.
- CT study: reduced 128 instances; full 2048 instances at 64 by 64 unsigned
  16-bit.
- native bulk data: reduced 64 MiB; full 1 GiB.
- nested Sequences: reduced depth 32 and 16 MiB; full depth 256 and 128 MiB.
- long metadata: reduced 1 MiB total/1024 Values; full 64 MiB/65536 Values,
  with every individual VR remaining within PS3.5 limits.
- WSI: reduced 1024 by 1024 RGB, 256 by 256 tiles, three levels; full 16384 by
  16384 RGB, 256 by 256 tiles, five levels, at most 512 MiB output.

Manifests record requested and actual instances, Frames, Fragments, payload and
output bytes, elapsed milliseconds, and peak RSS where the platform exposes it.
Full jobs are never added to ordinary CI.

## Media And Protocol Baseline

The first implementation is optional and DCMTK-first:

- use locked DCMTK tools for DICOMDIR generation and the initial DIMSE peer;
- retain dcm4che as the pinned independent second-peer target;
- keep Java, DCMTK media/protocol tools, and every network harness optional;
- use dedicated media and transaction schemas/reports rather than generation-
  backend responses or ordinary per-file conformance rows; and
- never treat DCMTK-versus-DCMTK results as independent interoperability
  evidence.

DICOMweb requires a separately pinned server before promotion. The preferred
target is a dcm4che/dcm4chee implementation, but choosing its exact Java
distribution or container packaging remains an implementation detail so long
as its dependency identity is locked and it remains opt-in.

## Synthetic PKI Fixtures

Repository-owned deterministic synthetic cryptographic fixtures are approved:

- a test-only root CA;
- signing, server, and client certificates and private keys;
- fixed subject names, serial numbers, validity interval, and extensions; and
- conspicuous documentation that none is trusted or suitable for production.

Private keys may be committed only because they are intentionally public test
fixtures. Generation and verification must use separate toolchains wherever
the promoted claim requires independence. The approval covers dataset digital
signatures, TLS transport, and authenticated User Identity tests; it does not
establish a Secure DICOM media creator or independent CMS verifier by itself.
