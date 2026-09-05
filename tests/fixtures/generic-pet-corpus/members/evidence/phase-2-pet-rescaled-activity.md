# Phase 2 PET Rescaled Activity Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/pet/rescaled_activity_explicit_le`
- Recipe ID: `classic_pet_rescaled_activity_explicit_le`
- PET Series activity units and correction metadata, PET Image rescale,
  mandatory empty isotope and orientation sequences, native stored pixels, and
  independently derived quantitative values

## Required Decision

Generate one single-frame Positron Emission Tomography Image Storage instance
with a 2 by 2 unsigned 16-bit native payload. Stored values `0, 100, 200, 400`
are transformed using Rescale Intercept `0` and Rescale Slope `2.5` into `0,
250, 500, 1000` Bq/ml. Units is `BQML`, Counts Source is `EMISSION`, Series
Type is `STATIC\\IMAGE`, Corrected Image is `DCAL`, Dose Calibration Factor is
`1`, and Decay Correction is `NONE`.

This slice validates activity concentration after the normative `U = m*SV+b`
mapping. It does not claim SUV semantics, radiopharmaceutical administration,
or decay correction. Radiopharmaceutical Information Sequence is present with
zero Items, as its Type 2 definition permits. STATIC and IMAGE deliberately
avoid gated, dynamic, and reprojection conditionals.

## KB Query

- Tools: `dicom_lookup_sop_class`, `dicom_lookup_iod`,
  `dicom_list_modules_for_iod`, `dicom_list_attributes_for_module`,
  `dicom_lookup_defined_terms`, `dicom_lookup_enumerated_values`, and
  `dicom_retrieve_standard_text`
- Input: Positron Emission Tomography Image Storage and Positron Emission
  Tomography Image; PET Series, PET Isotope, PET Image, and NM/PET Patient
  Orientation modules; Units, Counts Source, Series Type, Corrected Image, and
  Decay Correction contexts; PS3.3 Tables A.21.3-1, C.8-60, C.8-61, and C.8-63
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the SOP Class resolves to UID `1.2.840.10008.5.1.4.1.1.128` and the
  parsed IOD resolves when queried by its full standard name. The mandatory PET
  modules and their attribute types are parsed. The contextual Units lookup
  returns `BQML` as Becquerels/milliliter.
- Parser limitation: the abbreviated input `PET Image` does not resolve as an
  IOD name. The full name does resolve; this is a query-alias limitation, not a
  standards gap.

## Official Source Evidence

- PS3.3 Table A.21.3-1 makes PET Series, PET Isotope, NM/PET Patient
  Orientation, Frame of Reference, Image Plane, Image Pixel, and PET Image
  mandatory modules of the Positron Emission Tomography Image IOD.
- PS3.3 Table C.8-60 makes Units, Counts Source, Series Type, Number of Slices,
  and Decay Correction Type 1 and Corrected Image Type 2. `BQML`, `EMISSION`,
  `STATIC\\IMAGE`, `DCAL`, and `NONE` are defined or enumerated in their PET
  contexts.
- PS3.3 Table C.8-61 makes Radiopharmaceutical Information Sequence Type 2 and
  explicitly permits zero or more Items.
- PS3.3 Table C.8-63 requires 16 allocated and stored bits, High Bit 15, Rescale
  Intercept and Slope, Frame Reference Time, and Image Index. It states that
  PET Rescale Intercept is always zero and defines the mapping from stored
  values to the Units declared by the PET Series.
- Dose Calibration Factor records the factor used to scale from counts/sec to
  Bq/ml; `DCAL` in Corrected Image records dose-calibrator sensitivity
  calibration. An identity factor remains explicit and independently testable.

## Project Action

- Registry status: implemented. Generation, typed manifest and report
  contracts, internal and manifest-driven quantitative validation,
  determinism, exact independent native pixel extraction, and independent IOD
  gates all pass.
- Registry reason or linked issue: none.
- Should become KB patch: yes, for the missing `PET Image` IOD alias only.
- Expected cleanup after KB coverage exists: remove the local note about the
  alias limitation; retain this case-specific quantitative scope decision.

## Promotion Evidence

- Two seed-1 `core` generations each produced 44 files, were recursively
  byte-identical, and passed strict corpus validation with zero failures.
- The PET fixture SHA-256 is
  `78ced6c57926cafc6538ebf65459bb9efd7ecbb9a3c4ec90b28b4457cc795ce6`.
- Locked `dciodvfy` identifies only `PETImage`, and isolated `dcentvfy` is
  silent.
- DCMTK independently extracts the exact 8-byte native Pixel Data. Its
  SHA-256 is
  `03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784`,
  matching the manifest frame hash and unsigned stored samples `0, 100, 200,
  400`.
- The offline frozen environment managed by locked `uv` selects pydicom 3.0.2.
  It independently reads a `(2, 2)` unsigned array, derives `0, 250, 500,
  1000` Bq/ml with the declared rescale, verifies the empty mandatory
  sequences, and produces a byte-identical Part 10 rewrite. Both independent
  validators remain clean on that rewrite.
