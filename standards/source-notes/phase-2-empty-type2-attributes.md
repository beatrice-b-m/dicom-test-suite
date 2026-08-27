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

- Registry status: implemented after the typed manifest contract, native
  recipe, raw zero-length validation, reports, byte reproducibility,
  dicom3tools, DCMTK, and uv-locked pydicom gates passed.
- Manifest decision: record an exact typed list of tags, keywords, VRs, and
  zero Value Length expectations under `expected_metadata`.
- Should become KB patch: no; the official module rows and data dictionary
  resolve the required definitions.
- Expected cleanup after KB coverage exists: none.

## Conformance Proof

- Two seed-1 `core` generations were byte-identical and each produced 41 files;
  strict validation checked all 41 with zero failures.
- The fixture SHA-256 is
  `e70ce329e96932c6189e1bb31c39673456809036d169c243e3cbeeddb2be787d`.
- `dciodvfy` reported only the normal `SCImage` identification and no finding;
  isolated `dcentvfy` was silent.
- DCMTK `dcmdump` independently reported PN, DA, CS, PN, and SH with zero Value
  Length for the five locked attributes.
- The repository's locked `uv` environment selected pydicom 3.0.2, which read
  all five attributes at their exact VRs with VM 0 and empty values.
