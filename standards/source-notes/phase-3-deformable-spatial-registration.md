# Phase 3 Deformable Spatial Registration Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `derived/registration/deformable_ct_pair`
- Recipe ID: `derived_registration_deformable_ct_pair`
- Recommended provider: `rust_native`
- Registered image: `enhanced/ct/multiframe_shared_perframe_explicit_le`
- Source image: `classic/ct/mono2_i16_rescale_12bit_explicit_le`
- Output: Deformable Spatial Registration Storage
  (`1.2.840.10008.5.1.4.1.1.66.3`), Explicit VR Little Endian
- Future manifest field: `expected_deformable_spatial_registration`

## Locked IOD And Module Contract

The output Instance belongs to the Enhanced CT Study and uses the Enhanced CT
Frame of Reference as its Registered RCS. Modality is `REG`; Content Date and
Content Time are present; and the Content Identification Macro's Type 1 and
Type 2 Attributes are present. The output contains no Pixel Data.

PS3.3 Table A.39.2-1 makes the Patient, General Study, General Series, Spatial
Registration Series, Frame of Reference, General Equipment, Enhanced General
Equipment, Deformable Spatial Registration, Common Instance Reference, and SOP
Common Modules mandatory. The recipe includes exactly those mandatory Modules.
Clinical-trial and General Reference Modules remain absent.

The Type 1 Deformable Registration Sequence `(0064,0002)` contains exactly one
Item for the classic CT Source RCS. The Item contains Source Frame of Reference
UID `(0064,0003)`, exactly one Referenced Image Sequence `(0008,1140)` Item
identifying the complete classic CT Instance, and a present but empty Type 2
Registration Type Code Sequence `(0070,030D)`. It also contains exactly one
Pre Deformation Matrix Registration Sequence `(0064,000F)` Item, exactly one
Post Deformation Matrix Registration Sequence `(0064,0010)` Item, and exactly
one Deformable Registration Grid Sequence `(0064,0005)` Item. Referenced Frame
Number is omitted because the reference selects the complete single-frame
source Instance. Used Fiducials Sequence and the optional comment are absent.

Both pre and post matrix Items contain Frame of Reference Transformation Matrix
`(3006,00C6)` with exactly 16 DS values in row-major order and Matrix Type
`(0070,030C)` equal to `RIGID`. Both matrices are the identity matrix:

```text
[1, 0, 0, 0,
 0, 1, 0, 0,
 0, 0, 1, 0,
 0, 0, 0, 1]
```

Their explicit presence exercises both conditional matrix Sequences without
obscuring deformation semantics. They are applied around the vector offset in
the normative order `M_post(M_pre(P_registered) + D)`, and the result is a
coordinate in the Source RCS. This Registered RCS to Source RCS sampling
direction is the opposite of the Source RCS to Registered RCS matrix direction
locked for `derived/registration/spatial_ct_pair`.

## Deterministic Deformation Grid

The sole grid Item is axial and covers the four pixel centers of Enhanced CT
frame 2:

- Image Orientation (Patient) `(0020,0037)`: `[1,0,0,0,1,0]`
- Image Position (Patient) `(0020,0032)`: `[0,0,2.5]` in the Registered RCS
- Grid Dimensions `(0064,0007)`: UL VM 3, `[2,2,1]`
- Grid Resolution `(0064,0008)`: FD VM 3, `[0.75,0.75,2.5]` mm
- Vector Grid Data `(0064,0009)`: OF VM 1, 48 bytes containing 12 finite
  IEEE 754 binary32 values

Each grid voxel's position is its center. Vectors are encoded as consecutive
`[delta_x,delta_y,delta_z]` triples in mm, with `i` (left to right) varying
fastest, then `j` (top to bottom), then `k` (plane). The exact vector order is:

```text
i=0,j=0,k=0  [-0.625, -0.625, -2.5]
i=1,j=0,k=0  [-0.75,  -0.625, -2.5]
i=0,j=1,k=0  [-0.625, -0.75,  -2.5]
i=1,j=1,k=0  [-0.75,  -0.75,  -2.5]
```

Under Explicit VR Little Endian, each OF component is encoded as a little-endian
32-bit float. The exact 48-byte Value Field is:

```text
000020bf000020bf000020c0000040bf000020bf000020c0
000020bf000040bf000020c0000040bf000040bf000020c0
```

Its SHA-256 is
`d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47`.
The byte count invariant is `X_D * Y_D * Z_D * 3 * 4 = 48`.

With identity pre/post matrices, the four vectors map the Enhanced CT frame 2
pixel centers `[0,0,2.5]`, `[0.75,0,2.5]`, `[0,0.75,2.5]`, and
`[0.75,0.75,2.5]` exactly to classic CT source pixel centers
`[-0.625,-0.625,0]`, `[0,-0.625,0]`, `[-0.625,0,0]`, and `[0,0,0]`.
The first mapping is the locked landmark. This is a non-uniform displacement
field, so transposing X/Y vector order is observable even though both source
images are only 2 by 2.

## Common Instance Reference Hierarchy

The Common Instance Reference Module closes both input references. Because the
Deformable Spatial Registration Instance is in the Enhanced CT Study, the
Enhanced CT is grouped under Referenced Series Sequence `(0008,1115)` with its
exact Series, SOP Class, and SOP Instance identities. The classic CT is in a
different Study and is grouped under
Studies Containing Other Referenced Instances Sequence `(0008,1200)`, then
Referenced Series Sequence, with its
exact Study, Series, SOP Class, and SOP Instance identities. Sequence order is
fixed as registered Enhanced CT then source classic CT. Neither reference uses
Referenced Frame Number.

## KB Query And Locked Official Evidence

- Query: `dicom-kb lookup uid DeformableSpatialRegistrationStorage --edition 2026b`
- Edition: 2026b
- Result: `1.2.840.10008.5.1.4.1.1.66.3`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the KB registry evidence proves the SOP Class UID but does not
  expose the deformable module table, sampling direction, nested cardinalities,
  grid coordinate convention, or OF payload ordering required by this recipe.

The exact rules are anchored in PS3.3 A.39.2 and Table A.39.2-1 (IOD and
mandatory Modules), C.20.1 (Spatial Registration Series), C.20.3 and Table
C.20.3-1 (deformable Attributes and cardinalities), C.20.3.1.1 (registered-to-
source application order), C.20.3.1.2 (grid vectors located at voxel centers),
C.20.3.1.3 (vector triples, `i/j/k` order, undefined NaN triple, and byte-count
formula), C.7.4.1 (Frame of Reference), C.12.2 and Table C.12-8 (Common
Instance Reference), and Tables 10-3 and 10-12 (SOP reference and Content
Identification Macros). PS3.4 Table B.5-1 identifies the Storage SOP Class.
PS3.6 Tables A-1 and 6-1 lock the UID and these data element VR/VM values:
`(0064,0002)` SQ 1, `(0064,0003)` UI 1, `(0064,0005)` SQ 1,
`(0064,0007)` UL 3, `(0064,0008)` FD 3, `(0064,0009)` OF 1,
`(0064,000F)` SQ 1, and `(0064,0010)` SQ 1. PS3.5 Sections 6.2 and 7.3
govern OF binary32 and transfer-syntax byte ordering.

The repository `standards.lock.json` records official PS3.3, PS3.4, PS3.5,
and PS3.6 artifacts as `unavailable_not_downloaded`; none is committed. The
independently locked 2026b validator cache used for this check pins official
DocBook PS3.3 SHA-256
`4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`,
PS3.4 SHA-256
`8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`,
and PS3.6 SHA-256
`512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`
in `conformance-backends/dicom-validator/standard-lock.json`. The PS3.5
section anchors are recorded without claiming a locally downloaded artifact.

## Manifest, Validation, And Independent Acceptance Contract

`expected_deformable_spatial_registration` shall bind the Registered and Source
Frames of Reference, exact source case/path/hash and Study/Series/SOP
identities, complete-instance selection, every sequence cardinality, exact
identity pre/post matrices and types, grid geometry, vector order, OF VR/VM,
exact byte length and payload SHA-256, all 12 decoded finite float values, the
four point mappings, Common Instance Reference hierarchy, and no-pixel
invariant. Generation must reopen and hash both sources before writing.

Strict Rust validation owns the exact sampling mathematics, finite vector
checks, float decoding and byte order, sequence contract, source identities,
and landmark mappings. Promotion requires clean locked `dciodvfy -new`, locked
DCMTK `dcmdump` parsing, isolated locked `dcentvfy` closure over the REG and both
CT inputs, and the already locked `dicom-validator` 0.8.2 registration adapter
as additive secondary IOD evidence. Negative controls shall cover reversed
sampling direction, swapped vector order, truncated or big-endian OF payload,
wrong OF byte count, NaN in only part of a vector triple, wrong grid origin or
resolution, missing/duplicate grid Item, missing pre/post Item, non-identity or
non-rigid recipe matrices, redirected references, broken cross-Study grouping,
and added Pixel Data. No new finding may be silently allowlisted.

The exact 2 by 2 by 1 candidate, including both identity matrix Sequences,
completed `dciodvfy -new`, DCMTK 3.7.0 `dcmdump`, isolated `dcentvfy`, and the
`uv`-locked `dicom-validator` 0.8.2 adapter with exit code 0 and no findings.
The temporary 2,090-byte prototype had SHA-256
`926ab093e7f66bc9d7fb75ddaded704274325e19a878d3999d5ebd17de583672`.
It is qualification evidence only and is not committed.

Empirical mutations show that neither IOD validator owns the aggregate
at-least-one-grid condition, the OF byte-count equation, positive dimensions
and resolutions, complete-NaN versus partial-NaN vector rules, exact grid
geometry and sampling direction, or Source Frame of Reference consistency.
`dciodvfy` does detect incorrect VM and grid Item cardinality where
`dicom-validator` may not. Only isolated `dcentvfy` detected a dangling SOP
reference. These limitations define the strict Rust and entity-validation
responsibilities; they are not accepted findings.

## Project Action

- Registry status: planned; this note does not promote the case.
- Registry provider: `rust_native`. DICOM-rs can write the required nested
  Sequences and OF payload directly, allows a byte-stable little-endian
  contract, and preserves Python/pydicom as an independent secondary
  validation implementation.
- Registry blocker: `recipe_unimplemented`. The stale external-backend and
  unavailable-validator blockers are removed because the native construction
  path and locked additive validator are both proven. Promotion still requires
  the writer, strict payload validator, reference closure, deterministic
  manifests, report surfaces, and mutation tests.
- Should become KB patch: yes; expose the Deformable Spatial Registration
  module table, sampling direction, sequence cardinalities, grid semantics,
  and Vector Grid Data ordering as structured 2026b queries.
- Do not commit generated DICOM files, validator outputs, or official standards
  artifacts.
