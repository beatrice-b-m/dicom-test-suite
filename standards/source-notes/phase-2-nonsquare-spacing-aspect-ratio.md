# Phase 2 Non-square Spacing and Aspect-ratio Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/sc/nonsquare_pixel_spacing`
- Recipe ID: `classic_sc_nonsquare_pixel_spacing`
- Secondary Capture Image Storage, two deterministic files, native unsigned
  8-bit pixels, spatial-calibration metadata, manifests, validation, reports,
  and independent semantic evidence

## Required Decision

Implement two files under the existing one logical case. This is not a
Cartesian product: `SYSTEM_SPEC.md` Section 9.1 requires both independent
viewer axes.

1. `pixel-spacing.dcm` declares Pixel Spacing `(0028,0030)` and Nominal
   Scanned Pixel Spacing `(0018,2010)` as DS VM 2, both exactly `0.6\\0.3`.
   Pixel Aspect Ratio is absent. Equal Pixel and Nominal Scanned Pixel Spacing
   means the values are not a calibration correction; Pixel Spacing
   Calibration Type and Description remain absent. Image Position (Patient),
   Image Orientation (Patient), and Frame of Reference UID are absent, so this
   file does not claim patient-space Image Plane geometry.
2. `pixel-aspect-ratio.dcm` declares Pixel Aspect Ratio `(0028,0034)` as IS
   VM 2 exactly `2\\1`. Pixel Spacing, Nominal Scanned Pixel Spacing, and
   Imager Pixel Spacing are all absent. The first integer is vertical extent
   and the second is horizontal extent, yielding the same 2:1 row-to-column
   physical ratio without a millimetre calibration claim.

Both files use the same 4-row by 6-column MONOCHROME2 unsigned 8-bit image so
only the spatial interpretation changes. Their 24 checkerboard samples are
`0,255,0,255,0,255,255,0,255,0,255,0` repeated twice. Pixel Data SHA-256 is
`e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6`.

Do not combine all three spatial attributes in one file. Such an object can be
conformant when the values agree, but it would not test aspect-ratio behavior
when every physical-spacing attribute is absent.

## KB Query

- Tool: `dicom_lookup_iod`, `dicom_lookup_data_element`,
  `dicom_list_modules_for_iod`, `dicom_resolve_attribute_context`,
  `dicom_search_standard_text`, and `dicom_retrieve_standard_text`
- Input: Secondary Capture Image IOD; Pixel Aspect Ratio, Pixel Spacing, and
  Nominal Scanned Pixel Spacing; PS3.3 Sections A.8.1.3, C.7.6.3.1.7,
  C.7.6.3.3, C.8.6.2, 10.7.1.1, and 10.7.1.3
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the KB resolves the mandatory SC Image and Image Pixel modules, the
  optional Image Plane module, registry VR/VM, conditional Pixel Aspect Ratio,
  and spacing/calibration semantics.
- Why insufficient: no one typed result expresses the two-file project
  partition, exact lexical values, required absences, common payload, or the
  cross-file ratio equivalence. This source note freezes those decisions.

## Official Source Evidence

- PS3.3 Table A.8-1 makes the Image Pixel and SC Image Modules mandatory for
  Secondary Capture while Image Plane is optional.
- PS3.3 Table C.7-11c makes Pixel Aspect Ratio Type 1C when the ratio is not
  1:1 and Pixel Spacing, Imager Pixel Spacing, and Nominal Scanned Pixel
  Spacing are absent. Section C.7.6.3.1.7 requires positive vertical and
  horizontal integer extents in that order.
- PS3.3 Table C.8-25 defines Nominal Scanned Pixel Spacing as Type 3 and
  requires consistency with Pixel Aspect Ratio when both are present.
- PS3.3 Table 10-10 permits Pixel Spacing when it is not a calibration
  correction. Section 10.7.1.1 says equality with Nominal Scanned Pixel
  Spacing represents the uncorrected value. Section 10.7.1.3 defines the first
  DS value as row/vertical spacing and the second as column/horizontal spacing,
  both positive for this multi-row, multi-column image.
- PS3.6 Table 6-1 defines Pixel Aspect Ratio as IS VM 2 and both spacing
  attributes as DS VM 2.

## Validation and Qualification Plan

- The manifest contract is reserved to this case and records the variant ID,
  exact attribute presence/absence, VR/VM and lexical values, 2:1 ratio,
  uncalibrated state, absent patient-space geometry, and common pixel hash.
- Corpus validation requires exactly the two named variants and independently
  reopens every DICOM attribute and payload. Swapped ratios, zero or missing
  components, mismatched Pixel and Nominal spacing, injected off-variant tags,
  and missing variants are failures.
- Locked `dciodvfy` remains the primary IOD validator and `dcentvfy` the entity
  checker. The existing `uv`-locked pydicom/dicom-validator runtime will gain a
  case-scoped semantic mode that independently reads the exact tags, VM,
  positivity, ratio equivalence, image dimensions, and pixel hash. Generic IOD
  success alone cannot prove an intentional absence.

## Project Action

- Registry status: planned until both files complete generation, strict
  validation, reporting, deterministic regeneration, and independent
  conformance.
- Registry reason: deterministic two-variant implementation is outstanding.
- Should become KB patch: yes; expose the conditional Pixel Aspect Ratio
  alternatives and calibration equality rule as typed context.
- Expected cleanup after KB coverage exists: replace local condition
  interpretation with typed KB evidence while retaining exact project variant
  and payload decisions.
