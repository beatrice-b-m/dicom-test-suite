# Standalone product status

**Recorded:** 2026-08-31

**Contract:** `docs/standalone-productization-plan.md`

**Release readiness:** not ready; S0 through S4 and S5.1-S5.4 are complete

## Current gate state

| Gate | State | Current evidence or blocker |
| --- | --- | --- |
| S0 — product contract | Complete | ADR 0002 fixes CLI-primary/SDK-secondary integration and the three disjoint evidence workflows. The compatibility policy, CLI envelope/registry schemas and fixtures, current-command error mappings, structural-assembly design, and official-source foundation are committed and tested. |
| S1 — relocatable resources | Complete | Versioned embedded resources cover all first-party catalogs, recipes, schemas, locks, configs, and small assets. Production lookups are audited, explicit roots are integrity checked, manifests project resource identity, and the installed binary passed every resource-backed workflow from three unrelated directories. |
| S2 — automation protocol | Complete | CLI API `1.0.0` provides versioned discovery and command results, stable error codes and exit classes, typed file outcomes, explicit raw-report migration, warning-denied builds, and a schema-driven Python subprocess gate from outside the repository. |
| S3 — Rust SDK | Complete | The supported `dicom_test_suite::sdk` facade provides integrity-checked resources, typed discovery/compose/validate/report outcomes, schema-bound manifests, explicit asset roots, cancellation, stable errors, compiled docs, and a packaged-crate side-project gate. |
| S4 — structural assembly | Complete | The packaged CLI and SDK accept versioned bounded structural requests, use the neutral plan/executor/writer spine, validate exact values and bulk, publish no-IOD-claim manifests/reports, and pass positive, adversarial, transaction, determinism, and external-consumer gates. |
| S5 — packaging and guides | In progress | S5.1-S5.4 are complete: the verified source crate, target-bound native archive, installed operating model, and relocatable neutral example set are qualified. The maintainer release procedure remains open; Linux x86_64 remains unclaimed until its own archive passes the same gate. |
| S6 — release qualification | Not started | Existing source-tree qualification is strong, but no exact packaged release candidate has passed the black-box, relocation, SDK, assembly, or terminal security matrix. |
| S7 — promotion | Not started | The README still leads with `cargo run`; standalone release gates and compatibility ownership are not promoted. |

No terminal acceptance gate has passed against a standalone release candidate.
Existing curated and qualified-composition evidence remains valid only within
its recorded scope and is not being reinterpreted as standalone-product
evidence.

## S5.1 Cargo package gate

S5.1 completed on 2026-08-31 at `c190b86`. The crate now declares its public
repository, homepage, documentation, README, categories, keywords, standalone
description, and `MIT OR Apache-2.0` license texts. Its explicit include set
contains the source, first-party resources, qualification evidence, tests, and
CI contract required to verify an extracted crate. It excludes virtual
environments, Python bytecode, caches, egg-info, and generated backend build
trees.

The exact clean package command passed without metadata warnings:

```sh
cargo package --locked --offline
```

It produced 765 files, 13.5 MiB uncompressed and 2.2 MiB compressed. The crate
SHA-256 is
`603783bc6f3877accf0f6d552696f6f37ba2f3d30b9192ab5aef7b1ea5f0643e`.
Cargo verification completed in 46.17 seconds.

The extracted crate's complete `--all-targets --no-default-features` test
inventory was exercised with fail-fast resumption, so already-passed
heavyweight prefixes were not repeated after a localized repair. The initial
run passed 472 library tests with two intentional ignored process fixtures and
all integration binaries through the first failure. Each failure was repaired
and rerun from the exact extracted crate; the final package-sensitive bundle
passed 96 tests covering P8 composition, quantitative generation, curated
capability isolation, product-resource lookup and integrity, project
artifacts, the SDK facade, and public composition. Every remaining integration
binary then passed. The expensive measured slices were 681.37 seconds for
curated byte parity, 679.23 seconds for stress projection, 684.65 seconds for
streamed stress execution, 691.71 seconds for all-profile generation, 689.55
seconds for direct WSI parity, and 746.14 seconds for WSI pyramid generation.

Qualification exposed and closed three boundaries rather than weakening them:
prepared-backend tests now accept an explicit locked interpreter; the curated
unavailable-capability baseline removes ambient backend configuration from its
child process and excludes elapsed-time observations from its deterministic
digest; and the SDK names its default catalog through `ProductResources`
instead of an ambient path literal. Missing prepared capability still fails
closed unless the exact external interpreter is supplied.

S5.2 through S5.5 remain open. This crate is source-package evidence, not the
target-specific release archive or release candidate, and no terminal matrix
row is promoted by S5.1.

## S5.2 native archive gate

S5.2 completed on 2026-08-31 at `0348549`. The release builder refuses dirty
sources by default, builds a locked target-specific binary, verifies its
reported target and feature set, and archives the executable with both project
licenses, operating documentation, `Cargo.lock`, target-filtered dependency
license notices, machine-readable version and capability results, and a
schema-valid per-file resource manifest. A checksum is emitted beside the
archive. Discovery advertises release-manifest schema `1.0.0`.

The focused source qualification passed one archive relocation test, three
capability tests, three quantitative tests, six resource tests, and 73 schema
tests. The archive test verifies every payload size/hash, dependency notice
coverage, the manifest schema, archive checksum, unrelated-CWD version and
capability discovery, smoke generation, and strict validation.

The first clean optimized artifact was then built and exercised directly:

```text
archive: dicom-test-suite-0.1.0-aarch64-apple-darwin.tar.gz
archive SHA-256: 30fa8cda0114d660f92aaeebe51ab5febca80d9673242d843669617dda18a63a
source revision: 0348549d2c38cbe60f918ad478f0acbc34d082f4
source dirty: false
target: aarch64-apple-darwin
enabled features: []
embedded resource-set SHA-256: 49031f830d6e4def84244b4deb24f72d59561dc97c46c4353d42766991ac13c7
manifest payload files: 13
target-filtered third-party packages: 107
```

From an unrelated extracted directory, the exact optimized binary passed
`version --format json`, `capabilities --format json`, smoke generation with
seed 1, strict validation with three files checked and zero failures, and JSON
report execution. `shasum -a 256 -c` passed from the distribution directory.

Only macOS arm64 is qualified by this evidence. Linux x86_64 is not available
in this host environment and remains an explicit blocker to a general release;
it is not inferred from the portable builder or source tests. S5.3-S5.5 and all
terminal release-candidate rows remain open.

## S5.3 installed operating-guide gate

S5.3 completed on 2026-08-31 at `e5eb854`. The release archive now carries
dedicated installation/upgrade and automation/agent guides in addition to the
generation, SDK, assembly, compatibility, and dated status documents. The
guides distinguish installed-product usage from contributor builds and cover
archive selection, checksum verification, relocation, discovery, workflow
choice, stdout/stderr and exit contracts, manifest-driven artifact discovery,
strict validation, reproducibility classes, optional-runtime trust, side-by-
side upgrades, and explicit unavailable handling.

Focused verification passed two documentation-contract tests and the current-
target release archive test. The latter exercised the documented installed
`version`, `capabilities`, `list-cases`, smoke `generate`, `validate`, and
versioned JSON `report` commands from an unrelated directory, in addition to
archive checksum, extraction, manifest, file-hash, license-notice, and resource
identity checks. The harness caught and corrected an invalid global placement
of `--cli-api`; the operating guide now reflects the executable source of truth:
machine-only commands already emit the current envelope, while the historical
raw-report boundary selects `--cli-api 1.0.0` on `report` itself.

S5.4-S5.5 remain open. Existing composition and assembly guides are current,
but the small neutral installed example files and their complete CI execution
have not yet been added. No terminal acceptance row is promoted by S5.3.

## S5.4 installed example gate

S5.4 completed on 2026-08-31 at `4a56509`. Every release archive now contains
five self-contained JSON examples: raw 2x2 grayscale and RGB composition,
standard/private/Sequence metadata composition, a two-instance resolved
reference, and structural assembly with standard/private/Sequence values and
inline pixel data. All identifiers and bytes are synthetic and non-PHI; no
example depends on the checkout, a network, an external provider, or a caller
asset file. The installed examples guide supplies exact black-box commands and
retains the qualified-template, same-project-validation, and structural
no-IOD-claim boundaries.

Initial execution exposed a strict-validation defect for materialized inline
content. Commit `12e8cfc` corrected native composition manifests to hash the
resolved plan after materialization evidence is projected, so the published
content projection and `resolved_plan_sha256` are reconstructable. The
regression passed all six composition CLI tests and four P2 end-to-end tests;
the failure was repaired rather than excluding raw-pixel validation.

Focused gate verification passed three installed-document contract tests and
the current-target archive test:

```sh
cargo test --locked --no-default-features --test standalone_docs
cargo test --locked --no-default-features --test release_archive
git diff --check
```

The 21.57-second archive gate built and extracted the archive once, then ran
each of the five examples twice with seed 1 through the extracted executable
from an unrelated working directory. Every run used a fresh output root,
passed strict `validate`, passed versioned JSON `report`, retained the same
corpus-plan identity, and emitted byte-identical DICOM instances across the
pair. Direct pre-archive execution of all five examples also passed compose or
assemble, validate, and report.

S5.5 remains open. The examples qualify the installed workflow but do not
replace the maintainer release procedure, an exact clean release candidate, or
the target-specific terminal matrix. Linux x86_64 remains explicitly
unclaimed, and no terminal acceptance row is promoted by S5.4.

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

S5.5, S6, S7, and every terminal acceptance row remain open. S5.1-S5.4
establish package, archive, guide, and installed-example contracts, but not the
maintainer release procedure, an exact release candidate, either general
release target, or the terminal external-consumer matrix.
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

### S4 — structural assembly: complete

**Completed:** 2026-08-31

**Qualification source commit:** `50bcb44`

**Commits:**

- `b7c88cf`, `8958888`, `74c9893` — versioned typed request parsing,
  deterministic neutral `CorpusPlan` resolution, and publication through the
  sole shared Part 10 materializer and atomic executor;
- `c692a22`, `f5042c8` — integer, float, double-float, waveform, PDF, mesh,
  and general bulk placement with exact size/hash/shape/padding provenance,
  deterministic private-block allocation, and worker-count-independent plan
  identity;
- `a2f7a0a`, `8b8539b`, `96d26de` — structural manifest/report projection,
  exact reopened element/Sequence/private/bulk/reference validation, semantic
  tamper detection, and resolved identity/provenance evidence;
- `7618b49`, `8c5c6f4`, `75cbf80` — capability discovery, stable CLI result
  and error surfaces, and typed SDK assembly requests/outcomes/cancellation;
- `71170e8`, `133c841`, `82f7d29` — protected-field, malformed value, frame,
  traversal, symlink, resource, cancellation, destination-conflict, cleanup,
  and concurrent publication-race qualification;
- `760ef87` — schema-only Python and unrelated Rust consumers covering
  structural dry-run, publication, validation, report, and no-claim evidence;
- `0b19a18` — refreshed the three Cargo-lock backend fingerprints after the
  S4.1 dependency change, restoring fail-closed smoke generation; and
- `50bcb44` — current CLI, SDK, generation, workflow-selection, and structural
  assembly operating guides.

Discovery now advertises only Implicit and Explicit VR Little Endian and the
eleven qualified structural content categories. Structural output records
`iod_conformance = "not_assessed"` at run, instance, and report levels; its
schema forbids curated/template/profile claims, and reports cannot join the
coverage matrix. Explicit caller UIDs and deterministic UIDs remain
distinguishable per role. Caller asset paths are relative to an explicit root;
every component is checked for symlinks and containment before reading.

**Focused and phase-gate verification:**

```sh
cargo test --locked --no-default-features \
  --test assembly_request --test assembly_plan --test assembly_run \
  --test assembly_qualification --test assemble_cli \
  --test capabilities_cli --test sdk_facade --test cli_contract_schema \
  --test schema_artifacts --test generation_backend_contract \
  --test generation_backend_artifacts
cargo test --locked --no-default-features --doc
RUSTFLAGS='-D warnings' cargo check --locked --all-targets --no-default-features
cargo fmt --all -- --check
git diff --check
cargo package --locked --offline --no-verify
cargo build --manifest-path <extracted-package>/Cargo.toml \
  --locked --offline --no-default-features
python3 <extracted-package>/tests/black_box_cli_consumer.py \
  <extracted-package>/target/debug/dicom-test-suite <extracted-package>
DTS_SDK_PACKAGE_ROOT=<extracted-package> \
  cargo test --locked --no-default-features --test sdk_external_consumer
```

The focused phase bundle passed 116 tests: four CLI assembly, six request,
six plan, six execution/transaction, two all-content materialization, three
capability, seven SDK facade, four CLI contract, five backend-lock, and 73
schema/artifact tests. The compiled SDK doctest passed. The warning-denied
all-target check passed in 21.80 seconds. The source-tree Python and Rust side
projects passed in 10.13 and 33.56 seconds respectively.

The fresh offline package contained 767 files and was 2,340,853 bytes with
SHA-256
`7d82cf8d7d2b237572380c4b155605c108fa8e7ea9c522a3f6310567aec656e8`.
Its extracted binary passed the Python CLI consumer, and its extracted crate
passed the Rust side-project consumer in 35.09 seconds. The exact temporary
extraction directory was removed after qualification. Cargo still warned that
documentation, homepage, and repository metadata are missing, and the archive
has not yet been reduced to the intentional S5 release set. Those remain S5
blockers; this package is phase evidence, not a release candidate.

**Gate conclusion:** a side project can request deterministic caller-owned
elements and typed pixels without adding a repository recipe, through either
the packaged CLI or supported SDK, while evidence consumers cannot mistake the
result for qualified-template or curated coverage. S5-S7 and every terminal
release-candidate row remain open.
