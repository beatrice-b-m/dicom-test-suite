# Deterministic Build Policy

Generated DICOM payloads are build artifacts, but generation must be
reproducible according to each case's declared determinism level.

## Determinism Levels

- `byte_stable`: exact file SHA-256 reproducibility is expected.
- `semantic_stable`: decoded pixel/frame hashes, manifest content, and semantic
  expectations are stable, but encoded bytes may vary by codec version.
- `unstable`: allowed only with explicit warning and manifest metadata; do not
  use in `smoke` or `core`.

## Reproducibility Inputs

The generator output is defined by:

- case ID and recipe version;
- run profile and seed;
- standards lock hash;
- generator package version and git SHA when available;
- Rust toolchain and target triple;
- `Cargo.lock` hash;
- DICOM-rs crate versions;
- enabled feature flags;
- transfer syntax capability matrix entry;
- codec library versions and external validator versions when used.

Changing any of these inputs may legitimately change generated output and must
be reflected in `manifest.json`.

## UID Derivation

DICOM UIDs must use deterministic UUID-derived `2.25.<decimal uuid>` values.
UID inputs are:

- project namespace UUID;
- standards lock hash;
- case ID;
- recipe version;
- UID role;
- run seed;
- file index;
- frame or referenced-object index when applicable.

Required UID roles include:

- `study_instance_uid`;
- `series_instance_uid`;
- `sop_instance_uid`;
- `frame_of_reference_uid` when the IOD or geometry requires it;
- `implementation_class_uid`.

## Controlled Metadata

Generation must control:

- timestamps, dates, and datetimes used in generated metadata;
- attribute ordering where the writer permits control;
- sequence item ordering;
- 128-byte Part 10 preamble content;
- Implementation Class UID;
- Implementation Version Name;
- private creator strings;
- synthetic pixel pattern seed;
- codec feature flags and codec versions.

Default generated timestamps should be stable, not wall-clock time. If a case
intentionally varies time metadata, it must not be `byte_stable`.

## Hash Expectations

Every run manifest records:

- standards lock SHA-256;
- `Cargo.lock` SHA-256;
- file SHA-256 for every generated `.dcm`;
- decoded frame hashes where pixel decoding is available;
- semantic or visual hashes declared by the recipe.

`byte_stable` cases compare generated files byte-for-byte. `semantic_stable`
cases compare manifest semantics and decoded frame/preview hashes. `unstable`
cases are allowed only outside `smoke` and `core` and must include a reason.

## Required Verification

CI and local release checks must include a two-run smoke reproducibility check:

```sh
synth-dicom-gen generate --profile smoke --out /tmp/synth-dicom-gen-a --seed 1
synth-dicom-gen generate --profile smoke --out /tmp/synth-dicom-gen-b --seed 1
diff -r /tmp/synth-dicom-gen-a /tmp/synth-dicom-gen-b
```

Compressed or feature-gated cases declared `semantic_stable` must be compared by
decoded hashes and manifest semantics rather than raw file bytes.

## Generated Artifact Handling

Generated DICOM files, generated manifests, validation sidecars, reports,
standards caches, and generated knowledge bases must stay under ignored paths or
ignored extensions. Do not commit generated DICOM payloads.
