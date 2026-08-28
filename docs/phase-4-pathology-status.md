# Phase 4 Pathology and Tiled Microscopy Status

This status note records the completed dependency-ordered Phase 4 vertical
slices. Generated corpora and conformance evidence remain ignored and
uncommitted. Qualification is case-scoped; unresolved whole-corpus findings
remain visible and are not evidence against, or silently accepted for, these
slices.

## Single-frame visible light milestone

Milestone 1 is complete. VL Endoscopic and direct-patient VL Microscopic are
native byte-stable `extended` cases. Two seed-7 extended roots each contained
109 strictly valid files and produced byte-identical manifests and instances.
Both exact SOP Classes passed locked `dciodvfy`, the authorized `uv`-locked
secondary IOD route, DCMTK parsing, and independent native RGB extraction with
zero accepted findings.

## Small TILED_FULL WSI milestone

`vl/wsi/tiled_full_small` completes milestone 2 as a native byte-stable
`extended` case. Its single VL Whole Slide Microscopy Image is a `TILED_FULL`
VOLUME with four native 2 by 2 interleaved RGB Frames, one optical path, one
focal plane, and a 4 by 4 total pixel matrix. Strict validation binds the
specimen, optical path and nested ICC profile, slide label, physical geometry,
implicit Frame order, deterministic pixels, and required presences and
absences.

Two independent seed-7 extended generations each wrote 110 files and passed
strict validation with zero failures. Their byte-identical manifests have
SHA-256
`0dc0e975bcacc89a282130e69b2a84620cbe5d5e1eb736d074915781aa6fbe1a`.
The byte-identical WSI instance SHA-256 is
`a04f2f5b8e4f8526d1f2b7594427adeab255701087157d49c3db7a9622872f2b`.

Integrated conformance run
`530414e9b8b02637566f085c64234f23ec0cfe4e6f1520383d347ec09bb8c200`
records zero errors from both locked IOD validators for the exact WSI SOP
Class, clean `dcmdump` parsing, and zero accepted findings. The independent
reconstruction route is isolated from generation and runs highdicom 0.28.1
and pydicom 3.0.2 from its own `uv` lock. Its adapter composite SHA-256 is
`6b3f67bfc1aae4609ba7ccc399d78119e326556a64613621403b3b7b7a788716`.
It derives the implicit `TILED_FULL` positions and reconstructs the exact
4 by 4 interleaved RGB matrix with SHA-256
`62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a`.
Python is therefore optional conformance tooling and is not part of the native
generation path.

Whole-corpus conformance verification still reports 229 unrelated visible and
unallowlisted failures. This milestone does not reduce, accept, or conceal
them. The registry now contains 146 implemented and 36 planned logical cases.

## Small TILED_SPARSE WSI milestone

`vl/wsi/tiled_sparse_small` completes milestone 3 as a native byte-stable
`extended` case. Its two native 2 by 2 RGB Frames occupy the top-left and
bottom-right positions of a 4 by 4 total pixel matrix. The top-right and
bottom-left tiles are deliberately absent. Strict validation binds
`TILED_SPARSE`, the ordered dimension pointers and organization UID, the two
per-frame positions and dimension ordinals, physical slide coordinates,
occupancy, specimen and optical-path identities, nested ICC profile, exact
stored payload, and the zero-sentinel reconstruction oracle.

Two independent seed-7 extended generations each wrote 111 files and passed
strict validation with zero failures. Their complete output trees are
byte-identical. The manifest SHA-256 is
`456d571b7121bb67ece6593870dc4d6ef103b83c1488ccb74e84627f347186df`;
the 3,546-byte sparse instance SHA-256 is
`84251b2108b6cacb39c18de12c628bc00e0ab3d166310bcf5b82b6291955ceb3`.

Integrated conformance run
`0c347e699e40876d0fdd4ae20e8bbb76ecdb2859a10f596019202a8acefa26b1`
records zero errors from the authorized case-specific `uv`-locked
dicom-validator 0.8.2 authority and clean `dcmdump` parsing. The isolated
highdicom 0.28.1 reconstruction adapter is version 0.2.0 with composite
SHA-256
`a89f55577263f84a27291a6d3adf6659ccebedb76e68dd8b9c06f8b0b3ce7f4e`.
It independently reproduced the two Frame hashes, exact 24-byte payload hash,
explicit positions, `[present, absent, absent, present]` occupancy, and
sentinel-filled 4 by 4 matrix hash with transforms disabled.

Locked dicom3tools still reports its known full-grid Number of Frames error.
That result remains visible, unallowlisted characterization rather than a
passing IOD result or accepted finding. Whole-corpus conformance verification
continues to report 229 unrelated failures and zero accepted findings. The
registry now contains 147 implemented and 35 planned logical cases.

## Small multi-resolution WSI pyramid milestone

`vl/wsi/pyramid_multiresolution` completes milestone 4 as a native byte-stable
`stress` case. One logical case emits exactly three ordered VL Whole Slide
Microscopy Image instances: a four-Frame VOLUME layer, a one-Frame THUMBNAIL
apex layer, and a one-Frame LABEL companion. The two resolution layers share
the deterministic Pyramid UID; LABEL shares the Study, Series, Frame of
Reference, specimen, container, and optical-path identities but is correctly
excluded from pyramid membership.

Two independent seed-7 stress generations each wrote three files and six total
Frames. Strict validation accepted all three files with zero failures in both
roots, and the complete output trees compare byte-for-byte. Generation took
0.55 and 0.59 seconds in parallel qualification, below the locked five-second
ceiling. The group totals 8,694 DICOM bytes, below its 65,536-byte ceiling. Its
manifest SHA-256 is
`75c1ff84c0ab971f99308991308552640f593fcd199c652bd787908076ca6265`.
The qualified members are:

| Role | Bytes | SHA-256 |
| --- | ---: | --- |
| VOLUME | 2,934 | `fece75ee74a3e8d9902807b2c3ace1384e0896469c4b41358d3b2d6444de7b07` |
| THUMBNAIL | 2,914 | `159cf9c96bbb205966ee924ac5f6c4385c1e4474f672fa8a7410bcacb998defb` |
| LABEL | 2,846 | `aa6c79cb54c41cb1267425bc5602fa4c916bc91b9f3fa66fd9942be446f45438` |

Locked `dciodvfy` and the authorized independent `uv`-locked
dicom-validator each reported zero IOD errors for every role. The isolated
`uv`-locked highdicom 0.28.1/pydicom 3.0.2 reconstruction adapter version
0.3.0 derives roles from DICOM attributes rather than filenames and exactly
reconstructs the two pyramid layers, deterministic reduction, LABEL pixels,
and complete three-member identity and membership closure.

Integrated conformance run
`0188fc12678acf82e29f27c139d531dd060ec8e2f36363c9927d4d673d869f6d`
has zero entity findings, passing independent pixel evidence, zero accepted
findings, and zero verification failures against an empty exact-slice findings
set. No unavailable route was silently omitted. The registry now contains 148
implemented and 34 planned logical cases.

The post-promotion repository gate
`cargo test --locked --all-targets --no-default-features --quiet` passes in
full, including generator, manifest, report, registry, conformance-routing,
and legacy planned-WSI compatibility regressions.

Phase 4 milestone 4 is complete; milestone 5, multiple optical paths or focal
planes, is next. Ordinary `all` remains unchanged: this bounded pyramid is
selected only through the stress profile or explicit stress inclusion. A
genuinely full-size pyramid remains a separate planned case and will not enter
ordinary CI until the explicit dimensions, resource budgets, and scheduling
checkpoint required by the coverage plan is decided.

## Multiple optical paths WSI milestone

`vl/wsi/multiple_optical_paths` completes milestone 5 as a native,
byte-stable `extended` case. It stores two ordered optical paths,
`BRIGHTFIELD` and `ALTERNATE`, over the same 4 by 4 total pixel matrix and one
separately encoded focal plane. Eight native RGB Frames follow the normative
`TILED_FULL` path-major implicit order. Strict validation binds the exact path
descriptions, 550/650 nm wavelengths, illumination codes, two nested ICC
profiles, Frame and payload hashes, separate matrix reconstructions, inherited
specimen and slide geometry, and every locked absence.

Two independent seed-7 extended generations each wrote 112 files, passed
strict validation with zero failures, and compared byte-for-byte as complete
trees. Both full roots completed in 4.50 and 4.54 seconds. Their manifest
SHA-256 is
`473e822fe1b82b7217635a980757a1a88f77f3e2448b0e02964122d888a16bf3`;
the 3,916-byte multiple-path instance SHA-256 is
`a2099a90b53b4ecb9c76f895f02d4f7f62ff8655adcb54d8654e8e80507bea48`.

Locked `dciodvfy` and the authorized `uv`-locked dicom-validator both report
zero IOD errors, and DCMTK parses the file cleanly. The isolated highdicom
0.28.1/pydicom 3.0.2 adapter version 0.4.0 independently derives all eight
implicit positions, reproduces the aggregate and per-path payloads, constructs
two distinct 4 by 4 by 3 matrices, validates nested ICC evidence, and rejects
ambiguous unfiltered matrix access. Its composite fingerprint is
`fa838f3b3c398913f2f05e71cad2515cf038fba65dc8f1a30484f88164c48167`.

Exact-slice conformance run
`c2203223e9d8ce0b716175329769b7f3bb947ac48da44a510843d5a82d8b3dcc`
records silent entity validation, passing independent pixels, zero accepted
findings, and zero verification failures against an empty findings set. The
whole-corpus run retains all 229 unrelated visible failures and zero accepted
findings. The registry now contains 149 implemented and 33 planned logical
cases.

Phase 4 milestone 5 is complete. Milestone 6, a derived object referencing WSI
tiles, is next. The explicit full-size-pyramid dimensions, budget, and ordinary
CI scheduling checkpoint remains undecided and is not affected by this small
extended-profile case.
