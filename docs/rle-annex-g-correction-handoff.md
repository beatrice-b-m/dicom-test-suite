# RLE Annex G Correction Handoff

## Prepared corpus identity

- Generator implementation commit: `c7962df`
- Seed: `1`
- Profile: `extended`
- Prepared corpus root: `generated/viewer-rle-annex-g-c7962df/`
- Generated object root: `generated/viewer-rle-annex-g-c7962df/extended/`
- Manifest SHA-256:
  `2be9d253f8d255e70b50259a1005580352f5afc8aa14e03344ca64da9a4ddb8f`
- Frozen worklist content identity:
  `62dd0b4734b4c22a634c0f62355133f73710b8334a876ecd8ff80d1a4a365c94`
- Frozen worklist file SHA-256:
  `436b0e097bcc65d08466eae21b6c8bbdf8df3c2f971dbc764037dcb5b836ec57`

The immutable `generated/viewer-baseline-b4ea0a4/` evidence set was not
modified. The correction was generated into the separate root above.

## Correction

The project-owned RLE codec now maps each Annex G segment ordinal to the
corresponding native little-endian byte index in reverse order. For every
sample, the encoder therefore emits the most-significant byte plane first and
the internal decoder reconstructs the native little-endian bytes from that
standard segment order.

The independent DCMTK evidence adapter also no longer reverses extracted
multi-byte samples. `dcmdrle` produces Explicit VR Little Endian output and
`dcmdump +W` preserves those native bytes, so reversing them had hidden this
generator defect from the previous independent check.

## Regeneration comparison

The corrected `extended` root contains the same 58 RLE Lossless objects:

- 27 affected objects changed byte-for-byte and have new file hashes in the
  corrected manifest;
- 31 unaffected objects are byte-for-byte identical to the baseline;
- all 25 objects with Bits Allocated equal to 8 are unchanged;
- six 16-bit objects are also unchanged because their byte planes are
  byte-symmetric for the locked sample patterns;
- no declared decoded-frame hash changed;
- the original and corrected manifest SHA-256 values are respectively
  `a79753abc4722594211e61b294154d657386a3d270908378d9fac3c56168a3d1`
  and
  `2be9d253f8d255e70b50259a1005580352f5afc8aa14e03344ca64da9a4ddb8f`.

The confirmed CT example changed from file SHA-256
`3ef38657514e92fe196470cd25adf5ffb526ef9de078c193fd61da739eb5afef`
to
`c3f6a224f24da7b18910055e158e28459601f47fb77f067295de391f746797d4`.
Both DCMTK and DICOM View now decode its frame to the intended SHA-256
`d3e8d5fb105307e91174c36e8413e25cb8494efc509628cf515819478b217121`.

## Validation evidence

The project validator checked all 113 generated objects with zero failures.
The DICOM View scope freezer independently rehashed all 113 manifest-selected
files before creating the immutable worklist.

DCMTK 3.7.0 `dcmdrle` plus `dcmdump +W` independently decoded all 58 RLE
objects. All 75 frames, including every frame of every multiframe object,
matched their declared frame hashes. The conformance-run evidence SHA-256 is
`2913986efcd8f707d32a8809c79c8943e70141c7c6698b7febcf32c4eae26432`.

The representative independent checks included:

| Case | Covered axes | Frames | Result |
| --- | --- | ---: | --- |
| `classic/sc/mono2_i16_rle_lossless` | signed, MONOCHROME2 | 1 | exact |
| `classic/sc/mono1_u16_rle_lossless` | unsigned, MONOCHROME1 | 1 | exact |
| `classic/sc/mono2_u16_odd_3x3_rle_lossless` | odd-sized | 1 | exact |
| `classic/sc/mono1_i16_padding_multiframe_rle_lossless` | signed padding, MONOCHROME1, multiframe | 2 | exact |
| `classic/sc/rgb_planar0_multiframe_rle_lossless` | RGB, planar 0, multiframe | 2 | exact |
| `classic/sc/rgb_planar1_rle_lossless` | RGB, planar 1 | 1 | exact |
| `classic/sc/ybr_full_planar0_rle_lossless` | YBR_FULL, planar 0 | 1 | exact |
| `classic/sc/ybr_full_planar1_multiframe_rle_lossless` | YBR_FULL, planar 1, multiframe | 2 | exact |

DICOM View 0.2.10 at commit `7023932`, binary SHA-256
`c3a27d5494e1f659aca918c6e5f09f16bb99629b4ad0e289dc8a8739fe119d49`,
completed the 113-object compatibility campaign safely. For all 58 RLE
objects, the first and last display endpoints returned HTTP 200, the raw
endpoints returned HTTP 200, and every one of the 75 raw-frame hashes matched.
The report and normalized-report SHA-256 values are respectively
`03e44e89e7ea7ce24bf39ed1a1383f4645735c7b4cbcf0e8a0896da38b960cd7`
and
`af394831e3e8a538bb08c7573ace1e35179f8646f222631e97abaec7b952988e`.

## Reproduction commands

```sh
cargo run --locked --no-default-features -- generate \
  --profile extended \
  --out generated/viewer-rle-annex-g-c7962df/extended \
  --seed 1
cargo run --locked --no-default-features -- validate \
  generated/viewer-rle-annex-g-c7962df/extended
cargo run --locked --no-default-features -- conformance run \
  generated/viewer-rle-annex-g-c7962df/extended \
  --out generated/viewer-rle-annex-g-c7962df/conformance-extended
```

The DICOM View campaign artifacts are outside both repositories at
`/private/tmp/dcmview-rle-annex-g-c7962df-v3/`, as required by its compatibility
runner.
