# Corpus Consumption Guide

This guide defines how another project or agent obtains a complete, validated,
and traceable corpus from `dicom-test-suite`. It does not define how a viewer
must render the files, how a user interface must behave, or how downstream
results must be graded.

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
Generate `legacy` and `stress` separately when their opt-in coverage is
required.

## Prerequisites

Every run requires:

- Rust 1.85.0, selected by `rust-toolchain.toml`;
- the committed `Cargo.lock`, enforced with `--locked`; and
- `jq` for convenient manifest inspection.

The default build needs no external codec command. A complete all-features run
also requires these commands on `PATH`:

- `ojph_compress` for the `htj2k_openjph` feature;
- `dcmcjpeg` for the `legacy_jpeg_dcmtk` feature.

The implemented float32 Parametric Map is a separate optional runtime
capability, not a Cargo feature. Prepare its exact environment with:

```sh
uv python install 3.12.12
uv sync --project generation-backends/highdicom-pydicom \
  --locked --no-editable --python 3.12.12
```

If that runtime is absent, generation succeeds but records
`external_backend_unavailable` for the case. A handoff claiming complete
implemented quantitative coverage must prepare the runtime and confirm that
the Parametric Map appears in `files`, not `skipped_cases`.

Confirm the external commands before generation:

```sh
command -v ojph_compress
command -v dcmcjpeg
dcmcjpeg --version
```

OpenJPH's `ojph_compress` does not expose a portable version flag. The
generator records its resolved executable path and SHA-256 fingerprint when it
is used.

See [external-codec-verification.md](external-codec-verification.md) for the
runtime fingerprint and verification policy for these commands.

## Choose A Corpus Level

### Complete current corpus

Use this for a comprehensive review against every currently implemented case.
Use fresh output directories so evidence from an older run cannot be mistaken
for the new corpus.

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
manifest contains an unexplained skipped case.

Inspect the run identities and counts:

```sh
jq '{run, generator, files: (.files | length), skipped_cases}' \
  generated/review-all/manifest.json
jq '{run, generator, files: (.files | length), skipped_cases}' \
  generated/review-legacy/manifest.json
```

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

1. The exact repository commit used for generation.
2. Both complete-corpus `manifest.json` files.
3. The SHA-256 of each manifest.
4. The Rust and Cargo versions.
5. Active Cargo features.
6. External codec command versions and executable fingerprints.
7. External generator lock, runtime, entrypoint, and environment fingerprints.
8. Generator validation output showing zero failures.
9. JSON coverage reports.
10. Any independent conformance-validation evidence, including exact float
    payload hashes for quantitative cases.

Do not rename case directories or edit generated instances. Downstream findings
should identify both `case_id` and manifest-relative `path`, because a logical
case may generate more than one SOP Instance.

## Scope Boundary

“Complete” means complete for the currently implemented registry, not complete
coverage of the DICOM Standard. Current deferred areas include full-scale
stress execution, video transfer syntaxes, a genuine greater-than-4-GiB
Extended Offset Table stress object, and several lossy or legacy codec
variants. Negative results and payload-free fuzz qualifications are separate
from the valid corpus. Consult the registry, transfer-syntax capability matrix,
and generated coverage report before describing the scope of a downstream
review.
