# Phase 3 Comprehensive 3D SR SCOORD3D Evidence

Checked: 2026-08-27  
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `derived/sr/comprehensive3d_scoord3d`
- Recipe ID: `derived_sr_comprehensive3d_scoord3d`
- Provider: optional uv-locked `highdicom_pydicom` backend
- Source image: `enhanced/ct/multiframe_shared_perframe_explicit_le`, frames 1 and 2
- Output: Comprehensive 3D SR Storage, Explicit VR Little Endian

## Locked Semantic Contract

The output is a COMPLETE, FINAL, UNVERIFIED Comprehensive 3D SR with root
CONTAINER title `(126000, DCM, "Imaging Measurement Report")`. Its root
Content Template Sequence identifies Mapping Resource `DCMR` and Template
Identifier `1500`. TID 1001 supplies one deterministic device observer, and
the procedure reported is `(25045-6, LN, "CT unspecified body region")`.

Imaging Measurements `(126010, DCM)` contains one TID 1501 Measurement Group
`(125007, DCM)`. The group has deterministic tracking identities, finding
`(123037004, SCT, "Body structure")`, one NUM Distance measurement
`(121206, DCM)` with value `2.5` and units `(mm, UCUM, "millimeter")`, and one
direct Source of Measurement IMAGE `(121112, DCM)` that identifies frames 1
and 2 of the Enhanced CT.

The NUM contains one `INFERRED FROM` SCOORD3D `(260753009, SCT, "Source")`.
Its Graphic Type is `POLYLINE`, its Graphic Data is exactly
`[0.0, 0.0, 0.0, 0.0, 0.0, 2.5]`, and its Referenced Frame of Reference UID
equals the source CT Frame of Reference UID. A deterministic Fiducial UID is
present. The two points are the patient-space positions of the first pixel in
source frames 1 and 2. The source is axial with Image Position Patient values
`[0,0,0]` and `[0,0,2.5]`, Pixel Spacing `0.75 x 0.75` mm, and slice spacing
`2.5` mm, so the encoded distance is derived rather than arbitrary.

The Current Requested Procedure Evidence hierarchy contains exactly that one
Enhanced CT instance. The optional TID 1600 Image Library is omitted because
the direct Source of Measurement relationship closes the reference while
avoiding assumptions about top-level Pixel Spacing in Enhanced CT.

## KB Query

- Query: `dicom-kb lookup uid Comprehensive3DSRStorage --edition 2026b`
- Edition: 2026b
- Result: `1.2.840.10008.5.1.4.1.1.88.34`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the current KB parser surface covers PS3.3, PS3.4, and PS3.6,
  but does not provide structured PS3.16 template-row queries.

## Official Source Evidence

- PS3.6 Table A-1 identifies Comprehensive 3D SR Storage; Table 6-1 defines
  the template, content, evidence, numeric, SCOORD3D, and reference elements.
- PS3.3 A.35.13 and Table A.35.13-1 define the Comprehensive 3D SR IOD;
  A.35.13.3.1 and Table A.35.13-2 admit SCOORD3D content and relationships.
- PS3.3 C.17.3 and Table C.17-4 define the SR Document Content Module.
- PS3.3 C.18.8 and Table C.18.8-1 define SCOORD3D content; C.18.9 and Table
  C.18.9-1 define Graphic Type, Graphic Data, and patient-based coordinates.
- PS3.3 C.7.4.1 and C.7.6.2.1.1 define the patient Frame of Reference and
  image-plane geometry used to derive the two points.
- PS3.16 TID 1500 invokes TID 1001 Observation Context and Imaging
  Measurements; TID 1501 provides the measurement group; TID 300 permits the
  measurement to carry its referenced spatial coordinates.

PS3.16 is explicitly listed in `standards.lock.json` as
`unavailable_not_downloaded`; official 2026b CHTML was reviewed, and no
standard source artifact is committed.

## Backend Capability and Determinism

Highdicom 0.28.1 provides `CoordinatesForMeasurement3D`, `Measurement`,
`MeasurementsAndQualitativeEvaluations`, `SourceImageForMeasurementGroup`,
`MeasurementReport`, and `Comprehensive3DSR`. A disposable uv-locked prototype
using the exact Enhanced CT source emitted the contract above. Locked
`dciodvfy -new` and PixelMed 20260608 `DicomSRValidator -checktemplateid`
both completed with no warning or error; DCMTK `dcmdump` parsed the result.

Rust supplies deterministic series, SOP, tracking, observer, and fiducial UIDs
and validates the source hash, DICOM identities, Frame of Reference, functional
group geometry, and exact coordinate derivation before invocation. The backend
normalizes creation/content dates and times, timezone, equipment identity, and
file-meta implementation identity. The case is conservatively
`semantic_stable` even though two-run byte equality remains part of acceptance.

## Independent Acceptance Gate

Promotion requires all of the following with no pre-authorized findings:

1. locked dicom3tools `dciodvfy -new` is clean;
2. locked PixelMed 20260608 `DicomSRValidator -checktemplateid` recognizes and
   validates TID 1500, TID 1501, TID 300, relationships, and context groups;
3. locked DCMTK `dcmdump` completes as an independent parser;
4. locked `dcentvfy -f` resolves the CT and SR reference closure, with every
   finding retained for review rather than silently allowlisted;
5. Rust independently validates the exact tree, codes, numeric value, geometry,
   Frame of Reference, fiducial, source frames, evidence, flags, and no pixels;
6. strict conformance requires lock-matched PixelMed evidence for this case.

Negative controls replace the Frame of Reference UID, change a coordinate with
the manifest hash repaired, omit or redirect the source IMAGE, or violate the
POLYLINE point cardinality. No new allowlist entry is permitted.

## Project Action

- Registry status: implemented after the complete vertical gate passed.
- The backend protocol and independent IOD/template validator are already
  locked, so their obsolete blockers must not remain attached to this row.
- Should become KB patch: yes; expose PS3.16 template-row queries.
- Do not commit generated DICOM files, validator output, or official standards
  artifacts.

The isolated post-implementation `dcentvfy` run found no missing reference but
reported its empty-AccessionNumber Study-versus-Series information-entity
classification warning. That diagnostic remains visible and is not added to
the accepted-findings allowlist. Primary `dciodvfy`, independent `dcmdump`, and
mandatory PixelMed template/IOD validation remain clean.
