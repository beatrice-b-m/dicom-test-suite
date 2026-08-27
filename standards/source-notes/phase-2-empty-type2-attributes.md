# Phase 2 Empty Type 2 Attribute Evidence

Checked: 2026-08-26  
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `metadata/sc/empty_type2_attributes`
- Recipe ID: `metadata_sc_empty_type2_attributes`
- Manifest and validation rules for present Type 2 attributes with zero Value
  Length

## Required Decision

Generate one Secondary Capture instance with Patient Name `(0010,0010)`,
Patient Birth Date `(0010,0030)`, Patient Sex `(0010,0040)`, Referring
Physician's Name `(0008,0090)`, and Accession Number `(0008,0050)` present at
their required VRs with a zero Value Length. Other required Type 2 attributes
remain populated so the fixture isolates empty-value handling, and Laterality
remains `R` to avoid conflating emptiness with optional clinical metadata.

## KB Queries

- Tool: `resolve_attribute_context`
- Inputs: the five affected attributes in the Secondary Capture Image IOD.
- Tool: `dicom_lookup_data_element`
- Inputs: the five affected attribute keywords.
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Official Source Evidence

- PS3.3 Table C.7-1 requires Patient Name, Patient Birth Date, and Patient Sex
  as Type 2 attributes in the Patient Module.
- PS3.3 Table C.7-3 requires Referring Physician's Name and Accession Number as
  Type 2 attributes in the General Study Module.
- PS3.5 Section 7.4 states that a Type 2 Data Element shall be present and may
  have zero Value Length when its value is unknown.
- PS3.6 Table 6-1 defines the affected tags and their PN, DA, CS, PN, and SH
  VRs respectively.
- Source artifact identity: the locked DICOM 2026b KB source manifest above.

## Project Action

- Registry status: planned until the typed manifest contract, native recipe,
  raw zero-length validation, reports, and independent conformance gates pass.
- Manifest decision: record an exact typed list of tags, keywords, VRs, and
  zero Value Length expectations under `expected_metadata`.
- Should become KB patch: no; the official module rows and data dictionary
  resolve the required definitions.
- Expected cleanup after KB coverage exists: none.
