# Phase 3 Spatial Registration Evidence

Checked: 2026-08-27  
Standards baseline: 2026b, `standards.lock.json`  
Source manifest SHA-256:  
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `derived/registration/spatial_ct_pair`
- Recipe ID: `derived_registration_spatial_ct_pair`
- Provider: `rust_native`
- Registered image: `enhanced/ct/multiframe_shared_perframe_explicit_le`
- Moving image: `classic/ct/mono2_i16_rescale_12bit_explicit_le`
- Output: Spatial Registration Storage, Explicit VR Little Endian
- Manifest field: `expected_spatial_registration`

## Locked Semantic Contract

The Spatial Registration SOP Instance belongs to the Enhanced CT Study and
uses that Enhanced CT Frame of Reference as its Registered RCS. Its Modality is
`REG`, its Content Label is `DTS_RIGID_REG`, and the Content Identification
Macro's Type 1 and Type 2 Attributes are present. It contains no Pixel Data.

Registration Sequence `(0070,0308)` contains exactly two ordered Items. The
first Item references the complete two-frame Enhanced CT instance in its own
Frame of Reference and supplies one identity `RIGID` matrix. The second Item
references the complete single-frame classic CT instance in a distinct source
Frame of Reference and supplies one source-to-registered `RIGID` matrix. Both
Items contain exactly one Matrix Registration Sequence Item and exactly one
Matrix Sequence Item. Registration Type Code Sequence `(0070,030D)` is present
with zero Items because it is Type 2 and no registration-input method is
asserted. Referenced Frame Number is omitted because each reference applies to
the complete referenced instance.

The moving source-to-registered matrix is the following row-major 4 by 4
homogeneous transform:

```text
[1, 0, 0, 0.625,
 0, 1, 0, 0.625,
 0, 0, 1, 2.5,
 0, 0, 0, 1]
```

It is the geometry-derived translation `[+0.625,+0.625,+2.5]` mm: the classic
CT first-pixel center `[-0.625,-0.625,0]` maps to registered point
`[0,0,2.5]`, the first-pixel center of Enhanced CT frame 2.
The rotation submatrix is orthonormal, its determinant is one, and the final
row is `[0,0,0,1]`. The direction is deliberately locked as Source RCS to Registered RCS;
Deformable Spatial Registration uses the opposite sampling
direction and must not be used as precedent for this matrix.

The Common Instance Reference Module closes both references. The Enhanced CT
is grouped under Referenced Series Sequence because it shares the Registration
Study. The classic CT is grouped under Studies Containing Other Referenced Instances Sequence
with its exact Study, Series, SOP Class, and SOP Instance
identities. Sequence order is fixed as registered image then moving image.

## KB Query

- Query: `dicom-kb lookup uid SpatialRegistrationStorage --edition 2026b`
- Edition: 2026b
- Result: `1.2.840.10008.5.1.4.1.1.66.1`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the registry row previously proved only the UID. The exact
  matrix direction, nested sequence cardinalities, rigid constraints, and
  cross-Study reference hierarchy require the anchors below.

## Official Source Evidence

- PS3.4 Table B.5-1 identifies Spatial Registration Storage as a Storage SOP
  Class.
- PS3.6 Table A-1 assigns UID `1.2.840.10008.5.1.4.1.1.66.1`; Table 6-1
  defines the registration, matrix, Frame of Reference, and reference elements.
- PS3.3 A.39.1 and Table A.39.1-1 define the Spatial Registration IOD and make
  the Spatial Registration and Common Instance Reference Modules mandatory.
- PS3.3 C.20.1 and Table C.20.1-1 require Modality `REG`.
- PS3.3 C.20.2 and Table C.20.2-1 define Registration Sequence, its reference
  alternatives, the single Matrix Registration Sequence Item, the Type 2
  Registration Type Code Sequence, and one or more ordered Matrix Items.
- PS3.3 C.20.2.1.1 defines the matrix as mapping Source RCS coordinates to the
  Registered RCS and requires row-major element order. C.20.2.1.2 defines a
  `RIGID` transform as rotations and translations with an orthonormal matrix.
- PS3.3 C.7.4.1 defines Frame of Reference identity. PS3.3 C.12.2 and Table
  C.12-8 define same-Study and other-Study Common Instance Reference grouping.
  Tables 10-3, 10-11, and 10-12 define image/SOP references and Content
  Identification.
- PS3.17 O.1, O.3, and O.5 are informative corroboration for Source RCS to
  Registered RCS direction, reference use, and rigid matrix registration.

The official PS3.3, PS3.4, and PS3.6 source artifacts are recorded in
`standards.lock.json` as `unavailable_not_downloaded`; the project uses the
pinned 2026b KB source-manifest identity and concise anchors rather than
committing standard content. PS3.17 is informative and does not replace the
normative PS3.3 matrix rules.

## Manifest And Strict Validation Contract

`expected_spatial_registration` shall bind the Registered Frame of Reference,
matrix direction, ordered registration Items, exact source case/path/hash and
Study/Series/SOP/Frame of Reference identities, complete-instance selection,
nested sequence cardinalities, matrix type, all 16 row-major values, rigid
tolerances, and the `[-0.625,-0.625,0]` to `[0,0,2.5]` landmark. It shall also bind the
exact same-Study and other-Study Common Instance Reference hierarchy and assert
that Pixel Data is absent. The ordinary manifest reference list retains both
source hashes and identities.

Before writing the Registration instance, Rust reopens both source files and
verifies their hashes and identities. It verifies the Enhanced CT's two axial
frames, positions `[0,0,0]` and `[0,0,2.5]`, orientation, and Registered Frame
of Reference, plus the classic CT's first-pixel center, orientation, and distinct source
Frame of Reference. Strict output validation independently checks all locked
module, reference, matrix, geometry, and no-pixel invariants.

Negative controls replace a same-length Frame of Reference UID, reverse the
translation sign while repairing the manifest hash, make the declared `RIGID`
matrix non-orthonormal, change its VM or homogeneous final row, omit the Type 2
Registration Type Code Sequence, redirect a referenced SOP Instance, break the
cross-Study hierarchy, remove or reorder the identity Item, substitute SOP
Class `.66.3`, or add Pixel Data. A valid but different `AFFINE` or
`RIGID_SCALE` object is a project-contract mismatch, not automatically an IOD
error.

## Independent Acceptance Gate

Promotion requires a clean locked dicom3tools `dciodvfy -new` result, locked
DCMTK `dcmdump` parsing, and isolated locked `dcentvfy` reference closure. The
uv-locked pydicom `dicom-validator` 0.8.2 adapter with hash-locked official
2026b definitions completed its case-scoped empirical qualification: the valid
candidate is clean and omission of the Type 1 Matrix Registration Sequence is
detected. It is retained as secondary IOD evidence, not the sole authority.

The empirical controls also define its limits. `dicom-validator` does not
detect a VM 15 Frame of Reference Transformation Matrix, a non-orthonormal
matrix declared `RIGID`, or a dangling referenced SOP Instance. Locked
`dciodvfy` detects the VM 15 violation, strict Rust validation owns rigid
matrix mathematics and the exact semantic contract, and locked `dcentvfy`
owns reference closure. These misses remain explicit qualification evidence;
they are not accepted findings and must not be silently allowlisted.

## Project Action

- Registry status: implemented after the complete vertical gate passed.
- Registry provider: `rust_native`; generic DICOM-rs sequence construction is
  sufficient and avoids an unnecessary external generation dependency.
- Registry blockers: none. The deterministic native writer, exact manifest
  contract, strict validator, report surface, focused mutations, and locked
  independent-validator route are complete.
- Should become KB patch: yes; expose structured Spatial Registration module,
  matrix-direction, and Common Instance Reference queries.
- Do not commit generated DICOM files, validator output, or official standards
  artifacts.

## Promotion Evidence

Two seed-7 `extended` generations each wrote 92 files and produced identical
manifests with SHA-256
`f45a347517d43c1e810a0f6866aa4468478ff1f403edc0cbe39323045a82079e`.
The 2,328-byte Spatial Registration instances were byte-identical with SHA-256
`8b3b8498c3e90dc13e52cceb9c584fbb41d5898e28c2f3d3f86baf4a1654ac8`.
Both generated roots passed strict validation with zero failures.

Integrated conformance run
`522f2627658dd11ae6e5b88ad5e673659cacfdb2abf45fe4cb43adfb90feb7ea`
recorded stable instance key
`4c484723f1de25edb5830e18eb8447bbdc7ee53785dd35109f711b9bc0f6e06b`.
Locked `dciodvfy -new`, DCMTK 3.7.0 `dcmdump`, and the `uv`-locked
`dicom-validator` 0.8.2 secondary IOD adapter all completed with exit code 0;
the two IOD validators reported no errors. An isolated `dcentvfy` invocation
over the REG and its two referenced CT instances completed silently with exit
code 0. The Python dependency lock SHA-256 is
`8e977ab4b75f24373e3039a2bdd2ba8dc299432d0fcbcb49f2be41df9381c8fc`,
and the independent 2026b definition lock SHA-256 is
`95574ed005a8ff84fe9834fee65ba82c3cadbd6f433fd4b11bc54b5ce71e9f9b`.

Full-corpus conformance verification still reports 207 older or unrelated
validator-availability, IOD, and entity findings. They remain visible and were
not allowlisted or weakened for this promotion.
