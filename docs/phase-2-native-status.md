# Phase 2 Native Coverage Status

This status note records independently verified Phase 2 native vertical
slices. Generated corpora, validator output, and comparison files remain
ignored and uncommitted. A registry row is promoted from `planned` only after
its generator, manifest contract, internal validation, coverage reports,
focused tests, determinism check, and independent conformance checks agree.

## Geometry and series milestone

### Non-uniform CT slice spacing

`geometry/ct/nonuniform_slice_spacing` is an implemented `core` case containing
three axial CT instances at Image Position (Patient) Z coordinates 0, 4, and
10 mm. The adjacent intervals are therefore 4 and 6 mm. Instance Numbers 1,
2, and 3 agree with geometric order, so the case isolates unequal physical
spacing from sorting conflict.

The instances deliberately omit optional Spacing Between Slices `(0018,0088)`.
The manifest instead records every Image Position (Patient) and Image
Orientation (Patient) vector, the complete adjacent-spacing vector, geometric
rank, Instance Number state and rank, and `spacing_uniform: false`. Internal
validation reopens the files and derives these values from the serialized
objects. JSON and Markdown reports expose the same expectations.

### Gantry-tilted CT series

`geometry/ct/gantry_tilt_series` is an implemented `core` case containing
three CT instances with Gantry/Detector Tilt `(0018,1120)` equal to
11.30993247 degrees. Image Orientation (Patient) remains axial, while Image
Position (Patient) changes from `0\\0\\0` through `0\\-1\\5` to
`0\\-2\\10`. This makes the one-millimetre in-plane displacement per
five-millimetre normal interval explicit in patient coordinates.

The manifest records the exact positions, orientation, normal projections,
uniform adjacent spacing, numeric Instance Number ordering, and tilt. Internal
validation compares both the serialized patient-space geometry and the tilt
tag with their expectations. It does not infer positions from the tilt tag or
otherwise treat that metadata as a substitute for Image Position (Patient).
Reports expose the tilt alongside the complete sorting contract.

### Duplicate and empty Instance Number values

`geometry/ct/duplicate_missing_instance_number` is an implemented `core` case
with three geometrically ordered CT instances. The first two serialize the
same numeric Instance Number, `1`; the third contains Instance Number
`(0020,0013)` with IS VR and zero value length. The Type 2 element is present,
not omitted.

The manifest distinguishes `numeric` and `empty` states and records null for
the empty numeric value. Instance Number rank and sorting-conflict expectations
are null for every member because duplicates and an empty value do not define
a total numeric order. Internal validation and reports preserve those nulls
while continuing to enforce the complete geometric order.

### Multiple series with one shared Frame of Reference

`geometry/ct/multiseries_shared_frame_of_reference` is an implemented `core`
case with two two-slice CT series in one study. Both series occupy the same
positions at 0 and 5 mm and share one Frame of Reference UID, but they have
distinct Series Instance UIDs and Series Numbers. Instance Number resets to 1
and 2 within each series, while all four SOP Instance UIDs remain unique.

Every manifest row declares the organization group, study series count,
series ordinal, per-series instance count, and expected shared/distinct UID
relationships. Corpus validation reopens the files, binds dataset Study,
Series, and Frame of Reference UIDs to the manifest, requires serialized Series
Number to equal the declared ordinal, and then checks the cross-series group.
This prevents organization metadata from validating only its own claims.

### Temporal Enhanced MR frames

`enhanced/mr/multiframe_temporal_position_explicit_le` is an implemented
`extended` case with two frames at the same patient-space plane and Temporal
Position Time Offsets 0.0 and 1.5 seconds. Temporal Position Index, Dimension
Index Values, and Frame Acquisition Number are 1 and 2. The dimension pointers
name Temporal Position Time Offset and Temporal Position Sequence explicitly.

The repaired non-legacy Enhanced MR object now records the required image- and
frame-level `MAGNITUDE`/`UNKNOWN` semantics, `RESEARCH` content qualification,
IEC safety agency, current `(69536005, SCT, Head)` anatomy coding, no burned-in
annotation, never-lossy history, identity presentation LUT, head SAR, and three
IEC-normal operating modes. Existing MR timing coverage is retained. Internal
validation binds these top-level and nested values to the manifest, and JSON
and Markdown reports expose every temporal index, pointer, offset, and seconds
unit with strict companion-field checks.

### UTF-8 Person Name

`metadata/sc/utf8_person_name` is an implemented `core` Secondary Capture
instance declaring `SpecificCharacterSet = ISO_IR 192`. Patient Name is exactly
`Wang^XiaoDong=王^小東`, with ordered alphabetic and ideographic groups. The
manifest records all five components in each group, the 24-byte serialized PN
value, and its SHA-256
`64a9d3d6b55142162489a8679e8643caa94efcff26dd30bf24650ac5186c1382`.

Internal validation independently compares the reopened Unicode value and the
raw Explicit VR element against the declaration, VR, group order, component
order, byte length, and hash. JSON grouped coverage and the Markdown Metadata
and VR Expectations table expose the same contract. The synthetic General
Series carries `Laterality = R`, which satisfies the independent Type 2C IOD
check without a warning.

### ISO 2022 Person Name component groups

`metadata/sc/iso2022_person_name_component_groups` is an implemented
`extended` Secondary Capture instance using the exact PS3.5 H.3.1 Japanese
Example 1 contract. Specific Character Set has an empty first value for the
default ISO-IR 6 repertoire followed by `ISO 2022 IR 87`. Patient Name decodes
as `Yamada^Tarou=山田^太郎=やまだ^たろう` in alphabetic, ideographic, and
phonetic group order.

The native writer receives the controlled 60-byte PN value directly. The
manifest records its uppercase hexadecimal representation and SHA-256
`b206df163ce0b4d071469834428bf0b87b241931c81110362ce480d73d7490af`
alongside every group and component. Native validation requires the exact
charset declaration, PN VR, length, bytes, digest, and group structure.
Because dicom-rs does not semantically decode this multi-repertoire value, it
is not used as the Unicode oracle; independent DCMTK and uv-locked pydicom
reads provide that proof.

### DA, TM, DT, and timezone boundaries

`metadata/sc/timezone_boundaries` is an implemented `core` case with separate
`positive_max` and `negative_min` Secondary Capture instances. The first binds
leap-day `20240229`, `235959.999999`, and
`20240229235959.999999+1400` to `+1400`; the second binds the following local
day, `20240301`, `000000.000000`, and
`20240301000000.000000-1200` to `-1200`. They normalize to
`2024-02-29T09:59:59.999999Z` and `2024-03-01T12:00:00.000000Z`.

Each file has distinct deterministic Study, Series, and SOP Instance UIDs.
The manifest records exact DA/TM/DT/SH VRs, decoded values, padded raw bytes,
lengths, hashes, numeric offsets, and normalized UTC. Validation reparses the
Gregorian date and fractional time, enforces the asymmetric legal offset
range, requires the DT suffix to match the instance-wide offset, recomputes
both UTC paths, and requires exactly one instance of each boundary. Reports
retain the boundary ID so the two rows cannot collapse behind one case ID.

## Verification evidence

The following checks passed on 2026-08-26 for a seed-23 `core` corpus:

- focused registry, generation, manifest-schema, geometry-validation,
  coverage-report, gap-report, and recipe-inventory tests;
- two independent generation runs with byte-identical DICOM files and
  manifests;
- internal validation of all 27 generated `core` instances with zero
  failures;
- locked `dciodvfy` SHA-256
  `1aeb75d6ccd3f193e3b322b6da77742cdce2e0604868eaf2a2669c786cbc27e5`;
- `dciodvfy -new` on all three non-uniform CT instances with no findings;
- locked `dcentvfy` SHA-256
  `1b96e598f28f66deee1bfc1cb52ff460c316ab6b0625dae575d701f20c836e2c`;
- `dcentvfy` on the three-instance series with no findings;
- locked DCMTK `dcmdump` SHA-256
  `d2261944ea1ceb6743df9866f2237014b284fa39119c8a5eee226ae922ead45f`;
  and
- independent DCMTK extraction of positions 0, 4, and 10, axial orientation,
  Instance Numbers 1, 2, and 3, and absence of Spacing Between Slices.

The gantry-tilt slice additionally passed two byte-identical seed-29 `core`
runs, internal validation of all 30 files with zero failures, `dciodvfy -new`
on all three tilted instances with no findings, isolated three-file
`dcentvfy` with no findings, and DCMTK extraction of the declared tilt, axial
orientation, positions, Instance Numbers, and five-millimetre spacing.

The duplicate/empty slice additionally passed two byte-identical seed-31
`core` runs, internal validation of all 33 files with zero failures,
`dciodvfy -new` on all three instances without IOD errors, isolated
three-file `dcentvfy` with no findings, and DCMTK extraction of the duplicate
numeric values and exact zero-length IS. The locked `dciodvfy` emits one
DICOMDIR-usability warning for the empty value. Its exact path, validator
fingerprint, message fingerprint, Type 2 standards citation, rationale, and
recheck condition are committed as a narrow `generator_intent_confirmed`
disposition. A real conformance-framework run matched exactly that one new
disposition. Older unrelated `core` findings remain unresolved and were not
accepted or weakened.

The shared-Frame slice additionally passed two byte-identical seed-37 `core`
runs, internal validation of all 37 files with zero failures, `dciodvfy -new`
on all four instances with no findings, isolated four-file `dcentvfy` with no
findings, and DCMTK extraction confirming one Study UID, one Frame of Reference
UID, two Series UIDs and Series Numbers, reset Instance Numbers, and overlapping
positions.

The temporal slice passed two byte-identical seed-43 `extended` runs, each
generating 79 files in approximately 1.5 seconds and occupying 1.5 MiB on this
host. Internal validation reported zero failures. The locked `dciodvfy -new`
validator now reports no findings for the temporal, echo, or phase-velocity
Enhanced MR siblings, and isolated three-file `dcentvfy` is silent. DCMTK
independently extracted the temporal 16-byte native Pixel Data; splitting it
into two 8-byte frames produced SHA-256 values
`451ba3600c2b6ddbcb4fa8164e18ec217b5dc1eb04f48588a99dd21a3cf55bc9`
and `0335fbafa06dc1f6264cb86d8d1d668d2f92f928dee11232f202bdb54bc60338`,
exactly matching the manifest, with decoded unsigned samples
`[0,25,50,75]` and `[150,175,200,225]`.

The UTF-8 slice passed two byte-identical seed-37 `core` runs, producing 38
files and a 696 KiB local corpus with zero internal validation failures. The
locked `dciodvfy -new` and `dcentvfy` tools are silent for the native fixture.
DCMTK 3.7.0 `dcmconv +U8` rewrote the file and `dcmdump +U8` recovered the exact
character-set declaration and PN value; the rewritten file also passes both
dicom3tools checks. The locked optional Python environment is managed by
`uv 0.11.26` and contains CPython 3.12.12 with pydicom 3.0.2. A pydicom
read/write/read cycle preserved `ISO_IR 192`, the full PN, and the alphabetic,
ideographic, and empty phonetic projections exactly, and its output also passes
both dicom3tools checks. The locked DCMTK `dcmconv` SHA-256 is
`beae7cc9a01e780a4137e282436848b1349e209bb40365a76dfc599c51c14964`;
the other validator fingerprints remain those recorded above.

The ISO 2022 slice passed two byte-identical seed-37 `extended` runs, each
producing 80 files and a 1.5 MiB local corpus with zero internal validation
failures. The native ISO 2022 file is 1,040 bytes with SHA-256
`7815cf3bf2124f32c3240149c29e500a18c4894132a001b0016d2f424d8aff45`.
Locked `dciodvfy -new` reports only its normal `SCImage` identification and
no finding; isolated `dcentvfy` is silent. DCMTK 3.7.0 `dcmdump` confirms the
original `\\ISO 2022 IR 87` declaration and 60-byte escape-coded PN, while
`dcmdump +U8` recovers all three Unicode groups. `dcmconv +U8` produces a
conformant UTF-8 rewrite with SHA-256
`2b9dea60d495d59c5b4827f1591609653fe92894071b5f70e7cad27c36e573cb`.
The repository's `uv 0.11.26` lock selects CPython 3.12.12 and pydicom 3.0.2;
its read/write/read proof preserved both charset values and all three named
groups. That rewrite is byte-identical to the native input, and both rewritten
files pass `dciodvfy` and `dcentvfy`.

The timezone slice passed two byte-identical seed-37 `core` runs, each
producing 40 files with zero strict validation failures. Native SHA-256 values
are `6f8e29ac1785c61e0f2b0ac5e713a79cc798ed8419c0cf3334c0340b7d495478`
for `positive_max` and
`055eebc4c818f56fef8e8246ed6d2422aadac6f4f37d5ed36e864c0fafd17d57`
for `negative_min`. Locked `dciodvfy -new` reports only the normal `SCImage`
identification for each file, and isolated `dcentvfy` is silent. DCMTK 3.7.0
extracts the exact DA/TM/DT/SH values and VLs; `dcmconv` rewrites are conformant
with SHA-256 values
`31be92d406cef12c558bf9cd7b30ab516155198111ceb218c2705ef1a64d55a9`
and `f7af26ea1f78ec01ed53ec6aeceedc81d03c752c0dc34a2b2009991b256223e8`.
The uv-locked pydicom 3.0.2 read/write/read retains every original lexical
string and reports offsets of +50,400 and -43,200 seconds. Those rewrites are
byte-identical to the native inputs and also pass both dicom3tools validators.

The empty Type 2 slice passed two byte-identical seed-1 `core` runs, each
producing 41 files with zero strict validation failures. The fixture SHA-256 is
`7f457e4f9593a8d41dff970d32de86c8b5493841546dd6d60b219f311a7abc7c`.
Locked `dciodvfy` reports only the normal `SCImage` identification and no
finding, while isolated `dcentvfy` is silent. DCMTK 3.7.0 independently
reports Patient Name, Patient Birth Date, Patient Sex, Referring Physician's
Name, and Accession Number at PN, DA, CS, PN, and SH respectively, each with
zero Value Length. The uv-locked pydicom 3.0.2 reader reports the same VRs,
empty values, and VM 0 for all five attributes.

The long and multi-valued string slice passed two byte-identical seed-1
`extended` runs, each producing 81 files with zero strict validation failures.
The fixture SHA-256 is
`238f7478de59027060c3807a2075faf9deb9e32d2a4a33bf622170183470c5c2`.
Locked `dciodvfy` reports only the normal `SCImage` identification and no
finding, and isolated `dcentvfy` is silent. DCMTK 3.7.0 independently reports
the locked LT, LO, DS, and IS VRs with VL/VM pairs 10240/1, 130/2, 34/2, and
12/1. The uv-locked pydicom 3.0.2 reader preserves the exact values and numeric
lexemes, and its rewrite is byte-identical and clean under both dicom3tools
validators. Pydicom warns that the padded second LO component has length 65
because it counts the legal trailing pad; this reviewed tool behavior remains
visible and does not replace the clean independent conformance evidence.

The private creator block slice passed two byte-identical seed-1 `core` runs,
each producing 42 files with zero strict validation failures. The fixture
SHA-256 is
`5a0726a68554bb55a6dc5f7a74f639138dc365e8a46f444013303261705141e9`.
Locked `dciodvfy` identifies the SC Image and emits only its expected
informational warnings for the four unrecognized private data tags; isolated
`dcentvfy` is silent. DCMTK independently reports the three LO creator slots,
three private LO values, and private US value `4660` at their exact tags and
VRs. The uv-locked pydicom 3.0.2 read/write/read proof preserves all seven
typed values, produces a byte-identical rewrite, and passes the same
dicom3tools and DCMTK checks.

The defined/undefined sequence-length slice passed two byte-identical seed-1
`extended` runs, each producing 83 files with zero strict validation failures.
The defined-length fixture SHA-256 is
`8bb4d67a7774cb4276b0b632fb22f2123e709e52d2fbee87e6e30d804f25115a`;
the undefined-length fixture SHA-256 is
`0c734d4cd5f6419c916bcc95664654c8c7aaba68eaed84f6aa5ae40d3fcc5642`.
Raw validation and DCMTK confirm SQ Value Length `56` without a sequence
delimiter for the defined form, and `FFFFFFFFH` with a zero-length sequence
delimiter for the undefined form; both retain the same undefined-length item
and SCT code for Head. Locked `dciodvfy` reports only normal `SCImage`
identification, isolated `dcentvfy` is silent, and the offline uv-locked
pydicom 3.0.2 rewrites are byte-identical and retain the same clean independent
validator results.

The two matching `core` corpora each occupy 480 KiB in the local filesystem.
This measurement is implementation-environment evidence rather than a
portable byte-size guarantee; tracked generated artifacts remain forbidden.

## Clinical-family milestone

### Nuclear Medicine STATIC dimensions

`classic/nm/multiframe_explicit_le` is an implemented `core` Nuclear Medicine
Image Storage instance with four native unsigned 16-bit frames. Image Type is
`ORIGINAL\\PRIMARY\\STATIC\\EMISSION`. Frame Increment Pointer names Energy
Window Vector followed by Detector Vector, with ordered values `1,1,2,2` and
`1,2,1,2`; the four frames therefore bind to tuples `(1,1)`, `(1,2)`, `(2,1)`,
and `(2,2)` with the detector index changing fastest.

The two energy-window Items describe Tc99m photopeak 126–154 keV and scatter
100–120 keV ranges. The two PARA detector Items carry start angles 0 and 180
degrees and explicit patient orientations. Typed manifest and coverage-report
contracts expose these sequences and vectors, while internal and
manifest-driven validation enforce every count, one-based index, tuple, and
ordered native frame hash. Tamper tests independently exercise each boundary.

Two seed-1 `core` generations each produced 43 files and were byte-identical;
strict corpus validation reported zero failures. The NM fixture SHA-256 is
`6f0f857b35c1abd133043cb0ae27543b1f56add494891f4b6ea7f8d50c96a7f4`.
Locked `dciodvfy` identifies only `NMImage`, `dcentvfy` is silent, and DCMTK
extracts the exact 32-byte native payload and four declared frame hashes. The
offline pydicom 3.0.2 environment managed by locked `uv` independently decodes
the `(4, 2, 2)` array and all NM dimensions; its Part 10 rewrite is
byte-identical and retains the same clean validator results.

### PET rescaled activity concentration

`classic/pet/rescaled_activity_explicit_le` is an implemented `core`
Positron Emission Tomography Image Storage instance with one 2 by 2 native
unsigned 16-bit frame. Its stored samples `0, 100, 200, 400` are transformed
by Rescale Intercept `0` and Rescale Slope `2.5` into `0, 250, 500, 1000`
Bq/ml. Units is `BQML`; Counts Source is `EMISSION`; Series Type is
`STATIC\\IMAGE`; Corrected Image is `DCAL`; Dose Calibration Factor is `1`;
and Decay Correction is `NONE`.

The slice deliberately makes no SUV, radiopharmaceutical-administration, or
decay-correction claim. Its mandatory Radiopharmaceutical Information,
Patient Orientation Code, and Patient Gantry Relationship Code sequences are
present with zero Items. Typed manifest and coverage-report contracts expose
the stored and derived values, calibration fields, timing, image index, and
empty-sequence counts. Internal and manifest-driven validation independently
rederive the activity values and reject each tampered boundary.

Two seed-1 `core` generations each produced 44 files and were byte-identical;
strict corpus validation reported zero failures. The PET fixture SHA-256 is
`78ced6c57926cafc6538ebf65459bb9efd7ecbb9a3c4ec90b28b4457cc795ce6`.
Locked `dciodvfy` identifies only `PETImage`, `dcentvfy` is silent, and DCMTK
extracts the exact 8-byte native payload with frame SHA-256
`03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784`.
The offline frozen pydicom 3.0.2 environment managed by locked `uv`
independently decodes the `(2, 2)` unsigned array, derives the declared BQML
values, and produces a byte-identical Part 10 rewrite that retains the same
clean validator results.

### Ultrasound timed multi-frame cine

`classic/us/multiframe_explicit_le` is an implemented `core` Ultrasound
Multi-frame Image Storage instance with four 4 by 4 native unsigned 8-bit
MONOCHROME2 frames. Image Type is
`ORIGINAL\\PRIMARY\\ABDOMINAL\\0001`, Body Part Examined is `ABDOMEN`, and
Laterality is absent because the declared anatomy is not paired. Frame
Increment Pointer names Frame Time, which is exactly 100 ms, yielding ordered
relative frame starts at 0, 100, 200, and 300 ms.

The fixed grayscale frames move one 255-valued echo and have distinct ordered
hashes. Lossy Image Compression is `00` and Ultrasound Color Data Present is
zero. The fixture explicitly omits Frame Time Vector, Frame of Reference,
ultrasound region calibration, lossy ratio and method, spatial relationships,
color flow, and enhanced functional groups. Typed manifest and report
contracts expose the timing, frame order, hashes, and each non-claim;
manifest-driven validation rejects tampering at every boundary.

Two seed-1 `core` generations each produced 45 files and were byte-identical;
strict corpus validation reported zero failures. The fixture SHA-256 is
`6f97371d5746d00e10ddadbdf436a29717a7bc241f53993a5fa9bc21ea41206d`.
Locked `dciodvfy` identifies only `USMultiFrameImage`, `dcentvfy` is silent,
and DCMTK extracts the exact 64-byte native payload with SHA-256
`060e2c56c9728f787339515ef16bc8c1adfbfb4fb85b2d2c18f115c17b439bc9`.
The frozen pydicom 3.0.2 environment managed by locked `uv` independently
decodes the `(4, 4, 4)` unsigned array and every ordered frame hash. Its
read/write/read output is byte-identical and retains the same clean validator
results.

The later caller-capability qualification does not replace that historical
oracle. A structurally selected `classic/ultrasound/multiframe@1.0.0` tuple can
instead carry caller-owned identities, metadata, Image Type, timing, path and
ordered frames through the public CLI and SDK. Its unrelated-name proof uses
three one-byte semantic frames stored in a four-byte zero-padded Value Field;
strict validation hashes only semantic bytes and rejects nonzero padding. This
is same-project caller-boundary evidence, not an additional independent
conformance or viewer result.

### XA monoplane cardiac projection

`classic/xa/monoplane_explicit_le` is an implemented `core` X-Ray
Angiographic Image Storage instance with one 4 by 4 native unsigned 8-bit
MONOCHROME2 frame. Image Type is `ORIGINAL\\PRIMARY\\SINGLE PLANE`, Body Part
Examined is `HEART`, Patient Orientation is present with zero value length,
and Laterality is absent. The locked acquisition declares `LIN` pixel
intensity, `GR` radiation setting, 80 kVp, 4 mAs, 0.2 by 0.2 mm imager
spacing, primary and secondary angles of 15 and -10 degrees, 1200 mm
source-to-detector distance, 800 mm source-to-patient distance, and estimated
magnification 1.5.

The fixture explicitly makes no multi-frame cine, biplane, contrast,
subtraction, table-motion, patient-space geometry, calibrated patient Pixel
Spacing, modality LUT, VOI LUT, or lossy ratio or method claim. Lossy Image
Compression is explicitly `00`. Typed manifest and
report contracts expose each acquisition value and non-claim. Manifest-driven
validation binds the duplicate recipe contract, locked payload hash, actual
Pixel Data hash, geometric ratio, and all declared absences; focused tamper
tests exercise every boundary.

Two seed-1 `core` generations each produced 46 files and were byte-identical;
strict corpus validation reported zero failures. The XA fixture SHA-256 is
`7efc114021a4a292e7170055f92948823844192d3f3609509a73b8e2b97dc824`.
Locked `dciodvfy` identifies only `XAImage`, `dcentvfy` is silent, and DCMTK
extracts the exact 16-byte native payload with SHA-256
`0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e`.
The frozen pydicom 3.0.2 environment managed by locked `uv` independently
decodes the `(4, 4)` unsigned array, confirms the acquisition contract and
declared absences, and produces a byte-identical Part 10 rewrite that retains
the same clean validator results.

### XRF monoplane abdominal projection

`classic/xrf/monoplane_explicit_le` is an implemented `core` X-Ray
Radiofluoroscopic Image Storage instance with one 4 by 4 native unsigned
8-bit MONOCHROME2 frame. Image Type is
`ORIGINAL\\PRIMARY\\SINGLE PLANE`, Body Part Examined is `ABDOMEN`, Patient
Orientation is present with zero value length, and Laterality is absent. The
locked acquisition declares `LIN` pixel intensity, `SC` low-dose radiation
setting, 70 kVp, 1 mAs, and 0.2 by 0.2 mm receptor-housing spacing.

The selected XRF Positioner geometry declares a 1200 mm source-to-detector
distance, 800 mm source-to-patient distance, exact magnification 1.5, and
positive 10-degree equipment-coordinate Column Angulation. The fixture does
not reinterpret this as XA patient-relative primary or secondary angles. It
also makes no cine, biplane, contrast, subtraction, table position or motion,
tomography, calibrated Pixel Spacing, patient-space geometry, display,
detector-characteristic, collimation, shutter, overlay, or dose-product claim.
Typed manifest and report contracts expose every acquisition value and
non-claim; both generation-time and manifest-driven validation enforce them.

Two seed-1 `core` generations each produced 47 files and were byte-identical;
strict corpus validation reported zero failures. The XRF fixture SHA-256 is
`da7415ddb66c2cce4a3e8c27eb4f5a04a6f03b3bfb9402346fe13a41fadf30ff`.
Locked `dciodvfy` identifies only `XRFImage`, `dcentvfy` is silent, and DCMTK
extracts the exact 16-byte native payload with SHA-256
`0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e`.
The frozen offline pydicom 3.0.2 environment managed by locked `uv` confirms
the full acquisition and absence contract and produces a byte-identical Part
10 rewrite that retains the same clean validator results.

### Enhanced PET quantitative multi-frame

`enhanced/pet/multiframe_explicit_le` is an implemented `extended` Enhanced
PET Image Storage instance with two 2 by 2 native unsigned 16-bit axial frames.
Image Type and PET Frame Type are
`DERIVED\\PRIMARY\\STATIC\\MULTIPLICATION`; the mandatory View Code is
`(24422004, SCT, "Axial")`. View Modifier Code Sequence and Slice Progression
Direction are absent because their conditions are false. Unknown administered
Total Dose remains present with zero value length as required by its Type 2
contract rather than asserting a numeric clinical dose.

Shared and per-frame functional groups expose pixel measures, anatomy,
orientation, positions, dimension indices, and the PET quantitative mapping.
Both frames contain stored values `0, 100, 200, 400`; intercept `0` and slope
`2.5` map them to `0, 250, 500, 1000` Bq/ml. The fixture makes no SUV,
clinical calibration, decay correction, gating, motion, or reconstruction
claim. Typed manifests, coverage reports, reopen validation, strict
manifest-driven validation, and focused DICOM/manifest tamper tests enforce
the complete contract.

Two seed-1 `extended` generations each produced 84 files and were recursively
byte-identical; strict validation checked all 84 with zero failures. The
Enhanced PET instance SHA-256 is
`f40d03339b2344d0f415c3be9ed5194b3657dcf68a06680f131f1dfe0607125f`.
Locked `dciodvfy` identifies only `EnhancedPETImage`, `dcentvfy` is silent,
and DCMTK extracts the exact view, zero-length dose, and 16-byte Pixel Data.
The frozen offline pydicom 3.0.2 environment managed by locked `uv`
independently decodes both frames, reproduces their hashes, and recomputes the
declared Bq/ml values.

## Unsigned 32-bit native Secondary Capture

`classic/sc/mono2_u32_explicit_le` is the first completed Phase 2 pixel
milestone slice. It emits a 2 by 2 MONOCHROME2 Secondary Capture instance with
Bits Allocated/Stored/High Bit `32/32/31`, unsigned Pixel Representation, and
native OW Pixel Data. The exact little-endian stored values are `0`, `65535`,
`2147483648`, and `4294967295`, covering both sides of the signed boundary and
the full unsigned range.

Two seed-1 `extended` generations each produced 85 files and were recursively
byte-identical. Strict manifest-driven validation checked all 85 with zero
failures. The U32 instance SHA-256 is
`bec7dfedcb7cec08426f38f46f6d5deead6294c2a4a6e4464ba972bb97592630`;
its 16-byte Pixel Data and frame SHA-256 is
`56bca1a85c2838126b1d1a5fbedfe731839496d972df2c6ab33e1a1183392b41`.
JSON and Markdown reports expose the exact values, hash, little-endian word
order, and full-range state.

The locked dicom3tools IOD validator aborts on this standards-permitted format,
so the explicitly authorized alternative is pydicom `dicom-validator` 0.8.2 in
adapter 0.2.0. Its CPython 3.12.12 environment, exact dependencies, adapter,
official DICOM 2026b DocBook sources, and derived definitions are locked as a
single fingerprint. The clean candidate passes with zero IOD errors; missing
Conversion Type and invalid Pixel Representation controls are detected. The
same adapter independently extracts all unsigned values and hashes without
NumPy or a project decoder.

For entity consistency only, `dcentvfy` receives a hash-linked projection that
preserves bytes zero through 929 and omits the terminal Pixel Data element. The
untouched original remains the only IOD and payload input. An isolated real
conformance run has matched tools, silent entity validation, and zero strict
verification failures. Python remains optional for ordinary profile generation
and becomes required only when collecting conformance evidence for this one
declared case.

## One-bit native Multi-frame Secondary Capture

`classic/sc/mono2_u1_native` completes the second Phase 2 pixel slice. It is a
two-frame, 3 by 3 Multi-frame Single Bit Secondary Capture instance with
Samples per Pixel/Bits Allocated/Bits Stored `1/1/1`, High Bit and unsigned
Pixel Representation zero, MONOCHROME2 photometric interpretation, OB Pixel
Data, and no Planar Configuration. Frame Increment Pointer names Page Number
Vector with values `1\\2`.

The alternating checkerboards deliberately place the second frame at bit
offset nine. PS3.5 continuous packing therefore produces three significant
bytes `55 55 01`, with six unused high bits zero and one final zero byte for
even Value Field length. Per-frame byte padding is forbidden. The complete
Pixel Data SHA-256 is
`9d6baf87a79d40ef2b145f92945a05cf156a2741e2c2834a3a7721d52757594b`;
decoded-frame hashes are
`a6188710c09cfbc77383ee0588dec2f7affa6e03e78aa900e9ae597a8d8faba3`
and
`c520efb8f894a1125bb1a513a9b64ef957f7c2cd63835fd7e130357c47f989ae`.
The manifest, validator, JSON/Markdown reports, and schema all bind these
values and the least-significant-bit-first, cross-frame packing policy.

Two seed-1 `extended` generations each produced 86 files and were recursively
byte-identical; strict internal validation passed all 86. Locked
`dciodvfy -new` identifies `MultiframeSingleBitSCImage` without findings and
isolated `dcentvfy` is silent. Pydicom `dicom-validator` was not selected for
this case because an empirical invalid `8/8/7` control showed that its current
definitions do not enforce PS3.3 A.8.2.4.

Independent pixel evidence instead uses locked DCMTK 3.7.0 `dcm2img`,
executable SHA-256
`6a6103a7c516814b5eb44f53d198b111cbaf1678de5952ab7d31961732f112d5`,
to produce exact PGM frames, plus locked `dcmdump` raw extraction. Strict
verification binds both decoded hashes and the raw payload to the generated
file and manifest. Negative controls confirm that invalid `8/8/7` bit fields
produce enumerated-value and length findings, while forbidden Planar
Configuration produces normalized errors despite a zero tool exit code.

## ICC input profile handling

`vl/photo/rgb_icc_profile_explicit_le` completes the third Phase 2 pixel
slice. It clones the 2 by 2 RGB planar-0 VL Photographic image and adds the
optional ICC Profile Module: `(0028,2000)` is a 736-byte OB input-device
profile and `(0028,2002)` declares `SRGB`. The profile is DCMTK 3.7.0's
CC0 `DCMTK_SRGB_ICC_SAMPLE`, stored as reviewed source hex rather than a
platform-discovered profile. Its exact SHA-256 is
`8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef`.

The manifest, strict validator, JSON and Markdown reports, and schemas bind
the declared and actual size, ICC 2.1.0 version, `scnr` device class, `RGB `
input space, `XYZ ` connection space, `acsp` signature, perceptual intent,
nine bounded tag records, `sRGB` description, `CC0` copyright, and DICOM
Color Space. The generated Part 10 instance SHA-256 is
`23680ffd511565f585430e9cd3e6ac397b7c36c60027f190bee86a03afdd7ef0`.
Two seed-7 `extended` generations each produced 87 files and were recursively
byte-identical; strict internal validation passed every file.

The locked `dciodvfy -new` primary and isolated `dcentvfy` entity check are
silent. Complete OB extraction uses locked DCMTK 3.7.0. The authorized
case-scoped LittleCMS 2.19 composite lock covers `transicc` and
`liblcms2.2`; it processes fixed red, green, blue, and white inputs through
the extracted profile and reproduces four exact XYZ vectors. A fresh isolated
run reported matched locks and zero strict conformance failures. Negative
controls prove that dicom3tools alone does not reject corrupt profile headers
or label mismatch, while the composite path rejects unavailable tools,
profile/hash/header drift, and semantically relinked evidence.

## Non-square spacing and aspect ratio

`classic/sc/nonsquare_pixel_spacing` closes the Phase 2 pixel milestone with
two files over the same 4 by 6 MONOCHROME2 checkerboard. The physical-spacing
variant records Pixel Spacing and Nominal Scanned Pixel Spacing as exact DS VM
2 values `0.6\\0.3`; the independent aspect-ratio variant records only Pixel
Aspect Ratio as IS VM 2 `2\\1`. Both express a 2:1 row-to-column ratio without
patient-space geometry or a calibration correction. Their common 24-byte
Pixel Data SHA-256 is
`e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6`.

The manifest schema, strict validator, JSON and Markdown reports, and CLI tests
enforce exact values, mutual exclusivity, required absences, payload shape,
and both file paths. Two seed-7 `core` generations each produced 49 files and
were recursively byte-identical; strict corpus validation passed all 49. The
two instance SHA-256 values are
`50f897625dcc489d212a81674086d1183569d6e0ac7a847d55afc8dd599276d4`
and
`dc330a2b51d1381d943e5ba0f50086114eb95102852228e7ffcb62e0bdec93b9`.

The authorized `uv`-locked adapter 0.3.0 binds CPython 3.12.12,
dicom-validator 0.8.2, pydicom 3.0.2, the 2026b definition cache, and every
adapter input under composite fingerprint
`3f20de6ca7d310e2e9f2920f368912f97b6cf62bdfdf750e1417eb5dc4b335b6`.
Both files pass independent IOD validation with zero errors and case-scoped
semantic extraction of the exact VR, VM, values, absences, dimensions, and
pixel hash. Negative tests reject crossed axes and relinked evidence.

## Milestone gate

All planned Phase 2 geometry and series cases are implemented, and the UTF-8,
ISO 2022, timezone, empty Type 2, string boundary, private creator, and sequence
length metadata slices have passed their vertical gates. The latest seed-1
`core` corpus contains 49 files; the seed-1 `extended` corpus contains 87
files. The complete
locked no-default-feature, all-target test suite passes, including byte-stable
smoke, core, and extended regeneration. Each new CT slice and the temporal MR
slice has clean isolated IOD, entity, parser, and applicable independent pixel
evidence, except for the one exact reviewed DICOMDIR-usability warning described
above. Older corpus findings remain visible and unresolved. The dependency-
ordered Phase 2 metadata and VR milestone is complete. The Nuclear Medicine
STATIC multi-frame, PET rescaled-activity, timed Ultrasound multi-frame,
XA monoplane, XRF monoplane, and Enhanced PET clinical-family representatives
are complete. The dependency-ordered Phase 2 clinical-family milestone is
closed. The unsigned 32-bit, one-bit, ICC, and non-square spatial pixel slices
are complete. All dependency-ordered Phase 2 milestones are closed.
