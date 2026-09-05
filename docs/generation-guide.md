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

Curated manifest `1.0.0` produces coverage report `1.1.0`; legacy curated
manifests `0.2.0` and `0.3.0` retain coverage report `0.1.0`. Readers also retain
coverage report `1.0.0`. The additive `1.1.0` contract keeps non-generated
nonsquare rows explicit without inventing artifact observations; generated
rows retain their strict field requirements. Reporting preserves the source
identity projection and does not rerun strict corpus validation.

The supported Rust SDK and CLI verified-corpus runners produce external
manifest `2.0.0`. Raw `report <root> --format json` and `--format markdown`
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
Capabilities `3.0.0` advertises the external generation3/manifest2/report2
producer and validation windows; capabilities1/2 remain frozen. Embedded `generate` without `--corpus`
continues to emit generation-result `2.0.0` and curated manifest `1.0.0`.

#### Caller-defined native Secondary Capture

Caller-owned bundles support these native single-frame SC shapes independently
of case/recipe names and output paths:

| Pixel shape | Template at `1.0.0` |
| --- | --- |
| MONOCHROME1 unsigned 8-bit | `classic/secondary-capture/monochrome` |
| MONOCHROME2 unsigned 8/16-bit or signed 16-bit, including qualified padding | `classic/secondary-capture/monochrome` |
| PALETTE COLOR unsigned 8-bit | `classic/secondary-capture/rgb` |
| RGB unsigned 8-bit, planar 0 or 1 | `classic/secondary-capture/rgb` |
| YBR_FULL unsigned 8-bit, planar 0 | `classic/secondary-capture/rgb` |
| YBR_FULL_422 unsigned 8-bit, planar 0 and even column count | `classic/secondary-capture/rgb` |

The registry declares `rust_native`/`rust_native`, `dicom_instance`, and empty
feature/external-codec requirements. The DICOM recipe uses `native.sc_plan`,
empty provider parameters and exactly one artifact with positive dimensions,
one frame and an explicit safe output path. Artifact parameters are empty;
content is parameter-free `content.sc.pixel_pattern`. Planning order remains
required and globally unique. The typed pixel values, bit contract, hashes,
palette/padding/color declarations and matching template must agree.

Encoding is native Explicit VR Little Endian, with OB for 8-bit or OW for
16-bit pixels, default sequence/item policies, no offset table or fragment
count, native fragmentation, zero-filled preamble and standard file meta.
Recipe and artifact require `validation.sc.pixel`, applicable
`validation.sc.palette`, `.padding` or `.color`, and `projection.curated`.
No algorithm, external encoding provider, attribute override, metadata,
classic projection, nonsquare geometry, bit-packing, integer-word or
encapsulation-projection block belongs to this tuple.

Use the external generation, validation and reporting forms above. Caller
profile membership remains bundle-owned. Partial nonhistorical caller tuples
fail closed; historical namespace and EOT admission remain broader during
migration. This boundary does not generalize their codec, multiframe, geometry,
stress or exceptional contracts and supplies same-project evidence only.

#### Caller-defined classic CT capability

A caller-owned bundle can select the native classic CT planner without
copying an embedded case name or ordering convention. The complete supported
tuple is:

- registry provider kind and ID `rust_native`/`rust_native`, artifact kind
  `dicom_instance`, and empty feature and external-codec requirements;
- recipe kind `dicom`, `plan_provider_id = "native.classic_plan"`;
- every artifact uses template `classic/ct@1.0.0`, parameter-free
  `content.native_pixels`, `algorithm_provider_id = "algorithm.classic_ct"`,
  `classic_projection.family = "ct"`, strict typed provider/artifact
  parameters, and an explicit output path.

Any CT marker declares intent, so a partial or mixed tuple is rejected instead
of falling through to a case-name matcher. Artifact order must remain
contiguous and zero-based. `planning_order` is mandatory and globally unique
among migrated recipes, but it is scheduling metadata and does not dispatch
the CT planner. The caller chooses case ID, recipe ID, planning/projection
orders, logical artifact IDs, and output paths subject to the ordinary bundle
integrity and path rules.

This is a black-box CLI and supported `synth_dicom_gen::sdk` contract. It does
not authorize implementation-module imports or sibling-path discovery, and it
does not claim `native.stress_ct_plan`, generic support for another classic or
VL family through this CT tuple, independent DICOM conformance, viewer interoperability, package or
release qualification.

#### Caller-defined DX and mammography

The same external CLI and SDK support these native single-instance templates:

| Template at version `1.0.0` | Typed family and presentation intent |
| --- | --- |
| `classic/dx/for-presentation` | `dx`, `FOR PRESENTATION` |
| `classic/mammography/for-presentation` | `mammography`, `FOR PRESENTATION` |
| `classic/mammography/for-processing` | `mammography`, `FOR PROCESSING` |

The registry uses `rust_native`/`rust_native`, `dicom_instance`, and empty
feature/codec requirements. The DICOM recipe uses `native.classic_plan`,
parameter-free `content.native_pixels`, `algorithm.classic_dx_mg`, and
`classic_projection.family = "dx_mg"`. Template, family, presentation intent
and strict typed parameters must all agree. Any template, algorithm or
projection marker declares intent; incomplete or crossed tuples are rejected.

Each recipe has exactly one logical artifact named `instance` at order zero.
Case ID, recipe ID, planning/projection order and explicit output path are
caller-owned; planning order remains mandatory and globally unique. Existing
pixel, encoding, shutter and mammography constraints remain enforced, including
the declared historical Field of View Dimensions DS VR contract. This migration
does not change those bytes or establish independent IOD conformance.

Select these definitions with the same `generate --corpus`, `--profile core`
and repeated `--case-id` options shown above. CLI and SDK produce external
manifest2/report2 with separate strict-validation evidence. Other classic/VL
families and stress are not generalized by this DX/MG contract; release and
viewer qualification remain separate.

#### Caller-defined computed radiography

The external CLI and SDK accept a bounded native CR capability through
`native.classic_plan`, `classic/cr@1.0.0`, parameter-free
`content.native_pixels`, `algorithm.classic_mr_cr` and
`classic_projection.family = "mr_cr"`. The registry declares
`rust_native`/`rust_native`, `dicom_instance` and no feature/codec requirements.
A CR template or CR overlay/LUT parameters declares intent; partial, mixed and
crossed tuples reject. The shared MR/CR algorithm alone does not identify CR.

Declare one logical `instance` artifact at order zero with role `instance`, an
explicit safe path and caller-owned case/recipe names and planning/projection
orders. Planning order remains required and globally unique. Both recipe and
artifact use `validation.shared` and `projection.curated`. Provider/content
parameter extensions, dependencies, attribute/profile overrides and unrelated
artifact projections are excluded. CR semantic labels are required; MR/ICC
projection fields, appended standards evidence and implementation-version
projection overrides are outside this bounded capability.

The typed CR parameters describe one U8/OB MONOCHROME2 frame, checked dimensions,
sample range/extrema and frame hash. Overlay geometry matches the image, with
type G, origin `[1,1]`, bits allocated1/bit position0, even-byte padding and zero
unused bits. Both LUT descriptors are `[4,0,16]` with eight data bytes; modality
LUT type is `US` and VOI LUT type is absent. Encoding is native Explicit VR
Little Endian with default sequence/item lengths, no offsets or fragment count,
zero-filled preamble and standard file meta.

Typed CR dispatch precedes historical name matchers, including misleading MR,
PET or VL names. The named RLE route remains separately qualified. The catalog
limitation describes qualified CR composition, whose default is unsigned12-bit
in16-bit; this curated U8/OB capability does not expand composition qualification.
Use the same external `generate`, separate `validate` and `report` commands
above. CLI/SDK byte and manifest equality is same-project evidence; report2
projects the manifest and adds no independent conformance or viewer result.

#### Caller-defined native ultrasound

The external CLI and SDK accept one `classic/ultrasound/single-frame@1.0.0`
artifact through `native.classic_plan`, parameter-free `content.native_pixels`,
`algorithm.classic_nuclear` and `classic_projection.family = "nuclear"`. The
registry declares `rust_native`/`rust_native`, `dicom_instance` and no feature or
codec requirements. Either the US template or typed `ultrasound_single_frame`
family declares intent; incomplete and crossed tuples reject.

Use logical artifact `instance`, order zero and role `primary`, an explicit safe
path, and caller-owned case/recipe names and planning/projection orders. Both
orders are required and unique among migrated recipes. Recipe and artifact use
`validation.shared` and `projection.curated`. The bounded provider retains its
qualified fixed synthetic patient, study, equipment and acquisition metadata;
optional series date/time and body-part fields remain absent.

Pixels are one native U8/OB MONOCHROME2 frame with positive checked dimensions,
exact sample count, byte-range values, matching extrema and one computed frame
hash. Image Type is `ORIGINAL\PRIMARY`, Lossy Image Compression is `00`, and
Ultrasound Color Data Present is zero. Encoding is Explicit VR Little Endian,
default sequence/item lengths, no offsets or fragment count, zero-filled
preamble and standard file meta. Dependencies, attribute/profile overrides,
provider/content extensions and unrelated projection fields are excluded.

This capability emits no calibration region. Template calibration evidence does
not establish a region in this case. Historical RLE, multiframe US, PET and NM
retain their separate contracts. Typed US admission precedes historical family
name matchers; external validation follows declared evidence and captured
definition profiles. CLI/SDK equality and reopened strict validation are
same-project evidence; report2 projects the manifest without adding independent
conformance or viewer results.

For a caller bundle selecting `caller/acquisition/ultrasound`, run these from
the directory containing `definition.json` and `corpus-members`. Set `GENERATOR`
to the supported executable's absolute path and use a fresh output root:

```sh
"$GENERATOR" capabilities --corpus ./definition.json --asset-root corpus-members \
  --profile core --case-id caller/acquisition/ultrasound --seed 1 --parallelism 4 --format json
"$GENERATOR" generate --corpus ./definition.json --asset-root corpus-members \
  --profile core --case-id caller/acquisition/ultrasound --seed 1 --parallelism 4 \
  --out generated/us-proof --format json
"$GENERATOR" validate generated/us-proof --format json
"$GENERATOR" report generated/us-proof --format json --cli-api 1.0.0
```

#### Caller-defined native PET

The external CLI and SDK accept the complete `classic/pet@1.0.0` native tuple:
`native.classic_plan`, parameter-free `content.native_pixels`,
`algorithm.classic_nuclear`, nuclear classic projection and typed `family: pet`.
The registry declares `rust_native`/`rust_native`, `dicom_instance`, PET Image
Storage and modality `PT`, with no feature or external-codec requirements.
The PET template or typed family declares intent; incomplete and crossed tuples
reject. The shared nuclear algorithm alone does not identify PET.

Use one logical artifact `instance`, order zero, role `primary` and an explicit
safe output path. Case/recipe names and required unique planning/projection
orders are caller-owned. Recipe and artifact both require `validation.shared`
and `projection.curated`. The accepted fixture uses orders 900 and 901.
Encoding is native Explicit VR Little Endian with default sequence/item lengths,
no offsets or fragment count, zero-filled preamble and standard file meta.
Dependencies, attribute/profile overrides, content extensions and unrelated
projection fields are excluded.

The pixel and parameter contract is fixed to the source recipe: 2×2×1 U16/OW
MONOCHROME2, values `[0, 100, 200, 400]`, extrema 0/400 and their exact frame
hash. Image Type is `ORIGINAL\PRIMARY`; units are `BQML`, Counts Source is
`EMISSION`, Series Type is `STATIC\IMAGE`, Corrected Image is `DCAL` and
Decay Correction is `NONE`. The source strings for calibration factor,
intercept/slope, frame reference time and duration are respectively `1`, `0`,
`2.5`, `30000` and `60000`. Activity values are `[0, 250, 500, 1000]`; slice
count and image index are one. Spacing is `4\4`, orientation
`1\0\0\0\1\0`, position `0\0\0` and thickness `4`. Alternate numeric
spellings are outside this bounded contract.

All 20 synthetic provider fields remain source-fixed, including patient, study,
equipment and acquisition metadata, populated series date/time and body part
`HEAD`. Required empty sequences and conditional tag absences remain intact.
These synthetic BQML/rescale fields do not establish SUV, administered-dose or
clinical quantitative accuracy. US, NM, multiframe PET and codec qualifications
retain their separate contracts. CLI/SDK equality and reopened strict
validation are same-project evidence; report2 projects the manifest without
reopening payloads or adding independent conformance results.

The committed four-member fixture includes its local PET standards note. Set
`GENERATOR` to the supported executable's absolute path, `PET_FIXTURE` to the
absolute path of `tests/fixtures/generic-pet-corpus`, and `PET_OUTPUT` to a fresh
absolute output path. These commands can run from an unrelated directory:

```sh
"$GENERATOR" capabilities --corpus "$PET_FIXTURE/definition.json" --asset-root "$PET_FIXTURE/members" \
  --profile core --case-id caller/acquisition/activity --seed 1 --parallelism 4 --format json
"$GENERATOR" generate --corpus "$PET_FIXTURE/definition.json" --asset-root "$PET_FIXTURE/members" \
  --profile core --case-id caller/acquisition/activity --seed 1 --parallelism 4 \
  --out "$PET_OUTPUT" --format json
"$GENERATOR" validate "$PET_OUTPUT" --format json
"$GENERATOR" report "$PET_OUTPUT" --format json --cli-api 1.0.0
```

#### Caller-defined native XA and XRF

The external CLI and SDK accept one source-fixed native projection artifact per
recipe through `classic/xa@1.0.0` or `classic/xrf@1.0.0`,
`native.classic_plan`, parameter-free `content.native_pixels`,
`algorithm.classic_vl_projection` and the complete `vl_projection` contract.
Template, typed modality and SOP declare intent; incomplete or crossed tuples
reject. Registry modality must match `XA` or `RF`, with the matching XA or XRF
Image Storage SOP, `rust_native`/`rust_native` provider and no feature or
external-codec requirements.

Each recipe has logical artifact `instance`, order zero, role `primary_1`,
shared validation/projection rules and an explicit safe output path. Caller
case/recipe IDs and unique planning/projection orders are independent of the
historical names. Encoding is native Explicit VR Little Endian, with default
sequence/item lengths, no offsets or fragment count, zero-filled preamble and
standard file meta. Dependencies, attribute/profile overrides, unrelated
projection fields and content extensions are excluded.

Both tuples preserve the exact source 4×4×1 U8/OB MONOCHROME2 samples, extrema
0/255 and frame hash, Image Type `ORIGINAL\PRIMARY\SINGLE PLANE`, relationship
`LIN` and lossless marker `00`. Geometry strings remain spacing `0.2\0.2`,
distances `1200`/`800` and magnification `1.5`. XA preserves `HEART`, KVP `80`,
radiation setting `GR`, exposure `4` and positioner angles `15`/`-10`. XRF
preserves `ABDOMEN`, KVP `70`, setting `SC`, exposure `1` and column angulation
`10`. Alternate numeric spellings are outside this bounded contract.

All nine provider fields remain source-fixed: patient name, ID, birth date,
sex, study date, time, ID, manufacturer and software versions. The complete
source projection includes XA's inline standards evidence. These synthetic
single-plane cases do not establish calibrated patient-space geometry, cine,
biplane, contrast or subtraction coverage. RLE and enhanced XA/XRF are outside this bounded contract. Strict validation reopens payloads as same-project
evidence; report2 projects the manifest without reopening payloads or adding
independent conformance results.

The committed six-member fixture includes both local standards notes. Set
`GENERATOR` to the supported executable's absolute path, `PROJECTION_FIXTURE`
to the absolute path of `tests/fixtures/generic-xa-xrf-corpus`, and
`PROJECTION_OUTPUT` to a fresh absolute output path. Its two recipes use
planning orders 900/901 and projection orders 902/903. These commands can run
from an unrelated directory:

```sh
"$GENERATOR" capabilities --corpus "$PROJECTION_FIXTURE/definition.json" --asset-root "$PROJECTION_FIXTURE/members" \
  --profile core --case-id caller/acquisition/angiography --case-id caller/acquisition/fluoroscopy --seed 1 --parallelism 4 --format json
"$GENERATOR" generate --corpus "$PROJECTION_FIXTURE/definition.json" --asset-root "$PROJECTION_FIXTURE/members" \
  --profile core --case-id caller/acquisition/angiography --case-id caller/acquisition/fluoroscopy --seed 1 --parallelism 4 \
  --out "$PROJECTION_OUTPUT" --format json
"$GENERATOR" validate "$PROJECTION_OUTPUT" --format json
"$GENERATOR" report "$PROJECTION_OUTPUT" --format json --cli-api 1.0.0
```

#### Caller-defined Secondary Capture metadata

The external CLI and SDK accept independently named recipes for these typed
metadata variants:

| `metadata_sc.kind` | Matching content provider | Additional constraint |
| --- | --- | --- |
| `person_name` | `content.metadata.person_name` | Exactly `ISO_IR 192`; raw bytes equal the decoded PN's UTF-8 bytes and `native_unicode_round_trip` is true |
| `empty_type2` | `content.metadata.empty_type2` | Nonempty unique subset of the qualified tag/keyword/VR tuples below |
| `private_creators` | `content.metadata.private_creators` | Existing typed private-block allocation, value and hash checks |

The registry declares `rust_native`/`rust_native`, `dicom_instance`, and no
feature or external-codec requirements. The DICOM recipe uses
`native.metadata_sc_plan` with empty provider parameters and exactly one
artifact using `classic/secondary-capture/monochrome@1.0.0`. Its explicit output
path and case/recipe names are caller-owned; planning order remains required
and globally unique. Artifact and content parameter maps are empty. Both recipe
and artifact require `validation.sc.pixel` and the matching
`validation.metadata.person_name`, `.empty_type2` or `.private_creators` rule.

Qualified empty Type 2 tuples are PatientName (`0010,0010`, PN), PatientBirthDate
(`0010,0030`, DA), PatientSex (`0010,0040`, CS), ReferringPhysicianName
(`0008,0090`, PN) and AccessionNumber (`0008,0050`, SH). Arbitrary identity or
Type 1 attributes cannot be emptied through this capability.

Validated SC pixel semantics and native Explicit VR Little Endian encoding
remain required, with default sequence/item lengths, native fragmentation, no
offset table, zero-filled preamble and standard file meta. No algorithm,
attribute overrides, classic projection or nonsquare geometry is admitted.
Partial and crossed contracts fail closed. ISO2022, timezone, string-boundary
and sequence-length variants retain their existing admission rules.

Use the same `generate --corpus`, explicit member root, profile and case
selection forms above, then separate `validate` and `report` commands. Report2
is a manifest projection; this contract adds no independent conformance,
viewer or release qualification.

Inspect the same caller-owned bundle without an output path:

```sh
synth-dicom-gen capabilities --corpus ./definition.json \
  --asset-root corpus-members --format json
synth-dicom-gen capabilities --corpus ./definition.json \
  --asset-root corpus-members --profile smoke --seed 1 --parallelism 2 \
  --format json
```

The first command returns verified profiles/case metadata with assessment
`not_assessed`. The second assesses the selected scope using the same planner
as generation; repeat `--case-id ID` for direct cases within `--profile`.
Seed, parallelism, case IDs and stress options require a profile. Registry
`implemented`, installed provider declarations, and selected `ready` are
different facts. Inspection never runs generators/providers, discovers external
executables, creates a destination, or claims generation/validation success.
Qualified templates, compiled native provider support, unassessed external
providers, and exact unavailable reasons remain machine-readable.

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
