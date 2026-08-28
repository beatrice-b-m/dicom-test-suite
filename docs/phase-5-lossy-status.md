# Phase 5 Lossy Codec Status

Status date: 2026-08-28

## Promoted cases

Two policy-approved lossy cases are implemented in `extended`:

| Case | Encoder | Independent decoder | Observed result |
| --- | --- | --- | --- |
| `classic/sc/rgb_jpegxl_lossy` | `cjxl` 0.11.2, SHA-256 `5b7b6cdc09a1bdaef39e30d3660e29861a405fffc1bc1136f3bb91cfe6db658e` | jxl-oxide 0.10.2 through dicom-rs | RGB maximum errors `[8, 2, 7]`; overall RMSE `0.7918037162` |
| `classic/sc/mono2_u16_htj2k_lossy` | OpenJPH 0.27.3, SHA-256 `d21a8ea98ffce347928c34a2c51c61e424a068ca4eb746a6867a29d6c30b1627` | OpenJPEG through dicom-rs | maximum error `19`; overall RMSE `4.3548643779` |

JPEG XL uses fixed distance `0.05`, effort `7`, `num_threads=0`, raw codestream
output, and disabled container and modular modes. HTJ2K uses irreversible coding, quantization step
`0.00025`, two decompositions, no color transform, and LRCP progression. Each
manifest records the full argument vector and its fingerprint.

Both diagnostic images are 32 by 32 and contain ramps, hard edges, and either
color bars or high-contrast monochrome regions. Metrics cover every decoded
stored sample in declared channel order. The manifest and report record sample
counts, per-channel maximum error and RMSE, overall RMSE, compressed and
uncompressed byte counts, the computed ratio, DICOM ratio string, encoder
identity, and independent decoder identity. Lossy Image Compression is `01`;
methods are `ISO_18181_1` and `ISO_15444_15` respectively.

## Qualification

A feature-enabled seed-7 extended root generated 120 files and strict
validation checked all 120 with zero failures. Repeated lossy DICOM files were
byte-identical. The only whole-manifest difference between repeated extended
runs was the already modeled elapsed-millisecond field from an unrelated
external highdicom backend.

The final file hashes were:

- JPEG XL: `6fea71df4362f82ca20bcf5680a1dc16607f4b1e1d583c81ee78a39c0e55eff2`;
- HTJ2K: `906807fa9edde4a8648c1571949c8491b514666dfccdefb3aa2b8576e04e9450`.

dicom3tools `dciodvfy -new` exited successfully for both files. It emitted the
same reviewed advisory that empty Laterality is appropriate only when truly
unknown; these synthetic OT diagnostic images deliberately declare unknown
Laterality. DCMTK parsed both Part 10 files. Neither parser is used as pixel
proof; the independent decoders and numeric comparisons supply that evidence.

## Explicit unavailable coverage

- JPEG-LS Near-Lossless remains planned because the available writer and
  decoder are from the same CharLS implementation family.
- JPEG 2000 lossy remains planned until a writer/decoder pair with the required
  independence is integrated.
- JPEG Extended 12-bit remains planned until an independent 12-bit decoder is
  integrated.
- MPEG-2, AVC/H.264, and HEVC/H.265 remain planned because no pinned
  deterministic elementary-stream writer plus independent decode and timing
  validation route is integrated.
- A genuine greater-than-4-GiB Extended Offset Table file remains an opt-in
  full stress case. The small real EOT instance and non-file overflow
  qualification are already implemented.
