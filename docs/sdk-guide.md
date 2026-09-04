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
Loaded-corpus capability discovery remains pending; frozen capabilities2 does
not yet advertise these complete external result-validation windows.

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
