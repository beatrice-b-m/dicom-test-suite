# Phase 2 Ultrasound Multi-frame Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/us/multiframe_explicit_le`
- Recipe ID: `classic_us_multiframe_explicit_le`
- Classic Ultrasound Multi-frame Image timing, ordered native frames, and
  explicit exclusion of enhanced, calibrated, color, and spatial semantics

## Required Decision

Generate one classic Ultrasound Multi-frame Image Storage instance containing
four 4 by 4 unsigned 8-bit MONOCHROME2 frames. Image Type is
`ORIGINAL\\PRIMARY\\ABDOMINAL\\0001`: original, primary, abdominal, 2-D
imaging. Frame Increment Pointer `(0028,0009)` names Frame Time `(0018,1063)`,
which is exactly 100 ms. The ordered frame-relative start times are therefore
0, 100, 200, and 300 ms.

Each frame contains a fixed grayscale background and one moving 255-valued
echo. The native frame SHA-256 values, in order, are:

1. `be422fa58b70ec0d940f28a4dba3dadac62d4583b9ecba1e73d65b37ee9733e7`
2. `303d53edfa9bf6eeeb81dba8a6a4c1a9c2e1cb0ea773f90afb583d1132d88eee`
3. `7f8a6e2fa2665b2465075b9e0cf86dfb0646f6f21a2a647525476e5bb6e489bb`
4. `8c213da26d1c57661b68238ac5c1f1d9417f661e0ab578846bf84040e753f650`

The concatenated 64-byte payload SHA-256 is
`060e2c56c9728f787339515ef16bc8c1adfbfb4fb85b2d2c18f115c17b439bc9`.
Lossy Image Compression is `00`, and Ultrasound Color Data Present is zero.
The fixture is a synthetic navigation and constant-timing contract, not a
diagnostic image.

Body Part Examined `(0018,0015)` is `ABDOMEN`, matching Image Type Value 3.
The abdomen is not a paired structure, so Laterality `(0020,0060)` is absent.
The fixture therefore asserts an abdominal region without inventing right,
left, bilateral, or median anatomy.

The case makes no claim about patient-space geometry, spatially related
frames, Frame of Reference, ultrasound region calibration, measurement units,
color flow, contrast, gating, IVUS synchronization, volume acquisition, or
enhanced functional groups. Optional US Region Calibration and Frame Pointers
modules are absent. Optional Cine Rate, Recommended Display Frame Rate, Frame
Time Vector, and Effective Duration are also absent so that Frame Time is the
single timing authority.

## KB Query

- Tools: `dicom_lookup_sop_class`, `dicom_lookup_iod`,
  `dicom_list_modules_for_iod`, `dicom_list_attributes_for_module`,
  `dicom_resolve_attribute_context`, `dicom_lookup_defined_terms`, and
  `dicom_lookup_enumerated_values`
- Input: Ultrasound Multi-frame Image Storage and Ultrasound Multi-frame Image;
  Multi-frame, Cine, and US Image modules; Number of Frames, Frame Increment
  Pointer, Frame Time, Photometric Interpretation, Bits Allocated, Image Type,
  Lossy Image Compression, Ultrasound Color Data Present, Body Part Examined,
  and Laterality contexts
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the SOP Class resolves to non-retired UID
  `1.2.840.10008.5.1.4.1.1.3.1`; the IOD and mandatory module table resolve;
  Number of Frames and Frame Increment Pointer resolve as Type 1; and Frame
  Time resolves as Type 1C when selected by the pointer.
- Parser limitation: context-specific Image Type, Frame Increment Pointer,
  Lossy Image Compression, and Ultrasound Color Data Present value-term
  lookups do not resolve. Their module rows and persisted official standard
  text remain available, so this is a parsed-value coverage gap rather than a
  standards ambiguity.

## Official Source Evidence

- PS3.3 Table A.7-1 makes General Acquisition, Cine, Multi-frame, US Image,
  Image Pixel, and the common patient, study, series, equipment, image, and SOP
  modules mandatory. US Region Calibration is optional. Synchronization is
  conditional on IVUS, which this case does not claim.
- PS3.3 Table C.7-14 makes Number of Frames and Frame Increment Pointer Type 1.
  The pointer contains the tag of the attribute used to increment the frames.
- PS3.3 Table C.7-13 makes Frame Time Type 1C when Frame Increment Pointer names
  it and defines its units as milliseconds per frame. Frame Time Vector is the
  alternative conditional timing attribute and is not present here.
- PS3.3 Table C.7-5a defines Body Part Examined as Type 3 and Laterality as
  Type 2C for a paired body part when no image-, frame-, or measurement-level
  laterality is present. `ABDOMEN` aligns the General Series anatomy with the
  US Image Type specialization and leaves the paired-body-part condition false,
  so Laterality is absent.
- PS3.3 Table C.8-18 specializes the US Image pixel contract. For this
  monochrome case, Samples per Pixel is one, Photometric Interpretation is
  MONOCHROME2, Bits Allocated and Bits Stored are eight, High Bit is seven, and
  Pixel Representation is unsigned. Planar Configuration does not apply.
- The same US Image specialization defines Image Type Value 3 `ABDOMINAL` and
  Value 4 bitmap `0001` for 2-D imaging. It permits the explicit `00` no-lossy
  history and zero no-color declarations used by this fixture.
- Source artifact identity is the locked DICOM 2026b KB source manifest above.

## Project Action

- Registry status: planned until generation, typed manifest and report
  contracts, internal and manifest-driven validation, determinism, exact
  independent native frame extraction, and independent IOD gates pass.
- Registry reason or linked issue: `recipe_unimplemented`.
- Should become KB patch: yes; the missing context-specific value-term results
  are systematic parser coverage gaps.
- Expected cleanup after KB coverage exists: replace local value-term fallbacks
  with normal KB evidence while retaining this case-specific timing and
  non-claim decision.
