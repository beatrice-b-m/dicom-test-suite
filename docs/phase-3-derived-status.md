# Phase 3 Derived Object Status

This status note records dependency-ordered Phase 3 vertical slices. Generated
corpora and conformance evidence remain ignored and uncommitted. Registry
promotion requires a locked recipe, typed manifest contract, strict internal
validation, two-run reproducibility, and clean case-scoped independent
validation.

## Parametric Map milestone

The float32 and float64 Parametric Map cases are implemented through the
optional highdicom/pydicom backend. Integer Parametric Map remains planned with
its explicit provider-capability blocker; unavailable coverage was not reduced
or silently reclassified. The float variants use the repository's `uv`-locked
CPython 3.12.12, highdicom 0.28.1, and pydicom 3.0.2 environment.

## TID 1500 Measurement Report

`derived/sr/tid1500_ct_measurement_report` is an implemented `extended`
Comprehensive 3D SR case. It references frames 1 and 2 of
`enhanced/ct/multiframe_shared_perframe_explicit_le` through segment 1 of
`derived/seg/binary_multiframe_explicit_le`. The DCMR TID 1500 root contains a
TID 1411 Measurement Group with deterministic device-observer and tracking
UIDs, finding `(123037004,SCT,"Body structure")`, and a
`5.625 mm3` volume measurement. The evidence sequence is ordered Enhanced CT
then SEG.

Generation uses backend protocol `0.1.0` and the `uv` lock SHA-256
`d36e8258e63eb0efdd9ef1b401ee36fca795cf2adb360e735b95a90a663073a0`.
All backend-introduced dates, times, and Contribution Date Times are normalized
to the controlled recipe timestamp. Two seed-7 `extended` generations each
produced 89 files and passed strict validation with zero failures. Their TID
1500 files were byte-identical despite the conservative `semantic_stable`
declaration: 4,846 bytes with SHA-256
`defa75675e4c28e369323d22b1ed3e0dc427caa8034ff549c76c539a74f4e0e0`.

The reusable Rust validator reopens the promoted file before manifest emission
and again during corpus validation. It recursively binds the Part 10 identity,
document flags, DCMR/1500 root, exact eight-item observation context, DCMR/1411
group, tracking values, finding, DS and FD numeric values, UCUM units, SEG
reference with no frame selector, canonical source-image concept and CT frames,
ordered evidence closure, and absence of integer, float, and double-float Pixel
Data. A hash-adjusted content-tree mutation is rejected independently of the
manifest checksum.

## Independent conformance evidence

The integrated conformance run ID
`aaf98f3ed78755cdf0178fdb6ac32455600145799b6aa9a07f8769e8775fe995`
recorded stable instance key
`7d17414837ecffa8db55cacd5f5497b9a272160ecf16d252b465a9294a8ed660`.
Locked dicom3tools `dciodvfy` completed with exit code 0 and no findings.
Locked PixelMed 20260608, invoked with `-checktemplateid`, found the
Comprehensive 3D SR IOD and TID 1500 root and completed both template and IOD
validation with exit code 0 and no findings. The PixelMed composite binds Java
25.0.3 and `pixelmed.jar` SHA-256
`2c779091582f7ce81c0a8d4ae0cab0b937cd570fe827712d7e00d8a10c96b344`.

Full-corpus conformance verification still reports older unresolved findings
and unavailable case-specific validators. Those unrelated failures remain
visible and were not allowlisted or weakened for this slice. The TID 1500 case
itself has completed clean primary IOD, independent parser, and mandatory
PixelMed template-validation results.

## Next dependency

The next Phase 3 slice is `derived/sr/comprehensive3d_scoord3d`. It remains
planned until its SCOORD3D geometry contract, generator path, strict validator,
independent PixelMed evidence, reports, tests, and documentation are complete.
