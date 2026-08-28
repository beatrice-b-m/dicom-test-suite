# Phase 5 Lossy Codec Selection And Numeric Evidence

Checked: 2026-08-28
Standards baseline: 2026b, `standards.lock.json`
Policy: `docs/coverage-expansion-decisions-2026-08-28.md`

## Shared Stored-Sample Contract

Lossy validation compares independently decoded stored samples before modality,
VOI, palette, ICC, or other display transforms. Channel order and bit depth are
declared by the recipe. Every promoted case records sample count, per-channel
maximum absolute error and RMSE, overall RMSE, compressed codestream bytes,
uncompressed sample bytes, and the exact metadata Values for Lossy Image
Compression `(0028,2110)`, Lossy Image Compression Ratio `(0028,2112)`, and
Lossy Image Compression Method `(0028,2114)`.

The first diagnostic frames are at least 32 by 32. Monochrome frames contain
full-domain ramps and hard edges; RGB frames add saturated and mixed color bars.
The metric implementation uses integer absolute differences and a floating-
point mean of squared differences over the complete decoded sample population.
It does not sample pixels or use visual inspection as acceptance evidence.

Authorized thresholds are:

| Transfer Syntax | Stored sample domain | Maximum error | Overall RMSE |
| --- | --- | ---: | ---: |
| JPEG-LS Near-Lossless `.81`, `NEAR=2` | unsigned 8-bit monochrome | 2 | 2 |
| JPEG XL lossy `.112` | unsigned 8-bit RGB | 8 per channel | 3 |
| HTJ2K lossy `.203` | unsigned 16-bit monochrome | 64 | 16 |

## Selected Implementable Backends

### JPEG XL lossy

The selected encoder is the optional external command `cjxl` 0.11.2. The
locally qualified executable SHA-256 is
`5b7b6cdc09a1bdaef39e30d3660e29861a405fffc1bc1136f3bb91cfe6db658e`.
Input is a binary PPM with interleaved RGB samples. The wrapper fixes distance,
effort, thread, metadata, and container/codestream options and records the full
argument vector plus executable fingerprint. The promoted output is a raw JPEG
XL codestream encapsulated under UID `1.2.840.10008.1.2.4.112`.

The selected independent decoder is the pinned Rust `jxl-oxide` path already
used by DICOM-rs, not `djxl` from the same libjxl implementation as `cjxl`.
Generator-side libjxl decode may be retained as a diagnostic but does not count
as independent evidence. Byte-stable codestreams are not required; the case is
semantic-stable under a fixed executable fingerprint and option set.

### HTJ2K lossy

The selected encoder is the existing optional OpenJPH `ojph_compress` wrapper,
extended with irreversible transform and a fixed `qstep` selected to remain
inside the approved numeric limits. The locally qualified executable SHA-256 is
`d21a8ea98ffce347928c34a2c51c61e424a068ca4eb746a6867a29d6c30b1627`.
Input remains the proven unsigned 16-bit big-endian PGM path. The promoted raw
codestream uses UID `1.2.840.10008.1.2.4.203`.

The independent decoder is the pinned OpenJPEG-backed DICOM-rs HTJ2K reader.
OpenJPH and OpenJPEG are separate codec implementations. The wrapper records
the executable fingerprint, `qstep`, reversible flag, decomposition count, and
the complete argument vector. Promotion requires the approved maximum-error and
RMSE thresholds over the entire frame.

## Explicitly Unavailable Rows

- JPEG-LS Near-Lossless remains planned. The approved numeric policy removes
  its policy blocker, but the available generator and decoder both belong to
  the CharLS implementation family. DCMTK's `dcmdjpls` route does not establish
  the required independence.
- JPEG 2000 lossy remains planned. The installed project writer, DICOM-rs
  reader, and ImageMagick delegate all use OpenJPEG lineage.
- JPEG Extended 12-bit remains planned until a decoder independent of the
  DCMTK generation route is integrated.
- Video remains planned until an elementary-stream probe and decoder
  independent of its encoder are installed and locked.

Unavailable rows retain their exact blocker codes in registry, manifest, and
coverage reports. Approval of a tolerance does not convert same-implementation
decode into independent evidence.

## Standards Evidence

The repository's pinned 2026b KB identifies the four Transfer Syntax UIDs and
PS3.6 Table A-1 names. PS3.3 Image Pixel Module rules require lossy compression
metadata when lossy compression has been applied. PS3.5 Section A.4 supplies
the encapsulated Pixel Data Item layout. The existing transfer-syntax evidence
and backend fingerprints are recorded in
`transfer-syntax/backend-decisions.json` and
`transfer-syntax/capability-matrix.json`.

Promotion requires generator, manifest, reopened-file validation, exact metric
reporting, two-run semantic reproducibility, clean primary IOD validation, and
the independent decoder named above. No lossy row enters `core` under this
decision.
