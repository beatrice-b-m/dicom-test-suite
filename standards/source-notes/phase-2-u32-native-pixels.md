# Phase 2 Unsigned 32-bit Native Pixel Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/sc/mono2_u32_explicit_le`
- Recipe ID: `classic_sc_mono2_u32_explicit_le`
- Unsigned 32-bit Image Pixel attributes, native OW encoding, exact sample
  values, manifest semantics, validation, and report coverage

## Required Decision

Generate one 2 by 2 Secondary Capture Image Storage instance with unsigned
MONOCHROME2 samples `0`, `65535`, `2147483648`, and `4294967295`. Bits
Allocated and Bits Stored are 32, High Bit is 31, Pixel Representation is zero,
and Planar Configuration is absent. Pixel Data is native OW in Explicit VR
Little Endian, so the four samples are encoded as exact little-endian 32-bit
words and span both sides of the signed 32-bit boundary.

## KB Query

- Tool: `dicom_lookup_uid`, `dicom_list_modules_for_iod`,
  `dicom_list_attributes_for_module`, and `dicom_retrieve_standard_text`
- Input: Secondary Capture Image Storage; Secondary Capture Image IOD; Image
  Pixel Module; PS3.3 Sections A.8.1 and C.7.6.3; PS3.5 Sections 8.1.1 and A.2
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the parsed UID and IOD/module surfaces identify Secondary Capture
  Image Storage and its mandatory Image Pixel Module, but do not by themselves
  establish the permitted 32-bit native Pixel Data encoding and OW choice.
- Why insufficient: the existing registry evidence resolves only the SOP Class
  UID. The recipe needs the official IOD description and PS3.5 native encoding
  rules to prove that unsigned 32-bit integer samples are permitted and that
  their Pixel Data VR remains OW rather than OL.

## Official Source Evidence

- PS3.3 Section A.8.1.1 describes the legacy single-frame Secondary Capture
  Image IOD as having no constraint on pixel data format. Section A.8.1.3 and
  Table A.8-1 make the Image Pixel Module mandatory.
- PS3.3 Section C.7.6.3 and Table C.7-11a require the Image Pixel Module and
  its Pixel Data; Table C.7-11c supplies the Image Pixel Description Macro
  attributes used to describe the integer samples.
- PS3.5 Section 8.1.1 permits Bits Allocated to be one or a multiple of eight,
  requires Bits Stored not to exceed Bits Allocated, requires High Bit to equal
  Bits Stored minus one, and requires the complete Pixel Data Value Field to
  have even length. The selected 32/32/31 values satisfy those invariants.
- PS3.5 Section A.2 requires native Pixel Data with Bits Allocated greater than
  eight in Explicit VR Little Endian to use OW and little-endian encoding. Its
  note explicitly states that OL and OV are not used for Pixel Data even when
  Bits Allocated is 32 or 64.
- PS3.6 Table A-1 identifies Secondary Capture Image Storage as
  `1.2.840.10008.5.1.4.1.1.7` and Explicit VR Little Endian as
  `1.2.840.10008.1.2.1`.
- Source artifact identity: the locked DICOM 2026b KB source manifest above;
  official source artifacts remain uncommitted under repository policy.

## Project Action

- Registry status: planned until deterministic generation, typed manifest and
  report contracts, internal and manifest-driven validation, two-run byte
  identity, locked independent IOD validation, DCMTK raw extraction, and the
  frozen offline `uv`/pydicom unsigned decode gates all pass.
- Registry reason or linked issue: retain `recipe_unimplemented`; do not
  promote based only on generic Image Pixel validation.
- Should become KB patch: yes; the cross-part IOD-permission and native OW
  encoding decision should become a repeatable evidence query.
- Expected cleanup after KB coverage exists: replace the local cross-part
  fallback with linked KB evidence while retaining the exact case recipe and
  promotion record.
