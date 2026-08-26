# Phase 1 Proof Status

This status note records the completed work immediately before the external
runtime-manager decision checkpoint in the coverage expansion plan. Generated
corpora and validator evidence remain uncommitted.

## Shared backend platform

Protocol `0.1.0` has committed request, response, and backend-lock schemas.
`generation-backends.lock.json` records the native Rust provider as available
and every candidate external provider as planned, optional, and undiscoverable.
No Python or Java launcher or dependency manager has been selected.

The Rust runner validates requests and responses, invokes a canonical
executable directly without a shell, uses a private one-shot staging directory,
drains bounded stdout and stderr concurrently, enforces time and size limits,
and rejects identity mismatches, path traversal, symbolic links, missing files,
and undeclared files. It reopens every declared Part 10 object and compares the
dataset and File Meta SOP identities and Transfer Syntax before permitting an
atomic directory promotion. Re-entrant Rust fake-backend tests cover explicit
unavailability, request/response identity mismatch, timeout, and undeclared
output without adding an external test dependency.

## Native CT sorting proof

`geometry/ct/spatial_sort_conflicts_instance_number` is an implemented `core`
case with three axial CT instances. Geometric positions are 0, 5, and 10 mm;
Instance Numbers are 30, 10, and 20. The instances share Study, Series, and
Frame of Reference identities and have independently derived SOP Instance
UIDs. Image Laterality is `U`, which resolves the General Series Laterality
condition without inventing paired anatomy, and Patient Position is present as
an empty Type 2C value.

Each manifest row records a typed geometric rank, projected position,
Instance Number rank, tolerances, sort direction, and expected conflict. Corpus
validation reopens the files, recomputes the slice normal and position
projection, verifies spacing and shared identity, and compares geometric order
with Instance Number order. JSON and Markdown coverage reports expose both
ranks rather than merely listing the source tags.

## Verification evidence

The following checks passed on 2026-08-26:

- the focused CT generation, manifest, validation, report, and schema test;
- the complete default-feature generation CLI tests;
- two independent `core` runs with byte-identical DICOM files and manifests;
- unchanged seed-1 hashes for the pre-existing native and RLE classic CT
  instances;
- locked `dciodvfy` SHA-256
  `1aeb75d6ccd3f193e3b322b6da77742cdce2e0604868eaf2a2669c786cbc27e5`;
- locked `dcentvfy` SHA-256
  `1b96e598f28f66deee1bfc1cb52ff460c316ab6b0625dae575d701f20c836e2c`;
- `dciodvfy -new` on all three CT proof instances with no findings; and
- `dcentvfy` on the three-instance series with no findings.

A full `core` strict conformance verification still reports older findings in
pre-existing cases. Those unrelated findings were not allowlisted or weakened
as part of this slice. The Phase 1 CT proof itself is clean under both locked
dicom3tools validators.

## Decision checkpoint

The next external proof requires locking a highdicom/pydicom environment and
therefore selecting a long-term Python dependency/runtime manager. The plan
requires an explicit project decision before that selection. Until then, the
floating-point Parametric Map remains visibly planned and unavailable; it is
not silently omitted and no existing profile requires an external runtime.
