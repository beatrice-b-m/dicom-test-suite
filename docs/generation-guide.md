# Generating Representative DICOM Test Corpora

This guide explains how to choose, generate, validate, inspect, and consume the
suite's outputs. It is intended for viewer developers, parser and codec authors,
QA engineers, and agents automating compatibility work.

## 1. What “Representative” Means

This project does not sample patient data and does not claim statistical
representativeness of clinical populations. It creates small, synthetic cases
that deliberately span independent DICOM compatibility axes:

- **Object model:** classic and enhanced images, derived images, quantitative
  objects, presentation states, registration, SR, RT, waveform, encapsulated
  documents, mesh, visible light, and WSI.
- **Pixel model:** monochrome polarity, RGB and palette color, planar layout,
  signedness, stored/high-bit boundaries, one-bit packing, 8/16/32-bit samples,
  integer and floating pixels, multiple frames, padding, and overlays.
- **Encoding:** Explicit and Implicit VR Little Endian, Explicit VR Big Endian,
  native and encapsulated Pixel Data, RLE, multiple JPEG families, JPEG 2000,
  HTJ2K, JPEG XL, dataset deflate, and Deflated Image Frame.
- **Geometry and time:** slice ordering, gantry tilt, oblique orientation,
  non-uniform spacing, shared Frame of Reference, dimensions, concatenations,
  temporal positions, and per-frame functional groups.
- **Metadata:** empty Type 2 values, value-length boundaries, private creators,
  sequence lengths, UTF-8 and ISO 2022 names, timezone boundaries, ICC profiles,
  LUTs, and lossy-compression declarations.
- **Relationships:** source/derived references, CT/SEG/SR chains, registration,
  linked RT instances, WSI pyramids and tile references, and cross-instance
  identity closure.
- **Robustness and scale:** deterministic invalid encodings, bounded parser
  mutation, deep sequences, many values, many instances/frames/fragments, large
  native payloads, and reduced WSI pyramids.

Cases are intentionally orthogonal where practical. Test conclusions should be
made per `case_id`, not inferred from a modality name alone.

### Unified plan-first execution

Every retained DICOM artifact is declared before file creation. Registry
Registry selection, caller-authored qualified composition, and caller-authored
structural assembly are separate frontends, but all three
produce the same versioned `CorpusPlan` and use the same bounded DAG executor,
content/codec services, `Part10Materializer`, validation evidence model,
private staging, and atomic no-overwrite publication transaction. Native
recipes cannot import a temporary generated DICOM file back into a plan.
Expected-invalid files use typed mutations of private plan-first sources; fuzz
publishes qualification evidence only. The only full-file import path is a
named external provider boundary with locked request, tool, output, resource,
and semantic evidence.

## 2. Inspect Before Generating

The registry is the authority for available and planned coverage. Its CLI view
is tab-separated for people. Automation should use the versioned JSON result:

```sh
cargo run --locked -- list-cases
cargo run --locked -- list-cases --profile all
cargo run --locked -- list-cases \
  --profile extended --status planned
cargo run --locked -- list-cases \
  --profile extended --status planned --format json
cargo run --locked -- report gaps --format json > coverage-gaps.json
cargo run --locked -- report gaps --format markdown
```

Important columns include `case_id`, registry `status`, profile membership, SOP
Class and transfer syntax, standards-evidence coverage, artifact kind, provider,
object family, roadmap priority, and blocker codes. `implemented` means a recipe
exists; it can still be unavailable in a particular run when its feature or
external runtime is absent.

### Machine CLI contract

Discover the executable contract before selecting a request:

```sh
cargo run --locked -- version --format json
cargo run --locked -- capabilities --format json
```

Every automation command uses `--format json`. Success writes one object to
stdout and nothing to stderr; failure writes one error object to stderr and
nothing to stdout. Both carry `cli_api_version = "1.0.0"`, a stable command
name, and `status`. Exit classes are `2` for request/syntax errors, `3` for
unavailable capability, `4` for path or resource conflicts, `5` for generation
or evidence failure, and `6` for unexpected product I/O/internal failure.
Error codes are append-only and published in `product/cli-error-codes.json`.

Generation, qualified composition, structural assembly, and their dry-runs share typed file-producing
fields for the requested root, optional manifest, run and schema versions,
seed, product version, emitted count/bytes, unavailable summaries, corpus-plan
hash, publication state, validation state, and plan preview. For example:

```sh
cargo run --locked -- compose --spec request.json --out generated/preview \
  --dry-run --format json
cargo run --locked -- compose --spec request.json --out generated/result \
  --format json
cargo run --locked -- validate generated/result --format json
```

Validation and generated-root reporting first dispatch `manifest.json`
through one schema/version-aware loader. Curated readers accept exactly
`0.2.0`, `0.3.0`, and `1.0.0`; `1.0.0` requires its complete, schema-valid
split identity projection. Unknown versions and malformed current identities
fail before semantic validation or coverage reporting. The same loader
recognizes the supported composition and structural-assembly manifest versions.

Use `assemble --request assembly.json` when the requested data-element tree or
typed bulk has no qualified template. Its manifest and report are intentionally
separate from coverage and always state `iod_conformance = "not_assessed"`.
The complete request, private/Sequence/pixel example, and asset security model
are in the [structural assembly guide](assembly-guide.md).

Historical report JSON is the compatibility exception: `--format json` alone
returns the raw report. Add `--cli-api 1.0.0` to receive the common envelope;
the unchanged raw object is then at `result.report`.

During corpus separation, the supported Rust SDK verified-corpus runner can produce
external manifest `2.0.0`. Raw `report <root> --format json` and `--format markdown`
accept that manifest without reopening its payloads or consulting the embedded
registry. The resulting `coverage_report_schema_version = "2.0.0"`,
`report_kind = "external_corpus"` report retains the complete source manifest,
captured definitions and identities, with separate logical-case and artifact
counts. Reporting performs no new validation or independent conformance
assessment; recorded source evidence is preserved without upgrading its claims.
The [SDK corpus request](sdk-guide.md#generate-a-verified-caller-owned-corpus)
and `generate --corpus` support this external contract using explicit
descriptor/member inputs. External report envelopes use report-result `2.0.0`;
legacy report kinds keep report-result `1.0.0` unchanged.

### Generate a caller-owned definition bundle

Given a verified bundle1 descriptor `definition.json` and its dedicated member
directory `corpus-members`, these commands run from the caller's directory;
no repository or sibling-path lookup is used:

```sh
synth-dicom-gen generate --corpus ./definition.json --asset-root corpus-members \
  --profile smoke --out generated/cli --seed 1 --parallelism 2 --format json
synth-dicom-gen validate generated/cli --format json
synth-dicom-gen report generated/cli --format json --cli-api 1.0.0
```

`--asset-root` is mandatory and contains **all** declared registry, recipe,
evidence and asset members, not only binary assets. It is independent of the
descriptor's parent. File paths need an explicit parent (`./definition.json`,
not bare `definition.json`); all path ancestors must satisfy the loader's
no-symlink policy. The fixed bundle1 profile and closure rules remain unchanged.

Use repeated `--case-id ID` with an explicit profile to select direct members
within that scope; required dependencies are added separately. For example:

```sh
synth-dicom-gen generate --corpus ./definition.json --asset-root corpus-members \
  --profile all --case-id derived/registration/spatial_ct_pair \
  --out generated/ids --format json
synth-dicom-gen generate --corpus ./definition.json --asset-root corpus-members \
  --profile smoke --out generated/dry --dry-run --format json
```

External generation JSON uses CLI API `1.0.0` with generation-result `3.0.0`.
`--cli-api 1.0.0` also selects JSON when `--format` is omitted.
Its `outcome` is `published`, `planned`, or `no_executable_cases`. Only published
has a manifest path and passed generation-time validation. Nonpublished
outcomes have null manifest path, zero emitted-file count/bytes, and publication
and validation `not_run`. Dry-run remains `planned` even when no case can run;
no-executable is never reported as an empty generated corpus. Preview `ready`
is not `generated`. Every result retains exact selected/dependency definitions,
reasons, verified identities, selector and plan hash; file counts are distinct
from logical case counts. No private plan JSON is exposed.

Formats/options are checked before generation, existing destinations are never
overwritten, and SDK error codes cross into CLI errors without parsing prose.
Native/compiled support only is currently executable; unavailable providers
remain explicit. SDK cooperative cancellation exists, but this CLI does not
install a signal handler and makes no graceful SIGINT-cleanup claim.
Capabilities `2.0.0` retains its frozen pre-external result-validation window;
loaded-corpus and complete generation3/report2 discovery is deferred to the
next versioned discovery slice. Use these exact contracts in the interim,
not an inferred capability claim. Embedded `generate` without `--corpus`
continues to emit generation-result `2.0.0` and curated manifest `1.0.0`.

## 3. Select A Profile

### Valid file corpora

- `smoke` is the quickest ingestion sanity check: three tiny, byte-stable
  Secondary Capture files covering MONOCHROME1, MONOCHROME2, and RGB.
- `core` targets common valid viewer behavior and includes dependency source
  objects used by related cases.
- `extended` expands into enhanced multi-frame, compressed, derived,
  quantitative, non-image, visible-light, pathology, and less-common metadata
  behavior. It also reports planned cases that are not yet emitted.
- `legacy` is a separate opt-in valid corpus for retired or uncommon encoding.
- `all` is the union of `smoke`, `core`, and `extended`; it excludes `legacy`,
  `negative`, and `fuzz`, and excludes `stress` unless explicitly requested.

### Specialized profiles

- `stress` emits the qualified reduced-scale resource boundaries. It is valid
  DICOM, but its files may be large or numerous. Full-scale jobs—including a
  genuine greater-than-4-GiB Extended Offset Table object—remain unavailable.
- `negative` emits deterministic expected-invalid instances. They are isolated
  from valid coverage and carry mutation provenance, changed-byte ranges,
  expected failure layers, and bounded acceptable outcomes.
- `fuzz` executes a deterministic, bounded same-project parser qualification.
  It removes sources and candidates before promotion, so the output contains a
  manifest and qualification record but no DICOM payloads.

Use a fresh output root per profile. Combining materially different scopes
makes downstream pass/fail claims harder to interpret.

### Select individual embedded cases

Subsystem qualification can restrict generation to repeatable `--case-id`
arguments while retaining the profile as a compatibility boundary:

```sh
cargo run --locked --features deflate -- generate \
  --profile extended \
  --case-id classic/sc/mono2_u8_deflated_explicit_le \
  --case-id derived/seg/binary_multiframe_deflated_image_frame \
  --out generated/deflate-selected --seed 1
```

Every requested ID must be known, unique, and selectable by the named profile;
invalid requests fail before publication. Ordering the arguments differently
does not change the deterministic output. Required recipe dependencies are
expanded by the same curated planner, and generation still uses the ordinary
bounded executor, validation, and atomic no-overwrite publication path.

The resulting manifest is the selection evidence: every requested ID must
appear in `files` or in `skipped_cases` with an explicit unavailable outcome.
Dependency cases may also appear in `files`. Consumers must not interpret an
unavailable row as generated or passing. Omitting `--case-id` preserves the
existing full-profile behavior. This embedded-corpus selector is intended for
bounded qualification and does not define the later external corpus-definition
API.

## 4. Prepare Optional Capabilities

The default build generates all selected Rust-native, feature-free cases and
records the rest in `skipped_cases`.

### Cargo codec features

```sh
cargo build --locked --all-features
cargo run --locked --features jpeg,deflate -- \
  list-cases --profile extended
```

| Feature | Case family | Additional runtime |
| --- | --- | --- |
| `jpeg` | JPEG Baseline | None |
| `charls` | JPEG-LS Lossless | None beyond linked build dependency |
| `jpegxl` | JPEG XL lossless/lossy | `cjxl` is required for lossy generation |
| `jpeg2000` | JPEG 2000 Lossless | None beyond linked build dependency |
| `deflate` | Deflated Explicit VR and Deflated Image Frame | None |
| `htj2k_openjph` | HTJ2K lossless/lossy | `ojph_compress` |
| `legacy_jpeg_dcmtk` | JPEG Lossless Process 14/SV1 | `dcmcjpeg` |

The heavy codec matrix uses these representative case selections:

| Feature | Selected case IDs |
| --- | --- |
| `jpeg` | `classic/sc/rgb_planar0_jpeg_baseline_8bit` |
| `charls` | `classic/sc/mono2_u8_jpeg_ls_lossless` |
| `jpegxl` | `classic/sc/rgb_planar0_jpegxl_lossless`, `classic/sc/rgb_jpegxl_lossy` |
| `jpeg2000` | `classic/sc/mono2_u16_jpeg2000_lossless` |
| `deflate` | `classic/sc/mono2_u8_deflated_explicit_le`, `derived/seg/binary_multiframe_deflated_image_frame` |
| `htj2k_openjph` | `classic/sc/mono2_u16_htj2k_lossless`, `classic/sc/mono2_u16_htj2k_lossy` |
| `legacy_jpeg_dcmtk` | `classic/sc/mono2_u16_jpeg_lossless_process_14`, `classic/sc/mono2_u16_jpeg_lossless_sv1` |

The external-command rows remain explicit unavailable evidence when their
pinned executable is absent; compilation alone is not a codec pass.

External commands are discovered and fingerprinted. A feature flag does not
make a missing executable available, and generation never installs one.

### Locked highdicom/pydicom backend

The optional Python backend provides float32 and float64 Parametric Maps,
TID 1500 and SCOORD3D Structured Reports, and a WSI tile-referencing
Segmentation. Prepare it once from the repository root:

```sh
uv python install 3.12.12
uv sync --project generation-backends/highdicom-pydicom \
  --locked --no-editable --compile-bytecode --python 3.12.12
```

Set `DTS_HIGHDICOM_PYTHON` only when using an environment outside the backend's
default `.venv`. Discovery rejects a runtime whose interpreter, ABI, packages,
installed files, or entrypoint does not match the lock. If the backend is not
prepared, generation still completes and records an
`external_backend_unavailable` skipped row.

### Qualified adapter spellings retained during the product rename

The following 12 environment names belong to locked external adapters, not to
the product CLI. Their exact spellings remain part of qualified runtime and
fingerprint provenance until each adapter is independently renamed and
requalified:

- `DTS_BACKEND_DEPENDENCY_LOCK_SHA256`
- `DTS_BACKEND_ENVIRONMENT_FINGERPRINT`
- `DTS_BACKEND_EXECUTABLE_FINGERPRINT`
- `DTS_BACKEND_OUTPUTS`
- `DTS_BACKEND_REQUEST`
- `DTS_BACKEND_RESPONSE`
- `DTS_DICOM_VALIDATOR_PYTHON`
- `DTS_DICOM_VALIDATOR_STANDARD_HOME`
- `DTS_HIGHDICOM_PYTHON`
- `DTS_LCMS_HOME`
- `DTS_PIXELMED_HOME`
- `DTS_WSI_RECONSTRUCTION_PYTHON`

`SYNTH_DICOM_GEN_M6_SEGMENTATION_FIXTURE` is product-controlled and selects
the M6 qualification fixture; it is not a retained adapter spelling.

Likewise, locked module and command identities such as
`dts_highdicom_backend`, `dts_dicom_validator_adapter`,
`dts_wsi_reconstruction`, and `dts-wsi-reconstruct` are retained adapter
provenance. They are not aliases for the `synth-dicom-gen` executable. Missing
adapter qualification remains unavailable evidence; the rename does not imply
that those runtimes have been requalified.

## 5. Generate

The command requires a profile and a new output directory:

```sh
cargo run --locked -- generate \
  --profile core --out generated/core-seed-1 --seed 1
```

For the broadest currently implemented valid coverage:

```sh
cargo run --locked --all-features -- generate \
  --profile all --out generated/all-seed-1 --seed 1
cargo run --locked --all-features -- generate \
  --profile legacy --out generated/legacy-seed-1 --seed 1
```

Generate specialized scopes independently:

```sh
cargo run --locked -- generate \
  --profile negative --out generated/negative-seed-1 --seed 1
cargo run --locked -- generate \
  --profile fuzz --out generated/fuzz-seed-1 --seed 1
cargo run --locked -- generate \
  --profile stress --out generated/stress-seed-1 --seed 1
```

`--profile all --include-stress` adds stress-profile selections to the ordinary
`all` union. It does not add legacy, negative, or fuzz cases.

The generator refuses an existing path to prevent mixing runs. It writes to a
private staging root, performs generation-time checks, writes the manifest, and
then promotes the completed directory. A failed run does not constitute a
usable corpus.

### Compose caller-defined objects

Composition is a separate workflow from registry-selected generation. Inspect
qualified descriptors and create a default Secondary Capture object with:

```sh
cargo run --locked -- templates list
cargo run --locked -- templates reference --format markdown
cargo run --locked -- compose \
  --spec tests/fixtures/composition/valid/template-only.json \
  --out generated/composition-sc --seed 1
cargo run --locked -- validate generated/composition-sc
cargo run --locked -- report generated/composition-sc --format json
cargo run --locked -- report generated/composition-sc \
  --format json --cli-api 1.0.0
```

Composition roots use `run.kind = "composition"` and composition entries. Their
reports group templates and transfer syntaxes; they do not claim a registry
profile or `case_id` coverage. The qualified public scope covers every currently
implemented valid DICOM SOP Class through a catalog template or deterministic
bundle, including enhanced/WSI, derived quantitative and SR, RT, waveform,
document, and mesh families. Template descriptors expose deterministic default,
local-file, small-inline-fixture, and offline-provider sources. XA/XRF also
expose caller-supplied RLE frames under exact hash and independent-decode
checks. Read [the composition guide](composition-guide.md) for the specification
format and [the rendered template reference](composition-template-reference.md)
for exact versions, transfer syntaxes, content contracts, and independent
routes. External callers should also read the
[composition integration guide](composition-integration-guide.md) for the Rust
API, provider protocol, cancellation, bounded-memory, and reproducibility
contracts.

This is the completed Phase P8 catalog scope, not a generic unknown-SOP writer.
Unlisted templates, transfer syntaxes, semantic parameters, and content models
remain unavailable with the blockers recorded in the dated composition status.

## 6. Understand The Output

Each generated root has this conceptual structure:

```text
generated/<run>/
├── manifest.json
├── classic/...
├── enhanced/...
├── derived/...
├── non-image/...
└── vl/...
```

Some logical cases emit multiple instances, so never assume one file per case.
Use `manifest.json` rather than recursively discovering `.dcm` files.

The manifest records:

- immutable run inputs: profile, seed, features, toolchain and lock hashes;
- DICOM Standard edition and standards-evidence identity;
- generated files with `case_id`, relative path, SHA-256, determinism class,
  SOP/transfer-syntax/modality identities, UIDs, profile membership,
  `corpus_plan_sha256`, and `resolved_plan_sha256` for valid instances;
- pixel, frame, geometry, metadata, codec, visual-pattern, and specialized
  semantic expectations when applicable;
- references and expected graph relationships;
- generation backend identity and validation results;
- qualifications for stress, fuzz, and other non-file evidence; and
- `skipped_cases` with stable reason codes and standards evidence.

Useful inspection commands:

```sh
jq '.run, .generator.feature_flags' generated/all-seed-1/manifest.json
jq '.files | length' generated/all-seed-1/manifest.json
jq '.skipped_cases[] | {case_id, status, reason_code, message}' \
  generated/all-seed-1/manifest.json
jq -r '.files[] | [.case_id, .path, .sha256] | @tsv' \
  generated/all-seed-1/manifest.json
```

A skipped row can mean a planned case, a missing Cargo feature, or an unavailable
external backend. Report the reason; do not collapse all skipped rows into a
generic failure or ignore them when claiming coverage.

## 7. Validate

Run validation with the same codec features required to decode the generated
compressed cases:

```sh
cargo run --locked --all-features -- validate generated/all-seed-1
```

A successful run prints `validation_failures` as zero. Validation checks the
manifest schema and shape, retained paths and hashes, DICOM Part 10 parsing,
file-meta/dataset identity, native or decoded pixel contracts, encapsulation,
specialized metadata and object semantics, reference closure, and isolation of
negative, fuzz, and stress evidence.

Validation intentionally understands expected-invalid `negative` cases and the
payload-free `fuzz` profile. “Zero validation failures” means those profiles
satisfy their declared robustness contracts; it does not mean malformed files
became conformant DICOM.

### Independent conformance evidence

The built-in generator and validator share project code. For independent IOD,
entity, parser, pixel, waveform, SR, ICC, or WSI evidence, use the separately
locked validator framework:

```sh
cargo run --locked -- conformance check-tools
cargo run --locked -- conformance run \
  generated/all-seed-1 --out reports/conformance/all-seed-1
cargo run --locked -- conformance verify \
  reports/conformance/all-seed-1
```

Missing mandatory external tools are explicit failures. Parser success is not
substituted for IOD validation. See `conformance/README.md` before interpreting
or publishing these results.

## 8. Report Coverage

Coverage reports describe generated and unavailable scope; they do not grade a
viewer's rendering:

```sh
cargo run --locked --all-features -- report \
  generated/all-seed-1 --format json > generated/all-seed-1/coverage.json
cargo run --locked --all-features -- report \
  generated/all-seed-1 --format markdown > generated/all-seed-1/coverage.md
```

Reports group SOP Classes, transfer syntaxes, photometric interpretations, bit
depths, frame counts, geometry, metadata, codec backends, external generation
backends, determinism, stressors, validation states, negative outcomes, fuzz
outcomes, and unavailable reasons. Specialized object fields are included when
relevant rather than flattened into generic image columns.

For registry/standards gaps independent of a generated root:

```sh
cargo run --locked -- standards check-lock
cargo run --locked -- standards gaps --profile extended
cargo run --locked -- report gaps --format markdown
```

`standards verify-kb --edition 2026b` intentionally reports unavailable in the
standalone CLI because the live standards knowledge-base service is not exposed
to the executable. `standards check-lock` verifies the committed offline lock.

## 9. Use The Corpus In A Viewer Or Parser

For every manifest `files` entry:

1. Address the object by manifest-relative `path` and retain its `case_id`.
2. Load the file without rewriting it; verify its SHA-256 before testing when
   evidence integrity matters.
3. Use manifest identities and references to group related instances. Do not
   infer series or dependency graphs solely from directories.
4. Record the consumer outcome separately from generator validation. At minimum
   distinguish load/parse, decode, render, navigation, metadata, reference, and
   unsupported outcomes.
5. Associate findings with repository commit, manifest hash, `case_id`, path,
   consumer version, platform, and any relevant screenshot or diagnostic.

The manifest's expected semantics and stressors are assertions about file
content. They do not prescribe a particular UI or require every viewer to
support every SOP Class. An explicit “unsupported” result is more useful than
a false pass based only on parser acceptance.

Do not feed `negative` outputs into a clinical workflow or conformance run that
expects valid instances. Bound parsers and external tools with time and resource
limits when testing malformed or stress corpora.

## 10. Reproducibility

The seed is a deterministic input to UIDs and generated data. It is not a source
of PHI or uncontrolled randomness. Preserve these with every handoff:

- repository commit and dirty-state note;
- manifest and manifest SHA-256;
- Rust/Cargo versions, target triple, and `Cargo.lock` hash;
- active Cargo features;
- external encoder and generation-backend fingerprints;
- generation and validation command lines and outputs; and
- JSON coverage and any independent conformance evidence.

`byte_stable` cases are expected to reproduce byte-for-byte under the recorded
inputs. `semantic_stable` cases—typically involving an external or lossy
codec—are checked against declared decoded or bounded numeric semantics rather
than a universal compressed-byte promise. See
`docs/deterministic-build-policy.md` for the formal policy.

Do not infer the DICOM payload compatibility version from the package version.
The current `0.2.0` product continues to emit `0.1.0` in unchanged built-in
byte-stable DICOM Implementation Class UID and Software Versions derivations.
Product discovery, manifests, runtime evidence, packages, and releases still
report `0.2.0`. External highdicom SR and quantitative import providers remain
semantic-stable and use the current product/backend version; their bytes are
not silently promoted to the built-in byte-stable contract.

## 11. Interoperability Qualifications

Media and protocol evidence is separate from ordinary file-conformance rows.

The DICOMDIR command selects a bounded mixed set from an existing generated root
and invokes explicit tool paths:

```sh
cargo run --locked -- interoperate media-dicomdir generated/all-seed-1 \
  --dcmmkdir /path/to/dcmmkdir \
  --dcmdump /path/to/dcmdump \
  --dciodvfy /path/to/dciodvfy \
  --dcentvfy /path/to/dcentvfy \
  --format json --timeout-seconds 30
```

The protocol baseline emits deterministic availability transactions for DIMSE,
DICOMweb, and TLS/user-identity scenarios. It does not start a network exchange
when the required replaceable peer is absent:

```sh
cargo run --locked -- interoperate protocol-baseline \
  generated/all-seed-1 --format markdown --seed 1
```

Current promotion limits and the distinction between same-provider and
independent evidence are documented in `docs/phase-8-interoperability-status.md`.

## 12. Common Problems

### “output path already exists”

Choose a new run directory. The generator will not merge with or overwrite a
previous corpus.

### Feature-gated cases appear in `skipped_cases`

Enable the feature during generation, for example `--features jpeg2000`, and
use the same feature while validating. `--all-features` is convenient but also
activates cases whose external command may still be missing.

### An external backend is unavailable

Read the row's `reason_code` and `message`. Prepare the locked Python backend or
install/fingerprint the named codec executable; do not edit the manifest to
remove the gap.

### Validation cannot decode a compressed case

Re-run validation with the Cargo feature that owns that decoder. A corpus is
portable; a feature-free validator binary is not expected to decode every
optional transfer syntax.

### `all` did not include legacy, negative, fuzz, or stress

This is by design. Generate the first three as separate profiles. Use a separate
stress run or add `--include-stress` to `--profile all`.

### A successful viewer load is being treated as conformance

Loading is only one consumer outcome. Use strict built-in validation for the
manifest contract and the independent conformance framework for external DICOM
opinions. Neither is official certification.

## 13. Source Of Truth

Use these artifacts in this order:

1. `cases/registry.json` for case status, profile membership, requirements,
   standards evidence, provider, and blockers.
2. A generated `manifest.json` for what a particular run actually emitted.
3. A generated coverage report for grouped run scope and unavailable reasons.
4. `transfer-syntax/capability-matrix.json` for encoder/decoder capability.
5. Status documents under `docs/` for dated qualification evidence and known
   limitations.

The long-range plan and historical phase documents explain engineering
decisions. They must not override a newer registry entry or fresh run manifest.
