# Phase 4 Multi-Resolution WSI Pyramid Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `vl/wsi/pyramid_multiresolution`
- Recipe ID: `vl_wsi_pyramid_multiresolution`
- SOP Class: VL Whole Slide Microscopy Image Storage
  (`1.2.840.10008.5.1.4.1.1.77.1.6`)
- IOD: VL Whole Slide Microscopy Image
- Modality: `SM`
- Transfer Syntax: Explicit VR Little Endian
  (`1.2.840.10008.1.2.1`)
- Recommended provider: `rust_native`
- Recommended determinism: `byte_stable`
- Profile: `stress`
- Registry action during standards lock: remain `planned`
- Final qualification disposition: `implemented`

This milestone is one logical case with exactly three native DICOM instances:
a highest-resolution VOLUME layer, a THUMBNAIL apex layer, and a LABEL
companion. It is a deliberately tiny, opt-in stress-profile compatibility
fixture, not a diagnostic pathology fixture or the future full-size large-slide
case. UIDs, dates, times, identity, specimen and container metadata, geometry,
pixels, roles, order, and filenames shall be deterministic recipe inputs
independent of the host, locale, network, and clock. No generated slide is
committed.

## Required Three-Instance Contract

The exact ordered output group is:

| Ordinal | Role | Image Type | Rows x Columns | Frames | Pyramid member |
| ---: | --- | --- | ---: | ---: | --- |
| 1 | `volume` | `ORIGINAL\\PRIMARY\\VOLUME\\NONE` | `2 x 2` | 4 | yes |
| 2 | `thumbnail` | `DERIVED\\PRIMARY\\THUMBNAIL\\RESAMPLED` | `2 x 2` | 1 | yes, apex |
| 3 | `label` | `ORIGINAL\\PRIMARY\\LABEL\\NONE` | `2 x 2` | 1 | no |

The VOLUME is `TILED_FULL`. Its four 2 by 2 Frames form a 4 by 4 Total
Pixel Matrix and are stored in implicit row-major tile order: solid red,
green, blue, and white. The exact Frame SHA-256 values are the qualified
values from `phase-4-wsi-tiled-full.md`:

1. red:
   `fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8`;
2. green:
   `6c8f6d772829d493618e079a099cf4f20d8524ed3656f49db234f5bbf60a4e65`;
3. blue:
   `7263ad3fd60c6620abd423516d748baedf5e393b1fbdaaf780ff5803a443cc4f`;
   and
4. white:
   `8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21`.

Their ordered 48-byte payload SHA-256 is
`b40b0afc9b180d5ebfb54a7db428e13fe09a33dcc9a8f76220f395ba2c68d2db`,
and independent reconstruction shall produce the previously locked 4 by 4
matrix SHA-256
`62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a`.

The THUMBNAIL is the one-Frame apex of the same pyramid. Its four row-major
RGB pixels are red, green, blue, and white, a deterministic 2 by 2 reduction
of the VOLUME quadrant colors. Its exact 12-byte payload and reconstructed
matrix SHA-256 are both
`6733cdd08e5c7ef0453e2759ef0d28fbd43ea2aa7883b55422a13dac38e23ecc`.

The LABEL is a one-Frame companion containing only a synthetic,
non-identifying two-tone label marker. Its row-major RGB pixels are dark blue
`[0, 32, 96]`, white `[255, 255, 255]`, dark blue, and white. Its exact
12-byte payload SHA-256 is
`ad078f83d3ea66f075867d116c8c126e9c8a8a9dd873cd27280371c173d8ad02`.
The pixel pattern represents the locked synthetic specimen label only and
contains no patient-identifying annotation.

All three instances use native OB Pixel Data, RGB, Samples per Pixel `3`,
Planar Configuration `0`, unsigned 8-bit samples, Bits
Allocated/Stored/High Bit `8/8/7`, Pixel Representation `0`, and Lossy Image
Compression `00`. Image Type and the shared Whole Slide Microscopy Image
Frame Type Macro shall match exactly for each role. THUMBNAIL and LABEL are
single Frame as required by the IOD.

## Locked Pyramid Geometry And Membership

The VOLUME carries the qualified 4 by 4 geometry from the small tiled-full
case: 2 by 2 tiles, Pixel Spacing `0.5\\0.5` mm, Slice Thickness `0.001` mm,
Imaged Volume Width and Height `2.0` mm, Imaged Volume Depth `1.0`
micrometer, Total Pixel Matrix origin `0.0/0.0/0.0`, Image Orientation
(Slide) `1\\0\\0\\0\\1\\0`, Position Reference Indicator
`SLIDE_CORNER`, one optical path, and one focal plane. Its Dimension
Organization Type is `TILED_FULL`; Dimension Index Sequence and Per-Frame
Functional Groups Sequence are absent.

The THUMBNAIL is one 2 by 2 `TILED_FULL` tile with a 2 by 2 Total Pixel
Matrix, Pixel Spacing `1.0\\1.0` mm, and the same 2.0 by 2.0 mm imaged
extent, origin, orientation, optical path, and focal-plane identity. This
makes it a lower-resolution representation of the same synthetic image data
and the apex of the pyramid. Its Dimension Index Sequence and Per-Frame
Functional Groups Sequence are absent under the single-tile `TILED_FULL`
rule.

The LABEL is one 2 by 2 `TILED_FULL` tile with a 2 by 2 Total Pixel Matrix,
Pixel Spacing `0.5\\0.5` mm, a 1.0 by 1.0 mm imaged extent, and the shared
origin, orientation, optical path, and focal-plane identity. It likewise
omits Dimension Index Sequence and Per-Frame Functional Groups Sequence.

The VOLUME and THUMBNAIL contain the Multi-Resolution Pyramid Module and
share one deterministic Pyramid UID. The optional Pyramid Label and Pyramid
Description are absent. The LABEL is not
a resolution layer: it shall omit Pyramid UID and every other
Multi-Resolution Pyramid Module attribute. A LABEL that copies the Pyramid
UID is invalid even if all three instances otherwise share Series and Frame
of Reference identity.

All three instances share one deterministic Patient identity, Study Instance
UID, Series Instance UID, Frame of Reference UID, Container Identifier
`DTS-SLIDE-001`, Specimen Identifier `DTS-SPECIMEN-001`, Specimen UID,
optical-path identifier `RGB`, and the locked specimen/container semantics.
They have distinct deterministic SOP Instance UIDs derived from the semantic
roles `volume`, `thumbnail`, and `label`; no UID is selected from generation
order or a random source. The shared Study and Series bind the companion set,
while the shared Frame of Reference makes the LABEL eligible to accompany the
pyramid in that Series. No DICOM reference edge is invented where the IOD
does not require one.

## Locked Specimen, Optical Path, ICC, And Label Semantics

The complete specimen, container, and optical-path contract is inherited
from `phase-4-wsi-tiled-full.md`: one specimen and one `RGB` optical path,
Illumination Wave Length `550` nm, Illumination Type
`(111744, DCM, "Brightfield illumination")`, and the nested locked 736-byte
DCMTK sRGB input ICC Profile with SHA-256
`8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef`
and Color Space `SRGB`. A top-level Image Pixel Description ICC Profile is
absent.

The Slide Label Module is present on all three instances with Barcode Value
`DTS-SLIDE-001` and Label Text `DTS SYNTHETIC SLIDE 001`; it is mandatory for
the LABEL and optional but deliberately retained on both pyramid layers.
Specimen Label in Image is exactly `NO` for VOLUME and THUMBNAIL and exactly
`YES` for LABEL. Burned In Annotation is `NO` for every role: the LABEL pixels
encode only the synthetic specimen label marker and no patient-identifying
data. Focus Method is `AUTO`, Extended Depth of Field is `NO`, and Number of
Optical Paths is `1` throughout.

Acquisition Context Sequence is present and empty. Referenced Series
Sequence, Concatenation attributes, extended-depth-of-field focal-plane count
and distance, Lossy Image Compression Ratio and Method, and unrelated optional
modules are absent. The exact role-specific presence and absence rules are
manifest and strict-validation obligations, not informal documentation.

## Manifest Group Closure And Reports

One selected registry case shall emit exactly three manifest file entries
with the same case ID and an explicit ordered role field: `volume`,
`thumbnail`, then `label`. The case-level pyramid contract shall bind:

- exact member count, role order, relative paths, SOP Class and SOP Instance
  UIDs, and file SHA-256 values;
- shared Patient, Study, Series, Frame of Reference, container, specimen,
  optical-path, and ICC identities;
- distinct SOP Instance UIDs and exact role-specific Image Type, Frame Type,
  dimensions, frame counts, pixel hashes, and reconstructed matrix hashes;
- one Pyramid UID shared by exactly VOLUME and THUMBNAIL and absent from
  LABEL; and
- complete closure: no missing, duplicate, foreign, or unclaimed member and
  no fourth instance carrying that Pyramid UID in the generated root.

Strict validation shall reopen all three files before evaluating this graph.
JSON and Markdown reports shall expose group status, ordered roles, pyramid
membership, apex role, shared-identity closure, role-specific image flavor,
frame and matrix shape, pixel hashes, and external-validator disposition.
Generation succeeds only when the three reopened instances and manifest agree
exactly; reports shall not infer closure from filenames.

## Locked Standards Evidence

PS3.3 A.1.2.30 defines the Multi-Resolution Pyramid IE. Each resolution layer
is a separate DICOM Image with uniform Pixel Spacing; all layers share one
Frame of Reference and Series; only one instantiated pyramid may occupy that
Series. A LABEL, OVERVIEW, or THUMBNAIL may accompany the pyramid in the same
Series when it shares the Frame of Reference. A THUMBNAIL may be the apex.
Pyramid UID is shared by all and only the layers of one pyramid, while its
absence means the Multi-Resolution Pyramid IE is not instantiated.

PS3.3 A.32.8 and Table A.32.8-1 define the VL Whole Slide Microscopy Image
IOD. The Frame of Reference Module is required for VOLUME and THUMBNAIL and
may be present for LABEL. The Multi-Resolution Pyramid Module shall be present
only for VOLUME or THUMBNAIL. The Slide Label Module is required for LABEL and
may be present otherwise. A.32.8.1 permits a WSI image to be one layer of a
Multi-Resolution Pyramid.

PS3.3 C.7.11.1 and Table C.7.11.1-1 define Pyramid UID as Type 1 and Pyramid
Label and Pyramid Description as Type 3. PS3.3 C.8.12.4.1.1 and Tables
C.8.12.4-2 and C.8.12.4-3 define VOLUME, LABEL, and THUMBNAIL Image Type
flavors and the `NONE` and `RESAMPLED` derived-pixel values. VOLUME and
THUMBNAIL exclude the label; LABEL captures the slide label only; THUMBNAIL
may be the lowest-resolution layer.

PS3.3 C.8.12.4 requires Number of Frames `1` for THUMBNAIL, LABEL, and
OVERVIEW. It requires Specimen Label in Image `YES` for LABEL and `NO` for
THUMBNAIL or VOLUME. C.7.6.17.3 permits those single-Frame flavors to use
`TILED_FULL` for implicit placement even when their spatial extent differs
from the Total Pixel Matrix. C.8.12.14 defines Total Pixel Matrix geometry;
C.8.12.5, C.7.6.22, C.8.12.8, C.7.4.1, and C.7.6.16.2.1 continue to bind the
optical path, specimen, slide label, slide Frame of Reference, and Pixel
Measures content.

PS3.4 Table B.5-1 identifies the composite storage SOP Class. PS3.6 Tables A-1
and 6-1 identify the SOP Class, Transfer Syntax, Pyramid UID, and attribute
registry properties.

The local `dicom-standard-kb` query
`dicom-kb lookup uid VLWholeSlideMicroscopyImageStorage --edition 2026b`
establishes the SOP Class UID but does not bind pyramid membership, apex and
companion semantics, role-specific module conditions, group closure, or pixel
relationships. The official locked 2026b evidence is therefore required. Its
SHA-256 identities are:

- PS3.3 DocBook:
  `4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`;
- PS3.4 DocBook:
  `8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`;
- PS3.6 DocBook:
  `512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`;
  and
- generated IOD definitions:
  `ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683`.

The repository lock records official source artifacts as
`unavailable_not_downloaded`; the separately provisioned validator cache
provides the hash-locked read-only copies used for this review. No official or
generated standard artifact is committed. This slice does not currently
require a KB patch; future typed pyramid, WSI image-flavor, and conditional
module queries would supersede the hand-authored summaries in this note.

## Independent Qualification And Negative Controls

A temporary pydicom prototype encoded the exact three-instance topology. The
VOLUME, THUMBNAIL, and LABEL Part 10 files were respectively 2,966, 2,946,
and 2,882 bytes, for 8,794 total DICOM bytes. Locked `dciodvfy` and the
independently `uv`-locked dicom-validator each validated all three instances
with zero warnings and zero errors; the six IOD invocations completed together
in 0.5 seconds wall time. Locked DCMTK `dcmdump` parsed all three files.
Prototype generation took 0.09 seconds wall time.

The final native implementation completed the independent gate. Two
independent seed-7 stress roots each contain exactly three instances and six
total Frames, pass strict validation 3/3 with zero failures, and compare
byte-for-byte as complete trees. Their manifest SHA-256 is
`75c1ff84c0ab971f99308991308552640f593fcd199c652bd787908076ca6265`.
The final member evidence is:

| Role | Bytes | SHA-256 |
| --- | ---: | --- |
| VOLUME | 2,934 | `fece75ee74a3e8d9902807b2c3ace1384e0896469c4b41358d3b2d6444de7b07` |
| THUMBNAIL | 2,914 | `159cf9c96bbb205966ee924ac5f6c4385c1e4474f672fa8a7410bcacb998defb` |
| LABEL | 2,846 | `aa6c79cb54c41cb1267425bc5602fa4c916bc91b9f3fa66fd9942be446f45438` |

The qualified native group totals 8,694 bytes. Parallel generation completed
in 0.55 and 0.59 seconds, preserving the locked three-instance, six-Frame,
65,536-byte, and five-second ceilings. Both locked `dciodvfy` and the
independent `uv`-locked dicom-validator reported zero IOD errors for each role.

The isolated `uv`-locked highdicom 0.28.1/pydicom 3.0.2 reconstruction route,
adapter version 0.3.0, imports no generator code. It independently derived the
VOLUME, THUMBNAIL, and LABEL roles from DICOM attributes and reproduced the
exact payloads, matrices, VOLUME-to-THUMBNAIL reduction, identity sharing,
pyramid membership, and LABEL exclusion. Integrated run
`0188fc12678acf82e29f27c139d531dd060ec8e2f36363c9927d4d673d869f6d`
records zero entity findings, passing independent pixel evidence, zero
accepted findings, and zero verification failures against an empty exact-slice
findings set. Strict Rust validation separately owns the complete IOD,
manifest, group, ICC, and absence contract; no validator finding was silently
dropped or converted into an accepted finding. The registry disposition is
therefore 148 implemented and 34 planned cases.

At minimum, qualification shall reject controls that:

- omit, duplicate, reorder, or add a group member, role, or manifest entry;
- change or duplicate a SOP Instance UID, or break shared Study, Series,
  Frame of Reference, container, specimen, or optical-path identity;
- remove Pyramid UID from either VOLUME or THUMBNAIL, give them different
  Pyramid UIDs, or add any Pyramid UID to LABEL;
- change Image Type or shared Frame Type, including treating LABEL as a
  pyramid layer, THUMBNAIL as a non-apex companion, or derived THUMBNAIL pixels
  as `NONE`;
- change Specimen Label in Image from `NO/NO/YES` for
  VOLUME/THUMBNAIL/LABEL, remove required LABEL slide-label content, or place
  the synthetic label pattern in VOLUME or THUMBNAIL;
- alter TILED_FULL organization, frame counts, tile order, matrix extents,
  spacing, physical extent, origin, orientation, optical-path or focal-plane
  count, ICC bytes, stored payloads, or reconstructed hashes;
- make the THUMBNAIL pixels differ from the exact deterministic VOLUME
  quadrant reduction; or
- relink the executable, adapter, source manifest, official standard inputs,
  generated definitions, or a `uv` lock.

## Provider And Budget Decision

Use the native Rust writer and declare byte stability only after two
independent same-seed stress roots compare byte-for-byte. Native serialization
reuses the qualified WSI module, ICC, tile, and deterministic UID machinery;
Python remains optional independent conformance infrastructure rather than a
generation-time requirement.

The opt-in qualification ceiling is exactly three instances, six total
Frames, no more than 65,536 total DICOM bytes, and no more than 5 seconds of
generation wall time on the qualification host. These limits provide broad
headroom over the qualified 8,694 bytes and 0.59-second maximum without
pretending to define a large-slide workload. Any breach fails the slice rather
than silently reducing its dimensions or membership.

| Measure | Qualified result | Locked ceiling |
| --- | ---: | ---: |
| Instance count | 3 | exactly 3 |
| Total Frame count | 6 | exactly 6 |
| Total DICOM bytes for the group | 8,694 | 65,536 |
| Largest single instance bytes | 2,934 | included in total ceiling |
| Generation wall time | 0.55 / 0.59 seconds | 5 seconds |
| Prototype six IOD invocations | 0.5 seconds | evidence only |

The instance, Frame, byte, and generation-time ceilings shall be enforced by
an opt-in stress qualification test. No memory ceiling is asserted without a
measurement, and no per-validator performance requirement is added. The
implementation reproduced the locked topology within every budget and
completed the semantic and independent-reconstruction gates.

This small three-instance group is distinct from
`stress/wsi/large_pyramid`. It adds no full-size fixture or ordinary-CI job.
Selecting dimensions, tile count, memory/runtime ceilings, and CI scheduling
for a genuinely large pyramid remains the plan's explicit decision checkpoint
and is not authorized by promotion of this small opt-in slice.

## Qualification Disposition

- Registry status is implemented after generation, manifests, strict group
  validation, reports, independent reconstruction, both IOD opinions,
  negative controls, measured stress budgets, and two-run reproducibility
  completed without accepted findings.
- Promoted provider: `rust_native`; determinism: `byte_stable`; profile:
  `stress`.
- Independent reconstruction binds both pyramid layers, the deterministic
  reduction, and the non-member LABEL companion to the exact manifest closure.
- Promotion of this bounded slice does not authorize a full-size pyramid in
  ordinary CI and does not resolve the separate large-pyramid registry case.
- Should become KB patch: yes; expose Multi-Resolution Pyramid IE membership,
  role-specific WSI module conditions, Image Type flavors, and single-Frame
  companion rules as stable typed query results.
