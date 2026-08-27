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

Phase 4 milestone 3, the `TILED_SPARSE` counterpart with deliberately absent
tiles and explicit per-frame positions, is next.
