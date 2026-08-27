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

The two matching `core` corpora each occupy 480 KiB in the local filesystem.
This measurement is implementation-environment evidence rather than a
portable byte-size guarantee; tracked generated artifacts remain forbidden.

## Remaining work

All newly planned CT cases in the geometry and series milestone are now
implemented. The existing enhanced MR temporal case supplies the planned
temporal/dynamic coverage. Milestone completion still requires the combined
regression, runtime/size, and conformance audit described below.
