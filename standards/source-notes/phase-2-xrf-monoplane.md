# Phase 2 XRF Monoplane Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/xrf/monoplane_explicit_le`
- Recipe ID: `classic_xrf_monoplane_explicit_le`
- Classic X-Ray Radiofluoroscopic Image acquisition and equipment-coordinate
  column geometry without cine, tomography, or patient-space claims

## Required Decision

Generate one single-frame X-Ray Radiofluoroscopic Image Storage instance using
Explicit VR Little Endian. The 4 by 4 native unsigned 8-bit MONOCHROME2 image
has Image Type `ORIGINAL\\PRIMARY\\SINGLE PLANE`, Pixel Intensity
Relationship `LIN`, Lossy Image Compression `00`, and this ordered payload:

```text
  0  16  32  48
 16  64  96  64
 32  96 255  96
 48  64  96  64
```

The 16-byte frame and payload SHA-256 is
`0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e`.
Reusing the XA pattern isolates SOP Class, RF acquisition, and XRF positioner
handling rather than introducing a second pixel experiment. The pattern is
synthetic display input, not a diagnostic fluoroscopic image.

The X-Ray Acquisition contract uses KVP 70, Radiation Setting `SC`, and
Exposure 1 mAs. `SC` is low-dose exposure generally corresponding to
fluoroscopic settings. Exposure satisfies the Type 2C branch while Exposure
Time, X-Ray Tube Current, Radiation Mode, and Average Pulse Width remain
absent, so the fixture does not invent pulse duration, current, or continuous
versus pulsed operation. Imager Pixel Spacing is `0.2\\0.2` mm at the receptor
housing. Pixel Spacing is absent, so no patient- or object-space calibration
is claimed.

The optional XRF Positioner contract is deliberately present. Distance Source
to Detector is 1200 mm, Distance Source to Patient is 800 mm, and Estimated
Radiographic Magnification Factor is exactly 1.5, the declared SID/SOD ratio.
Column Angulation is positive 10 degrees in the equipment-based coordinate
system: the beam tilts toward the head of the table, with the detector plane
assumed parallel to the table plane. XA Positioner patient-relative RAO/LAO
and CAU/CRA angles are absent; the case must not translate Column Angulation
into those semantically different attributes.

The case is single-frame and single-plane. Number of Frames, Frame Increment
Pointer, Cine timing, biplane references, and tomography Scan Options are
absent. It makes no claim about contrast, subtraction or masks, X-Ray Table
position, motion, or tilt, calibration phantoms, Modality or VOI LUTs, display
processing, dose product, collimation, DX Detector Module characteristics,
overlays, shutters, referenced images, or patient-space geometry. Body Part
Examined is `ABDOMEN`, a non-paired region suitable for general RF coverage,
so Laterality is absent.

Patient Orientation is present with CS VR and zero Value Length. The fixture
uses equipment-coordinate Column Angulation and omits Image Orientation
(Patient), Image Position (Patient), and Frame of Reference UID. Instance
Number is `1`.

## KB Query

- Tools: `lookup_sop_class`, `lookup_uid`, `lookup_iod`,
  `list_modules_for_iod`, `list_attributes_for_module`,
  `lookup_defined_terms`, `lookup_enumerated_values`, `search_standard_text`, and
  `retrieve_standard_text`
- Input: X-Ray Radiofluoroscopic Image Storage, Explicit VR Little Endian,
  X-Ray Radiofluoroscopic Image, X-Ray Image, X-Ray Acquisition, XRF
  Positioner, General Image, Image Type, Pixel Intensity Relationship,
  Radiation Setting, Modality, Column Angulation, receptor distances,
  magnification, and imager spacing
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the SOP Class resolves to the non-retired UID
  `1.2.840.10008.5.1.4.1.1.12.2`; the SOP resolves to the X-Ray
  Radiofluoroscopic Image IOD in PS3.3 Table A.16-1; and the mandatory X-Ray
  Image and X-Ray Acquisition modules plus optional XRF Positioner resolve to
  Tables C.8-26, C.8-27, and C.8-31. Explicit VR Little Endian resolves to
  `1.2.840.10008.1.2.1` in PS3.6 Table A-1. Persisted PS3.3 Section
  C.7.3.1.1.1 identifies `RF` as Radio Fluoroscopy.
- Parser limitation: the context-specific defined-term lookup for Modality and
  Pixel Intensity Relationship, and enumerated-value lookups for Image Type
  and Radiation Setting, return no parsed terms. Persisted official sections
  and module tables provide the locked values. This is a parsed-value coverage
  gap, not a standards ambiguity.

## Official Source Evidence

- PS3.3 Section A.16.1 defines XRF for table-and-column RF systems using an
  equipment-based coordinate system and explicitly permits either a
  single-frame image or a cine run in one multi-frame image.
- PS3.3 Section C.7.3.1.1.1 identifies Modality `RF` as Radio Fluoroscopy.
- PS3.3 Table A.16-1 makes X-Ray Image and X-Ray Acquisition mandatory. Cine
  and Multi-frame are conditional on multi-frame cine data; Contrast/Bolus is
  conditional on contrast use; Mask is conditional on a subtractable image;
  and X-Ray Tomography Acquisition is conditional on Scan Options `TOMO`.
  Those conditions are false for this case. XRF Positioner is optional at the
  IOD level and is selected here as the slice-defining geometry module.
- PS3.3 Table C.8-26 and Section C.8.7.1.1.1 specialize Image Type Value 3 as
  `SINGLE PLANE`. The biplane referenced-image condition and single-frame
  Frame Increment Pointer condition are therefore false.
- PS3.3 Section C.8.7.1.1.2 defines `LIN` as approximately proportional to
  X-Ray beam intensity. Selecting it avoids the conditional Modality LUT
  requirement associated with `LOG` and makes no display-ready `DISP` claim.
- PS3.3 Table C.8-27 makes Radiation Setting Type 1 and KVP Type 2. Its
  acquisition branch requires Exposure when Exposure Time or X-Ray Tube
  Current is absent. The same table defines `SC` as low-dose exposure
  generally corresponding to fluoroscopic settings.
- PS3.3 Table C.8-27 defines Imager Pixel Spacing at the receptor housing and
  distinguishes it from calibrated Pixel Spacing. The fixture preserves that
  distinction by omitting Pixel Spacing.
- PS3.3 Table C.8-31 defines optional source-to-detector,
  source-to-patient, magnification, and Column Angulation values. Positive
  Column Angulation tilts toward the head of the table with detector plane
  parallel to table plane; it is not an XA patient-relative primary or
  secondary angle.
- PS3.3 Table C.7-9 supports a zero-length Patient Orientation when other
  orientation attributes apply and patient Image Orientation and Image
  Position are not used. The fixture preserves that Type 2C distinction.
- Source artifact identity is the locked DICOM 2026b KB source manifest above;
  official source artifacts remain `unavailable_not_downloaded` as recorded
  in `standards.lock.json` and are not committed.

## Independent Conformance Evidence

Two seed-1 `core` generations each produced 47 files and were recursively
byte-identical. Strict corpus validation checked all 47 files with zero
failures. The XRF instance SHA-256 is
`da7415ddb66c2cce4a3e8c27eb4f5a04a6f03b3bfb9402346fe13a41fadf30ff`;
the independently extracted 16-byte Pixel Data SHA-256 is
`0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e`.

Locked `dciodvfy -new` identifies only `XRFImage` and exits successfully;
`dcentvfy` is silent and successful. DCMTK independently reports RF modality,
the exact SOP Class, Image Type, acquisition values, receptor spacing,
source distances, magnification, Column Angulation, OB VR, and ordered pixel
bytes. The frozen offline pydicom 3.0.2 environment managed by locked `uv`
independently reads the complete contract and declared absences and writes a
byte-identical Part 10 file with the same SHA-256. Both independent validators
also accept that rewrite without a finding.

## Project Action

- Registry status: implemented after typed manifest and report contracts,
  internal and manifest-driven validation, two-run determinism, exact
  independent native pixel extraction, and independent IOD gates passed.
- Registry reason or linked issue: none; `recipe_unimplemented` was removed.
- Should become KB patch: yes; the missing context-specific value-term results
  are systematic parser coverage gaps.
- Expected cleanup after KB coverage exists: replace local value-term
  fallbacks with normal KB evidence while retaining this XRF-specific
  acquisition, equipment-coordinate geometry, and non-claim decision.
