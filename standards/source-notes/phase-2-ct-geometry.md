# Phase 2 Classic CT Geometry

Checked: 2026-08-26
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- `geometry/ct/duplicate_missing_instance_number`
- `geometry/ct/nonuniform_slice_spacing`
- `geometry/ct/gantry_tilt_series`
- `geometry/ct/multiseries_shared_frame_of_reference`
- Manifest geometry and series-organization expectations
- Internal geometry validation and coverage reporting

## Pinned KB Evidence

The configured `dicom-standard-kb` tools returned edition `2026b` and the
source-manifest identity above for each query used here.

- `dicom_lookup_sop_class CTImageStorage` maps CT Image Storage to the CT Image
  IOD (PS3.4 Table B.5-1 and PS3.3 Table A.3-1).
- `dicom_list_modules_for_iod "CT Image"` makes General Study, General Series,
  Frame of Reference, General Image, Image Plane, CT Image, Image Pixel, and
  SOP Common mandatory modules (PS3.3 Table A.3-1).
- `dicom_resolve_attribute_context InstanceNumber --iod "CT Image"` resolves
  Instance Number `(0020,0013)` to Type 2 through the mandatory General Image
  Module (PS3.3 Table C.7-9).
- `dicom_resolve_attribute_context ImagePositionPatient --iod "CT Image"` and
  `ImageOrientationPatient` resolve both attributes to Type 1 in the mandatory
  Image Plane Module (PS3.3 Table C.7-10).
- `dicom_resolve_attribute_context SpacingBetweenSlices --iod "CT Image"`
  resolves it to Type 3. Table C.7-10 defines it as adjacent slice
  center-to-center spacing and permits omission.
- `dicom_resolve_attribute_context GantryDetectorTilt --iod "CT Image"`
  identifies Gantry/Detector Tilt `(0018,1120)` as DS VM 1 in the CT Image
  Module (PS3.3 Table C.8-3). It is nominal acquisition metadata and is not the
  mathematical source of image-plane geometry.
- `dicom_resolve_attribute_context StudyInstanceUID`, `SeriesInstanceUID`, and
  `FrameOfReferenceUID --iod "CT Image"` resolve the identity attributes used
  to state same-study, distinct-series, and shared-frame-of-reference
  relationships (PS3.3 Tables C.7-3, C.7-5a, and C.7-6).

## Implementation Invariants

- Valid-profile files always include Instance Number. The planned “missing”
  member is encoded as a present zero-length Type 2 value; literal omission is
  reserved for a later negative-profile mutation.
- Geometric ordering is derived from Image Position (Patient) projected onto
  the normal computed from Image Orientation (Patient). Instance Number never
  substitutes for spatial geometry.
- A non-uniform series records its exact adjacent projected intervals and
  omits the optional scalar Spacing Between Slices rather than claiming one
  value describes unequal intervals.
- Gantry/Detector Tilt is compared as declared metadata, while row, column,
  normal, position, and displacement expectations independently describe the
  image planes.
- A multi-series case uses one Study Instance UID, distinct Series Instance
  UIDs, and one shared Frame of Reference UID. Reports state these relations
  explicitly instead of treating raw UID presence as proof.

## Project Action

- Registry action: retain each case as planned until its full vertical slice
  passes schema, generation, validation, report, reproducibility, and
  independent conformance checks; then promote it to implemented.
- Independent validation: locked dicom3tools `dciodvfy` for every instance and
  `dcentvfy` for each complete case group, plus DCMTK parsing and project-owned
  semantic comparison of reopened geometry.
- KB patch: not required; the pinned parser surface covers the decisions above.
- Official standards artifacts and generated KB databases remain uncommitted.
