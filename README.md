# dicom-test-suite

`dicom-test-suite` is a Rust project for generating a comprehensive local corpus of synthetic DICOM files for viewer compatibility testing.

The suite is standard-first: it is not designed around the current behavior of any one viewer. Its generated files should expose compatibility gaps in DICOM parsers and viewers across legacy single-frame images, enhanced multi-frame images, mammography, color and palette images, overlays, presentation states, segmentations, structured reports, and relevant transfer syntaxes.

Generated DICOM files are intentionally not committed. The repository should contain deterministic generation code, case recipes, validation rules, manifests, and reports. Local output belongs under ignored paths such as `generated/`, `out/`, or `target/`.

## Requirements

- Rust 1.85.0, selected automatically by `rust-toolchain.toml`.
- `jq` for the same JSON artifact checks used by CI.
- Optional external codec commands only when enabling their features.
- Optional `uv` 0.11.26 and managed CPython 3.12.12 for float32/float64
  Parametric Maps and the TID 1500 Measurement Report.

## Commands

```sh
cargo run --locked -- list-cases
cargo run --locked -- list-cases --profile smoke
cargo run --locked -- generate --profile smoke --out generated/smoke --seed 1
cargo run --locked -- generate --profile core --out generated/core --seed 1
cargo run --locked -- generate --profile extended --out generated/extended --seed 1
cargo run --locked -- validate generated/extended
cargo run --locked -- report generated/extended --format markdown
cargo run --locked -- standards check-lock
cargo run --locked -- conformance check-tools
cargo run --locked -- conformance run generated/extended --out reports/conformance/extended
cargo run --locked -- conformance verify reports/conformance/extended
```

Generation writes DICOM Part 10 files plus a versioned `manifest.json`. The
default build requires no external codec tools. Cases whose Cargo features are
disabled remain visible in manifests and reports as feature-gated unavailable
coverage.

The `extended` profile always generates and manifests the native CT and SEG
dependencies for the external Parametric Map and TID 1500 recipes. If the
prepared `uv` environment is absent, each derived case is retained as an
explicit `external_backend_unavailable` row. To enable them:

```sh
uv python install 3.12.12
uv sync --project generation-backends/highdicom-pydicom \
  --locked --no-editable --python 3.12.12
cargo run --locked -- generate \
  --profile extended --out generated/extended --seed 1
```

Generation never invokes `uv` or performs network access. Runtime preparation,
exact versions, fingerprints, and licenses are documented in
[generation-backends/highdicom-pydicom/README.md](generation-backends/highdicom-pydicom/README.md).
The current quantitative and SR gate evidence is recorded in
[docs/phase-3-derived-status.md](docs/phase-3-derived-status.md).

## Profiles

- `smoke`: smallest byte-stable sanity corpus.
- `core`: common viewer-relevant native cases and required source objects.
- `extended`: enhanced, derived, non-image, VL, compressed, and broader
  compatibility cases.
- `legacy`: valid retired or uncommon behavior.
- `all`: smoke, core, and extended; legacy remains opt-in.

The future `stress`, `negative`, and `fuzz` scopes are not part of the completed
current-term corpus.

## Codec Features

| Feature | Coverage | Extra runtime requirement |
|---|---|---|
| `jpeg` | JPEG Baseline 8-bit | none |
| `charls` | JPEG-LS Lossless | build dependency only |
| `jpegxl` | JPEG XL Lossless | none |
| `jpeg2000` | JPEG 2000 Lossless | build dependency only |
| `deflate` | dataset deflate and Deflated Image Frame | none |
| `htj2k_openjph` | HTJ2K Lossless | `ojph_compress` on `PATH` |
| `legacy_jpeg_dcmtk` | JPEG Lossless Process 14 and SV1 | `dcmcjpeg` on `PATH` |

For example:

```sh
cargo test --locked --all-targets --features jpeg
cargo run --locked --features jpeg -- generate \
  --profile extended --out generated/extended-jpeg --seed 1
cargo run --locked --features jpeg -- validate generated/extended-jpeg
```

See
[docs/external-codec-verification.md](docs/external-codec-verification.md) for
the required OpenJPH and DCMTK runtime verification cadence.

For downstream projects, see
[docs/corpus-consumption.md](docs/corpus-consumption.md) for the complete,
portable, and fast generation workflows; manifest handoff requirements; and the
scope boundary of the generated corpus.

The independent validation framework, tool matrix, and acceptance status are in
[conformance/README.md](conformance/README.md) and
[docs/conformance-acceptance.md](docs/conformance-acceptance.md). External tool
gaps are explicit failures; parser success is never substituted for IOD
validation.

The post-current-term implementation sequence for broader object-family,
pathology, codec, stress, robustness, media, and protocol coverage is in
[docs/coverage-expansion-plan.md](docs/coverage-expansion-plan.md).
The completed backend platform, native CT proof, external float32 Parametric
Map proof, and independent validation evidence are in
[docs/phase-1-proof-status.md](docs/phase-1-proof-status.md).
Independently verified Phase 2 native slices and their remaining milestone work
are tracked in
[docs/phase-2-native-status.md](docs/phase-2-native-status.md).

## Verification

GitHub Actions runs the default corpus and a separate matrix for every
in-process codec feature. External-command features receive compile coverage in
normal CI and require explicit runtime verification. Locally, the main
regression command is:

```sh
cargo test --locked --all-targets --no-default-features
```

## Design Principles

- Use the current DICOM standard as the authority for IODs, modules, attributes, SOP Class UIDs, and transfer syntax behavior.
- Use DICOM-rs for writing valid Part 10 files, starting with the latest verified `dicom` crate family version.
- Generate synthetic, deterministic, non-PHI data.
- Store expected behavior in machine-readable manifests next to generated output.
- Cover orthogonal compatibility axes, not only common happy-path examples.
- Keep `dcmview` and other viewers as consumers of this suite, not as constraints on what the suite can generate.

See [SYSTEM_SPEC.md](SYSTEM_SPEC.md) for the architecture and requirements and
[CURRENT_PROGRESS.md](CURRENT_PROGRESS.md) for detailed verification history.
