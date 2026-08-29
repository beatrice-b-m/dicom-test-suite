# dicom-test-suite

`dicom-test-suite` deterministically generates synthetic DICOM corpora for
viewer, parser, decoder, and interoperability testing. It produces DICOM Part
10 files together with a machine-readable manifest that states what each file
is intended to exercise, how it was generated, what it references, and which
checks passed.

The suite is standard-first and viewer-neutral. It deliberately combines common
clinical objects with compatibility boundaries that are easy for a narrow test
set to miss: classic and enhanced images, native and encapsulated pixels,
geometry and metadata variations, presentation and quantitative objects,
cross-instance reference graphs, pathology, waveforms, radiotherapy, malformed
inputs, and bounded scale cases.

Generated DICOM files are synthetic, deterministic, and non-PHI. They are not
committed; write them beneath an ignored path such as `generated/` or `out/`.

## Quick Start

Rust 1.85.0 is selected by `rust-toolchain.toml`. A default build needs no
external codec executable or Python environment.

```sh
# Inspect the cases selected by a profile.
cargo run --locked -- list-cases --profile smoke

# Generate a small, byte-stable corpus into a new directory.
cargo run --locked -- generate \
  --profile smoke --out generated/smoke --seed 1

# Verify the files against their manifest contracts.
cargo run --locked -- validate generated/smoke

# Summarize exactly what was generated and skipped.
cargo run --locked -- report \
  generated/smoke --format markdown > generated/smoke/coverage.md
```

The output directory must not already exist. Generation is staged and promoted
as a complete directory, and the result always includes `manifest.json`.

For caller-defined objects, the public composition slice currently qualifies
native monochrome and RGB Secondary Capture templates:

```sh
cargo run --locked -- templates list
cargo run --locked -- compose \
  --spec tests/fixtures/composition/valid/template-only.json \
  --out generated/composition-sc --seed 1
cargo run --locked -- validate generated/composition-sc
```

`compose` is standards-aware but does not project curated registry coverage.
See the [composition guide](docs/composition-guide.md) for raw pixels, typed
attribute operations, dry runs, resource limits, manifests, and evidence
boundaries.

For profile selection, optional codecs, negative/fuzz/stress workflows,
manifest consumption, validation levels, and troubleshooting, read the
[generation and usage guide](docs/generation-guide.md). For handing a generated
corpus to another project, read the
[corpus consumption guide](docs/corpus-consumption.md).

## What It Can Generate

The implemented registry covers these representative families:

- classic CR, CT, MR, mammography, DX, PET, NM, ultrasound, XA, XRF, and
  Secondary Capture images;
- Enhanced CT, MR, and PET multi-frame objects, including dimensions,
  concatenations, temporal information, and functional groups;
- native monochrome, color, palette, signed, unsigned, one-bit, 8/16/32-bit,
  planar, multi-frame, padding, overlay, LUT, ICC, spacing, and character-set
  variations;
- RLE Lossless, JPEG Baseline, JPEG-LS Lossless, JPEG XL, JPEG 2000, HTJ2K,
  legacy JPEG Lossless, Deflated Explicit VR, and Deflated Image Frame cases,
  subject to the feature/runtime requirements below;
- binary, fractional, labelmap, and WSI-referencing Segmentations; Parametric
  Maps; Real World Value Mapping; registration; presentation states; Key Object
  Selection; and multiple Structured Report forms;
- linked RT objects, ECG waveforms, Encapsulated PDF and STL, visible-light
  images, tiled WSI, sparse WSI, multi-resolution pyramids, and multiple optical
  paths;
- deterministic malformed instances in the isolated `negative` profile;
- reduced, resource-bounded large-object cases in the `stress` profile; and
- a bounded, payload-free parser robustness qualification in the `fuzz`
  profile.

The authoritative inventory is `cases/registry.json`, not this summary:

```sh
cargo run --locked -- list-cases
cargo run --locked -- list-cases --profile all
cargo run --locked -- list-cases --profile extended --status planned
cargo run --locked -- report gaps --format markdown
```

Planned and unavailable cases stay visible in manifests and reports. This is an
intentional coverage signal: the suite never silently treats a missing feature,
external backend, validator, media peer, or protocol peer as a passing case.

## Profiles

| Profile | Purpose | Included by `all` |
| --- | --- | --- |
| `smoke` | Three tiny, byte-stable Secondary Capture ingestion checks. | Yes |
| `core` | Common valid viewer-relevant native objects and dependency sources. | Yes |
| `extended` | Broad valid enhanced, compressed, derived, non-image, and VL coverage. | Yes |
| `legacy` | Valid retired or uncommon behavior. | No |
| `stress` | Reduced-scale large, deep, many-frame, many-instance, and encapsulation boundaries. | Only with `--include-stress` |
| `negative` | Deterministic expected-invalid mutations with explicit acceptable outcomes. | No |
| `fuzz` | Bounded reproducible robustness qualification; retains no DICOM payloads. | No |
| `all` | Union of `smoke`, `core`, and `extended`. | N/A |

Use separate output roots for `all`, `legacy`, `negative`, and `fuzz`. Prefer a
separate `stress` run for clearer resource evidence; `--profile all
--include-stress` is available when a combined valid corpus is useful.

## Optional Generation Capabilities

All codec features are disabled by default. Enable only what the intended
consumer needs, or use `--all-features` for the broadest valid corpus.

| Cargo feature | Generated coverage | Runtime command |
| --- | --- | --- |
| `jpeg` | JPEG Baseline 8-bit | None |
| `charls` | JPEG-LS Lossless | None beyond the build dependency |
| `jpegxl` | JPEG XL lossless and lossy | `cjxl` for the lossy case |
| `jpeg2000` | JPEG 2000 Lossless | None beyond the build dependency |
| `deflate` | Deflated dataset and Deflated Image Frame | None |
| `htj2k_openjph` | HTJ2K lossless and lossy | `ojph_compress` on `PATH` |
| `legacy_jpeg_dcmtk` | JPEG Lossless Process 14 and SV1 | `dcmcjpeg` on `PATH` |

The optional locked highdicom/pydicom backend generates float32 and float64
Parametric Maps, TID 1500 and SCOORD3D reports, and WSI tile Segmentation. It is
prepared explicitly; generation itself never downloads software or accesses
the network:

```sh
uv python install 3.12.12
uv sync --project generation-backends/highdicom-pydicom \
  --locked --no-editable --python 3.12.12
```

See the [backend README](generation-backends/highdicom-pydicom/README.md) and
[external codec verification policy](docs/external-codec-verification.md) for
exact versions, discovery, fingerprints, and licenses. Validate a corpus with
the same feature set used to generate its feature-gated compressed files.

## Validation And Evidence

`generate` performs recipe-specific checks before publishing an output root.
`validate` then reopens the root, validates `manifest.json`, hashes and parses
every retained instance, checks file/meta identities, pixel and encapsulation
contracts, references, profile isolation, and specialized object semantics.

```sh
cargo run --locked --all-features -- generate \
  --profile all --out generated/all --seed 1
cargo run --locked --all-features -- validate generated/all
cargo run --locked --all-features -- report \
  generated/all --format json > generated/all/coverage.json
```

These are strong same-project checks, not independent DICOM certification.
Independent conformance collection uses pinned external validators:

```sh
cargo run --locked -- conformance check-tools
cargo run --locked -- conformance run \
  generated/all --out reports/conformance/all
cargo run --locked -- conformance verify reports/conformance/all
```

Tool installation, exact-case routing, accepted-finding policy, and evidence
limitations are documented in [conformance/README.md](conformance/README.md).

## Command Map

```text
generate          Create one profile in a new output root.
compose           Create caller-specified objects in a new output root.
templates         List or describe qualified composition templates.
list-cases        Inspect registry selection, status, providers, and blockers.
validate          Strictly check a generated root against its manifest.
report            Render generated coverage or registry/standards gaps.
conformance       Discover, run, and verify independent validators.
interoperate      Qualify DICOMDIR media or report protocol availability.
standards         Check the standards lock and registry evidence gaps.
```

Run `cargo run --locked -- --help` and the relevant subcommand with `--help`
for the exact syntax. The complete examples and output interpretation are in
[docs/generation-guide.md](docs/generation-guide.md).

## Reproducibility And Scope

The seed controls deterministic identities and case data; it is not a request
for randomized clinical content. Byte-stable cases should reproduce when their
recorded inputs, toolchain, feature flags, and external backend identities are
the same. Semantic-stable external codec cases record the bounded comparison
appropriate to that codec instead of promising identical compressed bytes.

“All” means all cases selected from the implemented `smoke`, `core`, and
`extended` registry entries that are available in the current build and
runtime. It does not mean every DICOM Standard object, and it excludes legacy,
negative, fuzz, and (by default) stress scopes. Always preserve the manifest and
coverage report with downstream findings.

The architecture and normative project requirements are in
[SYSTEM_SPEC.md](SYSTEM_SPEC.md). Current implementation evidence is recorded in
the phase/status documents under `docs/`; they are historical engineering
records, not substitutes for the registry or a fresh generated report.
Use the [documentation map](docs/README.md) to distinguish current operating
guides from dated qualification and planning records.
The pathology and tiled-microscopy evidence is retained in
[docs/phase-4-pathology-status.md](docs/phase-4-pathology-status.md), including
the next explicit full-size
pyramid checkpoint and its promotion boundary.

## Development

The main regression command is:

```sh
cargo test --locked --all-targets --no-default-features
```

Contributors and coding agents must follow [AGENTS.md](AGENTS.md), including its
granular commit policy. Generated corpora belong under ignored output paths and
must not be committed.
