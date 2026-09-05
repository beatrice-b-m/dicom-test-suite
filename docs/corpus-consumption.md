# Corpus Consumption Guide

This guide defines how another project or agent obtains a complete, validated,
and traceable corpus from `synth-dicom-gen`. It does not define how a viewer
must render the files, how a user interface must behave, or how downstream
results must be graded.

Read [generation-guide.md](generation-guide.md) first for the capability model,
profile-selection guidance, optional runtime setup, and output interpretation.
This document focuses on producing and preserving a review handoff.

## Profile Contract

Profiles are explicit case selections:

- `smoke` is the smallest byte-stable sanity corpus.
- `core` contains common native viewer-relevant cases and required sources.
- `extended` contains broader enhanced, derived, non-image, VL, and compressed
  coverage.
- `legacy` contains valid retired or uncommon cases and is always opt-in.
- `all` is the union of `smoke`, `core`, and `extended`. It excludes `legacy`.
- `stress` contains the promoted reduced resource-boundary corpus and is
  opt-in. Its full scale remains explicitly unavailable.
- `negative` contains deterministic expected-invalid mutations and is kept
  separate from valid coverage.
- `fuzz` emits a bounded, payload-free runtime qualification and does not add
  committed DICOM payloads.

`list-cases --profile all` and `generate --profile all` use the same union.
Generate `legacy` separately when retired or uncommon valid coverage is
required. Generate `stress` separately for the reduced resource-boundary
corpus.

## Prerequisites

Every run requires:

- Rust 1.85.0, selected by `rust-toolchain.toml`;
- the committed `Cargo.lock`, enforced with `--locked`.

`jq` is recommended for the inspection examples below but is not used by the
generator itself.

The default build needs no external codec command. A complete all-features run
also requires these commands on `PATH`:

- `ojph_compress` for the `htj2k_openjph` feature;
- `dcmcjpeg` for the `legacy_jpeg_dcmtk` feature; and
- `cjxl` 0.11.2 for the lossy case enabled by `jpegxl`.

The highdicom/pydicom recipes are a separate optional runtime capability, not a
Cargo feature. They generate float32 and float64 Parametric Maps, TID 1500 and
SCOORD3D Structured Reports, and WSI tile-referencing Segmentation. Prepare the
exact environment with:

```sh
uv python install 3.12.12
uv sync --project generation-backends/highdicom-pydicom \
  --locked --no-editable --compile-bytecode --python 3.12.12
```

If that runtime is absent, generation succeeds but records
`external_backend_unavailable` for each selected backend case. A handoff
claiming complete implemented derived and quantitative coverage must prepare
the runtime and confirm that all five backend cases appear in `files`, not
`skipped_cases`.

Confirm the external commands before generation:

```sh
command -v ojph_compress
command -v dcmcjpeg
command -v cjxl
dcmcjpeg --version
cjxl --version
```

OpenJPH's `ojph_compress` does not expose a portable version flag. The
generator records its resolved executable path and SHA-256 fingerprint when it
is used.

See [external-codec-verification.md](external-codec-verification.md) for the
runtime fingerprint and verification policy for these commands.

## Caller-defined composition handoff

A composition root is a custom, template-qualified corpus, not a registry
profile. Create it from the exact spec and preserve the spec, seed, repository
revision, `manifest.json`, and report together:

```sh
cargo run --locked -- templates describe \
  classic/secondary-capture/monochrome --format json
cargo run --locked -- compose \
  --spec tests/fixtures/composition/valid/template-only.json \
  --out generated/review-composition --seed 1
cargo run --locked -- validate generated/review-composition
cargo run --locked -- report \
  generated/review-composition --format markdown
```

Require `run.kind = "composition"`, successful validation for every entry, a
closed bundle/reference graph, and no unexplained unavailable capabilities.
Verify each manifest asset path, size, and SHA-256 against the supplied local,
inline, encoded-frame, or provider input. Do not translate template IDs into
curated `case_id`, profile, or coverage claims. Independent conformance remains
the route named by the descriptor and must be handed off with its pinned tool
identity or an explicit unavailable outcome.

## Caller-defined classic CT bundle handoff

The external corpus CLI and supported Rust SDK accept a caller-named classic
CT recipe through the complete capability tuple, not an embedded case prefix,
historical planning order, or output path. Preserve a registry row with
`rust_native`/`rust_native`, DICOM artifact kind, and no feature or codec
requirements; a `native.classic_plan` recipe; and, for every artifact,
`classic/ct@1.0.0`, parameter-free `content.native_pixels`,
`algorithm.classic_ct`, `classic_projection.family = "ct"`, strict typed CT
parameters, and an explicit output. Keep artifact order contiguous from zero
and `planning_order` mandatory and globally unique; planning order is not the
planner discriminator. Partial and mixed tuples fail closed.

Handoff the descriptor and its dedicated member root, generated manifest,
strict validation result, and report. A consumer must not import generator
internals or locate a sibling checkout. This contract excludes
`native.stress_ct_plan`, other classic/VL genericity, independent conformance,
viewer results, and release qualification; record those separately when they
are actually run.

## Caller-defined CR bundle handoff

The bounded [native CR contract](generation-guide.md#caller-defined-computed-radiography)
uses the same descriptor/member-root, manifest, strict-validation and report
handoff. Preserve the exact definition identity, selected case IDs and generator
artifact identity. Caller names and paths are independent of historical family
names; native U8/OB overlay/LUT parameters and the complete typed tuple remain
required. Report2 is a manifest projection, while strict validation reopens the
payload. Keep this curated CR evidence separate from qualified CR composition,
RLE, viewer observations and independent conformance.

## Caller-defined native ultrasound bundle handoff

The bounded [native US contract](generation-guide.md#caller-defined-native-ultrasound)
accepts distinct single-frame and multiframe tuples. For multiframe handoff,
preserve the exact definition identity, caller metadata, Image Type, Frame
Increment Pointer, Frame Time, ordered relative times, semantic payload hash and
ordered frame hashes. A legal stored zero pad byte is transport structure, not
an additional sample; consumers must reject a nonzero pad and must not include
the pad in semantic frame hashes.

Handoff the descriptor/member root, selected-case capabilities result, generated
manifest, strict reopened validation and report2 projection as separate
artifacts. Do not replace the accepted embedded multiframe oracle or its original
generator/reporter pins with caller-output evidence. RLE, independent
conformance, viewer observations, packaging and release remain separate gates.

## Caller-defined native Nuclear Medicine bundle handoff

The bounded [native NM contract](generation-guide.md#caller-defined-native-nuclear-medicine-multiframe)
uses the same descriptor/member-root, selected-case capability, manifest,
strict-validation and report2 handoff. Preserve caller metadata, Image Type,
pixel spacing, window and detector sequences, dimension vectors, duration,
counts, ordered frame hashes and the exact definition identity. Do not replace
the accepted embedded NM payload oracle or its independent evidence with this
same-project caller proof; codec, viewer, package and release gates remain
separate.

## Choose A Corpus Level

### Broadest valid file corpus

Use this for a comprehensive review against the currently implemented ordinary
and legacy valid-file cases. It does not include the specialized stress,
expected-invalid negative, or payload-free fuzz scopes. Use fresh output
directories so evidence from an older run cannot be mistaken for the new
corpus.

```sh
cargo run --locked --all-features -- list-cases --profile all
cargo run --locked --all-features -- list-cases --profile legacy

cargo run --locked --all-features -- generate \
  --profile all --out generated/review-all --seed 1
cargo run --locked --all-features -- generate \
  --profile legacy --out generated/review-legacy --seed 1

cargo run --locked --all-features -- validate generated/review-all
cargo run --locked --all-features -- validate generated/review-legacy
```

Both validation commands must finish successfully with
`validation_failures\t0`. Generation is not a complete handoff if either
manifest contains an unexplained skipped case. Planned registry rows are
expected coverage gaps; feature- or runtime-unavailable implemented rows must be
explained in the handoff.

Inspect the run identities and counts:

```sh
jq '{run, generator, files: (.files | length), skipped_cases}' \
  generated/review-all/manifest.json
jq '{run, generator, files: (.files | length), skipped_cases}' \
  generated/review-legacy/manifest.json
```

### Specialized robustness and scale corpora

Generate these only when the receiving system is prepared to interpret and
bound them correctly:

```sh
cargo run --locked -- generate \
  --profile stress --out generated/review-stress --seed 1
cargo run --locked -- generate \
  --profile negative --out generated/review-negative --seed 1
cargo run --locked -- generate \
  --profile fuzz --out generated/review-fuzz --seed 1

cargo run --locked -- validate generated/review-stress
cargo run --locked -- validate generated/review-negative
cargo run --locked -- validate generated/review-fuzz
```

The negative root contains deliberately malformed DICOM instances. The fuzz
root contains no DICOM payloads; its manifest records a bounded runtime
qualification. The reduced stress root can be large or expensive relative to
ordinary profiles. Keep all three roots separate from valid conformance input.

### Portable default corpus

Use this only when external or optional codec dependencies are deliberately out
of scope:

```sh
cargo run --locked --no-default-features -- generate \
  --profile all --out generated/review-portable --seed 1
cargo run --locked --no-default-features -- validate generated/review-portable
```

Feature-gated cases remain in `skipped_cases` and coverage reports as
unavailable. A portable run must not be described as complete codec coverage.

### Fast development corpus

Use `smoke` for quick ingestion checks and `core` for common native-image work:

```sh
cargo run --locked --no-default-features -- generate \
  --profile smoke --out generated/review-smoke --seed 1
cargo run --locked --no-default-features -- generate \
  --profile core --out generated/review-core --seed 1
```

## Produce Coverage Reports

Reports describe what was generated and what was unavailable. They do not
contain viewer pass/fail judgments.

```sh
cargo run --locked --all-features -- report \
  generated/review-all --format json > generated/review-all/coverage.json
cargo run --locked --all-features -- report \
  generated/review-all --format markdown > generated/review-all/coverage.md
```

Review at least:

- generated, unavailable, skipped, and blocked counts;
- SOP Class and transfer syntax coverage;
- photometric interpretation, bit depth, frame count, and geometry coverage;
- codec features and backends;
- external generation backend identity and determinism;
- derived references and known stressors; and
- validation status.

Create separate reports for legacy, stress, negative, and fuzz roots when those
profiles are included in the handoff. The negative and fuzz sections have
profile-specific outcome semantics and must not be merged into valid generated
coverage counts.

## Manifest Contract

`manifest.json` is the primary handoff artifact. A consumer should use its
relative `path` values instead of discovering `.dcm` files and guessing their
identity. Each generated file records:

- stable `case_id` and profile membership;
- file SHA-256 and deterministic generation metadata;
- SOP Class, transfer syntax, modality, and UID identities;
- image and Pixel Data organization where applicable;
- source-object references for related instances;
- expected capabilities and semantics;
- expected source pixel or frame hashes where applicable;
- concise visual-pattern labels;
- standards evidence and generator validation results; and
- known compatibility stressors.

The manifest describes expected file content. It intentionally does not require
a consumer to use a particular viewer runner, screenshot mechanism, comparison
algorithm, or result schema.

## Consumer Handoff Checklist

Preserve these items together for every downstream review:

1. The exact repository commit used for generation and whether the worktree was
   dirty.
2. Every profile root's `manifest.json` file.
3. The SHA-256 of each manifest.
4. The Rust and Cargo versions.
5. Active Cargo features.
6. External codec command versions and executable fingerprints.
7. External generator lock, runtime, entrypoint, and environment fingerprints.
8. Generator validation output showing zero failures.
9. JSON coverage reports.
10. Any independent conformance-validation evidence, including exact float
    payload hashes for quantitative cases.
11. The consumer name, version, platform, and outcome vocabulary used to grade
    load, decode, render, reference, and unsupported behavior.

Do not rename case directories or edit generated instances. Downstream findings
should identify both `case_id` and manifest-relative `path`, because a logical
case may generate more than one SOP Instance.

## Scope Boundary

“Complete” must always name its boundary: selected profiles, implemented
registry cases, active features, and available external runtimes. It never
means complete coverage of the DICOM Standard. Current deferred areas include
full-scale stress execution, video transfer syntaxes, a genuine
greater-than-4-GiB Extended Offset Table stress object, and several lossy or
legacy codec variants. Negative results and payload-free fuzz qualifications
are separate from the valid corpus. Media and protocol qualifications use their
own opt-in commands and reports; they never become ordinary file-conformance
rows. Consult the registry, transfer-syntax capability matrix, manifest
`skipped_cases`, and generated coverage report before describing the scope of a
downstream review.
