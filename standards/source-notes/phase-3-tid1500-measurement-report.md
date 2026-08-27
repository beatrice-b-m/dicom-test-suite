# Phase 3 TID 1500 Measurement Report Evidence

Checked: 2026-08-27  
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `derived/sr/tid1500_ct_measurement_report`
- Recipe ID: `derived_sr_tid1500_ct_measurement_report`
- Provider: optional uv-locked `highdicom_pydicom` backend
- Source image: `enhanced/ct/multiframe_shared_perframe_explicit_le`, frames 1 and 2
- Derived ROI: `derived/seg/binary_multiframe_explicit_le`, segment 1
- Output: Comprehensive 3D SR Storage, Explicit VR Little Endian

## Locked Semantic Contract

The output is a COMPLETE, FINAL, UNVERIFIED Comprehensive 3D SR with root
CONTAINER title `(126000, DCM, "Imaging Measurement Report")`. Its root
Content Template Sequence identifies Mapping Resource `DCMR` and Template
Identifier `1500`.

TID 1001 supplies one deterministic device observer. TID 1500 contains Imaging
Measurements `(126010, DCM)`, which contains one TID 1411 Volumetric ROI
Measurement Group `(125007, DCM)`. The group has a deterministic Tracking
Identifier and Tracking Unique Identifier, finding `(123037004, SCT,
"Body structure")`, and one Referenced Segment content item that identifies
segment 1 of the committed binary SEG. The segment reference applies to all
frames of segment 1 and therefore omits Referenced Frame Number. Its nested
Source Image for Segmentation `(121233, DCM, "Source image for segmentation")`
identifies frames 1 and 2 of the Enhanced CT. The evidence hierarchy records both source
instances, and every SOP Class, SOP Instance, series, frame, and segment
identity must agree with the manifest and generated corpus.

The group contains one TID 1419/TID 300 NUM measurement:

- name: `(118565006, SCT, "Volume")`;
- value: `5.625`;
- units: `(mm3, UCUM, "cubic millimeter")`.

The value is derived, not arbitrary: the binary SEG asserts four voxels across
the two source frames, and the Enhanced CT Pixel Measures are `0.75 × 0.75 ×
2.5` mm, giving `4 × 0.75 × 0.75 × 2.5 = 5.625 mm3`. The procedure reported is
`(25045-6, LN, "CT unspecified body region")`, a CID 100 concept.

The optional TID 1600 Image Library is deliberately omitted. Highdicom 0.28.1
expects top-level Pixel Spacing while constructing that optional descriptor,
whereas the valid Enhanced CT stores Pixel Measures in Functional Groups. The
CT remains normatively linked through the Referenced Segment source-image
relationship and the evidence hierarchy.

## KB Query

- Query: `dicom-kb lookup uid Comprehensive3DSRStorage --edition 2026b`
- Edition: 2026b
- Result: `1.2.840.10008.5.1.4.1.1.88.34`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the current KB parser surface covers PS3.3, PS3.4, and PS3.6,
  but does not provide structured PS3.16 template-row or context-group queries.

## Official Source Evidence

- PS3.6 Table A-1 identifies Comprehensive 3D SR Storage; Table 6-1 defines
  the template, content, evidence, numeric, and reference data elements.
- PS3.3 A.35.13 and Table A.35.13-1 define the Comprehensive 3D SR IOD modules;
  A.35.13.3.1 and Table A.35.13-2 define allowed content and relationships.
- PS3.3 C.17.3 and Table C.17-4 define the root CONTAINER, document title, and
  nested Content Sequence.
- PS3.3 C.18.8, Table C.18.8-1, and C.18.8.1.2 require `DCMR` and `1500` when
  the named root template is used.
- PS3.16 TID 1500 requires TID 1001 Observation Context and at least one report
  content branch; this slice selects Imaging Measurements.
- PS3.16 TID 1411 binds a volumetric measurement group to one referenced SEG
  segment and its source images or series. TID 1419 invokes the TID 300 numeric
  measurement semantics used here.

PS3.16 is now listed explicitly in `standards.lock.json` as
`unavailable_not_downloaded`; official 2026b CHTML was reviewed, and no
standard source artifact is committed.

## Backend Capability and Determinism

Highdicom 0.28.1 provides `MeasurementReport`,
`VolumetricROIMeasurementsAndQualitativeEvaluations`, `ReferencedSegment`,
`Measurement`, and `Comprehensive3DSR`. A disposable uv-locked prototype using
the exact source CT and SEG produced SOP Class `.88.34`, TID 1500 and TID 1411
identification, the NUM and Referenced Segment content, and a two-instance
evidence hierarchy; locked `dciodvfy -new` reported no finding.

All series, SOP, tracking, and observer UIDs originate from Rust deterministic
UID roles. The backend must normalize creation/content date and time, timezone,
equipment identity, and file-meta implementation identity. Source and evidence
order is fixed as CT then SEG. The case remains `semantic_stable`; two runs must
reproduce the complete content-tree semantics, references, UIDs, and manifest
provenance even if no cross-version byte-stability claim is made.

## Independent Acceptance Gate

Promotion requires all of the following with no pre-authorized findings:

1. locked dicom3tools `dciodvfy -new` completes without an error or warning;
2. locked PixelMed 20260608 `DicomSRValidator -checktemplateid` recognizes the
   Comprehensive 3D SR and validates TID 1500, included templates, relationships,
   and context groups without an error or warning;
3. locked DCMTK `dcmdump` completes as an independent parser;
4. locked `dcentvfy -f` is silent over the CT, SEG, and SR reference closure;
5. Rust independently reopens the output and validates the exact template IDs,
   codes, relationships, tracking identities, measurement, segment/source
   references, evidence hierarchy, completion flags, and absence of Pixel Data;
6. strict conformance verification requires available, lock-matched PixelMed
   evidence for this case rather than treating optional-tool absence as success.

## Project Action

- Keep the registry row planned until the backend operation, Rust acceptance
  contract, manifest, reports, tests, and empirical conformance evidence pass.
- Do not add Segment Tracking ID/UID to the existing byte-stable SEG; TID 1411
  permits SR-only tracking when the SEG attributes are absent.
- Should become KB patch: yes; expose PS3.16 template and context-group queries.
- Do not commit generated DICOM files, validator output, or official standard
  artifacts.
