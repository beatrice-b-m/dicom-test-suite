# Phase 3 Complex Object Status

Checked: 2026-08-27  
Standards baseline: DICOM 2026b

## Parametric Map milestone

The dependency-ordered Parametric Map milestone has reached the furthest
coverage currently supported by its locked providers:

- `derived/parametric-map/float32_ct_derived_explicit_le` remains implemented
  through the optional uv-locked highdicom/pydicom backend.
- `derived/parametric-map/float64_ct_derived_explicit_le` is implemented through
  the same backend with a distinct binary64 recipe and native OD payload.
- `derived/parametric-map/integer_ct_derived_explicit_le` remains planned. Its
  selected cross-implementation provider is locked dcmqi 1.5.7, whose
  Parametric Map converter exposes only a floating-point image type and cannot
  emit the required integer OW module. This is recorded as
  `provider_capability_unavailable`, not hidden as reduced coverage.

The integer provider blocker does not apply to validation: locked dicom3tools
recognizes an independently constructed integer Parametric Map feasibility
object. The remaining blocker is generation capability, so changing providers
would be a separate architectural decision rather than a validation workaround.

## Float64 contract

The float64 recipe derives three 2 by 2 frames from the generated CT spatial
sorting series. It multiplies stored CT values by `0.25` and adds a spatial-rank
increment of `2^-30`, preserving distinctions below binary32 precision. Rust
recomputes every binary64 word from the CT sources before accepting the backend
output. The manifest binds:

- Double Float Pixel Data `(7FE0,0009)` with OD VR;
- 64 Bits Allocated and absent OF, OW, Bits Stored, High Bit, Pixel
  Representation, and Planar Configuration;
- a 96-byte native payload and three exact frame hashes;
- per-frame derivation, Common Instance References, multi-frame dimensions,
  Real World Value Mapping, and backend fingerprints; and
- `render_double_float_pixels` as the explicit consumer capability.

Two seed-7 extended runs generated 88 files each. Both float64 Part 10 files
were byte-identical with SHA-256
`1f50196e425771c51284f03893826e7dcb7910b4529190445151e26677358d21`;
strict internal corpus validation reported zero failures.

## Independent qualification

The primary IOD result is finding-free locked dicom3tools `dciodvfy -new`.
Locked DCMTK 3.7.0 independently extracts OD values, reconstructs 96 exact
little-endian bytes, and matches these frame hashes:

1. `921a8e74cc86e767d5436be2a4eb0c6d383bf3f210ec4c32e8f8c43c239f8abe`
2. `be480ba76c1931f10052029005c539dd45b565f7020cc94a41a89825c3b6ea44`
3. `ce1600d46bb7468f4a0f60c2d58cf96430234a89e50f0cacdd56bfd86bc3ec90`

The uv-locked pydicom `dicom-validator` candidate was assessed under the
authorized independent-validator checkpoint. Its locked 2026b definition set
reports nine Parametric Map functional-group macro gaps for both floating
widths, so it was not promoted over the clean dciodvfy oracle and no findings
were allowlisted. It remains qualified only for its existing U32 and non-square
SC routes.

Generated corpora and conformance evidence remain disposable and ignored. The
next Phase 3 dependency milestone is the linked TID 1500 Measurement Report
slice; it must retain PixelMed plus primary IOD validation and resolve every
source/derived reference before registry promotion.
