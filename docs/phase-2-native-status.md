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

The two matching `core` corpora each occupy 480 KiB in the local filesystem.
This measurement is implementation-environment evidence rather than a
portable byte-size guarantee; tracked generated artifacts remain forbidden.

## Milestone gate

All planned Phase 2 geometry and series cases are implemented. The final
seed-37 `core` corpus contains 37 files and occupies 680 KiB on this host; the
seed-43 `extended` corpus contains 79 files and occupies 1.5 MiB. The complete
locked no-default-feature, all-target test suite passes, including byte-stable
smoke, core, and extended regeneration. Each new CT slice and the temporal MR
slice has clean isolated IOD, entity, parser, and applicable independent pixel
evidence, except for the one exact reviewed DICOMDIR-usability warning described
above. Older corpus findings remain visible and unresolved. The dependency-
ordered Phase 2 metadata and VR milestone may proceed.
