# synth-dicom-gen

`synth-dicom-gen` deterministically generates synthetic DICOM corpora for
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

The renamed `0.2.0` product does not yet have a qualified release archive.
The dated [standalone product status](docs/standalone-product-status-2026-08-31.md)
applies only to the immutable historical `0.1.0` candidate and must not be
reused as evidence for this name. Once a target-specific `0.2.0` archive has a
new dated qualification row, keep its adjacent `.sha256` file and use the
following naming and verification flow; do not infer support from the example
target:

```sh
ARCHIVE=synth-dicom-gen-0.2.0-aarch64-apple-darwin.tar.gz
shasum -a 256 -c "$ARCHIVE.sha256"
tar -xzf "$ARCHIVE"
GENERATOR="$PWD/${ARCHIVE%.tar.gz}/bin/synth-dicom-gen"
"$GENERATOR" version --format json
"$GENERATOR" capabilities --format json
```

The default installed binary needs no external codec executable or Python
environment for the smoke workflow:

```sh
# Inspect the cases selected by a profile.
"$GENERATOR" list-cases --profile smoke

# Generate a small, byte-stable corpus into a new directory.
"$GENERATOR" generate \
  --profile smoke --out generated/smoke --seed 1

# Verify the files against their manifest contracts.
"$GENERATOR" validate generated/smoke

# Summarize exactly what was generated and skipped.
"$GENERATOR" report \
  generated/smoke --format markdown > generated/smoke/coverage.md
```

The output directory must not already exist. Generation is staged and promoted
as a complete directory, and the result always includes `manifest.json`.

For automation, discover the versioned contract and request JSON explicitly:

```sh
"$GENERATOR" version --format json
"$GENERATOR" capabilities --format json
"$GENERATOR" generate \
  --profile smoke --out generated/smoke-machine --seed 1 --format json
```

Machine success uses one versioned envelope on stdout; failure uses one stable
error envelope on stderr and exit class `2` through `6`. Human output is not an
automation contract. Historical report JSON stays raw unless the caller adds
`--cli-api 1.0.0`, which wraps the unchanged report at `result.report`.

Rust consumers should use the narrow supported `synth_dicom_gen::sdk` facade;
see the [Rust SDK guide](docs/sdk-guide.md). Existing public implementation
modules remain visible during migration but are not standalone compatibility
surfaces, as recorded by the
[dated Rust API audit](docs/rust-api-compatibility-audit-2026-08-31.md).

All three public generation workflows use one plan-first spine. `generate`
resolves registry-selected, versioned case recipes into an immutable
`CorpusPlan`; `compose` resolves caller specifications and qualified templates
into the same model; `assemble` resolves bounded caller-owned element trees and
typed bulk without assigning IOD or coverage claims. One bounded executor then schedules the dependency graph,
materializes Part 10 through the shared writer, validates, projects the
frontend-specific manifest, cleans private assets, and atomically publishes.
Curated case coverage and composition template evidence remain deliberately
separate. Generated manifest entries expose corpus-plan and resolved-instance
hashes so the construction provenance is auditable without reopening a file as
a planning input.

For caller-defined objects, the Phase P8 composition platform qualifies every
currently implemented valid DICOM SOP Class through a template or deterministic
bundle, with bounded typed content models:

```sh
"$GENERATOR" templates list
"$GENERATOR" templates reference --format markdown
"$GENERATOR" compose \
  --spec "${ARCHIVE%.tar.gz}/examples/compose-raw-grayscale.json" \
  --out generated/composition-sc --seed 1
"$GENERATOR" validate generated/composition-sc
```

`compose` is standards-aware but does not project curated registry coverage.
Specs can use deterministic defaults, local files, small inline fixtures,
fingerprinted offline providers, and—for the qualified XA/XRF RLE contract—
pre-encoded frames. See the [composition guide](docs/composition-guide.md) and generated
[template reference](docs/composition-template-reference.md) for raw pixels,
waveforms, quantitative data, documents, meshes, structured references, RT
graphs, typed attribute operations, transfer syntaxes, dry runs, resource
limits, manifests, and evidence boundaries.
External CLI, Rust API, cancellation, provider, bounded-memory, and
reproducibility integration is documented in the
[composition integration guide](docs/composition-integration-guide.md).

When no qualified template matches, structural assembly can place arbitrary
supported standard, explicit-VR unknown, managed private, recursive Sequence,
and typed bulk values:

```sh
"$GENERATOR" assemble \
  --request "${ARCHIVE%.tar.gz}/examples/assemble-structural.json" \
  --out generated/structural --seed 1
"$GENERATOR" validate generated/structural
```

Structural output always records `iod_conformance = "not_assessed"`; it cannot
be counted as curated case coverage or qualified-template evidence. See the
[structural assembly guide](docs/assembly-guide.md) for the versioned request,
dry-run, caller-asset, resource, manifest, CLI, and SDK contracts.

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
"$GENERATOR" list-cases
"$GENERATOR" list-cases --profile all
"$GENERATOR" list-cases --profile extended --status planned
"$GENERATOR" report gaps --format markdown
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
  --locked --no-editable --compile-bytecode --python 3.12.12
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
Manifest dispatch is fail closed before those semantic checks: curated
manifest readers accept exactly `0.2.0`, `0.3.0`, and `1.0.0`, with a required
schema-valid split identity projection in `1.0.0`; supported composition and
structural-assembly manifests use the same version-aware loader.

```sh
"$GENERATOR" generate \
  --profile all --out generated/all --seed 1
"$GENERATOR" validate generated/all
"$GENERATOR" report \
  generated/all --format json > generated/all/coverage.json
```

These are strong same-project checks, not independent DICOM certification.
Independent conformance collection uses pinned external validators:

```sh
"$GENERATOR" conformance check-tools
"$GENERATOR" conformance run \
  generated/all --out reports/conformance/all
"$GENERATOR" conformance verify reports/conformance/all
```

Tool installation, exact-case routing, accepted-finding policy, and evidence
limitations are documented in [conformance/README.md](conformance/README.md).

## Command Map

```text
version           Report product, target, feature, and resource identity.
capabilities      Discover supported versions and live availability.
generate          Create one profile in a new output root.
compose           Create caller-specified objects in a new output root.
assemble          Create structurally checked, no-IOD-claim objects.
templates         List or describe qualified composition templates.
list-cases        Inspect registry selection, status, providers, and blockers.
validate          Strictly check a generated root against its manifest.
report            Render generated coverage or registry/standards gaps.
conformance       Discover, run, and verify independent validators.
interoperate      Qualify DICOMDIR media or report protocol availability.
standards         Check the standards lock and registry evidence gaps.
```

Run `"$GENERATOR" --help` and the relevant subcommand with `--help`
for the exact syntax. The complete examples and output interpretation are in
[docs/generation-guide.md](docs/generation-guide.md).

## Reproducibility And Scope

The seed controls deterministic identities and case data; it is not a request
for randomized clinical content. Byte-stable cases should reproduce when their
recorded inputs, toolchain, feature flags, and external backend identities are
the same. Semantic-stable external codec cases record the bounded comparison
appropriate to that codec instead of promising identical compressed bytes.

The product release version and the byte-stable DICOM payload contract are
separate identities. Release `0.2.0` reports `0.2.0` in discovery, manifests,
runtime evidence, packages, and releases, while unchanged built-in
`byte_stable` DICOM Implementation Class UIDs and Software Versions remain
bound to payload contract `0.1.0`. Semantic-stable external highdicom SR and
quantitative imports retain the current product/backend version because their
external bytes are not part of the byte-stable promise.

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

Repository development uses the pinned Rust 1.85.0 toolchain. These commands
are contributor workflows, not consumer installation instructions:

```sh
cargo test --locked --all-targets --no-default-features
python3 scripts/route-changed-tests.py --path src/sdk.rs --dry-run
cargo run --locked -- version --format json
cargo run --locked -- generate --profile smoke --out generated/dev-smoke --seed 1
```

The broad Cargo command is the ordinary regression suite; it skips six
explicitly ignored heavyweight corpus entries. Run an affected heavy slice via
`scripts/run-heavy-qualification.sh byte-parity`, `all-profile`, `wsi`, or
`stress`. Scheduled Nightly and exact release-candidate qualification use
`scripts/run-heavy-qualification.sh all` once after the ordinary suite.
Fast CI always runs its two contract harnesses, then uses the fail-closed
change router to execute only list-proven ordinary Fast/subsystem bundles. Its
JSON output keeps feature codecs, native providers, explicit heavy entries,
package/release work, and the future independent corpus workflow visible as
deferred evidence rather than implying they passed.

Contributors and coding agents must follow [AGENTS.md](AGENTS.md), including its
granular commit policy. Generated corpora belong under ignored output paths and
must not be committed.
