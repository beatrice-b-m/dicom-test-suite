# Phase 2 Enhanced MR Temporal Evidence

This source note records the locked DICOM 2026b review for
`enhanced/mr/multiframe_temporal_position_explicit_le`. Every successful query
below resolved against source manifest
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728` on
2026-08-26. The initial parsed-term lookups for Content Qualification, Complex
Image Component, and Acquisition Contrast returned `not_found`, so their terms
were reviewed from the cited persisted PS3.3 table text instead of being
guessed or treated as unavailable.

## Locked queries and decisions

- `dicom_resolve_attribute_context PatientPosition --iod "Enhanced MR Image"`
  resolves Patient Position `(0018,5100)` as Type 2C through General Series
  Table C.7-5a when Patient Orientation Code Sequence is absent. The recipe
  therefore writes the element with an empty value rather than omitting it or
  asserting an unsupported equipment-relative position.
- `dicom_retrieve_standard_text PS3.3 table_C.8-79` requires Burned In
  Annotation `NO` for non-legacy Enhanced MR, Lossy Image Compression `00` for
  this never-lossy synthetic payload, and Presentation LUT Shape `IDENTITY`
  for MONOCHROME2.
- `dicom_retrieve_standard_text PS3.3 table_C.8-83` requires Content
  Qualification and Applicable Safety Standard Agency for non-legacy Enhanced
  MR. The synthetic research fixture uses `RESEARCH` and `IEC`.
- `dicom_resolve_attribute_context ComplexImageComponent --iod "Enhanced MR
  Image"`, `dicom_resolve_attribute_context AcquisitionContrast --iod
  "Enhanced MR Image"`, and `dicom_retrieve_standard_text PS3.3 table_C.8-84`
  require both image-level and frame-level values. The unsigned synthetic
  intensity frames consistently use `MAGNITUDE` and `UNKNOWN`; no `MIXED`
  aggregate is claimed.
- `dicom_retrieve_standard_text PS3.3 table_C.8-88` confirms the shared MR
  Image Frame Type Sequence is the correct location when all frames agree.
- `dicom_retrieve_standard_text PS3.3 table_C.8-89` defines the retained MR
  Timing and Related Parameters macro. Retaining it preserves existing timing
  compatibility coverage, so the synthetic IEC acquisition explicitly records
  head SAR definition `IEC_HEAD`, SAR value `0.1` W/kg, and `IEC_NORMAL`
  operating modes for `STATIC FIELD`, `RF`, and `GRADIENT`. Empty sequences or
  bare `NORMAL` values are not substituted.
- `dicom_search_standard_text "69536005 T-D1100 Head" --part PS3.16` resolves
  current Common Anatomic Region CID 4031 coding as `(69536005, SCT, Head)`.
  The deprecated SNOMED-RT tuple `(T-D1100, SRT, Head)` is not retained.
- `dicom_retrieve_standard_text PS3.3 table_C.7.6.16.2.23-1` defines Temporal
  Position Time Offset `(0020,930D)` in **seconds**. The two offsets are 0.0 and
  1.5 seconds at the same patient-space plane, with Temporal Position Index and
  Dimension Index Values 1 and 2.

The frame type remains `DERIVED\\PRIMARY\\DYNAMIC\\NONE`. Changing it to
`ORIGINAL` would activate additional acquisition and timing conditions and
would misstate the semantic intent of this derived temporal compatibility
fixture.
