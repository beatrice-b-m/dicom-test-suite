# Phase 2 XA Monoplane Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/xa/monoplane_explicit_le`
- Recipe ID: `classic_xa_monoplane_explicit_le`
- Classic X-Ray Angiographic Image acquisition and equipment-based projection
  geometry without contrast, subtraction, or patient-space claims

## Required Decision

Generate one single-frame X-Ray Angiographic Image Storage instance using
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
The pattern is synthetic display input, not a diagnostic angiogram.

The X-Ray Acquisition contract uses KVP 80, Radiation Setting `GR`, and
Exposure 4 mAs. Exposure satisfies the Type 2C acquisition branch while
Exposure Time and X-Ray Tube Current remain absent, so the fixture does not
invent a pulse duration or current. Imager Pixel Spacing is `0.2\\0.2` mm at
the front plane of the image receptor housing. Pixel Spacing is absent, so no
patient- or object-space calibration is claimed.

The XA Positioner contract uses a 15-degree primary angle and -10-degree
secondary angle. Distance Source to Detector is 1200 mm and Distance Source to
Patient is 800 mm; Estimated Radiographic Magnification Factor is exactly 1.5,
the declared SID/SOD ratio. These are equipment geometry and nominal field
center values, not Image Position (Patient), Image Orientation (Patient), or a
Frame of Reference.

The case is single-frame and single-plane. Number of Frames, Frame Increment
Pointer, Cine timing, Positioner Motion, angle increments, and biplane
references are absent. It makes no claim about contrast administration,
subtraction or masks, table motion, calibration phantoms, Modality or VOI LUTs,
display processing, overlays, shutters, referenced images, or patient-space
geometry. Body Part Examined is `HEART`, a narrow clinical region consistent
with angiographic projection. The heart is not paired, so Laterality is absent
rather than inventing right, left, bilateral, or median anatomy.

Patient Orientation is present with CS VR and zero Value Length. The XA IOD
uses equipment positioner angles rather than patient Image Orientation and
Image Position, so the General Image Type 2C branch requires the empty element
without assigning patient-relative row or column directions. Instance Number
is `1`.

## KB Query

- Tools: `lookup_sop_class`, `lookup_uid`, `list_modules_for_iod`,
  `list_attributes_for_module`, `lookup_enumerated_values`, and
  `lookup_defined_terms`
- Input: X-Ray Angiographic Image Storage, Explicit VR Little Endian,
  X-Ray Angiographic Image, X-Ray Image, X-Ray Acquisition, XA Positioner,
  Image Type, Pixel Intensity Relationship, Radiation Setting, positioner
  angles, receptor distances, magnification, and imager spacing
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the SOP Class resolves to non-retired UID
  `1.2.840.10008.5.1.4.1.1.12.1` and the X-Ray Angiographic Image IOD resolves
  to PS3.3 Table A.14-1. The mandatory X-Ray Image, X-Ray Acquisition, and XA
  Positioner modules resolve to Tables C.8-26, C.8-27, and C.8-30.
- Parser limitation: context-specific enumerated and defined-term lookups for
  Image Type, Pixel Intensity Relationship, and Radiation Setting return no
  parsed terms even though the module rows and persisted official text expose
  them. This is a parsed-value coverage gap, not a standards ambiguity.

## Official Source Evidence

- PS3.3 Table A.14-1 makes X-Ray Image, X-Ray Acquisition, XA Positioner,
  Image Pixel, and the common patient, study, series, equipment, acquisition,
  image, and SOP modules mandatory. Cine and Multi-frame are conditional on
  multi-frame cine data; Contrast/Bolus is conditional on contrast use; Mask
  is conditional on a subtractable image; and X-Ray Table is conditional on
  table motion. Those conditions are false for this case.
- PS3.3 Table C.8-26 and Section C.8.7.1.1.1 specialize Image Type Value 3 as
  `SINGLE PLANE`. The biplane referenced-image condition is therefore false.
  The same table permits the explicit `00` no-lossy history and requires no
  Frame Increment Pointer for a single-frame image.
- PS3.3 Section C.8.7.1.1.2 defines `LIN` as approximately proportional to
  X-Ray beam intensity. Selecting it avoids the conditional Modality LUT
  requirement associated with `LOG` while making no display-ready `DISP`
  claim.
- PS3.3 Table C.8-27 makes Radiation Setting Type 1 and KVP Type 2. Its
  acquisition branch requires Exposure when Exposure Time or X-Ray Tube
  Current is absent. `GR` is the diagnostic-quality exposure setting used by
  this case, and Exposure 4 supplies that branch.
- PS3.3 Table C.8-27 defines Imager Pixel Spacing at the receptor housing and
  explicitly distinguishes it from calibrated Pixel Spacing. The fixture
  preserves that distinction by omitting Pixel Spacing.
- PS3.3 Table C.8-30 makes both positioner angles Type 2 and defines optional
  source-to-detector, source-to-patient, and magnification geometry. Positioner
  Motion is conditional on more than one frame; angle increments are
  conditional on dynamic motion. Neither condition applies.
- PS3.3 Table C.7-9 requires zero-length Patient Orientation when an IOD uses
  other orientation Attributes and does not require patient Image Orientation
  and Image Position. The fixture preserves that empty Type 2C distinction.
- PS3.3 Table C.7-5a defines Body Part Examined as Type 3 and Laterality as
  Type 2C for paired anatomy when no lower-level laterality is present. `HEART`
  leaves the paired-body-part condition false, so Laterality is absent.
- Source artifact identity is the locked DICOM 2026b KB source manifest above.

## Project Action

- Registry status: planned until generation, typed manifest and report
  contracts, internal and manifest-driven validation, determinism, exact
  independent native pixel extraction, and independent IOD gates pass.
- Registry reason or linked issue: `recipe_unimplemented`.
- Should become KB patch: yes; the missing context-specific value-term results
  are systematic parser coverage gaps.
- Expected cleanup after KB coverage exists: replace local value-term fallbacks
  with normal KB evidence while retaining this case-specific acquisition,
  equipment-geometry, and non-claim decision.
