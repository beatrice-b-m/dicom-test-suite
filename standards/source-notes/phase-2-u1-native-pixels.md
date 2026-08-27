# Phase 2 One-bit Native Pixel Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/sc/mono2_u1_native`
- Recipe ID: `classic_sc_mono2_u1_native`
- Multi-frame Single Bit Secondary Capture IOD identity, packed native Pixel
  Data, cross-frame bit continuity, manifest semantics, validation, and reports

## Required Decision

Generate one three-frame, 3 by 3 Multi-frame Single Bit Secondary Capture
instance. Each frame contains nine samples, so frames two and three begin in
the middle of a byte. Concatenate all 27 samples before final Value Field
padding, pack the first sample into the least significant bit, and emit the
four-byte payload `aa aa 5e 07`. The unused high five bits of the last byte are
zero. The decoded frame patterns are:

```text
frame 1: 0 1 0 / 1 0 1 / 0 1 0
frame 2: 1 0 1 / 0 1 0 / 1 0 1
frame 3: 1 1 1 / 0 1 0 / 1 1 1
```

Set Samples per Pixel, Bits Allocated, and Bits Stored to one; High Bit and
Pixel Representation to zero; Photometric Interpretation to MONOCHROME2; and
omit Planar Configuration. Use Explicit VR Little Endian native Pixel Data
with OB VR. Number of Frames is three. Frame Increment Pointer references a
three-value Page Number Vector so the required multi-frame increment is
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
  Data Value Field receives even-length padding.
- PS3.5 Section D.1 describes the packed stream from the least significant bit
  of the first Pixel Cell upward. Section A.2 permits OB or OW for native Pixel
  Data when Bits Allocated is at most eight; this recipe selects OB.
- PS3.4 Table B.5-1 and PS3.6 Table A-1 identify SOP Class UID
  `1.2.840.10008.5.1.4.1.1.7.1`; PS3.6 Table A-1 identifies Explicit VR Little
  Endian as `1.2.840.10008.1.2.1`.
- Source artifact identity: the locked DICOM 2026b KB source manifest above;
  official source artifacts remain uncommitted under repository policy.

## Project Action

- Registry status: planned until the complete vertical slice passes internal
  and independent conformance validation.
- Registry reason: deterministic recipe implementation remains outstanding.
- Should become KB patch: no; the current query surface covers the required
  IOD, module, content-constraint, and packing decisions.
- Expected cleanup after KB coverage exists: none.

