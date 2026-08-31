# Rust SDK guide

The supported Rust integration is the narrow `dicom_test_suite::sdk` facade.
Other public modules remain available during migration but are not part of the
standalone compatibility commitment. The command-line JSON API remains the
primary language-neutral integration.

## Construct the product

Normal use selects the immutable embedded product resources and verifies their
identity before returning a handle:

```rust
use dicom_test_suite::sdk::DicomTestSuite;

let product = DicomTestSuite::embedded()?;
let version = product.version()?;
let capabilities = product.capabilities()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`DicomTestSuite::explicit_resource_root(path)` is the opt-in alternative. The
root must contain the complete byte-identical resource set. Missing or changed
resources fail closed; the SDK never falls back to the checkout or embedded
resources after that constructor is selected.

## Compose from a file or bytes

Both entry points use the exact same plan-first execution pipeline. A file
request derives its caller-asset root from the request file's parent unless it
is overridden explicitly:

```rust
use dicom_test_suite::sdk::{ComposeRequest, DicomTestSuite};

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
# use dicom_test_suite::sdk::{ComposeRequest, DicomTestSuite};
# let product = DicomTestSuite::embedded()?;
# let spec = br#"{"composition_spec_schema_version":"0.1.0","instances":[]}"#;
let request = ComposeRequest::from_json_bytes(spec, "caller-assets", "generated/result");
let outcome = product.compose(request)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Call `.dry_run(true)` to resolve the same canonical plan without publishing.
The typed outcome then has `published() == false`, no manifest, and a
`PlanPreview`; it retains the same corpus-plan hash as publication.

## Validate and report

Published roots are consumed through typed requests and results:

```rust
# use dicom_test_suite::sdk::{DicomTestSuite, ReportRequest, ValidateRequest};
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

Long-running composition accepts a cooperative token:

```rust
# use dicom_test_suite::sdk::{CancellationToken, ComposeRequest, DicomTestSuite};
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
composition returns `generation.execution.cancelled`, is retryable, removes
private staging, and publishes no destination.

The facade does not convert missing codecs, providers, validators, or peers
into passes. Inspect `capabilities()` first and handle unavailable results
explicitly.
