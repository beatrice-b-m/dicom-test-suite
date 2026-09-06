# Phase 2 Long and Multi-valued String Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `metadata/sc/long_multivalue_text_numeric_strings`
- Recipe ID: `metadata_sc_long_multivalue_text_numeric_strings`
- Manifest, generation, raw validation, and reporting rules for LT, LO, DS,
  and IS boundary values

## Required Decision

Generate one Secondary Capture instance containing:

- Image Comments `(0020,4000)` LT with exactly 10,240 ASCII bytes;
- Software Versions `(0018,1020)` LO VM 2 with two 64-character values;
- Pixel Spacing `(0028,0030)` DS VM 2 with two 16-character values,
  `0.12345678901234` and `0.98765432109876`;
- Acquisition Number `(0020,0012)` IS with the 12-character lexical form
  `+02147483647`, representing the maximum signed 32-bit integer.

The LO and DS encodings include the required value separator and one trailing
space to make their total Value Length even. The LT and IS values are already
even length and receive no padding.

## KB Queries

- Tool: `dicom_lookup_data_element`
- Inputs: `ImageComments`, `SoftwareVersions`, `PixelSpacing`, and
  `AcquisitionNumber`.
- Tool: `resolve_attribute_context`
- Inputs: the four attributes in the Secondary Capture Image IOD.
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Official Source Evidence

- PS3.5 Table 6.2-1 limits LT to 10,240 characters, each LO value to 64
  characters, and DS and IS values to 16 and 12 bytes respectively.
- PS3.5 Section 6.4 defines reverse-solidus as the multi-value separator and
  requires the separator bytes to participate in the encoded Value Length.
- PS3.5 Section 6.2 requires even Value Length and space padding for these
  character VRs when padding is needed.
- PS3.6 Table 6-1 defines Image Comments as LT VM 1, Software Versions as LO
  VM 1-n, Pixel Spacing as DS VM 2, and Acquisition Number as IS VM 1.
- Source artifact identity: the locked DICOM 2026b KB source manifest above.

## Project Action

- Registry status: implemented after the typed manifest contract, native
  recipe, exact raw validation, reports, byte reproducibility, dicom3tools,
  DCMTK, and uv-locked pydicom gates passed.
- Manifest decision: record typed per-element VR, VM, decoded values and
  lengths, raw Value Length and SHA-256, and padding semantics without
  duplicating the 10,240-byte LT payload as manifest hex.
- Should become KB patch: no; the official VR table and data dictionary resolve
  the required limits and multiplicities.
- Expected cleanup after KB coverage exists: none.

## Conformance Proof

- Two seed-1 `extended` generations were byte-identical and each produced 81
  files; strict validation checked all 81 with zero failures.
- The fixture SHA-256 is
  `238f7478de59027060c3807a2075faf9deb9e32d2a4a33bf622170183470c5c2`.
- `dciodvfy` reported only the normal `SCImage` identification and no finding;
  isolated `dcentvfy` was silent.
- DCMTK `dcmdump` independently reported VL/VM pairs 10240/1, 130/2, 34/2,
  and 12/1 for LT, LO, DS, and IS respectively.
- The uv-locked pydicom 3.0.2 reader preserved all component strings and
  numeric lexemes, and its rewrite was byte-identical and clean under both
  dicom3tools validators. Pydicom emitted a boundary warning because it counts
  the legal trailing space pad against the second 64-character LO component;
  the warning is retained here rather than weakening the two-max-component
  fixture. The raw contract, DCMTK VM 2 parse, and both conformance validators
  independently confirm the encoding.
