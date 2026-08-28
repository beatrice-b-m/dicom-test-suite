# Viewer Testing Handoff

## Baseline identity

- Source implementation commit:
  `b4ea0a450b63408f9f62709691b50dd50cb64594`
- Seed: `1`
- Generated corpus root: `generated/viewer-baseline-b4ea0a4/`
- Host: macOS 26.5.1 (build 25F80), arm64
- Rust: `rustc 1.85.0 (4d91de4e4 2025-02-17)`
- Cargo: `cargo 1.85.0 (d73d2caf9 2024-12-31)`
- Rust target recorded by the manifests: `aarch64-apple-darwin`
- Base corpus features: none (`--no-default-features`)
- Feature corpus features, one per isolated root: `jpeg`, `charls`, `jpegxl`,
  `jpeg2000`, and `deflate`
- Registry inventory: 150 implemented and 32 planned logical cases

The source commit identifies the implementation used to generate every corpus
in this handoff. Documentation-only commits made after generation do not change
the generator or generated evidence.

## Corpus inventory

Every root below contains `manifest.json`, `coverage.json`, and `coverage.md`.
The generated count agrees with both the manifest `files` length and the number
of `.dcm` files on disk. `Unavailable` is the report's `skipped` count for
implemented optional cases; planned rows are shown separately. Every report
has zero blocked rows.

| Corpus | Enabled features | Generated | Unavailable | Planned | Validation failures |
| --- | --- | ---: | ---: | ---: | ---: |
| `generated/viewer-baseline-b4ea0a4/smoke/` | none | 3 | 0 | 0 | 0 |
| `generated/viewer-baseline-b4ea0a4/smoke-repro/` | none | 3 | 0 | 0 | 0 |
| `generated/viewer-baseline-b4ea0a4/core/` | none | 49 | 0 | 0 | 0 |
| `generated/viewer-baseline-b4ea0a4/extended/` | none | 113 | 9 | 13 | 0 |
| `generated/viewer-baseline-b4ea0a4/stress/` | none | 3 | 0 | 5 | 0 |
| `generated/viewer-baseline-b4ea0a4/extended-jpeg/` | `jpeg` | 114 | 8 | 13 | 0 |
| `generated/viewer-baseline-b4ea0a4/extended-charls/` | `charls` | 114 | 8 | 13 | 0 |
| `generated/viewer-baseline-b4ea0a4/extended-jpegxl/` | `jpegxl` | 114 | 8 | 13 | 0 |
| `generated/viewer-baseline-b4ea0a4/extended-jpeg2000/` | `jpeg2000` | 114 | 8 | 13 | 0 |
| `generated/viewer-baseline-b4ea0a4/extended-deflate/` | `deflate` | 115 | 7 | 13 | 0 |

The two smoke roots were compared with:

```sh
diff -r generated/viewer-baseline-b4ea0a4/smoke \
  generated/viewer-baseline-b4ea0a4/smoke-repro
```

The command exited successfully with no differences. The two manifest SHA-256
values are both
`aa996cab9b052915ccb12584d0af201cf34abf5a494e68dfaa8907eb6baf8ffa`.

The prepared highdicom/pydicom runtime was available and generated both
Parametric Map cases in the extended roots. The manifests record CPython
3.12.12, highdicom 0.28.1, pydicom 3.0.2, backend version 0.5.0,
environment fingerprint
`e9e04d18283f71acc9476d6d873638c33d77b340ed0780f70ab727ddc44d7d2f`,
entrypoint fingerprint
`10a8ebce9cc39e76ccec93995f6516b9d495f8dec7ade53dfaee4f817c6e8194`,
and executable fingerprint
`cf450e6bc0b00adecd12b7b13024de7000c7350801addc802bd3b45782104e79`.

## Optional cases

The no-feature extended report explicitly marks these nine implemented cases
unavailable because their Cargo features were not enabled:

- `classic/sc/mono2_u16_htj2k_lossless`
- `classic/sc/mono2_u16_jpeg2000_lossless`
- `classic/sc/mono2_u16_jpeg_lossless_process_14`
- `classic/sc/mono2_u16_jpeg_lossless_sv1`
- `classic/sc/mono2_u8_deflated_explicit_le`
- `classic/sc/mono2_u8_jpeg_ls_lossless`
- `classic/sc/rgb_planar0_jpeg_baseline_8bit`
- `classic/sc/rgb_planar0_jpegxl_lossless`
- `derived/seg/binary_multiframe_deflated_image_frame`

Each in-process feature root promotes only its corresponding case or cases.
The remaining unavailable rows stay explicit. The two OpenJPH/DCMTK external
codec features were compile-checked only, so their dependent cases are not
claimed as runtime-requalified by this run.

## Repository and component gates

The repository gate passed at the source implementation commit:

```sh
cargo fmt -- --check
jq empty cases/registry.json schemas/*.json transfer-syntax/*.json standards.lock.json
cargo test --locked --all-targets --no-default-features
cargo run --locked --no-default-features -- standards check-lock
```

Results:

- formatting and every selected JSON document passed;
- the Rust library target passed 359 tests with 2 intentionally ignored, and
  every integration target passed;
- the standards lock returned `status ok`, with eight already documented
  warnings for unavailable local standard-artifact or KB fingerprints.

The locked offline Python component commands and exact results were:

```sh
uv run --project generation-backends/highdicom-pydicom \
  --locked --offline python -m unittest discover \
  -s generation-backends/highdicom-pydicom/tests
# 17 passed

UV_CACHE_DIR=/private/tmp/dts-uv-cache \
  uv run --locked --offline python -m unittest discover -s tests
# Run from conformance-backends/wsi-reconstruction: 25 passed

uv run --project conformance-backends/dicom-validator \
  --locked --offline pytest -q conformance-backends/dicom-validator/tests
# 21 passed, 5 skipped, 21 subtests passed
```

The five in-process codec commands ran independently and passed all targets:

```sh
cargo test --locked --all-targets --no-default-features --features jpeg
cargo test --locked --all-targets --no-default-features --features charls
cargo test --locked --all-targets --no-default-features --features jpegxl
cargo test --locked --all-targets --no-default-features --features jpeg2000
cargo test --locked --all-targets --no-default-features --features deflate
```

The feature-specific library totals were 360 passed and 2 ignored for `jpeg`,
and 362 passed and 2 ignored for each of `charls`, `jpegxl`, `jpeg2000`, and
`deflate`. Every integration target also passed.

The two external-command feature paths compiled without runtime execution:

```sh
cargo test --locked --all-targets --no-default-features \
  --features htj2k_openjph --no-run
cargo test --locked --all-targets --no-default-features \
  --features legacy_jpeg_dcmtk --no-run
```

Both compile-only commands passed.

## External codec identity

The installed external codec commands were fingerprinted before any possible
use. Their generated output was not used in this baseline.

| Feature | Executable | Version | SHA-256 |
| --- | --- | --- | --- |
| `htj2k_openjph` | `/opt/homebrew/bin/ojph_compress` | OpenJPH 0.27.3 (Homebrew package identity; the executable has no portable version flag) | `d21a8ea98ffce347928c34a2c51c61e424a068ca4eb746a6867a29d6c30b1627` |
| `legacy_jpeg_dcmtk` | `/opt/homebrew/bin/dcmcjpeg` | DCMTK 3.7.0, 2025-12-15 | `28707b3dd7dcbd0b2f710ae691602c07c460bf9917d9b944da7cfa052095b120` |

## External conformance inventory

`cargo run --locked --no-default-features -- conformance check-tools` reported
9 available, 10 absent, and 2 misconfigured adapter entries.

Available:

- `dicom3tools-dciodvfy`
- `dicom3tools-dciodvfy-wsi-sparse-characterization`
- `dicom3tools-dcentvfy`
- `dcmtk-dcmdump`
- `dcmtk-dcmdrle`
- `dcmtk-dcm2img-u1`
- `dcmtk-dcm2img-rt-image`
- `dcmtk-dcm2img-visible-light`
- `dcmtk-dcmdjpeg`

Absent:

- `pydicom-dicom-validator-wsi-sparse`
- `pydicom-dicom-validator-u32`
- `pydicom-dicom-validator-registration`
- `pydicom-dicom-validator-presentation-state`
- `pydicom-dicom-validator-rt`
- `pydicom-dicom-validator-rt-radiation`
- `pydicom-dicom-validator-waveform`
- `pydicom-dicom-validator-visible-light`
- `pydicom-dicom-validator-wsi-tile-segmentation`
- `highdicom-wsi-reconstruction`

Misconfigured:

- `pixelmed-sr-validator`: Java was found, but the configured PixelMed
  artifacts were unavailable.
- `littlecms-transicc-icc`: `transicc` was found, but its configured lock
  identity was unavailable.

The available inventory supports exploratory viewer testing. Cases dependent
on absent or misconfigured adapters must not be described as independently
requalified by this run.

## Generation and acceptance procedure

Each root was created from a nonexistent path with seed 1. Base profiles used:

```sh
cargo run --locked --no-default-features -- generate \
  --profile PROFILE --out generated/viewer-baseline-b4ea0a4/ROOT --seed 1
cargo run --locked --no-default-features -- validate \
  generated/viewer-baseline-b4ea0a4/ROOT
```

`PROFILE`/`ROOT` pairs were `smoke`/`smoke`, `smoke`/`smoke-repro`,
`core`/`core`, `extended`/`extended`, and `stress`/`stress`. Stress was selected
directly and was not broadened into `all`.

Feature roots used the same commands with one
`--features jpeg|charls|jpegxl|jpeg2000|deflate` option and the `extended`
profile. Every root then received reports with:

```sh
cargo run --locked --no-default-features [--features FEATURE] -- report \
  generated/viewer-baseline-b4ea0a4/ROOT --format json \
  > generated/viewer-baseline-b4ea0a4/ROOT/coverage.json
cargo run --locked --no-default-features [--features FEATURE] -- report \
  generated/viewer-baseline-b4ea0a4/ROOT --format markdown \
  > generated/viewer-baseline-b4ea0a4/ROOT/coverage.md
```

All generated files, manifests, reports, caches, and environments remain
ignored and uncommitted.

## Scope warning and exclusions

Viewer behavior measures compatibility, not DICOM conformance. A viewer pass
does not prove that an object conforms to the DICOM Standard, and a viewer
failure does not by itself prove nonconformance.

This handoff deliberately excludes lossy and video breadth, large-scale stress,
negative and fuzz profiles, media, DIMSE, and DICOMweb. It does not implement
planned registry rows, broaden profiles, enable stress through `all`, begin
Phases 5–8, or claim that the entire coverage-expansion plan is complete.
