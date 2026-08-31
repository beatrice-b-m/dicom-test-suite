# Standalone product status

**Recorded:** 2026-08-31

**Contract:** `docs/standalone-productization-plan.md`

**Release readiness:** not ready; S0 through S3 are complete and S4 is next

## Current gate state

| Gate | State | Current evidence or blocker |
| --- | --- | --- |
| S0 — product contract | Complete | ADR 0002 fixes CLI-primary/SDK-secondary integration and the three disjoint evidence workflows. The compatibility policy, CLI envelope/registry schemas and fixtures, current-command error mappings, structural-assembly design, and official-source foundation are committed and tested. |
| S1 — relocatable resources | Complete | Versioned embedded resources cover all first-party catalogs, recipes, schemas, locks, configs, and small assets. Production lookups are audited, explicit roots are integrity checked, manifests project resource identity, and the installed binary passed every resource-backed workflow from three unrelated directories. |
| S2 — automation protocol | Complete | CLI API `1.0.0` provides versioned discovery and command results, stable error codes and exit classes, typed file outcomes, explicit raw-report migration, warning-denied builds, and a schema-driven Python subprocess gate from outside the repository. |
| S3 — Rust SDK | Complete | The supported `dicom_test_suite::sdk` facade provides integrity-checked resources, typed discovery/compose/validate/report outcomes, schema-bound manifests, explicit asset roots, cancellation, stable errors, compiled docs, and a packaged-crate side-project gate. |
| S4 — structural assembly | Not started | There is no `assemble` CLI/SDK workflow, assembly request schema, or structural-assembly manifest branch. |
| S5 — packaging and guides | Not started | Cargo metadata and public quick starts do not yet satisfy the release-archive and installed-product contract. |
| S6 — release qualification | Not started | Existing source-tree qualification is strong, but no exact packaged release candidate has passed the black-box, relocation, SDK, assembly, or terminal security matrix. |
| S7 — promotion | Not started | The README still leads with `cargo run`; standalone release gates and compatibility ownership are not promoted. |

No terminal acceptance gate has passed against a standalone release candidate.
Existing curated and qualified-composition evidence remains valid only within
its recorded scope and is not being reinterpreted as standalone-product
evidence.

## Baseline audit evidence

The following read-only checks established the initial state on 2026-08-31:

```sh
git status --short
git log --oneline -12
rg -n "ProductResources|cli_api_version|structural_assembly|assemble|standalone|capabilities|version --format|resource-root|sdk" \
  src tests docs schemas Cargo.toml .github
rg -n "CARGO_MANIFEST_DIR|cases/|templates/|schemas/|standards\\.lock|generation-backends\\.lock|transfer-syntax/" \
  src build.rs
```

The worktree was clean before productization work began. The newest commit was
`85c14d7`, which added the proposed standalone plan. The searches found the
existing plan terminology but no implemented standalone public contracts. They
also identified ambient production lookups in `src/main.rs`, `src/lib.rs`,
`src/conformance.rs`, `src/composition/run.rs`, and related defaults. Tests and
test-only fixtures are not classified by this initial production audit.

## Preserved boundaries

- `generate` remains registry-led curated evidence.
- `compose` remains catalog-qualified and does not project curated case or
  profile credit.
- Planned, feature-gated, backend-unavailable, validator-unavailable, and
  peer-unavailable outcomes remain explicit.
- `negative`, `fuzz`, opt-in `stress`, media, and protocol evidence retain
  their isolation rules.
- Same-project validation is not independent conformance evidence.
- No structural-assembly output may claim IOD conformance or qualified
  template coverage.

## Remaining blockers

S4 through S7 and every terminal acceptance row remain open. S3 qualifies the
SDK surface through a temporary Cargo package, but does not establish the S5
package metadata/archive contract, an exact release candidate, structural
assembly, either release target, or the terminal external-consumer matrix.
Linux x86_64 and macOS arm64 cannot be claimed as standalone release targets
until the exact target archives pass the required external-consumer matrix.
Optional runtimes are accepted only when discovered and fingerprint-qualified;
their absence will remain an unavailable result rather than a waived gate.

## Update rule

This record is updated at each phase gate with the exact commit, commands,
artifact identities, results, unsupported targets, and remaining blockers. A
gate is marked complete only after its acceptance criteria pass without
narrowing the product contract.

## Gate evidence

### S0 — product contract: complete

**Completed:** 2026-08-31

**Commits:**

- `2ec2791` — accepted standalone product ADR with the supported compatibility
  surface, non-goals, and change-classification test;
- `6bf6da3` — independent version domains, additive/breaking examples, support
  windows, negotiation, deprecation, and upgrade evidence policy;
- `47d16d9` — CLI success/error/registry schemas, append-only code meanings,
  six exit classes, every current public command's failure-stage mapping, and
  positive/adversarial fixtures; and
- `3ae4002` — structural request/manifest design plus the locked DICOM
  standards foundation, including protected fields, typed bulk, validation
  ceiling, and permanent `iod_conformance = "not_assessed"` semantics.

**Verification:**

```sh
cargo fmt --check
cargo test --locked --no-default-features \
  --test cli_contract_schema --test schema_artifacts
git diff --check
```

The focused run passed 4 CLI contract tests and 73 schema artifact tests.
Envelope adversarial fixtures rejected wrong API versions, unknown top-level
fields, non-namespaced codes, and nested context objects that could expose
private staging details. The error-registry test proved unique registered codes,
the exact `0/2/3/4/5/6` exit set, referentially valid failure mappings, and
coverage for all 17 current public command/subcommand forms. Existing library
dead-code warnings were present during the Rust test build; they are not
machine-command stdout, and eliminating compiler/debug leakage remains an S2
acceptance item.

**Gate conclusion:** a proposed change can be classified as public compatible,
versioned-breaking, internal, curated, qualified composition, or structural
assembly by the accepted ADR and normative policy examples. No undocumented
judgment or IOD inference is needed. This gate does not claim that any S1-S7 or
terminal release-candidate criterion has passed.

### S1 — relocatable resources: complete

**Completed:** 2026-08-31

**Commits:**

- `4c8807a`, `d3907c7` — classify production filesystem access and enforce a
  zero-allowlist audit for ambient first-party resource lookups;
- `d98f550`, `ed5b87b`, `06abd91`, `6478c32` — embed the versioned product
  resource set and route generation, composition, reporting, conformance, and
  CLI defaults through `ProductResources`;
- `8960dd1`, `46bd0c8` — fail closed on explicit-root hash drift and project
  complete resource identity into curated and composition manifests;
- `8628ad0` — execute templates, compose, generate, validate, and report from
  three unrelated working directories using a copied installed binary with
  checkout/cache discovery inputs removed;
- `2393085`, `36ff1ed`, `b37f093`, `f757030`, `d533395`, `56709e6` — retain
  compatibility expectations, require explicit prepared external backends in
  qualification tests, preserve transaction failure coverage independently of
  repository files, and accept every still-supported manifest schema version.

**Resource identity:** embedded set version `1.0.0`, 224 logical resources,
resource-set SHA-256
`a2bed94aa5b3f30de8ecb94d4fe3531cba7c5763e196350f6db4ddd84a3c6809`.
An explicit root must contain the identical logical set and bytes. Drift fails
before materialization with `evidence.integrity.failed`. The prepared
highdicom/pydicom runtime remains an explicit external capability selected by
`DTS_HIGHDICOM_PYTHON`; its absence remains unavailable, not passed.

**Focused verification:**

```sh
cargo fmt --all -- --check
cargo test --locked --no-default-features \
  --test product_resources \
  --test product_resource_lookup_audit \
  --test standalone_generate_resources \
  --test standalone_compose_resources \
  --test installed_artifact_relocation \
  --test cli_contract_schema \
  --test schema_artifacts
cargo test --locked --no-default-features \
  --test composition_quantitative \
  --test composition_structured_reports \
  --test composition_p8_qualification \
  --test composition_curated_migration
cargo test --locked --no-default-features \
  --test curated_generate_integration
git diff --check
```

The relocation test passed from all three unrelated directories. Resource
identity/integrity, ambient-lookup audit, standalone generation/composition,
CLI schema, and all 73 schema-artifact tests passed. Explicit prepared-backend
qualification passed for the full catalog, quantitative content, structured
reports, and curated migration. The four curated generation tests passed,
including a post-planning publication failure that left no destination or
private staging.

**Regression closure and test modularity:**

The repository baseline was exercised across all targets. The exact command

```sh
cargo test --locked --all-targets --no-default-features
```

passed 472 library tests (2 ignored) and every integration target through
`general_ecg_waveform`, then exposed a compatibility-test helper that expected
the manifest version schema to remain a single `const`. After `56709e6`, the
affected `generate_cli` target passed 7/7, resource tests passed 6/6, schema
tests passed 73/73, standalone generation passed 1/1, and a lexical tail bundle
ran every integration target after `generate_cli` to completion. This is
segmented regression evidence; the exact all-targets command has not yet
produced one uninterrupted exit-zero run at this commit and remains mandatory
against the terminal release candidate.

Measured heavyweight tests are deliberately isolated:

- `case_recipe_catalog::data_first_sc_and_metadata_values_and_hashes_match_current_generator_bytes`
  — 683.69 seconds;
- `curated_stress_manifest::typed_stress_projection_matches_frozen_file_values_and_resources`
  — 681.02 seconds;
- `curated_stress_sc_integration::all_stress_sc_cases_execute_through_private_streaming_services`
  — 688.21 seconds;
- `generate_cli::generate_command_writes_all_profile_union_and_skips_planned_cases`
  — 686.18 seconds;
- `wsi_direct_plan::ordinary_wsi_direct_plans_match_fresh_seed_one_bytes_and_manifest_facts`
  — 691.07 seconds; and
- `wsi_pyramid::stress_profile_emits_complete_three_instance_wsi_pyramid`
  — 685.37 seconds.

Subsequent work uses dependency-aware tiers: formatting/diff checks and named
tests per commit; a small subsystem bundle per numbered plan item; a
phase-specific black-box or packaged gate at phase completion; and the
heavyweight/full matrix only for changes to its generation, execution,
projection, manifest, resource, stress, or WSI dependency surface and for the
exact S6/S7 release candidate. A passed broad gate is rerun only when later
changes intersect that gate's dependency surface. This preserves the terminal
acceptance matrix without spending roughly 68 minutes of heavyweight runtime
on unrelated CLI or SDK edits.

The builds emitted 38 existing dead-code warnings. They did not enter binary
runtime stdout, but clean machine stdout/stderr and elimination of compiler or
debug leakage remain explicit S2 work.

**Gate conclusion:** the binary resolves first-party runtime resources without
the checkout or Cargo cache, works after relocation for every S1 workflow, and
fails explicit resource drift closed. No release target or terminal matrix row
is claimed yet; packaging, external-consumer, exact release-candidate, and
platform qualification remain open.

### S2 — automation protocol: complete

**Completed:** 2026-08-31

**Commits:**

- `064b570`, `dcd4c6b`, `f07df3b` — version, conservative capability
  discovery, and advertised result-schema versions;
- `390ad21`, `8bd6177` — shared machine failures, stable workflow taxonomy,
  the exact `0/2/3/4/5/6` exit classes, and human-detail separation;
- `4daec6c`, `5ee2397`, `7b78302`, `784fa0a`, `811d731`, `fe30992`,
  `bbace41`, `580775d` — versioned typed success results across generation,
  composition, templates, validation, reporting, inventory, standards,
  conformance, and interoperability;
- `c5b0270` — warning-denied all-target build hygiene;
- `2d6116a` — public automation, stream, exit, result-location, and raw-report
  migration documentation; and
- `9c306ee` — schema-driven Python subprocess consumer and stable machine-error
  regression assertions.

Both discovery responses derive live data from embedded resources, compiled
features, configured external tools, transfer-syntax availability, schemas,
and resource ceilings. An unavailable runtime or the not-yet-implemented
assembly workflow remains explicitly unavailable. File-producing publish and
dry-run results share one typed shape and distinguish state through fields.
Historical report JSON remains byte-for-byte the raw result unless callers
explicitly select `--cli-api 1.0.0`, which wraps it at `result.report`.

**Phase-gate verification:**

```sh
cargo test --locked --no-default-features \
  --test capabilities_cli --test version_cli --test cli_contract_schema \
  --test cli_error_golden --test compose_cli --test templates_cli \
  --test list_cases_cli --test standards_cli --test conformance_check_tools \
  --test interoperate_cli --test coverage_gaps_cli --test schema_artifacts \
  --test non_rust_cli_consumer
cargo test --locked --no-default-features --test generate_cli \
  generate_machine_result_is_clean_typed_and_manifest_bounded
cargo test --locked --no-default-features --test generate_cli \
  generate_command_writes_smoke_part10_files_and_manifest
RUSTFLAGS='-D warnings' cargo check --locked --all-targets --no-default-features
cargo fmt --all -- --check
git diff --check
```

The phase bundle passed 128 tests, including the black-box Python consumer,
from an unrelated temporary working directory in 23.82 seconds. The consumer
uses committed schemas and typed fields for discovery, inventory, templates,
standards, conformance-tool discovery, compose dry-run and publication,
validation, raw/wrapped report equality, real smoke generation, and exits
`2` through `6`; it does not parse diagnostic prose. The two named generation
tests passed in 1.85 and 1.72 seconds. The warning-denied all-target check
passed in 20.75 seconds, followed by formatting and diff hygiene.

The phase gate initially caught two legacy tests that scraped detailed machine
error prose for interoperability and template failures. Their product behavior
was already correct; the tests now assert the registered error code and exit
class, preserving prose as a human-output concern.

**Gate conclusion:** a non-Rust subprocess consumer can select and execute the
current workflows using schemas, exit classes, and error codes alone. This is
source-build evidence, not packaged release-candidate evidence. All S3-S7 and
terminal acceptance gates remain open.

### S3 — supported Rust SDK: complete

**Completed:** 2026-08-31

**Qualification source commit:** `e897a69`

**Commits:**

- `bad24ab` — supported facade, integrity-checked resource constructors,
  typed discovery, and non-exhaustive stable public errors;
- `d8481cd` — file/byte qualified composition, typed publish/dry-run outcomes,
  cancellation token, and schema-bound manifest wrapper;
- `58686e9` — persisted-manifest schema validation plus typed validation and
  report requests/outcomes;
- `df693df` — deterministic file/byte equivalence, explicit caller-asset-root,
  cancellation error, and no-publication tests;
- `71811e9`, `e897a69` — an isolated external Cargo consumer importing only
  `dicom_test_suite::sdk` across discovery, compose, validate, report, typed
  manifest access, and cancellation;
- `dc1fb17` — compiled Rustdoc and the public SDK operating guide; and
- `dd3a704` — legacy public-module inventory and deliberate semver retention
  plan before any visibility reduction.

File and byte requests enter the same byte-based composition pipeline and use
an explicit caller-asset root. Published manifests are re-read from disk and
validated against the immutable embedded schema before the SDK returns their
typed wrapper. Public primary outcomes contain no `serde_json::Value`.
Cancellation shares the underlying executor token; both an immediate SDK
cancellation and an in-flight provider cancellation publish no destination.

**Focused and phase-gate verification:**

```sh
cargo test --locked --no-default-features --test sdk_facade
cargo test --locked --no-default-features --test composition_resources \
  cancellation_terminates_a_provider_and_publishes_nothing
cargo test --locked --no-default-features --doc
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-default-features --no-deps
RUSTFLAGS='-D warnings' cargo check --locked --all-targets --no-default-features
cargo fmt --all -- --check
git diff --check
cargo package --locked --offline --no-verify
DTS_SDK_PACKAGE_ROOT=<extracted-package-root> \
  cargo test --locked --no-default-features --test sdk_external_consumer
```

The six facade tests passed in 1.23 seconds. The existing in-flight provider
cancellation test passed in 0.90 seconds. The compiled Rustdoc example passed,
warning-denied documentation passed, and the warning-denied all-target check
passed in 19.73 seconds. The isolated side project passed against the extracted
package in 32.22 seconds with clean runtime stdout/stderr.

The temporary qualification package contained 752 files and was 2,307,672
bytes with SHA-256
`6f81dc0e12f1bb266ddc6a6c5e2137c426161d03d58a5b80c78414af318f0fd1`.
It was built offline with `--no-verify` solely to prove packaged-crate SDK
consumption. Cargo warned that documentation/homepage/repository metadata is
missing, and the package is broader than the intentional release set. Those
are explicit S5 blockers; this artifact is not a release candidate and no
packaging or target-platform terminal row has passed. The exact temporary
extraction directory was removed after qualification.

**Gate conclusion:** an unrelated Rust project can depend on the packaged
crate through the documented facade with typed results and stable errors,
without importing internal modules. S4-S7 and every terminal row remain open.
