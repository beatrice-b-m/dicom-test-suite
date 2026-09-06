# Phase 4 Single-frame VL Endoscopic And Microscopic Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `vl/endoscopic/rgb_explicit_le`
- Recipe ID: `vl_endoscopic_rgb_explicit_le`
- SOP Class: VL Endoscopic Image Storage
  (`1.2.840.10008.5.1.4.1.1.77.1.1`)
- Modality: `ES`
- Case ID: `vl/microscopic/rgb_explicit_le`
- Recipe ID: `vl_microscopic_rgb_explicit_le`
- SOP Class: VL Microscopic Image Storage
  (`1.2.840.10008.5.1.4.1.1.77.1.2`)
- Modality: `GM`
- Transfer Syntax: Explicit VR Little Endian
  (`1.2.840.10008.1.2.1`)
- Recommended provider: `rust_native`
- Recommended determinism: `byte_stable`
- Profile: `extended`

These are synthetic viewer-compatibility images, not diagnostic or pathology
fixtures. UIDs, dates, times, identity, anatomy, pixels, and equipment values
shall be deterministic recipe inputs independent of the host, locale, network,
and clock.

## Required Decision

Implement the two Phase 4 milestone-1 cases as native specializations of the
existing single-frame RGB VL pixel path. The common stored-pixel contract is a
single 2 by 2 frame, RGB, color-by-pixel Planar Configuration `0`, unsigned
8-bit samples, native OB Pixel Data, Image Type `ORIGINAL\\PRIMARY`, and Lossy
Image Compression `00`. Acquisition Context Sequence `(0040,0555)` is the
mandatory Type 2 empty Sequence. Number of Frames and Frame of Reference UID
are absent.

The Endoscopic case represents examination of the right lung: Body Part
Examined `(0018,0015)` is `LUNG` and Laterality `(0020,0060)` is `R`. The
Microscopic case represents direct microscopic imaging of the patient's right
eye: Body Part Examined is `EYE` and Laterality is `R`. These paired-anatomy
choices make the General Series Type 2C Laterality requirement unambiguous and
prevent an empty value from being used merely to satisfy a validator.

The Microscopic recipe deliberately selects direct patient imaging rather than
a specimen. It therefore omits the conditional Specimen Module. It also omits
both optional, mutually exclusive ICC Profile and Optical Path Modules.
Microscopic images with slide coordinates shall not use this IOD; slide and
whole-slide semantics belong to later Phase 4 milestones.

## Locked IOD And Module Contract

PS3.3 A.32.1 and Table A.32.1-1 define the VL Endoscopic Image IOD. Patient,
General Study, General Series, General Equipment, General Acquisition, General
Image, Image Pixel, VL Image, Acquisition Context, and SOP Common are mandatory.
Section A.32.1.4.1 requires Modality `ES`. The IOD has no Frame of Reference IE.

PS3.3 A.32.2 and the standard's Table A.32.1-2 identifier define the VL
Microscopic Image IOD. The same common modules are mandatory. Section
A.32.2.4.1 requires Modality `GM`. Section A.32.2.1 permits direct microscopic
imaging of the Patient as well as specimen imaging and prohibits encoding
microscopic images with slide coordinates using this IOD. The IOD has no Frame
of Reference IE.

PS3.3 C.8.12.1 and Table C.8-77 constrain both recipes through the VL Image
Module:

- Photometric Interpretation `(0028,0004)` is `RGB`;
- Samples per Pixel `(0028,0002)` is `3`;
- Planar Configuration `(0028,0006)` is `0`;
- Bits Allocated, Bits Stored, and High Bit are `8`, `8`, and `7`;
- Pixel Representation `(0028,0103)` is unsigned `0`;
- Image Type `(0008,0008)` is `ORIGINAL\\PRIMARY`; and
- Lossy Image Compression `(0028,2110)` is `00`.

PS3.3 C.7.3.1 and Table C.7-5a make Laterality Type 2C for a paired body part
when image-, frame-, or measurement-level laterality is absent. These recipes
select non-empty Series Laterality `R`. Body Part Examined is optional but is
present to make the selected anatomy and the condition explicit. PS3.3
C.7.6.14 and Table C.7.6.14-1 require Acquisition Context; the empty Sequence
states that no additional acquisition context items are supplied.

PS3.4 Table B.5-1 identifies both composite storage SOP Classes. PS3.6 Table
A-1 identifies both SOP Class UIDs and Explicit VR Little Endian. PS3.6 Table
6-1 defines the registry properties of the referenced attributes.

## KB Query And Locked Local Evidence

- Query: `dicom-kb lookup uid VLEndoscopicImageStorage --edition 2026b`
- Result: `1.2.840.10008.5.1.4.1.1.77.1.1`
- Query: `dicom-kb lookup uid VLMicroscopicImageStorage --edition 2026b`
- Result: `1.2.840.10008.5.1.4.1.1.77.1.2`
- Edition: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the current registry evidence proves only the two UID identities;
  it does not bind the IOD module tables, modality constraints, direct-patient
  microscopic choice, VL pixel constraints, or Laterality decision needed by
  these recipes.

The repository lock records official source artifacts as
`unavailable_not_downloaded`. The independently locked validator cache pins
official PS3.3 DocBook SHA-256
`4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`,
PS3.4 SHA-256
`8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`,
PS3.6 SHA-256
`512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`,
and generated IOD definitions SHA-256
`ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683`.
No downloaded standard artifact is committed.

## Independent Validator Qualification

A read-only prototype audit specialized the existing native planar-0 VL RGB
instance to each exact SOP Class and modality, then added the locked anatomy
and Laterality values. Both prototypes passed locked dicom3tools `dciodvfy
-new` and the separately implemented, `uv`-locked pydicom `dicom-validator`
0.8.2 against the locked 2026b definitions with zero IOD errors. DCMTK 3.7.0
`dcmdump` parsed both objects, and DCMTK `dcm2img` reconstructed the exact
binary P6 RGB samples.

The locked dicom3tools executable SHA-256 is
`1aeb75d6ccd3f193e3b322b6da77742cdce2e0604868eaf2a2669c786cbc27e5`.
The shared pydicom adapter fingerprint is
`2813c20e61cd625955429a999de42c52c9b1fec25f3e2a3b168dc0b41b46b72c`.
The DCMTK `dcmdump` and `dcm2img` executable SHA-256 values are respectively
`d2261944ea1ceb6743df9866f2237014b284fa39119c8a5eee226ae922ead45f`
and
`6a6103a7c516814b5eb44f53d198b111cbaf1678de5952ab7d31961732f112d5`.

The prototype without Laterality failed `dciodvfy` with a missing Type 2C
General Series Laterality finding, while the pydicom IOD validator did not
report it. Qualification must therefore retain both independent IOD opinions,
strict manifest-driven validation, and the meaningful non-empty anatomy and
Laterality contract. Independent parsing alone is not IOD validation, and an
IOD validator is not an independent pixel decoder.

Promotion requires exact-case conformance routing, negative controls for wrong
SOP Class, modality, anatomy, Laterality, Image Type, Acquisition Context,
pixel shape, and pixel bytes, plus two-run byte reproducibility. Unavailable
optional tools must remain explicit; no finding may be silently allowlisted.

## Decision Checkpoint Audit

Proceeding with Phase 4 milestone 1 triggers no explicit decision checkpoint
in `docs/coverage-expansion-plan.md`. Native generation adds no required
runtime or codec. The user has adopted `uv` for Python tooling and authorized
selecting and locking another independent IOD validator; the pydicom route is
additive and does not replace dicom3tools. The cases add no identifiable
pathology fixture, lossy policy, stress job, certificate, key, protocol rule,
or change to `all` profile semantics.

## Qualification Result

- Registry status: implemented; provider `rust_native`; determinism
  `byte_stable`.
- Two seed-7 extended roots each contain 109 files and validate with zero
  failures. Their byte-identical manifest SHA-256 is
  `169ed3a7878986cb289420cef935c6f8598467f240c9a8ce88bf960d30fb1958`.
- Endoscopic instance SHA-256:
  `dc3b2e155c9be0b728412df6fed7432a238a150512b176305fc6104c63bd6a3e`.
- Microscopic instance SHA-256:
  `5785f387d79f79e4b168390bb1def6520d165ac7279374b141beb2c2804f41e3`.
- Integrated conformance run
  `f410d948b8761b9a1f6802f4fce81c2b90355c62214f5f333ac33ffba130b0d3`
  contains clean primary and secondary IOD results, clean parser results, and
  passing independent pixel evidence for both exact cases. Its canonical
  `conformance-run.json` SHA-256 is
  `1c5c2a6477b81f01222d61f30ce7499046a1299522c45c6c5691e3fcfa92159b`.
- The first real pixel run rejected the configured ASCII `+opn 8` output
  because the locked collector requires binary P6. Qualification corrected the
  policy to DCMTK raw PNM `+op` and reran both exact cases successfully rather
  than weakening the parser.
- Integrated conformance accepts no finding. The existing 229 unrelated whole-
  corpus failures remain visible and unallowlisted.
- The registry now contains 145 implemented and 37 planned logical cases.
- Milestone 2, small `TILED_FULL` WSI, is the next dependency-ordered slice.
- Should become KB patch: yes; expose the two IOD module tables and content
  constraints as stable typed query results.
- Expected cleanup after KB coverage exists: replace local module and modality
  summaries with direct typed KB evidence while retaining the patient-versus-
  specimen choice, anatomy/laterality decision, validator qualification, and
  deterministic recipe contract.
