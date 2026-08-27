# Float32 Parametric Map

Checked: 2026-08-26  
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `derived/parametric-map/float32_ct_derived_explicit_le`
- Recipe ID: `derived_parametric_map_float32_ct_derived_explicit_le`
- Validation: Float Pixel Data, multi-frame dimensions, Real World Value
  Mapping, and source-image references

## Required Decision

The Phase 1 proof is a multi-frame Parametric Map derived from the generated CT
sorting series. It uses 32-bit native floating-point samples in Float Pixel Data
`(7FE0,0008)`, not integer Pixel Data `(7FE0,0010)`. Each output frame corresponds
to one source CT instance and preserves the source spatial order. A linear Real
World Value Mapping with explicit units and quantity definition describes the
stored float values.

## KB Query

- Tool: `dicom-standard-kb`
- Input: `lookup uid ParametricMapStorage --edition 2026b`
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: Parametric Map Storage SOP Class UID
  `1.2.840.10008.5.1.4.1.1.30`
- Why insufficient: the committed registry query establishes the SOP Class but
  does not capture the conditional pixel module, mandatory multi-frame modules,
  or Real World Value Mapping macro attributes needed for this recipe.

## Official Source Evidence

- Part: PS3.3
- Anchors: A.75.1, Table A.75-1, C.7.6.24, C.7.6.16.2.11,
  Table C.7.6.16-12, C.8.32.1, and C.8.32.2
- Source artifact identity: the PS3.3 source artifact is recorded as
  `unavailable_not_downloaded` in `standards.lock.json`; the official 2026b
  CHTML was reviewed without committing a copy.
- Evidence summary: the Parametric Map IOD represents integer or floating-point
  Real World Values in a multi-frame image. Table A.75-1 requires the Floating
  Point Image Pixel Module for 32-bit floating pixels and requires Multi-frame
  Functional Groups, Multi-frame Dimension, Parametric Map Image, and
  Acquisition Context. The Floating Point Image Pixel Module uses Float Pixel
  Data and omits Bits Stored, High Bit, and Pixel Representation. The Real World
  Value Mapping macro requires floating-point slope and intercept when Float
  Pixel Data is present and records measurement units; Quantity Definition may
  identify the represented quantity. Common Instance Reference is required when
  derivation or referenced-image functional groups are present.

## Project Action

- Registry status: implemented after the external proof is integrated and
  independently validated
- Generator policy: highdicom/pydicom may construct the object, but Rust must
  independently reopen it and verify identities before promotion
- Independent validation: locked dicom3tools IOD validation plus a
  backend-independent Rust semantic decoder for values, dimensions, mappings,
  and references
- Should become KB patch: yes; module and attribute queries should eventually
  replace the narrow note
- Expected cleanup: retain the implementation invariant while replacing the KB
  gap explanation after equivalent pinned queries are available
