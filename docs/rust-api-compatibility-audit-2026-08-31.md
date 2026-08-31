# Rust API compatibility audit

**Recorded:** 2026-08-31

**Product line:** `0.1.x`

## Supported facade

`dicom_test_suite::sdk` is the only supported Rust product API. Its public
constructors, request/result models, schema-bound wrappers, cancellation token,
and error taxonomy follow the product compatibility policy. The public product
surface currently comprises:

- `DicomTestSuite` with embedded and explicit-resource constructors;
- typed version and capability discovery;
- `ComposeRequest`, `ComposeOutcome`, `PlanPreview`, and `CancellationToken`;
- `ValidateRequest` and `ValidationOutcome`;
- `ReportRequest` and `ReportOutcome`;
- `SchemaBoundManifest`, `ManifestKind`, and `ReportKind`; and
- `SdkError` and the non-exhaustive `SdkErrorKind`.

Primary SDK operations do not return `serde_json::Value`, executor services,
planners, recipes, materializers, or composition-internal types. Extensible
public SDK error and evidence-kind enums are non-exhaustive. Stable SDK error
codes are the registered CLI API `1.0.0` codes; diagnostic prose is not a
branching contract.

## Existing exposure retained during migration

The crate exposed implementation modules before the supported facade existed.
The following root modules remain public and source-compatible for the `0.1.x`
migration; they are not standalone product compatibility surfaces:

```text
cli_protocol                 codecs
composition                  conformance
corpus_plan                  coverage_gaps
curated_execution            curated_manifest
curated_plan                 curated_validation
discovery                    encapsulation
encoded_content              executor
fuzz                         generation_backends
media                        media_runner
media_sources                mutation
native_pixel                 negative
negative_plan                part10_locator
planning                     planning_preview
product_resources            protocol
protocol_baseline            qualification_plan
quantitative_evidence        recipes
runtime_capabilities         sr_rt_manifest
sr_rt_validation             stress
uid
```

Legacy root types and functions in `lib.rs`, plus the root re-exports from
`coverage_gaps` and `uid`, have the same retained-but-unsupported status. They
are used by repository qualification tests and earlier source consumers, so
this phase makes no visibility reduction and adds no deprecation warning that
would break warning-denied builds.

`cli_protocol`, `discovery`, and `product_resources` contain models also used
by the facade. Their direct module paths are still implementation exposure;
SDK method signatures are the supported route even when a returned concrete
type is presently defined in one of those modules. Moving those concrete types
would itself require a supported SDK compatibility review.

## Deprecation and retention plan

1. Keep all existing module visibility unchanged throughout standalone
   productization and release-candidate qualification.
2. Add new supported Rust behavior only through `sdk`; do not ask consumers to
   assemble planners, executors, catalogs, or materializers.
3. Inventory downstream use before proposing any visibility reduction.
4. Announce intended removals with migration examples to the equivalent SDK
   entry point and retain them for at least the published deprecation window.
5. Treat a supported SDK removal, rename, field-meaning change, or error-code
   semantic change as product-semver breaking under `docs/compatibility-policy.md`.
6. Perform reductions only in a deliberate semver release; never fold them
   into an unrelated productization commit.

## Verification boundary

`tests/sdk_external_consumer.rs` constructs an unrelated Cargo project whose
source imports only `dicom_test_suite::sdk`. It executes discovery, typed
composition, schema-bound manifest access, and typed cancellation failure with
isolated build state. `docs/sdk-guide.md` and the module Rustdoc provide the
supported migration target. Packaged-crate execution is a phase gate and exact
release-candidate execution remains required at S6; this audit does not claim
either release target.
