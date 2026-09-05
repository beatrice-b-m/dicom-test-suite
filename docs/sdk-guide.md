# Rust SDK guide

The supported Rust integration is the narrow `synth_dicom_gen::sdk` facade.
Other public modules remain available during migration but are not part of the
standalone compatibility commitment. The command-line JSON API remains the
primary language-neutral integration.

## Construct the product

Normal use selects the immutable embedded product resources and verifies their
identity before returning a handle:

```rust
use synth_dicom_gen::sdk::DicomTestSuite;

let product = DicomTestSuite::embedded()?;
let version = product.version()?;
let capabilities = product.capabilities()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`DicomTestSuite::explicit_resource_root(path)` is the opt-in alternative. The
root must contain the complete byte-identical resource set. Missing or changed
resources fail closed; the SDK never falls back to the checkout or embedded
resources after that constructor is selected.

## Generate a verified caller-owned corpus

`GenerateCorpusRequest` accepts a frozen corpus-definition bundle `1.0.0`.
Both constructors require the dedicated member/asset root, output root, and
an explicit selector; descriptor-file location never supplies an implicit
member root. Descriptor file paths require an explicit parent: use
`./definition.json` for the current directory, not bare `definition.json`,
which is rejected as `resource.document.invalid`. Inputs are captured when
`generate_corpus` is called, not when the request is constructed. Changed,
missing, symlinked, undeclared, or
hash-mismatched inputs fail closed. Relative paths are caller-relative; use
real, non-symlinked ancestor paths (including on macOS temporary directories).

```rust
use synth_dicom_gen::sdk::{CorpusSelector, DicomTestSuite, GenerateCorpusOutcome,
    GenerateCorpusRequest, ReportRequest, ValidateRequest};

let product = DicomTestSuite::embedded()?;
let request = GenerateCorpusRequest::from_file(
    "./definition.json", "corpus-members", "generated/caller-smoke",
    CorpusSelector::Profile { profile: "smoke".into(), include_stress: false },
).with_seed(1).with_parallelism(2);
match product.generate_corpus(request)? {
    GenerateCorpusOutcome::Published(run) => {
        assert!(product.validate(ValidateRequest::new(run.output_root()))?.is_valid());
        let report = product.report(ReportRequest::new(run.output_root()))?;
        assert_eq!(report.schema_version(), "2.0.0");
    }
    GenerateCorpusOutcome::Planned(preview)
    | GenerateCorpusOutcome::NoExecutableCases(preview) => {
        for case in preview.cases() {
            println!("{}: {:?}", case.case_id(), case.disposition());
        }
    }
    _ => {}
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

For descriptor bytes use `GenerateCorpusRequest::from_json_bytes(bytes,
member_root, output_root, selector)`. `CorpusSelector::CaseIds` takes
`profile`, `include_stress`, and a nonempty, unique `case_ids` vector. The
profile is a scope constraint, not a fallback selection. Direct selections
retain every status; dependency closure is separate. The frozen bundle profile
rules still apply: `all` excludes legacy/negative/fuzz; stress is opt-in only
with `all`. No arbitrary profile contract is implied.

`dry_run(true)` returns `Planned` even when no case is executable. A real request
with no executable cases returns `NoExecutableCases`, never an empty successful
corpus. Both have publication and validation `NotRun`, no manifest path, and no
output directory. `Ready` is a planning disposition, not a generation pass.
Preview accessors are SDK evidence contract `1.0.0`, **not** a standalone JSON
document schema. They expose seed, selector, plan hash, artifact IDs, typed case
dispositions, and lossless ledger/identity Values with manifest `2.0.0` field
meanings (except preview-only `ready`). They never expose an internal plan.

Published outcomes carry `ManifestKind::ExternalCorpus` manifest `2.0.0`, typed
emitted-file count/bytes and plan hash. SDK validate/report consume that manifest
and return `ReportKind::ExternalCorpus` report `2.0.0`. Reports preserve all source
evidence; creating one performs no new validation or independent conformance.
After capture the runner does not reopen caller inputs; persisted output can
be moved and validated/reported after the source bundle is removed.

Execution currently uses native/compiled support only. Missing providers and
codecs remain explicit unavailable dispositions; no ambient tool discovery is
performed. The [external corpus CLI](generation-guide.md#generate-a-caller-owned-definition-bundle)
uses this facade and emits generation-result3/report-result2 in CLI API1.
Capabilities `3.0.0` advertises these external producer and validation windows;
the earlier capability schemas remain frozen.

### Caller-defined native Secondary Capture

The same `InspectCorpusRequest` and `GenerateCorpusRequest` interfaces accept
the bounded [native SC contract](generation-guide.md#caller-defined-native-secondary-capture).
It binds `native.sc_plan`, one native single-frame artifact, parameter-free
`content.sc.pixel_pattern`, a matching monochrome/RGB SC template@1 and the
qualified pixel/layout/validation tuple. Case/recipe names, unique planning
orders and explicit safe paths are caller-owned. CLI/SDK manifests and payloads
agree, with separate strict validation and report2 projection. Historical
specialized namespace fallbacks remain outside this bounded genericity claim.

### Caller-defined classic CT

The external runner accepts a name-, order-, and output-independent classic CT
definition when the entire capability tuple agrees. Its registry row uses
provider kind/ID `rust_native`/`rust_native`, artifact kind `dicom_instance`,
and no feature or external-codec requirements. Its DICOM recipe uses
`native.classic_plan`; every artifact uses `classic/ct@1.0.0`, parameter-free
`content.native_pixels`, `algorithm.classic_ct`,
`classic_projection.family = "ct"`, strict typed CT provider/artifact
parameters, and an explicit output path. Artifact order is contiguous from
zero. `planning_order` is still mandatory and globally unique, but is not the
dispatch key.

Any partial or mixed CT tuple fails closed. Callers may choose their own case
ID, recipe ID, planning/projection orders, logical artifact IDs, and paths;
neither `GenerateCorpusRequest` nor the CLI consults internal modules or a
sibling checkout. This boundary excludes `native.stress_ct_plan`, other
classic/VL family genericity, independent conformance, viewer interoperability,
and release qualification.

### Caller-defined DX and mammography

`GenerateCorpusRequest` also accepts caller-named DX/MG definitions through
`native.classic_plan`, `content.native_pixels`, `algorithm.classic_dx_mg` and
`classic_projection.family = "dx_mg"`. The matching version1 template and
strict typed family/presentation parameters must agree; see the
[generation contract](generation-guide.md#caller-defined-dx-and-mammography).
The registry provider remains `rust_native`/`rust_native` without feature or
codec requirements. One `instance` artifact at order zero is required; its
output path and the case/recipe names and unique planning order are caller-owned.
Partial tuples fail closed. Use the same inspection, generation, validation and
report requests above; no internal-module import or sibling lookup is needed.
This preserves the historical DX/MG pixel and VR contracts and supplies
same-project evidence only.

### Caller-defined computed radiography

The same `InspectCorpusRequest` and `GenerateCorpusRequest` interfaces accept
bounded native CR through the complete
[CR contract](generation-guide.md#caller-defined-computed-radiography).
One `classic/cr@1.0.0` artifact uses native U8/OB pixels, a bounded overlay and
four-entry modality/VOI LUTs. Case/recipe names, explicit path and unique order
are caller-owned; typed CR takes precedence over historical family names.
Separate strict validation and report2 projection preserve their evidence roles.
The curated capability does not change the qualified composition default or
extend RLE, viewer or independent conformance evidence.

### Caller-defined native ultrasound

`InspectCorpusRequest` and `GenerateCorpusRequest` also accept the bounded
[native US contract](generation-guide.md#caller-defined-native-ultrasound): one
`classic/ultrasound/single-frame@1.0.0` artifact with checked native U8 pixels
and fixed synthetic provider metadata. Caller names, explicit paths and unique
orders are independent of dispatch. CLI and SDK agree on capabilities, complete
manifests, payloads and reports, with separate reopened strict validation.
Calibration-region, multiframe, RLE and independent-evidence claims remain
scoped to their respective qualifications.

### Caller-defined native PET

The same `InspectCorpusRequest` and `GenerateCorpusRequest` interfaces accept
`caller/acquisition/activity` from `tests/fixtures/generic-pet-corpus`, using
`definition.json` and its explicit `members` root. Select that case under
`core`, seed 1 and parallelism 4. The complete
[PET contract](generation-guide.md#caller-defined-native-pet) preserves one
fixed 2×2 U16 activity image, exact source parameter spellings and all synthetic
provider metadata while permitting caller identity, order and output path.
The four-member fixture includes the PET standards note. CLI/SDK capabilities,
raw manifests, payloads and full reports agree; reopened strict validation is
separate. This is synthetic BQML evidence, with no SUV or clinical dosing claim.

### Caller-defined Secondary Capture metadata

The same inspection and `GenerateCorpusRequest` interfaces accept independently
named UTF-8 Person Name, qualified empty Type 2 and private-creator recipes.
The complete [metadata contract](generation-guide.md#caller-defined-secondary-capture-metadata)
binds `native.metadata_sc_plan`, one monochrome SC template@1 artifact, matching
typed content and validation rules, and native Explicit VR Little Endian
encoding. Explicit paths and unique planning orders remain caller-owned.
Malformed or crossed tuples reject before publication. CLI/SDK manifests and
payloads agree; strict validation remains separate from report2 projection.
Other metadata variants retain their existing admission rules.

## Inspect a caller-owned corpus before submitting

```rust
use synth_dicom_gen::sdk::{CorpusSelector, DicomTestSuite, InspectCorpusRequest};

let product = DicomTestSuite::embedded()?;
let inspection = product.inspect_corpus(
    InspectCorpusRequest::from_file("./definition.json", "corpus-members")
        .with_selection(CorpusSelector::Profile {
            profile: "smoke".into(), include_stress: false,
        })
        .with_seed(1)
        .with_parallelism(2),
)?;
let assessment = inspection.assessment().expect("selection was supplied");
assert_eq!(assessment.seed(), 1);
for case in assessment.cases() {
    println!("{} {:?} {:?}", case.case_id(), case.disposition(), case.reason_code());
}
# Ok::<(), synth_dicom_gen::sdk::SdkError>(())
```

File and `from_json_bytes` inspection require an explicit dedicated member
root, with the same capture/limits/closure rules as generation. No destination
is accepted, checked, or created. Without `with_selection`, only verified
profiles and case definitions are returned: runtime/selection assessment was
not performed. Case status is not runtime availability. Selected assessment
uses generation's same captured planner and preserves seed, parallelism,
scope, dependencies and unavailable reasons. `Ready` means planned, never
generated or validated; publication and validation remain `NotRun`.

`capabilities_with_corpus(request)` combines the same captured inspection with
installed engine, qualified-template, transfer-syntax and provider declarations.
Its top-level and nested corpus identities refer to that one verified capture.
`provider_support` distinguishes compiled native support from unassessed
external declarations; actual selected feasibility is in the assessment.
No provider, codec or executable discovery is invoked. Cancellation is
cooperative before/after bounded capture/planning through
`inspect_corpus_cancellable`; this is not a CLI signal-handling claim.
Inspection/assessment accessors are SDK evidence version `1.0.0`, not standalone
serialized documents or private plans. CLI serialization is capabilities3.

### Explicit isolated-source consumer proof

Maintainers can invoke `scripts/prove-isolated-corpus-consumer.py` at an
explicit supported-boundary gate, not on ordinary PRs. It requires a clean
checkout, an exact full committed `--revision`, a new absolute private
`--artifacts` root, and a new `--retain` directory immediately under the ignored
workspace `generated/` directory. The harness archives that revision, builds
the SDK-only fixture and CLI offline from the extracted source, records lock
alignment and build measurements, removes the extracted generator and consumer
source trees, and then runs from an unrelated directory with an empty PATH.

Receipts, the source archive, binaries, caller-owned smoke/planned bundles and
complete output evidence are copied to the durable retained directory. Target
trees are measured and removed. This proves an isolated committed-source
consumer boundary using an existing offline dependency cache; it is **not**
Cargo package verification, remote-fetch/clean-clone evidence, installed-release
qualification, or an independent DICOM conformance assessment.

## Compose from a file or bytes

Both entry points use the exact same plan-first execution pipeline. A file
request derives its caller-asset root from the request file's parent unless it
is overridden explicitly:

```rust
use synth_dicom_gen::sdk::{ComposeRequest, DicomTestSuite};

let product = DicomTestSuite::embedded()?;
let request = ComposeRequest::from_file("request.json", "generated/result")
    .with_caller_asset_root("caller-assets")
    .with_seed(1);
let outcome = product.compose(request)?;
assert!(outcome.published());
let manifest = outcome.manifest().expect("published runs have manifests");
println!("{}", manifest.path().display());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Byte requests require the caller-asset root as a constructor argument:

```rust
# use synth_dicom_gen::sdk::{ComposeRequest, DicomTestSuite};
# let product = DicomTestSuite::embedded()?;
# let spec = br#"{"composition_spec_schema_version":"0.1.0","instances":[]}"#;
let request = ComposeRequest::from_json_bytes(spec, "caller-assets", "generated/result");
let outcome = product.compose(request)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Call `.dry_run(true)` to resolve the same canonical plan without publishing.
The typed outcome then has `published() == false`, no manifest, and a
`PlanPreview`; it retains the same corpus-plan hash as publication.

## Assemble caller-owned structure

`AssembleRequest` has the same file/byte, explicit asset-root, seed,
parallelism, dry-run, cancellation, and typed-outcome conventions. Its
manifest kind is `StructuralAssembly` and never represents qualified IOD or
curated coverage evidence:

```rust
use synth_dicom_gen::sdk::{AssembleRequest, DicomTestSuite, ManifestKind};

let product = DicomTestSuite::embedded()?;
let request = br#"{
  "assembly_request_schema_version":"1.0.0",
  "instances":[{"instance_id":"primary","sop_class_uid":"1.2.840.10008.5.1.4.1.1.7","elements":[]}]
}"#;
let outcome = product.assemble(AssembleRequest::from_json_bytes(
    request.as_slice(),
    "caller-assets",
    "generated/structural",
))?;
assert_eq!(
    outcome.manifest().expect("published manifest").kind(),
    ManifestKind::StructuralAssembly
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

See the [structural assembly guide](assembly-guide.md) before constructing a
request; runtime `capabilities()` is authoritative for supported versions,
content kinds, transfer syntaxes, and ceilings.

## Validate and report

Published roots are consumed through typed requests and results:

```rust
# use synth_dicom_gen::sdk::{DicomTestSuite, ReportRequest, ValidateRequest};
# let product = DicomTestSuite::embedded()?;
let validation = product.validate(ValidateRequest::new("generated/result"))?;
if !validation.is_valid() {
    for failure in validation.failures() {
        eprintln!("{failure}");
    }
}
let report = product.report(ReportRequest::new("generated/result"))?;
println!("report schema {}", report.schema_version());
# Ok::<(), Box<dyn std::error::Error>>(())
```

`SchemaBoundManifest` validates the persisted manifest against the embedded
schema before the SDK returns it. Manifest and report wrappers expose typed
identity, kind, and version fields plus canonical JSON bytes. Generic
`deserialize` is an opt-in adapter for a consumer-owned typed model; no SDK
operation returns `serde_json::Value` as its primary result.

## Cancellation and errors

Corpus generation (`generate_corpus_cancellable`), composition, and structural
assembly accept a cooperative token:

```rust
# use synth_dicom_gen::sdk::{CancellationToken, ComposeRequest, DicomTestSuite};
# let product = DicomTestSuite::embedded()?;
# let request = ComposeRequest::from_json_bytes(b"{}", ".", "generated/result");
let cancellation = CancellationToken::new();
let worker_token = cancellation.clone();
// Another thread may call `cancellation.cancel()`.
let result = product.compose_cancellable(request, &worker_token);
# let _ = result;
```

Branch on `SdkError::code`, never `Display` or `diagnostic` text. Codes use the
same append-only taxonomy as CLI API `1.0.0`; `kind` provides the stable broad
request, unavailable, output, execution, or internal category. A cancelled
operation returns `generation.execution.cancelled`, is retryable, removes
private staging, and publishes no destination. If cleanup itself fails, corpus
generation instead returns `io.cleanup.failed` and preserves the diagnostic;
it does not claim successful cleanup. Corpus definition and execution failures
are mapped from typed causes, not diagnostic substring matching.

The facade does not convert missing codecs, providers, validators, or peers
into passes. Inspect `capabilities()` first and handle unavailable results
explicitly.
