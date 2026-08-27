# Phase 1 Proof Status

This status note records completion of both Phase 1 proof lanes after the
project selected `uv` for the optional Python backend. Generated corpora and
validator evidence remain ignored and uncommitted.

## Shared backend platform

Protocol `0.1.0` has committed request, response, and backend-lock schemas.
`generation-backends.lock.json` records both the native Rust provider and the
highdicom/pydicom provider as available. The latter remains optional and is
discovered only through its prepared `.venv` interpreter or the controlled
`DTS_HIGHDICOM_PYTHON` override. Other external providers remain planned and
undiscoverable.

The highdicom backend is a nested `uv` project with an exact `uv.lock`, CPython
3.12.12, highdicom 0.28.1, pydicom 3.0.2, and a reviewed permissive license
set. Provisioning uses `uv sync --locked --offline --no-editable`; ordinary
tests and native profiles do not provision or require it. Discovery verifies
the dependency lock, interpreter, installed distributions, source entrypoints,
fixed arguments, and runtime identity before invocation.

The Rust runner validates requests and responses, invokes a canonical
executable directly without a shell, uses a private one-shot staging directory,
drains bounded stdout and stderr concurrently, enforces time and size limits,
and rejects identity mismatches, path traversal, symbolic links, missing files,
and undeclared files. Rust hashes the canonical executable, derives the locked
platform and fixed-argument environment identity, and rejects dependency,
executable, or environment fingerprints that do not match the response. It
reopens every declared Part 10 object and compares the
dataset and File Meta SOP identities and Transfer Syntax before permitting an
atomic directory promotion. Re-entrant Rust fake-backend tests cover explicit
unavailability, request/response identity mismatch, timeout, and undeclared
output without adding an external test dependency.

## External float32 Parametric Map proof

`derived/parametric-map/float32_ct_derived_explicit_le` is an implemented,
optional-runtime `extended` case. The extended profile also includes the three
CT sorting-proof sources. When the prepared runtime is absent, generation
retains the 78-file native baseline and writes a structured
`external_backend_unavailable` row. When present, it writes one three-frame
Parametric Map and records the exact backend version, lock, executable,
entrypoint, environment, and installed-distribution identities.

The generated object uses native Float Pixel Data `(7FE0,0008)` with OF VR,
32-bit samples, multi-frame dimensions, per-frame source derivation, Common
Instance References, and a UCUM no-units Real World Value Mapping for DCM
X-Ray Attenuation. Rust independently derives each binary32 value from the CT
stored samples, normalizes highdicom's spatial frame order, compares exact
serialized bytes and hashes, rejects integer-only pixel attributes, reopens
the promoted Part 10 object, and emits canonical mapping and reference
metadata. Internal validation repeats the OF length and frame-hash checks.

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
- exact `uv.lock` SHA-256
  `d36e8258e63eb0efdd9ef1b401ee36fca795cf2adb360e735b95a90a663073a0`;
- two seed-7 external runs with identical source-derived float semantics,
  identities, mappings, references, backend provenance, and OF payload;
- internal validation of all 79 extended files with zero failures;
- `dciodvfy -new` on the Parametric Map with no findings;
- locked DCMTK `dcmdump` SHA-256
  `d2261944ea1ceb6743df9866f2237014b284fa39119c8a5eee226ae922ead45f`;
- DCMTK extraction of all 12 float values with exact independent per-frame
  SHA-256 matches; and
- `dcentvfy` on the three CT sources and derived Parametric Map with no
  findings.

A full `extended` strict conformance verification still reports 228 older
unresolved findings across pre-existing cases and entity metadata. Those
unrelated findings were not allowlisted or weakened as part of this slice. The
two Phase 1 proofs themselves are clean under the locked dicom3tools IOD and
entity validators, and the external proof also has passing independent parser
and pixel evidence.

## Decision checkpoint resolution

The project explicitly selected `uv`. This resolved the runtime-manager
checkpoint without making Python mandatory for an existing profile: absence is
still a first-class, tested outcome. The independent-IOD-validation checkpoint
is also satisfied for this generator. Phase 2 native breadth can proceed
without another Section 11 decision checkpoint.
