# Phase 2 Enhanced PET Multi-frame Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `enhanced/pet/multiframe_explicit_le`
- Recipe ID: `enhanced_pet_multiframe_explicit_le`
- Two-frame derived Enhanced PET volume with functional groups, spatial
  dimensions, isotope metadata, and a BQML real-world value mapping

## Required Decision

Generate one Enhanced PET Image Storage instance using Explicit VR Little
Endian. It contains two 2 by 2 native unsigned 16-bit MONOCHROME2 frames at
patient positions `0\\0\\0` and `0\\0\\5` mm. Both frames use stored values
`0, 100, 200, 400`, with ordered per-frame SHA-256
`03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784`.
The whole 16-byte Pixel Data SHA-256 is
`3a43b45e2f6d4d04fe4fc357dfc0efaa21caa5415ffc5db96fc19428d34a7bb5`.

Image Type and PET Frame Type are
`DERIVED\\PRIMARY\\STATIC\\EMISSION`. Selecting `DERIVED` deliberately keeps
the acquisition start and termination conditions, detector geometry, energy
window, PET acquisition, detector-motion, position, correction-factor,
reconstruction, and table-dynamics functional groups outside this slice. The
required Derivation Image Sequence is present with zero Items, as explicitly
permitted by its Type 2 macro, because this synthetic quantitative fixture has
no source SOP Instance. It therefore makes no unresolved cross-profile
reference claim.

The Image and common frame-description contract is PT modality, Pixel
Presentation `MONOCHROME`, Volumetric Properties `VOLUME`, Volume Based
Calculation Technique `NONE`, Content Qualification `RESEARCH`, Burned In
Annotation `NO`, Lossy Image Compression `00`, and Presentation LUT Shape
`IDENTITY`. Acquisition Context Sequence is present with zero Items. Enhanced
General Equipment carries deterministic nonempty manufacturer, model, serial,
and software values. Frame of Reference is present.

The mandatory Enhanced PET Acquisition attributes declare Table Motion
`STATIC` and Time of Flight Information Used `FALSE`; no scanner motion or TOF
processing is claimed. The mandatory Enhanced PET Corrections module declares
Counts Source `EMISSION` and sets Decay, Attenuation, Scatter, Dead Time,
Gantry Motion, Patient Motion, Count Loss Normalization, Randoms, Non-uniform
Radial Sampling, Sensitivity Calibration, and Detector Normalization
correction flags to `NO`. No correction method, source, relationship, or date
is present.

The mandatory Radiopharmaceutical Information Sequence has one synthetic
Item. Radiopharmaceutical Agent Number is `1`; Radionuclide is
`(77004003, SCT, "^18^Fluorine")`; Administration Route is
`(47625008, SCT, "Intravenous route")`; start DateTime is
`20260101000000`; Total Dose is synthetic `0` MBq; Half Life is `6586.2`
seconds; Positron Fraction is `0.967`; and Radiopharmaceutical is
`(35321007, SCT, "Fluorodeoxyglucose F^18^")`. These values exercise the
mandatory Enhanced PET isotope structure and are not a real administration
record. The same Agent Number `1` is referenced from the shared
Radiopharmaceutical Usage Functional Group.

Shared Functional Groups contains exactly one Item with mandatory Pixel
Measures, Plane Orientation (Patient), Frame Anatomy, Pixel Value
Transformation, Frame VOI LUT, Real World Value Mapping,
Radiopharmaceutical Usage, PET Frame Type, and the empty Derivation Image
Sequence. Pixel Spacing is `2\\2` mm; Slice Thickness and Spacing Between
Slices are `5` mm; orientation is `1\\0\\0\\0\\1\\0`; anatomy is HEAD with
Frame Laterality `U`. Pixel Value Transformation uses intercept `0`, slope
`2.5`, and Rescale Type `US`, as required for PT by that macro. Frame VOI uses
center `500` and width `1000`.

The Real World Value Mapping covers stored values 0 through 400 with FD
intercept `0` and slope `2.5`. LUT Label is `BQML`, explanation is
`Activity concentration`, and Measurement Units is
`(Bq/ml, UCUM, "Becquerels/milliliter")`. Each frame therefore maps to
`0, 250, 500, 1000` Bq/ml. This is a synthetic activity-concentration mapping,
not SUV, body-weight, body-surface-area, decay-corrected, or clinically
calibrated quantitative data.

Dimension Organization and Dimension Index Sequences contain one deterministic
organization and one index pointing to Image Position (Patient) through Plane
Position Sequence. Per-Frame Functional Groups contains exactly two Items.
Each has a Frame Content Item with Dimension Index Value `1` or `2` and the
mandatory Enhanced PET Temporal Position Index `1`, plus a Plane Position
Item at the ordered z location. Frame Content is never shared.

## KB Query

- Tools: `lookup_uid`, `lookup_sop_class`, `lookup_iod`,
  `list_modules_for_iod`, `list_attributes_for_module`,
  `lookup_data_element`, `lookup_defined_terms`, `lookup_enumerated_values`,
  `search_standard_text`, and `retrieve_standard_text`
- Input: Enhanced PET Image Storage, Enhanced PET Image, Enhanced PET Series,
  Isotope, Acquisition, Image and Corrections modules, Enhanced PET functional
  groups, isotope and administration codes, and BQML mapping attributes
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the non-retired SOP Class UID is
  `1.2.840.10008.5.1.4.1.1.130`; the IOD resolves to Tables A.56-1 and
  A.56-2; mandatory PET modules and functional groups resolve to the tables
  cited below; and the coded isotope, route, radiopharmaceutical, and units
  resolve to the pinned PS3.16 context groups.
- Parser limitations: there is no macro-listing API; contextual Image Type
  term parsing is incomplete; Enhanced PET Acquisition term extraction mixes
  attribute contexts; the A.56-2 Measurement Units invocation is truncated;
  and context-group concepts require standard-text retrieval rather than a
  structured CID lookup. These are KB coverage gaps, not standards ambiguity.

## Official Source Evidence

- PS3.4 Table B.5-1 and PS3.6 Table A-1 identify Enhanced PET Image Storage.
- PS3.3 Table A.56-1 makes Patient, General Study and Series, Enhanced PET
  Series, Frame of Reference, General and Enhanced General Equipment, Image
  Pixel, Acquisition Context, Multi-frame Functional Groups and Dimension,
  Enhanced PET Isotope, Acquisition, Image and Corrections, and SOP Common
  mandatory.
- PS3.3 Table A.56-2 makes Pixel Measures, Frame Content, Plane Position and
  Orientation, Frame Anatomy, Pixel Value Transformation, Frame VOI LUT, Real
  World Value Mapping, Radiopharmaceutical Usage, and PET Frame Type mandatory.
  The ORIGINAL-only PET acquisition macros are not required for this case.
- PS3.3 Tables C.8.22-1, C.8.22-2, C.8.22-3, C.8.22-9, and C.8.22-19 define
  the Enhanced PET series, acquisition, image, isotope, and corrections
  attributes used here.
- PS3.3 Tables C.7.6.16-1 through C.7.6.16-12b,
  C.7.6.16.2-20, and C.8.22-10 define the selected multi-frame, spatial,
  quantitative, isotope-usage, and PET Frame Type macros. Table C.7.6.16-7
  explicitly permits zero Items in Derivation Image Sequence.
- PS3.16 CID 4020 identifies `(77004003, SCT, "^18^Fluorine")`; CID 11
  identifies `(47625008, SCT, "Intravenous route")`; CID 4021 identifies
  `(35321007, SCT, "Fluorodeoxyglucose F^18^")`; and CID 84 supplies the
  UCUM activity-concentration unit. The F-18 physical constants are locked
  synthetic metadata consistent with the selected radionuclide.
- Source artifact identity is the locked DICOM 2026b KB source manifest above;
  official source artifacts remain `unavailable_not_downloaded` as recorded
  in `standards.lock.json` and are not committed.

## Project Action

- Registry status: planned until generation, typed manifest and report
  contracts, internal and manifest-driven validation, two-run determinism,
  exact independent native pixel extraction and quantitative recomputation,
  and independent IOD gates pass.
- Registry reason or linked issue: `recipe_unimplemented`.
- Should become KB patch: yes; macro and context-specific value-term coverage
  should become structured KB data.
- Expected cleanup after KB coverage exists: replace standard-text fallbacks
  with normal macro and CID queries while retaining this slice-specific
  derived-data, quantitative, and non-claim decision.
