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

## Comprehensive 3D SCOORD3D

`derived/sr/comprehensive3d_scoord3d` is an implemented `extended`
Comprehensive 3D SR case derived from frames 1 and 2 of
`enhanced/ct/multiframe_shared_perframe_explicit_le`. A DCMR TID 1500 root
contains one TID 1501 Measurement Group with a `2.5 mm` Distance NUM. Its
single `INFERRED FROM` SCOORD3D POLYLINE has patient-space endpoints
`[0,0,0]` and `[0,0,2.5]`, the source Frame of Reference UID, and a
deterministic Fiducial UID. A direct Source of Measurement IMAGE selects CT
frames 1 and 2, and the evidence hierarchy contains exactly that CT instance.

Generation uses highdicom/pydicom backend 0.4.0 through protocol `0.1.0` and
the uv dependency lock SHA-256
`8623ce132cf886ce43bc7f9022df126ad754a02fd8b9b91c1e0d5355308e7e35`.
Rust independently reopens the Enhanced CT and checks its SOP, study, series,
Frame of Reference, two-frame count, `0.75 x 0.75` mm Pixel Spacing, `2.5` mm
slice thickness and spacing, axial orientation, and the two patient-space
positions before invoking Python.

Two seed-7 `extended` generations each produced 90 files and passed strict
validation with zero failures. Their 4,696-byte SCOORD3D reports were
byte-identical with SHA-256
`b13ec046baf600f1b47a918b80dc450b86e1f6eb7d79a7cbe274b48935c86379`.
The strict recursive validator checks the ordered TID 1500/TID 1501 tree,
tracking and observer context, NUM DS and FD values, SCOORD3D relationship,
code, FL coordinates, Frame of Reference and Fiducial UIDs, source frames,
evidence closure, and absence of all pixel payload elements. A hash-repaired
Graphic Data mutation is rejected.

Integrated conformance run
`2601144c7df81cc9b5999b67c707ed747b66e2b76e35c2e55e76216ed70f95d1`
recorded stable instance key
`68bc95709add383d0f6cb06c2607e29046c22b83c56354bf6a6897abc2d87f32`.
Locked `dciodvfy -new`, DCMTK `dcmdump`, and PixelMed 20260608
`DicomSRValidator -checktemplateid` all completed with exit code 0 and no
findings; PixelMed identified Comprehensive3DSR, TID 1500, and completed root
template and IOD validation.

An isolated CT/SR `dcentvfy` run resolved the reference closure but emitted its
existing empty-AccessionNumber information-entity classification warning. The
warning is not allowlisted or hidden. Full-corpus conformance verification
reported 195 unresolved older failures, including unavailable independent
payload tools and existing IOD/entity findings; none was reclassified for this
slice.

## Spatial Registration

`derived/registration/spatial_ct_pair` is an implemented, byte-stable
`extended` Spatial Registration Storage case. Rust constructs an identity
registration for the Enhanced CT target and a geometry-derived rigid transform
from the classic CT source Frame of Reference into the target Frame of
Reference. The moving first-pixel center `[-0.625,-0.625,0]` maps exactly to
Enhanced CT frame 2 at `[0,0,2.5]`. Same-Study and other-Study Common Instance
References close both whole-instance references.

The manifest binds both source hashes and identities, ordered registration
items, exact row-major matrices, rigid tolerances, landmark mapping, reference
topology, and absence of pixel payloads. Rust reopens both CT sources before
construction and the corpus validator reconstructs the strict REG contract
from the manifest. Focused tests reject a hash-repaired non-rigid matrix and a
source-hash closure mutation. JSON and Markdown reports expose matrix
direction/type, item count, reference topology and relationships, landmark
mapping, and pixel absence.

Two seed-7 extended generations each wrote 92 files. Their full manifests were
identical with SHA-256
`f45a347517d43c1e810a0f6866aa4468478ff1f403edc0cbe39323045a82079e`;
the 2,328-byte REG instances were identical with SHA-256
`8b3b8498c3e90dc13e52cceb9c584fbb41d5898e28c2f3d3f86baf4a1654ac8`.
Both roots passed strict validation with zero failures.

Integrated conformance run
`522f2627658dd11ae6e5b88ad5e673659cacfdb2abf45fe4cb43adfb90feb7ea`
recorded stable instance key
`4c484723f1de25edb5830e18eb8447bbdc7ee53785dd35109f711b9bc0f6e06b`.
Locked dicom3tools `dciodvfy -new`, DCMTK 3.7.0 `dcmdump`, and the independently
implemented `uv`-locked `dicom-validator` 0.8.2 adapter completed cleanly.
Isolated `dcentvfy` reference closure was silent and successful. Full-corpus
verification still exposes 207 older or unrelated findings; none was
allowlisted for this slice.

## Deformable Spatial Registration

`derived/registration/deformable_ct_pair` is an implemented, byte-stable
Deformable Spatial Registration Storage case. Its single 2 by 2 by 1 grid maps
Enhanced CT frame 2 centers from the Registered RCS into classic CT source
pixel centers. Identity pre/post `RIGID` matrices keep the displacement field
observable, and the 48-byte little-endian OF payload is locked by exact hash,
decoded vector order, and four registered-to-source point mappings.

The manifest binds source hashes and Study/Series/SOP/Frame-of-Reference
identities, complete-instance selection, grid geometry and cardinalities,
matrix types and values, Common Instance Reference topology, and pixel
absence. Strict Rust validation covers the byte-count equation, finite-vector
rules, i-fastest ordering, sampling mathematics, point mappings, and reference
closure that neither independent IOD validator fully owns. JSON and Markdown
reports expose the sampling direction, grid dimensions and resolution, vector
count and payload hash, matrix types, reference topology, and mapping count.

Two seed-7 extended generations each wrote 93 files. Their byte-identical
manifests have SHA-256
`9a449b434db4863b3f6f848edf761b920ce5cc713e3d5142fd1801106ed912fe`;
the byte-identical 2,128-byte REG instances have SHA-256
`d8c539ad4ac9e72a8a597f9bf8a6588feac4d110d97464a70f6d543a033e5114`.
Both roots passed strict validation with zero failures.

Integrated conformance run
`225bef48a5503e4ed2adc88490d9f28d9f8c314e0bc34d3fa8bff0d144b4127e`
recorded stable instance key
`e6a78f3868532d08691c6570ad52e137ffce85661b0cc4ebb810cdff234e63ca`.
Locked `dciodvfy -new` and DCMTK `dcmdump` were clean; the independently
implemented, `uv`-locked `dicom-validator` 0.8.2 adapter passed with zero
errors; and isolated `dcentvfy` was silent. Full-corpus verification continues
to expose 208 older or unrelated findings, with no new allowlisting.

## Color Softcopy Presentation State

`derived/presentation-state/color_softcopy` is an implemented, byte-stable
Color Softcopy Presentation State Storage case. An `extended` run now
materializes its smoke-profile 2 by 2 interleaved RGB Secondary Capture source
as an explicit cross-profile dependency, reopens and hashes it, and binds the
same Study with a distinct Presentation Series. The Presentation State selects
the complete source Instance, applies one global `[1,1]` through `[2,2]`
`SCALE TO FIT` displayed area with aspect ratio `1\\1`, and carries the exact
736-byte locked sRGB ICC profile without Pixel Data or optional rendering
modules.

The manifest closes the source path, hash, Study/Series/SOP identities, image
shape, relationship cardinality, displayed-area semantics, ICC header and
hash, and absence invariants. Strict Rust validation rejects redirected or
dangling references, partial-frame selection, geometry drift, ICC corruption,
unexpected graphics, transforms, overlays, shutters, and Pixel Data. JSON and
Markdown reports expose the source topology, displayed area, ICC identity,
optional-module absence, and pixel absence.

Two seed-7 extended generations each wrote 95 files. Their byte-identical
manifests have SHA-256
`99832aaabe9ca4e36e4c108db44974de352b113ca1ccf0e4a41df74e88ced62a`;
the byte-identical 2,036-byte Presentation State instances have SHA-256
`4e737e1429b7b2463bc412e4c6ff330411259f321070b32d9ce68cdef0bc0543`;
and the byte-identical materialized RGB source has SHA-256
`53208a21ccd2153118b20a5c6da2cbf9ba0d92c70475fbf4ae74add140b0de55`.
Both roots passed strict validation with zero failures.

Integrated conformance run
`b1e494962d40634300fb488fdf95c92ad80bad9b2d1e0f0be6bff9b4e8503b0a`
recorded stable instance key
`3dad35670aba58140d84cd326fd2624348b8f6215cd72e30d3ca76d35eae1801`.
Locked `dciodvfy -new` and DCMTK `dcmdump` were clean; the independently
implemented, `uv`-locked `dicom-validator` 0.8.2 adapter passed with zero
errors; and isolated source-plus-Presentation-State `dcentvfy` was silent. An
initial external run rejected non-IOD Content Date and Content Time, which
were removed and are now strictly absent rather than allowlisted. Full-corpus
verification retains 213 older or unrelated findings with no new acceptance
dispositions.

## Next dependency

The next Phase 3 dependency is Advanced Blending Presentation State, followed
by Blending Presentation State to complete milestone 4 breadth.
