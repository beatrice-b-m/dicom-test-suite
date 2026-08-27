# Phase 2 DA, TM, DT, and Timezone Boundary Evidence

Checked: 2026-08-26  
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `metadata/sc/timezone_boundaries`
- Recipe ID: `metadata_sc_timezone_boundaries`
- Manifest, raw validation, normalization, and reporting rules for DA, TM, DT,
  and Timezone Offset From UTC

## Required Decision

Generate two independent Secondary Capture instances so the instance-wide
Timezone Offset From UTC remains unambiguous:

1. `positive_max`: `20240229`, `235959.999999`,
   `20240229235959.999999+1400`, and `+1400`, normalizing to
   `2024-02-29T09:59:59.999999Z`.
2. `negative_min`: `20240301`, `000000.000000`,
   `20240301000000.000000-1200`, and `-1200`, normalizing to
   `2024-03-01T12:00:00.000000Z`.

The slice therefore exercises leap-day rollover, both legal timezone extrema,
six fractional digits, maximum-length DT values, and exact even-length DICOM
padding without assigning conflicting offsets to one instance.

## KB Queries

- Tool: `dicom_search_standard_text`
- Inputs: `DA VR YYYYMMDD range leap year`, `TM VR HHMMSS fractional midnight
  24 hours`, `DT VR timezone offset +1400 -1200`, and `Timezone Offset From UTC
  +1400 -1200`; PS3.5 filter used for the VR queries.
- Tool: `dicom_lookup_data_element`
- Input: `TimezoneOffsetFromUTC`.
- Tool: `resolve_attribute_context`
- Inputs: `TimezoneOffsetFromUTC`, `AcquisitionDateTime`, `StudyDate`, and
  `StudyTime` in the Secondary Capture Image IOD.
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Official Source Evidence

- PS3.5 Table 6.2-1 defines DA as the fixed eight-character Gregorian
  `YYYYMMDD` form, TM as a 24-hour time with one through six fractional digits,
  and DT with a local-minus-UTC suffix in the inclusive `-1200` through `+1400`
  range.
- PS3.6 Table 6-1 defines Timezone Offset From UTC `(0008,0201)` as SH, VM 1.
- PS3.3 Table C.12-1 places Timezone Offset From UTC in SOP Common as Type 3.
- PS3.3 Table C.7.10.1-1 places Acquisition DateTime in General Acquisition as
  DT, VM 1, Type 3.
- PS3.3 Table C.7-3 requires Study Date and Study Time as Type 2 DA/TM values.
- Source artifact identity: the locked DICOM 2026b KB source manifest above.

## Project Action

- Registry status: implemented after both boundaries, exact raw values, UTC
  normalization, byte-identical same-seed generation, dicom3tools, DCMTK, and
  uv-locked pydicom gates passed.
- Manifest decision: use a typed `temporal` metadata contract rather than
  adding loose fields to `expected_semantics`.
- Should become KB patch: no; the official table and module rows resolve the
  required definitions.
- Expected cleanup after KB coverage exists: none.
