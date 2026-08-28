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

## Advanced Blending Presentation State

`derived/presentation-state/advanced_blending` is an implemented, byte-stable
Advanced Blending Presentation State Storage case. An `extended` run
materializes four single-frame CT Images as two ordered Series in one Study
and shared Frame of Reference, reopens and hashes every source, and writes a
distinct Presentation Series. The exact blending graph has two inputs in
order `[1,2]`, complete two-Image references for each source Series,
`FALSE/FALSE` time-series flags, `TRUE/FALSE` geometry-for-display flags, and
one final `EQUAL` display operation consuming inputs `[1,2]`. It carries
`TRUE_COLOR`, the exact 736-byte locked sRGB ICC profile, mirrored Common
Instance Reference closure over all four CT Instances, no optional input
transforms, and no Pixel Data.

The manifest closes all source paths, hashes, Study/Series/Frame-of-Reference
and SOP identities, source geometry and ordering, the two-input graph, final
display operation, ICC identity, common-reference topology, and absence
invariants. Strict Rust validation owns cardinality, ordering, uniqueness,
graph integrity, complete source closure, geometry, ICC bytes, and absences.
JSON and Markdown reports expose those source, graph, display, ICC,
common-reference, optional-transform, pixel-absence, and unresolved external
validator dimensions.

Two seed-7 extended generations each wrote 100 files. Their byte-identical
manifests have SHA-256
`52ae3faf72563b66069cb9546396e9d291ae324ec7302012f2eaadf3c491786a`;
the byte-identical Advanced Blending Presentation State instances have
SHA-256
`4bf58b3a29f168c6d24398603f98ebaa5b40ee62353eb30449e3c193b84ad75d`.
Both roots passed strict validation for all 100 files with zero failures.

Integrated conformance run
`c6a017c46b7e489059dd3bc71b1be66e1ff70008af853aaf393880a4e4f69c73`
recorded stable instance key
`88b7a58c777e556f56aaa2c8fdaed070a094e35646079acd8a17ca9f559e1663`.
DCMTK `dcmdump` was clean; the independently implemented, `uv`-locked
`dicom-validator` 0.8.2 adapter passed with zero errors; and isolated
four-CT-plus-Presentation-State `dcentvfy` was silent. `dciodvfy -new` emitted
exactly two unresolved, contradictory warnings claiming that Frame of
Reference UID and Position Reference Indicator are outside the standard IOD,
despite the locked standard making the Frame of Reference Module mandatory.
The attributes and warnings are retained and not allowlisted. Full-corpus
verification reports 211 older or unrelated failures plus those two
documented warnings, with `accepted_findings` remaining zero.

## Blending Softcopy Presentation State

`derived/presentation-state/blending` is an implemented, byte-stable
Blending Softcopy Presentation State Storage case. An `extended` run
materializes four 2 by 2 single-frame CT Images as two ordered Series in one
Study and shared Frame of Reference, with matching Image Position (Patient)
values `[0,0,0]` and `[0,0,5]` in each Series. Generation reopens and hashes
every source and writes a distinct Presentation Series without copying Frame
of Reference attributes into the Presentation State. Its exact two-item
blending topology binds Series 1 as `UNDERLYING` and Series 2 as
`SUPERIMPOSED`, references both complete Images in slice order, applies
per-item rescale intercept `-1024`, slope `1`, and type `HU`, and locks
Relative Opacity to `0.5`.

One global displayed area selects `[1,1]` through `[2,2]` with `SCALE TO FIT`
and aspect ratio `[1,1]`. The mandatory palette has three `[256,0,16]`
descriptors and exact 512-byte 16-bit identity-ramp data per channel, while
the ICC module carries the exact 736-byte locked sRGB profile. Pixel Data,
standalone VOI transformations, registration references, spatial transforms,
graphics, overlays, shutters, segmented palette data, and Palette Color LUT
UID are absent.

The manifest closes all four source paths, hashes, Study/Series/Frame-of-
Reference and SOP identities, geometry and ordering, blending positions,
complete-instance references, rescale, opacity, displayed-area semantics,
palette and ICC byte identities, and absence invariants. Strict Rust
validation owns their cardinality, order, uniqueness, exact values, reference
closure, byte payloads, and absences, including gaps observed in the
independent validators. JSON and Markdown reports expose the source topology,
positions, rescale and opacity, displayed area, palette descriptors and
hashes, ICC identity, forbidden-module absence, Pixel Data absence, and
external-validator disposition.

Two seed-7 extended generations each wrote 101 files. Their byte-identical
manifests have SHA-256
`0e5a934186cdba5667b4cef14ad7475d0d222f8e0286b8a49a29bb3106b5a200`;
the byte-identical Blending Softcopy Presentation State instances have
SHA-256
`d6fd50ea537157dea62e878e6c455d69f8bb239ce7456c3d7bb5a2893f159918`.
Both roots passed strict validation for all 101 files with zero failures.

Integrated conformance run
`5df5c921ae704341109f1c095258b0f99ebf856e0b91a2eb60deab6531a4a1e3`
recorded stable instance key
`a35121ba42d4f1ad15a46aeefa6d95d2b8c0603ccc1ebc0f2a48f9284756ae8a`.
Locked `dciodvfy -new` and DCMTK `dcmdump` were clean; the independently
implemented, `uv`-locked `dicom-validator` 0.8.2 adapter reported `Passed`
with zero errors; and isolated four-CT-plus-Presentation-State `dcentvfy` was
silent. Full-corpus verification keeps `accepted_findings` at zero and
reports 211 older or unrelated failures, including the two already documented
Advanced Blending warnings; Blending adds no external finding.

## Twelve-lead ECG Waveform

`non-image/waveform/twelve_lead_ecg` is complete as a byte-stable native
extended-profile slice using Twelve-lead ECG Waveform Storage. Its single
waveform group contains the ordered I, II, III, aVR, aVL, aVF, and V1 through
V6 channels, with 500 samples per channel at 500 Hz. Signed 16-bit `SS`
samples are packed channel-then-sample into an `OW` payload of exactly 12,000
bytes from the locked deterministic formula. The manifest carries the typed
waveform, channel, storage, payload, and absence contract; strict validation
owns the IOD modules, channel definitions, arithmetic, formula, interleave,
hashes, extrema, and forbidden attributes; and JSON and Markdown reports
expose the same contract and external-validator disposition.

Two seed-7 extended generations each wrote 102 files. Their byte-identical
manifests have SHA-256
`898ccec3c6c8e09f91ddcc255a45e397ca19ae69c32b41c1aec4aa5240a9ba3d`;
the byte-identical ECG instances have SHA-256
`1a14c3f7097e8c7482deb6c5c228b9dd33dbbc97206a3c3f865d3118d713e4c6`.
Both roots passed strict validation for all 102 files with zero failures.
Locked `dciodvfy -new`, `dcmdump`, and isolated `dcentvfy` were clean. The
independently implemented, `uv`-locked `dicom-validator` 0.8.2 IOD route
reported `Passed` with zero errors, and its separate waveform payload route
reproduced every locked length, value, and hash.

Integrated conformance run
`09391f4644f6ad827a2a635ccc0df6d74201e5d6cc45ee8b2d2144d9c0d8e232`
produced run JSON SHA-256
`aa9d4311ad176ab7c83b6abc2f98c1d8f97db347f07207884cf4b3c8f6396838`,
recorded stable instance key
`b28021744fc73da06f3b1c4af979eb2c61084102558ba0e6c3831bc77f705ce6`,
and bound waveform sidecar SHA-256
`6e0c8f5880ccf65ba78f031b4687c6ea33ca62560e883e78e487935b6c795faf`
to the exact manifest contract and locked tool. Full-corpus verification keeps
`accepted_findings` at zero and reports 211 older or unrelated failures; the
Twelve-lead ECG adds no finding.

## General ECG Waveform

`non-image/waveform/general_ecg` completes Phase 3 milestone 5 as a
byte-stable native extended-profile General ECG Waveform Storage slice. Its two
ordered heterogeneous multiplex groups are `12x1000@250Hz; 4x4000@1000Hz`:
sixteen channels share a four-second duration while retaining
separate sampling models. The signed `SS` samples occupy two `OW` payloads of
24,000 and 32,000 bytes, for an ordered aggregate of exactly 56,000 bytes. The
group payload SHA-256 values are
`e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb` and
`5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe`;
their concatenated aggregate SHA-256 is
`c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea`.

Two seed-7 extended generations each wrote 103 files. Their byte-identical
manifests have SHA-256
`cb2e19a667a302f781e4ce8c1f44041fbb96273acff2debbecbad8160929d301`;
the byte-identical General ECG instances have SHA-256
`a656720538672c95aacdf068ba89b0c6d6f78042610f3a665d55065d0a4ab40c`.
Both roots passed strict validation for all 103 files with zero failures.
Locked `dciodvfy -new` identified the object as GeneralECG and completed
cleanly; `dcmdump` parsed both groups, and isolated `dcentvfy` was silent. The
independently implemented, `uv`-locked `dicom-validator` 0.8.2 IOD route and
its independent raw waveform route both passed, with the raw route reproducing
every group shape, sample, payload length, group/channel hash, and aggregate
hash.

Integrated conformance run
`16175e687c81729fd428510c26a60c518a7271553afc4a22a5a127f32a47168a`
produced run JSON SHA-256
`8b262be912c625cc16df43e3935fef2fa1dfbd0d5fea4ba3cb6dba535b6048df`,
recorded stable instance key
`e2613c273b6fe464a6b3308c4ec4a768103af61d0702033d8999e509dc69d23d`,
and bound waveform sidecar SHA-256
`565f7db1d5f26cb74256bc9a6d84b6319667d90c7b6a07ef7ddc5be03f929d2c`
to the exact manifest contract and locked tools. Full-corpus verification keeps
`accepted_findings` at zero and reports 211 older or unrelated failures; the
General ECG adds no finding.

## Linked RT Plan and RT Image

`non-image/rt/plan_linked` and `non-image/rt/image_linked` complete the legacy
linked-object portion of Phase 3 milestone 6. Both are byte-stable native
extended-profile slices. The Plan closes over the existing RT Structure Set
and RT Dose, with one fraction group, one static photon beam, and two ordered
control points. The Image shares the Plan Study and Frame of Reference,
selects beam and fraction group 1, and carries an exact native 4 by 4
monochrome gradient.

Two promoted seed-7 extended generations each wrote 105 files and passed
strict validation with zero failures. Their manifests are byte-identical with
SHA-256
`b061e5f654eb426bbab0da9cce0ac945aadcf3cf506182eb6bf33acd3d7a3659`.
The byte-identical Plan and Image instance SHA-256 values are respectively
`e9337a6c46fe85b56f1f563120dd3caf56ea1335355792db42386db959be6db2`
and
`460d525ab06aaf74df963029f3ab39c2536e4e1c5bf4b75fcf16b500382db20c`.
At this intermediate checkpoint the registry contained 141 implemented and 41
planned logical cases. The milestone-6 decision checkpoint then authorized a
registered native C-Arm Photon-Electron Radiation companion followed by the
minimal RT Radiation Set; their completed qualification is recorded below.

Locked `dciodvfy -new` and the separately implemented, `uv`-locked
`dicom-validator` 0.8.2 route accept both IODs. The Image's DCMTK route proves
the exact decoded and raw native OB SHA-256
`a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811`.
All 20 Plan and all 20 Image qualification mutations remained parseable;
their independent detection boundaries and strict-Rust owners are recorded in
the linked source note. Strict validation additionally binds the Image's
locked Plan digest to the generated Plan entry, so a syntactically valid stale
digest fails graph closure.

Integrated conformance run
`d0d78ffccf44218a27944cf1b80dec63c8afa7162b0e085532feb51706a04714`
has run JSON SHA-256
`87846c587a4f721b90624008a3f7abfc9ae70a31d83e28449e82528b408b3ce7`
and pixel sidecar SHA-256
`071b32384d1648222424f77a0392e90ca11d6e51df0d5bd1fc0a241754bec1fc`.
The linked RT routes add no finding and accept none. The two immutable upstream
Study ID diagnostics remain visible and unallowlisted, while valid entity
closure adds no missing or dangling reference finding.

## C-Arm Photon-Electron Radiation and RT Radiation Set

The dependency-ordered Plan and Image work is complete. On 2026-08-27 the
milestone-6 decision checkpoint authorized selecting and locking the minimal
current RT Radiation Set slice and another independent IOD validator.
Standards review proved that the Set requires a registered second-generation
C-Arm Photon-Electron Radiation companion, so the authorized work became a
paired two-IOD graph. Both cases are now implemented as byte-stable native
extended-profile slices. The Radiation references the existing Plan definition
source; the Set closes over both objects with exact Treatment Position Group,
direct Radiation, and Common Instance references.

Two seed-7 extended generations each wrote 107 files and passed strict
validation with zero failures. Their manifests are byte-identical with SHA-256
`9f3d9f4a56918b8dc8acea0e2285dca924c0a621828c2c294e2ee62c1690d41b`.
The byte-identical Radiation and Set instance SHA-256 values are respectively
`f0fa4fb17cf78e7c1127bb60367e34b8d9cf28bc515f99118a87a78991d4d998`
and
`ac67664893936ce5d32ba39da7c1f74de5a8f2920a210dce60712097b5c7fb75`.
The registry now contains 143 implemented and 39 planned logical cases.

The selected `uv`-locked `dicom-validator` 0.8.2 adapter 0.7.0 reports
`Passed`, zero errors, for both exact SOP Classes. Its fail-closed corrections
retain both Recorded RT Control Point branches and both Device Alternate
Identifier branches: empty/companions-absent and non-empty/companions-present
pass, while a non-empty identifier missing either companion fails. DCMTK 3.7.0
parses both files cleanly. Integrated conformance run
`574fa1caa3248a75b8c19f754a2ce70eb6452addb037f6fe9f5c8a9d1fc62d43`
contains passing exact-case IOD and parser results and zero accepted findings.
Whole-corpus strict conformance verification still reports 229 visible failures,
including older unavailable-tool and unrelated corpus findings. `dcentvfy`
also identifies the two current SOP Classes as unrecognized and therefore does
not provide graph semantics for them; those warnings remain visible and
unallowlisted, while strict Rust owns exact graph closure and every semantic
absence. Phase 3 milestone 6 is complete.

## Encapsulated STL Mesh Representative

`derived/mesh/encapsulated_stl` completes Phase 3 milestone 7 as a native,
byte-stable Encapsulated STL Storage slice. The selected representative is a
standalone binary-STL manufacturing model rather than Surface Segmentation:
locked highdicom 0.28.1 exposes no Surface Segmentation domain constructor,
while the Encapsulated STL IOD provides a smaller independently checkable mesh
contract without claiming segmentation-overlay semantics.

The payload is a closed tetrahedron with four outward-wound, nondegenerate
triangles, six manifold edges, four vertices, millimetre UCUM units, bounds
`[0,0,0]` through `[10,10,10]`, and positive signed volume
`166.66666666666666`. Its exact 284-byte binary STL SHA-256 is
`3c3049d231f8e98c0d2fe7cb81cf6805141bcac39dd04b9cf7f8063ec44bbfb2`.
The generated 1,692-byte Part 10 instance SHA-256 is
`d624f6392186cf505dfe38d5790008dc11af09a233ab4f7cb65b6de08954811a`.

Two seed-7 extended generations each wrote 114 files and produced identical
Encapsulated STL bytes and identical canonical manifest entries. The complete
manifests differ only in an existing semantic-stable external WSI SEG backend
elapsed-time field, so reproducibility is asserted at the byte-stable mesh
case boundary rather than by deleting provenance. Strict CLI validation
checked all 114 files with zero failures. JSON and Markdown reporting expose
the payload identity, units, geometry, topology states, and required
independent-validator disposition.

Locked `dciodvfy -new` recognized the exact object as `EncapsulatedSTL` and
reported no finding. The separately implemented, `uv`-locked
`pydicom-encapsulated-stl-payload` adapter extracted the OB value using
Encapsulated Document Length, independently parsed every binary record, and
confirmed finite geometry, unit normals agreeing with winding, outward
orientation, nondegenerate faces, opposite directed incidence on every closed
manifold edge, exact bounds, triangle count, signed volume, and payload hash.
Its mutation suite rejects count/length drift, non-finite values, non-zero
attributes, degeneracy, wrong normals or winding, open/non-manifold edges,
bounds drift, and payload drift.

The registry now contains 151 implemented and 40 planned logical cases. The
integer Parametric Map variant remains an explicit
`provider_capability_unavailable` row under the Phase 3 acceptance rule rather
than being silently omitted or generated without a qualified provider. With
that unavailable coverage visible and milestone 7 qualified, Phase 3 is
closed. The next dependency-ordered valid-file work is Phase 5 Extended Offset
Table infrastructure.
