# Phase 4 WSI tile-referencing segmentation

- Affected case: `derived/seg/wsi_tile_reference`
- Recipe: `derived_seg_wsi_tile_reference` version `0.1.0`
- Source case: `vl/wsi/tiled_full_small`
- Checked: 2026-08-27
- Resulting registry action: remain planned until the locked external route,
  independent Rust reconstruction, and both IOD gates qualify; then promote to
  implemented without changing the contract below.

## Locked decision

Generate one Explicit VR Little Endian Segmentation Storage instance with the
locked `highdicom_pydicom` backend. The instance is semantic-stable, belongs to
`extended`, and shares the source WSI Study and Frame of Reference. It encodes
one `FRACTIONAL`/`OCCUPANCY` segment with Maximum Fractional Value 255,
`MONOCHROME2`, unsigned 8/8/7 pixels, Lossy Image Compression `00`, and Segments
Overlap `NO`.

The SEG is a 4 by 4 `TILED_SPARSE` total pixel matrix with 2 by 2 stored Frames.
Only the two non-empty diagonal source tiles are represented:

| SEG Frame | Source WSI Frame | Column/row | X/Y/Z | Stored pixels |
| --- | --- | --- | --- | --- |
| 1 | 1 | 1/1 | 0/0/0 | `ff 00 00 ff` |
| 2 | 4 | 3/3 | 1/1/0 | `00 ff ff 00` |

Source WSI Frames 2 and 3 are intentionally absent from the derivation graph.
The ordered payload SHA-256 is
`74fa7cbb10160e0eb1f16f35fa9ad0e7f2712af56019996e88cf1034be92635e`.
The Frame SHA-256 values are
`34aaa746c25a0f105c4316bbb1f009aa359f49582656ee97d73c58132d563423`
and
`10db5223d19bd1d58c2b8eb3c723b0ba104cf17564f9434e53e1b9e642fb3b37`.
The zero-filled 4 by 4 reconstructed matrix SHA-256 is
`a8ec6f910c0fb02685163a3251bed92517d1016c9173f1e4f021e6b4194f2467`.

Shared Functional Groups contain exactly Pixel Measures and Segment
Identification. Each Per-Frame Functional Groups Item contains exactly Frame
Content, Plane Position (Slide), and Derivation Image. Dimension indices are
Referenced Segment Number, Row Position in Total Image Pixel Matrix, and Column
Position in Total Image Pixel Matrix. Their ordered values are `[1,1,1]` and
`[1,2,2]`. Each derivation binds the exact source SOP Class, SOP Instance, and
Referenced Frame Number, `Spatial Locations Preserved = YES`, purpose
`(121322, DCM, "Source Image for Image Processing Operation")`, and derivation
`(113076, DCM, "Segmentation")`. Common Instance Reference contains exactly the
source WSI Series and Instance.

The SEG inherits the source total-matrix origin, orientation, pixel measures,
focal-plane count, specimen UID, and container identifier. It does not contain
patient-coordinate functional groups, `TILED_FULL`, pyramid or concatenation
metadata, references to source Frames 2 or 3, palette or ICC data, pixel
padding, lossy ratio or method, tracking identifiers, or algorithm
identification metadata. The segment uses the already-qualified project Tissue
and Organ coded semantics and `MANUAL` algorithm type.

The resource ceiling is exactly one derived instance and two stored Frames,
at most 16 KiB and at most five seconds for the backend invocation. Generation
must fail rather than add Frames, omit a required reference, or exceed a
ceiling.

FRACTIONAL is deliberate. The locked highdicom 0.28.1 runtime separately pads
sub-byte BINARY Frames, while native one-bit Pixel Data requires a continuous
bit stream. The locally locked official sources do not include PS3.5 bytes, so
this milestone does not make an unaudited Part 5 assumption. The 8-bit contract
also makes Rust/DICOM-rs payload reconstruction independent of the generating
highdicom/pydicom implementation.

## Standards evidence

The pinned `dicom-standard-kb` 2026b queries
`dicom_lookup_sop_class Segmentation Storage`,
`dicom_lookup_uid ExplicitVRLittleEndian`, `dicom_lookup_iod Segmentation`,
`dicom_list_modules_for_iod Segmentation`, and module attribute lookups for
Segmentation Image, Multi-frame Dimension, Microscope Slide Layer Tile
Organization, and Common Instance Reference all succeeded. Their source
manifest SHA-256 is
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`.
The broad text query `Segmentation Whole Slide Microscopy referenced frame
source image` returned no match, so exact text was retrieved from the official
2026b source through the KB fallback surface.

The controlling official anchors are PS3.3 A.51 and Table A.51-1; A.51.5,
Table A.51-2, and A.51.5.1; C.7.6.16.2.6 and Table C.7.6.16-7; Table 10-3;
C.7.6.17.3; C.8.20.2 and Table C.8.20-2; C.8.20.4 and Table C.8.20-4; and
C.8.12.14. SOP and transfer-syntax identity are anchored by PS3.4 Table B.5-1
and B.5.1.25, plus PS3.6 Tables A-1 and 6-1.

The exact official-source hashes recorded by the authorized independent
validator lock are PS3.3
`4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`,
PS3.4
`8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`,
PS3.6
`512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`,
IOD definitions
`ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683`,
and module definitions
`9f4853924ef520dd9b97ada0f14abd206fb15e6d8622e4d24a90f8b404a3e8c3`.
This narrow source note is sufficient; no reusable KB parser gap was found.

## Independent qualification requirement

Rust/DICOM-rs must independently reopen the SEG and source WSI, prove the
complete identity and reference graph, decode both 8-bit Frames, reconstruct
the zero-filled total matrix, and match every locked hash and absence. Locked
`dciodvfy` and the separately implemented authorized `uv`-locked
dicom-validator must each report zero IOD errors; DCMTK parsing is additive.
The generating highdicom/pydicom backend is never counted as its own payload or
reference authority.
