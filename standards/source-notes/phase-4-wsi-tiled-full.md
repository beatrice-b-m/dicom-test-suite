# Phase 4 Small TILED_FULL Whole Slide Microscopy Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `vl/wsi/tiled_full_small`
- Recipe ID: `vl_wsi_tiled_full_small`
- SOP Class: VL Whole Slide Microscopy Image Storage
  (`1.2.840.10008.5.1.4.1.1.77.1.6`)
- IOD: VL Whole Slide Microscopy Image
- Modality: `SM`
- Transfer Syntax: Explicit VR Little Endian
  (`1.2.840.10008.1.2.1`)
- Recommended provider: `rust_native`
- Recommended determinism: `byte_stable`
- Profile: `extended`

This is a small synthetic viewer-compatibility volume. It is not a diagnostic
pathology fixture and contains no identifiable slide or patient data. UIDs,
dates, times, identity, specimen metadata, optical-path metadata, geometry,
pixels, and equipment values shall be deterministic recipe inputs independent
of the host, locale, network, and clock. No generated slide is committed.

## Required Decision

Implement a single native `TILED_FULL` VOLUME instance with four native RGB
frames. Each Frame is a 2 by 2 tile; the Total Pixel Matrix is 4 by 4 pixels.
There is one optical path, one focal plane, and no Concatenation. The Frames
are stored in the implicit order required by `TILED_FULL`: first left to right
along the row direction and then top to bottom along the column direction.

The four deterministic tiles are solid red, green, blue, and white in that
order. Their stored, interleaved RGB frame SHA-256 values are:

1. red:
   `fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8`
2. green:
   `6c8f6d772829d493618e079a099cf4f20d8524ed3656f49db234f5bbf60a4e65`
3. blue:
   `7263ad3fd60c6620abd423516d748baedf5e393b1fbdaaf780ff5803a443cc4f`
4. white:
   `8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21`

Reconstructing those tiles into row-major 4 by 4 interleaved RGB samples
produces SHA-256
`62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a`.
This matrix hash is distinct from the concatenated-frame hash and is the
independent mapping oracle.

The exact pixel contract is RGB, Samples per Pixel `3`, Planar Configuration
`0`, unsigned 8-bit samples, native OB Pixel Data, Bits Allocated/Stored/High
Bit `8/8/7`, Lossy Image Compression `00`, and Number of Frames `4`. Image Type
and the shared Whole Slide Microscopy Image Frame Type are both
`ORIGINAL\PRIMARY\VOLUME\NONE`.

## Locked Tile Geometry And Implicit Positions

The tile geometry is:

- Rows and Columns: `2` and `2`;
- Total Pixel Matrix Rows and Columns: `4` and `4`;
- Total Pixel Matrix Focal Planes: `1`;
- Pixel Spacing: `0.5\0.5` mm;
- Slice Thickness: `0.001` mm;
- Imaged Volume Width and Height: `2.0` mm and `2.0` mm;
- Imaged Volume Depth: `1.0` micrometer;
- Total Pixel Matrix Origin: X/Y/Z `0.0/0.0/0.0`;
- Image Orientation (Slide): `1\0\0\0\1\0`; and
- Position Reference Indicator: `SLIDE_CORNER`.

Dimension Organization Type is `TILED_FULL`. Dimension Organization Sequence
contains one deterministic Dimension Organization UID. Dimension Index
Sequence and Per-Frame Functional Groups Sequence are absent. The shared
Functional Groups item contains exactly Pixel Measures and Whole Slide
Microscopy Image Frame Type.

The standard's implicit ordering reconstructs these one-based total-matrix
positions and slide-coordinate offsets:

| Frame | Optical path | Focal plane | Column | Row | X mm | Y mm | Z |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | `RGB` | 1 | 1 | 1 | 0.0 | 0.0 | 0.0 |
| 2 | `RGB` | 1 | 3 | 1 | 1.0 | 0.0 | 0.0 |
| 3 | `RGB` | 1 | 1 | 3 | 0.0 | 1.0 | 0.0 |
| 4 | `RGB` | 1 | 3 | 3 | 1.0 | 1.0 | 0.0 |

Plane Position (Slide) and Optical Path Identification Functional Group Macros
may be present for `TILED_FULL`, but the standard permits both to be omitted
because their values are implicit. The locked primary validator emitted six
warnings when a conformant prototype redundantly encoded Plane Position
(Slide) per Frame, treating the optional macro content as outside its selected
conditional path. The exact case therefore omits Per-Frame Functional Groups.
This is not a reduction in coverage: independently reconstructing the implicit
positions from origin, orientation, spacing, dimensions, focal planes, optical
path order, and Frame order is the central milestone gate. The later
`TILED_SPARSE` case will require explicit per-frame positions.

## Locked Specimen, Optical Path, And Slide Label Contract

The Specimen Module contains Container Identifier `DTS-SLIDE-001`, an empty
Type 2 Issuer of the Container Identifier Sequence, and an empty Type 2
Container Type Code Sequence. Specimen Description Sequence has exactly one
Item with Specimen Identifier `DTS-SPECIMEN-001`, a deterministic Specimen UID,
an empty Type 2 Issuer of the Specimen Identifier Sequence, and an empty Type 2
Specimen Preparation Sequence. The optional Specimen Reference Functional
Group is absent because a single specimen is described for the entire image.

Number of Optical Paths is `1`. Optical Path Sequence has one Item identified
as `RGB`, with Illumination Wave Length `550` nm and one Illumination Type Code
Sequence Item `(111744, DCM, "Brightfield illumination")`. Because the stored
pixels are RGB, the Item contains the existing locked 736-byte DCMTK sRGB input
ICC Profile. The nested profile bytes and SHA-256 must equal the profile already
qualified by `standards/source-notes/phase-2-icc-profile.md`; a top-level Image
Pixel Description ICC Profile is not used when the Optical Path Module is used.

The optional Slide Label Module is present for this VOLUME image with Barcode
Value `DTS-SLIDE-001` and Label Text `DTS SYNTHETIC SLIDE 001`. Specimen Label
in Image remains `NO`: label metadata does not claim that label pixels appear
inside this volume. Burned In Annotation is `NO`, Focus Method is `AUTO`,
Extended Depth of Field is `NO`, and the conditional Number of Focal Planes and
Distance Between Focal Planes are absent.

Acquisition Context Sequence is present and empty. Since the instance has no
references, Referenced Series Sequence is absent; encoding it as an empty
Sequence is an IOD error. Multi-Resolution Pyramid, Concatenation attributes,
Lossy Image Compression Ratio and Method, Dimension Index Sequence, and
Per-Frame Functional Groups Sequence are also explicitly absent.

## Locked IOD And Module Contract

PS3.3 A.32.8 and Table A.32.8-1 define the VL Whole Slide Microscopy Image
IOD. Patient, General Study, General Series, Whole Slide Microscopy Series,
Frame of Reference for VOLUME, General and Enhanced Equipment, General
Acquisition, General Image, Microscope Slide Layer Tile Organization, Image
Pixel, Acquisition Context, Multi-frame Functional Groups, Multi-frame
Dimension, Specimen, Whole Slide Microscopy Image, Optical Path, SOP Common,
and Common Instance Reference are mandatory. The Slide Label Module may be
present for VOLUME images.

PS3.3 C.8.12.3 requires Modality `SM`. C.8.12.4 defines the WSI image and pixel
attributes, requires Image Type, permits native RGB, requires Planar
Configuration `0` for color, constrains Bits Stored and High Bit, and requires
the physical extent attributes for VOLUME. It requires Specimen Label in Image
to be `NO` for VOLUME. C.8.12.5 requires one or more Optical Path Sequence
Items, requires Number of Optical Paths for `TILED_FULL`, and requires a nested
ICC Profile for RGB.

PS3.3 C.8.12.14 defines Total Pixel Matrix rows, columns, focal planes, origin,
and Image Orientation (Slide). C.7.6.17 defines `TILED_FULL`; C.7.6.17.3 fixes
the implicit order across columns, rows, focal planes, optical paths, and any
segments. A.32.8.4 and Table A.32.8-2 require Pixel Measures and WSI Frame Type
as shared Functional Groups and permit omission of per-frame Plane Position
(Slide) and Optical Path Identification for `TILED_FULL`. C.8.12.9 defines the
shared Frame Type Macro.

PS3.3 C.7.6.22 defines the mandatory specimen and container identity. C.8.12.8
defines Barcode Value and Label Text as Type 2 when the Slide Label Module is
included. C.7.4.1 defines the slide Frame of Reference. C.7.6.16.2.1 defines
Pixel Measures. PS3.4 Table B.5-1 identifies the composite storage SOP Class.
PS3.6 Tables A-1 and 6-1 identify the SOP Class, Transfer Syntax, and attribute
registry properties.

## KB Query And Locked Local Evidence

- Query:
  `dicom-kb lookup uid VLWholeSlideMicroscopyImageStorage --edition 2026b`
- Result: `1.2.840.10008.5.1.4.1.1.77.1.6`
- Edition: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the registry's existing evidence proves only the UID identity.
  It does not bind the IOD module table, `TILED_FULL` implicit order, physical
  geometry, Functional Group placement, specimen, optical-path ICC, slide
  label, or absence decisions needed by this recipe.

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
No downloaded standard artifact is committed, and this note requires no
change to `standards.lock.json` because source-note accounting is not stored
there.

## Independent Validator And Reconstruction Qualification

A temporary read-only pydicom prototype encoded the exact minimal structural
contract. The first run exposed two genuine errors: Position Reference
Indicator had to be `SLIDE_CORNER`, and an empty Referenced Series Sequence was
invalid when there were no references. After correcting those errors and
omitting redundant per-frame positions, locked dicom3tools `dciodvfy -new`
reported the exact VL Whole Slide Microscopy Image IOD with zero findings.
Locked DCMTK 3.7.0 `dcmdump` parsed the Part 10 object.

The separately implemented, `uv`-locked highdicom 0.28.1 and pydicom 3.0.2
stack then reopened the object. `iter_tiled_full_frame_data()` independently
derived the four exact optical-path, focal-plane, total-matrix, and slide
coordinates listed above. `Image.from_dataset(...).get_total_pixel_matrix()`
with display, real-world, VOI, palette, presentation, and ICC transforms
disabled reconstructed a `(4, 4, 3)` unsigned-byte array with the locked matrix
SHA-256. Disabling ICC transformation is intentional: this evidence validates
stored samples and tile placement, while strict validation separately binds
the nested ICC profile bytes.

Promotion requires the primary `dciodvfy` route, the existing `uv`-locked
pydicom dicom-validator as an additive secondary 2026b IOD opinion, DCMTK
parsing, and a dedicated case-scoped highdicom reconstruction adapter. The
reconstruction adapter must reject a swapped, missing, or changed Frame; wrong
matrix extent, origin, orientation, spacing, optical-path count, or focal-plane
count; a `TILED_SPARSE` substitution; and any source-manifest, executable,
adapter, or lock relinking. Strict reopened-file validation owns the complete
module and absence contract. No finding may be silently allowlisted.

The prototype Part 10 object was approximately 2.4 KiB with a placeholder ICC
payload. Replacing it with the locked 736-byte profile remains only a few KiB,
well within the `extended` profile and far below a stress threshold.

## Provider Decision

Use the native Rust writer. The locked highdicom release provides specimen,
slide-coordinate, tiled-image reading, and reconstruction utilities but no VL
Whole Slide Microscopy SOP constructor. An external provider would therefore
hand-build the same pydicom dataset and retain only semantic stability. Native
generation reuses project-owned deterministic UIDs, sequences, native RGB
Pixel Data, multi-frame validation, and the locked ICC asset, permits byte
stability, and leaves Python optional for conformance rather than making it a
generation requirement of `extended`.

Generator independence is preserved because highdicom and pydicom only reopen
and reconstruct the Rust-produced object. Their implementation is not used to
choose or serialize the Frames, positions, specimen, or optical-path content.

## Decision Checkpoint Audit

Proceeding with Phase 4 milestone 2 triggers no explicit decision checkpoint
in `docs/coverage-expansion-plan.md`. Native generation adds no mandatory
runtime or codec. The user has adopted `uv` as the external Python runtime
manager and has authorized selecting and locking another independent IOD
validator. Python remains optional conformance tooling and is not made
mandatory for an existing profile. The case is lossless, small, and remains in
`extended`; it adds no identifiable fixture, certificate, key, protocol rule,
full-size stress job, accepted finding, or change to `all` profile semantics.

## Qualification Disposition

- Registry status remains planned until generation, manifest, strict
  validation, reports, independent reconstruction, two-run reproducibility,
  and integrated conformance are complete.
- Recommended promotion: provider `rust_native`, determinism `byte_stable`,
  profile `extended`.
- The plan's frame-to-total-pixel-matrix gate is satisfied only by comparing
  both independently derived positions and reconstructed matrix bytes with the
  locked manifest contract.
- Milestone 3, the deliberately incomplete `TILED_SPARSE` counterpart, remains
  dependency-ordered after this slice and must use explicit per-frame
  positions.
- Should become KB patch: yes; expose the WSI IOD module table, shared and
  per-frame Functional Group rules, `TILED_FULL` implicit ordering, and
  specimen/optical-path constraints as stable typed query results.
- Expected cleanup after KB coverage exists: replace local module summaries
  with direct typed KB evidence while retaining the exact synthetic recipe,
  implicit-position decision, validator qualification, reconstruction oracle,
  and deterministic provider rationale.
