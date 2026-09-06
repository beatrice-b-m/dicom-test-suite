# Phase 2 One-bit Native Pixel Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/sc/mono2_u1_native`
- Recipe ID: `classic_sc_mono2_u1_native`
- Multi-frame Single Bit Secondary Capture IOD identity, packed native Pixel
  Data, cross-frame bit continuity, manifest semantics, validation, and reports

## Required Decision

Generate one two-frame, 3 by 3 Multi-frame Single Bit Secondary Capture
instance. Each frame contains nine samples, so frame two begins in the middle
of a byte. Concatenate all 18 samples before final Value Field padding and pack
the first sample into the least significant bit. The three significant payload
bytes are `55 55 01`; six unused high bits in the last significant byte are
zero, and a final zero byte pads the complete Value Field to even length. The
decoded frame patterns are:

```text
frame 1: 1 0 1 / 0 1 0 / 1 0 1
frame 2: 0 1 0 / 1 0 1 / 0 1 0
```

Set Samples per Pixel, Bits Allocated, and Bits Stored to one; High Bit and
Pixel Representation to zero; Photometric Interpretation to MONOCHROME2; and
omit Planar Configuration. Use Explicit VR Little Endian native Pixel Data
with OB VR. Number of Frames is two. Frame Increment Pointer references a
two-value Page Number Vector so the required multi-frame increment is
explicit without selecting the conditional Cine Module.

## KB Query

- Tool: `dicom_lookup_sop_class`, `dicom_lookup_uid`, `dicom_lookup_iod`,
  `dicom_list_modules_for_iod`, `dicom_list_attributes_for_module`,
  `dicom_search_standard_text`, and `dicom_retrieve_standard_text`
- Input: Multi-frame Single Bit Secondary Capture Image Storage and Image IOD;
  Image Pixel, Multi-frame, SC Multi-frame Image, and SC Multi-frame Vector
  Modules; PS3.3 Sections A.8.2.1 through A.8.2.4; PS3.5 Sections 8.1.1, A.2,
  and D.1
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the KB resolves the SOP Class and IOD, mandatory module set, exact
  single-bit content constraints, native multi-frame packing rule, and
  least-significant-bit-first example.
- Why insufficient: no gap remains for the selected recipe. This source note
  freezes the cross-part recipe decision and the deliberately non-byte-aligned
  frame boundary in one reviewable artifact.

## Official Source Evidence

- PS3.3 Section A.8.2.1 identifies the IOD as the modality-independent format
  for converted bitmap images and scanned documents.
- PS3.3 Table A.8-2 makes Image Pixel, Multi-frame, SC Multi-frame Image, and
  the other base modules mandatory; SC Multi-frame Vector is required when
  Number of Frames is greater than one.
- PS3.3 Section A.8.2.4 fixes Samples per Pixel, Bits Allocated, and Bits Stored
  at one; High Bit and Pixel Representation at zero; Photometric
  Interpretation at MONOCHROME2; and forbids Planar Configuration.
- PS3.3 Tables C.7-14 and C.8-25c require Number of Frames and Frame Increment
  Pointer and permit Page Number Vector as the referenced frame increment.
- PS3.5 Section 8.1.1 requires native one-bit multi-frame samples to continue
  across frame boundaries without per-frame padding. Only the complete Pixel
  Data Value Field receives even-length padding; this recipe exercises both
  rules in the same four-byte Value Field `55 55 01 00`.
- PS3.5 Section D.1 describes the packed stream from the least significant bit
  of the first Pixel Cell upward. Section A.2 permits OB or OW for native Pixel
  Data when Bits Allocated is at most eight; this recipe selects OB.
- PS3.4 Table B.5-1 and PS3.6 Table A-1 identify SOP Class UID
  `1.2.840.10008.5.1.4.1.1.7.1`; PS3.6 Table A-1 identifies Explicit VR Little
  Endian as `1.2.840.10008.1.2.1`.
- Source artifact identity: the locked DICOM 2026b KB source manifest above;
  official source artifacts remain uncommitted under repository policy.

## Project Action

- Registry status: implemented after the complete vertical slice passed
  deterministic generation, strict internal validation, case-scoped IOD
  validation, independent pixel decoding, manifest/report schema validation,
  and focused tamper tests.
- Registry reason: none; the former deterministic-recipe blocker is resolved.
- Should become KB patch: no; the current query surface covers the required
  IOD, module, content-constraint, and packing decisions.
- Expected cleanup after KB coverage exists: none.

## Qualification Evidence

- Two seed-1 `extended` generations produce the same bytes and manifests. The
  profile now contains 86 generated files and strict internal validation
  reports 86 passed, zero failed.
- The U1 Pixel Data Value Field is exactly `55 55 01 00`, SHA-256
  `9d6baf87a79d40ef2b145f92945a05cf156a2741e2c2834a3a7721d52757594b`.
  Independent frame hashes are
  `a6188710c09cfbc77383ee0588dec2f7affa6e03e78aa900e9ae597a8d8faba3`
  and
  `c520efb8f894a1125bb1a513a9b64ef957f7c2cd63835fd7e130357c47f989ae`.
- Locked dicom3tools `dciodvfy -new` identifies
  `MultiframeSingleBitSCImage` without findings; isolated `dcentvfy` is
  silent. The primary IOD validator remains independent of the Rust
  generator. Pydicom `dicom-validator` 0.8.2 was evaluated but rejected for
  this route because it did not enforce the A.8.2.4 single-bit constraints.
- Locked DCMTK `dcm2img` 3.7.0, executable SHA-256
  `6a6103a7c516814b5eb44f53d198b111cbaf1678de5952ab7d31961732f112d5`,
  decodes two exact PGM frames with maximum sample value one. Locked
  `dcmdump` independently extracts the four raw bytes.
- Negative controls are finding-driven rather than exit-code-only. Changing
  Bits Allocated/Stored/High Bit to `8/8/7` produces incorrect-length and
  enumerated-value errors with exit 1. Inserting forbidden Planar
  Configuration produces two conditional-presence errors even though this
  dicom3tools build exits 0, proving why normalized findings are authoritative.
