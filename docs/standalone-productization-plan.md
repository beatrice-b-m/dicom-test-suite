# Standalone DICOM Generation Productization Plan

**Status:** proposed execution plan; current repository behavior remains the
source of truth until each gate is implemented and promoted

**Prepared:** 2026-08-31

**Goal:** make `dicom-test-suite` a versioned, relocatable, automation-safe DICOM
generation product that side projects can consume as a black-box executable or
a narrow Rust SDK without depending on the repository layout or internal
modules.

## 1. Target Outcome

The completed product has three public workflows over the existing shared
plan-first executor:

1. `generate` emits registry-selected, evidence-bearing test corpora exactly as
   it does today.
2. `compose` emits standards-qualified, caller-customized objects through the
   versioned template catalog. It remains the default choice for valid DICOM.
3. `assemble` emits deterministic, structurally well-formed DICOM Part 10 from
   a caller-owned element tree and typed bulk-content declarations when no
   qualified template fits. It explicitly makes no IOD-conformance or curated
   coverage claim.

The CLI is the primary language-neutral integration boundary. A deliberately
small Rust SDK exposes the same request, result, error, resource, and
provenance contracts. Both interfaces use embedded first-party resources by
default, run without network access, and work from any current working
directory after the source checkout has been removed.

At program completion:

- a consumer can download or install one supported artifact, inspect its
  version and capabilities, submit a versioned request, and parse a versioned
  result without reading this repository;
- all successful machine-facing commands and all failures have documented,
  schema-validated output shapes and stable error codes;
- installed-binary, packaged-crate, and external-consumer tests prove that no
  public workflow depends on the repository working directory or a compile-time
  source path;
- the current qualified composition catalog, deterministic output, atomic
  publication, bounded resources, validation, and evidence boundaries remain
  intact;
- arbitrary caller attributes, private elements, recursive Sequences, and
  typed pixel or other bulk payloads have a supported structural assembly
  route without being mislabeled as qualified DICOM; and
- release artifacts, checksums, licenses, compatibility policy, installation
  instructions, and end-to-end human and agent examples are published
  together.

## 2. Product Boundary

### 2.1 Qualified composition

`compose` remains closed to catalog-listed templates, transfer syntaxes,
content slots, and reference roles. It owns valid deterministic defaults,
protected structural attributes, bundle closure, template validation, and the
existing independent-evidence accounting.

Use `compose` whenever the requested SOP Class and content model are available
in `templates list`. A composition manifest uses `run.kind = "composition"`
and never projects a registry `case_id` or profile.

### 2.2 Structural assembly

`assemble` is a separate expert surface for deterministic Part 10 construction.
It accepts:

- an explicit SOP Class UID, modality when applicable, transfer syntax from
  the assembler capability list, and deterministic or explicit identities;
- standard elements by tag or keyword with explicit typed values;
- explicit-VR elements not present in the bundled dictionary;
- managed private creators and private elements;
- primitive, multi-valued, binary, empty, and recursive Sequence values;
- typed native Pixel Data, Float Pixel Data, Double Float Pixel Data, waveform,
  document, mesh, or other supported top-level bulk values; and
- multi-instance identity sharing and logical references where the generic
  reference model can express them.

The assembler still protects File Meta Information consistency, SOP instance
identity, transfer-syntax encoding, generated UID fields, and typed bulk-shape
attributes from contradictory raw overrides. It enforces serialization safety,
VR/value representation, path safety, resource bounds, deterministic identity,
hashing, cleanup, and atomic publication.

It does not infer an IOD, synthesize missing Type 1/2 attributes, validate
conditional modules, or claim that the selected elements constitute a valid
instance of the supplied SOP Class. Its manifest uses
`run.kind = "structural_assembly"`, records `iod_conformance = "not_assessed"`,
and contains no template qualification or curated coverage credit. Caller-
defined malformed byte streams and post-serialization corruptions remain out
of scope for this program; they require a separately designed mutation surface
that preserves negative-profile isolation.

### 2.3 Curated generation

`generate` remains registry-led. This plan must not change profile membership,
case identity, status, availability, standards evidence, determinism
classification, independent-validation meaning, or the isolation of valid,
legacy, stress, negative, fuzz, media, and protocol evidence.

### 2.4 Non-goals

- A network service, daemon, hosted API, or multi-tenant execution platform.
- Network fetching of input content, templates, codecs, validators, or locks.
- Domain-specific medical image synthesis, anatomy generation, or training-data
  generation.
- A promise that arbitrary structural assembly is DICOM IOD conformance.
- Automatic decoding of every image or scientific container format. Container
  support is added only through separately qualified typed adapters.
- Portable OS-level sandboxing of untrusted provider executables.
- Bundling every optional external codec or independent validator into the
  base executable.
- Windows release support until provider process, path, and atomic-publication
  contracts have equivalent qualification there.

## 3. Non-Negotiable Invariants

1. Executable behavior, schemas, the registry, generated manifests, the
   transfer-syntax capability matrix, and current dated evidence retain the
   source-of-truth order in `AGENTS.md`.
2. Curated, qualified-composition, and structural-assembly manifests remain
   distinguishable and cannot inflate one another's coverage or evidence.
3. Missing features, codecs, providers, validators, and peers remain explicit
   unavailable outcomes, never implied passes.
4. The default product performs no download or network access at generation
   time.
5. First-party runtime resources are immutable, versioned, hash-bound, and
   available without the repository checkout.
6. Caller assets remain external to the executable, are resolved relative to
   an explicit request root, and are subject to the existing path, symlink,
   file-count, size, and hash controls.
7. Output remains plan-first, bounded, validated according to its declared
   evidence class, cleaned, and atomically promoted without overwriting an
   existing root.
8. The manifest remains the authority for emitted artifacts; consumers must
   not be required to discover files recursively or infer one file per logical
   request.
9. Byte-stable output stays byte-stable across working directories, staging
   names, and supported parallelism when all recorded inputs and product
   identities match. Semantic-stable codec output retains its narrower claim.
10. No generated DICOM, ordinary run manifest/report, caller asset, cache,
    private key, or release signing secret is committed.

## 4. Supported Integration Contracts

### 4.1 Runtime resource model

Introduce one `ProductResources` abstraction used by every public frontend.
Production defaults are embedded in or installed beside the product artifact
and include all first-party data needed for the selected workflow:

- composition and assembly schemas;
- template catalog, inventory, and qualification-evidence identities;
- case registry and modular recipe documents required by shared defaults;
- standards, backend, validator, and transfer-syntax lock/configuration data
  required by the invoked command; and
- small first-party binary assets required by built-in materializers.

No production code may open `cases/...`, `templates/...`, `schemas/...`, or a
lock file by an ambient CWD-relative path. No runtime decision may depend on
`CARGO_MANIFEST_DIR` or another compile-time source-tree path.

An explicit `--resource-root PATH` and matching SDK constructor may remain for
repository development and qualified custom catalogs. It must never be an
implicit fallback. The manifest records whether resources were embedded or
explicit, their product/catalog/schema versions, and their hashes. Overrides
must pass the same schema and cross-artifact integrity checks as embedded data.

The implementation may use generated Rust resource tables, a read-only
archive, or a versioned installed directory. The acceptance contract—not the
storage mechanism—is fixed: a release artifact must work after both its build
checkout and the Cargo registry source cache are unavailable.

### 4.2 Version and capability discovery

Add machine-readable discovery commands:

```text
dicom-test-suite version --format json
dicom-test-suite capabilities --format json
```

`version` reports at least product version, CLI API version, target, Rust
toolchain identity, enabled Cargo features, embedded-resource set version, and
resource hashes. `capabilities` reports qualified templates, structural
assembly content and transfer-syntax capabilities, required optional runtimes,
resource ceilings, supported request/result schema versions, and availability
with stable reason codes.

Discovery output is versioned and schema-validated. Consumers must not parse
the human version banner, `--help`, or debug strings to determine capability.

### 4.3 Machine CLI protocol

Every consumer-facing command supports `--format json`. JSON success output is
the only stdout content and follows a common envelope:

```json
{
  "cli_api_version": "1.0.0",
  "command": "compose",
  "status": "success",
  "result": {}
}
```

Machine-mode failure writes one JSON error envelope to stderr and nothing to
stdout:

```json
{
  "cli_api_version": "1.0.0",
  "command": "compose",
  "status": "error",
  "error": {
    "code": "spec.schema.invalid",
    "message": "human-readable summary",
    "context": {},
    "retryable": false
  }
}
```

Add JSON Schemas for success and error envelopes and command-specific result
objects. Do not expose Rust enum `Debug` output as a supported format. Error
codes are namespaced, documented, and append-only within a CLI API major
version. Context values use stable logical identifiers and safe paths; they do
not expose private staging paths.

Document and test these exit classes:

| Exit | Meaning |
| --- | --- |
| `0` | command completed successfully |
| `2` | command syntax, request parsing, or schema error |
| `3` | requested capability or required runtime unavailable |
| `4` | output/path conflict or caller-controlled resource limit |
| `5` | generation, validation, conformance, or evidence failure |
| `6` | unexpected I/O or internal product failure |

Human-readable output may remain concise and command-specific. Documentation
must state that only JSON mode is an automation contract. Existing report
`--format json|markdown` behavior remains compatible while its JSON is wrapped
only under a separately announced CLI API version boundary; migration tests
must prevent accidental breaking changes.

### 4.4 Request and result shapes

Create typed, versioned public models for:

- qualified composition request and outcome;
- structural assembly request and outcome;
- generation request and outcome;
- validation request and outcome;
- capability/version response;
- manifest references and artifact summaries; and
- public errors.

A successful file-producing outcome consistently includes:

- requested output root and manifest path;
- run kind, seed, request schema version, manifest schema version, and product
  version;
- emitted artifact count and output bytes;
- unavailable capability count and stable summaries;
- corpus-plan hash; and
- publication and validation status.

The complete manifest remains in `manifest.json`; the command result does not
duplicate its full contents. Dry-run uses the same envelope and outcome type
with `published = false`, an absent manifest path, and canonical plan previews.
It must not change shape solely because output publication was disabled.

### 4.5 Rust SDK facade

Add a narrow `dicom_test_suite::sdk` facade. Its supported surface should be
small enough to document and maintain, for example:

```rust,ignore
let product = DicomTestSuite::embedded()?;
let outcome = product.compose(ComposeRequest::from_json(spec)?)?;
product.validate(ValidateRequest::new(outcome.output_root()))?;
```

The exact names may change, but the facade must provide:

- embedded and explicit-resource constructors;
- file and byte request entry points with an explicit caller-asset root;
- cancellation for long-running operations;
- typed outcomes and typed manifests or schema-bound manifest wrappers instead
  of `serde_json::Value` tuples;
- a public error type with the same stable code taxonomy as the CLI;
- `#[non_exhaustive]` on public extensible enums and errors;
- no requirement to construct executor, planner, recipe, materializer, or
  internal composition types; and
- Rustdoc examples compiled from an external consumer crate.

Existing public modules remain available during migration, but the
compatibility policy identifies only `sdk`, versioned schemas, and documented
CLI behavior as supported product APIs. Internal exposure is reduced only in a
deliberate semver release with migration notes.

### 4.6 Output and manifest contracts

Keep composition and curated manifest semantics compatible. Add a separate
versioned assembly manifest projection or a discriminated run-kind branch in a
new manifest schema version. All three workflows must share common artifact
identity, path, size, SHA-256, plan hash, resource, validation, and publication
fields wherever the meanings are identical.

Structural assembly records:

- caller request hash and assembly schema version;
- SOP/transfer-syntax identities supplied or derived;
- resolved elements and value provenance;
- typed bulk shape, source identity, and per-frame/value hashes;
- generic Part 10 and data-element validation results;
- `iod_conformance = "not_assessed"`; and
- warnings when dictionary or template evidence is unavailable.

Every schema change includes positive and adversarial fixtures, backward-
compatibility tests for supported prior versions, and explicit migration or
rejection behavior.

## 5. Distribution And Release Contract

### 5.1 Installable artifacts

The first supported release channel is a checksummed native archive containing
the executable, licenses/notices, completion documentation, and any required
beside-binary immutable resources. `cargo install` may be supported as a second
channel only when its installed binary passes the same source-tree-independent
tests.

Release targets begin with the hosts actually qualified by CI and project
users. At minimum, complete Linux x86_64 and macOS arm64 qualification before
claiming a general standalone release. Other targets remain explicitly
unavailable until equivalent tests pass.

Each release publishes:

- semantic product version and immutable source revision;
- target-specific archive and SHA-256;
- dependency and third-party license notices;
- feature set and embedded-resource hashes;
- supported request, result, manifest, catalog, provider, and CLI API versions;
- changelog and migration notes; and
- reproducible installation and verification commands.

Container images and package-manager recipes are optional follow-ups, not
substitutes for the native artifact contract.

### 5.2 Package metadata and versioning

Complete Cargo package metadata with repository, homepage or project URL,
documentation URL, keywords/categories where appropriate, and an intentional
include/exclude set. `cargo package --locked` and verification of the packaged
crate—not the working tree—must pass.

Adopt and document these independent version domains:

- product/crate semantic version;
- CLI API version;
- composition and assembly request schema versions;
- manifest/report schema versions;
- template ID/version and catalog schema version; and
- provider protocol version.

A product patch may add a backward-compatible template, capability, field, or
error code. Removing or changing accepted input, field meaning, error meaning,
default identity/output, or qualified template behavior requires the
appropriate schema/template/CLI major boundary and migration documentation.
Determinism changes require an explicit recipe or template version even when
the product semantic version also changes.

Do not declare product `1.0.0` until the terminal acceptance matrix in this
plan has passed for a release candidate. Pre-1.0 releases still follow the
published compatibility policy for the supported black-box surface.

## 6. Documentation Contract

Create or promote current operating documents for:

1. installation and upgrade by release archive and any secondary channel;
2. a five-minute qualified composition example with raw monochrome and RGB
   pixels;
3. structural assembly with standard, private, Sequence, and pixel elements;
4. template/capability discovery and choosing `generate`, `compose`, or
   `assemble`;
5. machine CLI envelopes, exit codes, error-code catalog, and stdout/stderr
   rules;
6. Rust SDK setup, typed outcomes, cancellation, and error handling;
7. output layout, manifest consumption, unavailable capabilities, and
   reproducibility comparison;
8. optional codecs/providers and their trust/evidence boundaries;
9. upgrading schemas, templates, and product versions; and
10. a concise agent integration recipe that requires capability discovery,
    dry-run, fresh output roots, manifest-driven artifact discovery, validation,
    and explicit unavailable handling.

Examples must run from outside the repository, use only installed public
artifacts, write to fresh temporary roots, and be exercised in CI. Repository
developer commands using `cargo run` remain in contributor documentation but
must not be the only public quick start.

## 7. Phased Execution Plan

Each numbered work item is one reviewable logical unit unless implementation
forces a smaller split. Follow the granular commit policy in `AGENTS.md`; do not
batch unrelated phases into one commit. At every gate, update current operating
docs and the applicable dated status record without rewriting historical plans
as though they were current behavior.

### Phase S0 — Freeze the product contract

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| S0.1 | ADR for CLI-primary black-box integration, SDK-secondary integration, and three workflow/evidence classes. | ADR names compatibility surface and non-goals. |
| S0.2 | Versioning and compatibility policy for product, CLI, schemas, manifests, templates, reports, and providers. | Breaking/additive examples and upgrade rules are testable. |
| S0.3 | JSON schemas for common CLI success/error envelopes and stable error-code registry. | Positive/negative fixtures validate; every current public failure is mapped. |
| S0.4 | Structural assembly specification and manifest design. | Protected fields, typed bulk, validation ceiling, and no-IOD-claim boundary are explicit. |

**S0 gate:** maintainers can classify any proposed public change as compatible,
versioned-breaking, internal, qualified, structural, or curated without relying
on undocumented judgment.

### Phase S1 — Remove repository-layout coupling

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| S1.1 | Inventory every production filesystem lookup and classify it as embedded first-party resource, explicit caller asset, explicit external tool, or output. | Audit test fails on new ambient repository-relative reads. |
| S1.2 | `ProductResources` abstraction with embedded/default and explicit-root implementations. | Catalog, registry, recipes, schemas, locks, configs, and small assets resolve through it. |
| S1.3 | Replace ambient CWD-relative and `CARGO_MANIFEST_DIR` production access. | Release binary works from unrelated CWD after checkout and Cargo source cache are unavailable. |
| S1.4 | Resource identity and integrity projection into capabilities and manifests. | Tampered explicit resources fail with stable codes; embedded hashes are reported. |
| S1.5 | Installed-artifact relocation tests for `templates`, `compose`, `generate`, `validate`, and `report`. | Tests run the artifact from at least three unrelated directories. |

**S1 gate:** the executable is a relocatable product. No documented public
generation workflow requires the repository to exist at runtime.

### Phase S2 — Stabilize automation interfaces

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| S2.1 | `version` and `capabilities` commands with versioned JSON schemas. | Output is sufficient to select a supported request without parsing docs. |
| S2.2 | Shared CLI result/error envelope and command context. | JSON mode has clean stdout/stderr and no compiler warnings or debug formatting. |
| S2.3 | Stable error taxonomy and exit classes across generation, composition, validation, templates, reporting, and discovery. | Golden tests cover representative errors in every class. |
| S2.4 | Typed, consistent outcomes for publish and dry-run. | Dry-run and publish differ by fields, not top-level shape. |
| S2.5 | Backward-compatible handling for existing human output and report JSON. | Existing documented commands and regression tests remain valid. |

**S2 gate:** a subprocess consumer can integrate using schemas, exit classes,
and error codes alone; no string scraping is required.

### Phase S3 — Establish the supported Rust SDK

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| S3.1 | `sdk` facade with embedded resources and typed compose, validate, report, version, and capability requests/outcomes. | External crate compiles without importing internal modules. |
| S3.2 | Schema-bound typed manifest and public error models. | Public APIs return no untyped `serde_json::Value` as their primary outcome. |
| S3.3 | Cancellation and explicit caller-asset-root support. | File and byte requests share the exact pipeline and cancellation cleanup contract. |
| S3.4 | Rustdoc, compiled examples, and supported-API declaration. | `cargo doc` and external doctest/consumer project pass from the packaged crate. |
| S3.5 | Compatibility audit of currently public internal modules. | Deprecation or retention plan is documented before visibility changes. |

**S3 gate:** a Rust side project can depend on the packaged crate through one
documented facade with typed results and stable errors.

### Phase S4 — Add structural assembly

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| S4.1 | Versioned assembly request schema and typed parser. | Standard, unknown explicit-VR, managed private, binary, empty, multi, and recursive Sequence fixtures pass. |
| S4.2 | Generic identity, attribute, reference, and typed bulk planning into `CorpusPlan`. | No second writer or publication path is introduced. |
| S4.3 | Native integer, float, double-float, and selected general bulk placement using shared content services. | Exact hashes, shapes, padding, and value provenance are recorded. |
| S4.4 | Assembly-specific validation and manifest/report projection. | Output is structurally checked and always records `iod_conformance = "not_assessed"`. |
| S4.5 | CLI and SDK assembly surfaces with capability discovery and dry-run. | Machine result/error contracts match S2/S3. |
| S4.6 | Adversarial protection, path, resource, reference, and transaction tests. | Contradictions and unsafe inputs fail before publication with stable codes. |

**S4 gate:** a side project can request arbitrary deterministic element content
and typed pixels without adding a repository recipe, while evidence consumers
cannot mistake the result for qualified or curated coverage.

### Phase S5 — Package and document the product

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| S5.1 | Complete Cargo metadata and intentional package contents. | `cargo package --locked` and packaged-crate tests pass. |
| S5.2 | Target-specific release archive builder with checksums, licenses, resource manifest, and version metadata. | Extracted archive passes relocation tests on each claimed target. |
| S5.3 | Installation, automation, SDK, assembly, output, and upgrade operating guides. | Every command is exercised from outside the repository. |
| S5.4 | Neutral examples for raw grayscale/RGB, metadata/private/Sequence values, multi-instance references, and structural assembly. | Examples are small, synthetic, non-PHI, deterministic, and CI-tested. |
| S5.5 | Changelog and release/migration procedure. | A new maintainer can build, verify, checksum, and describe a release from a clean clone. |

**S5 gate:** a release candidate can be handed to a human or agent consumer
with no repository-specific setup knowledge.

### Phase S6 — Compatibility and release qualification

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| S6.1 | Black-box consumer harness in a separate temporary project. | It discovers, composes, assembles, validates, reports, and handles errors using only public contracts. |
| S6.2 | Cross-CWD and sequential/parallel determinism matrix. | Byte-stable outputs and canonical machine results match after normalizing explicit output paths. |
| S6.3 | Full qualified-template and structural-fixture qualification. | Every live qualified template and supported assembly content kind is exercised; inventory is queried, not hard-coded in docs. |
| S6.4 | Existing curated/profile/codec/backend regression matrix. | No evidence, profile, skipped-capability, or determinism regression is accepted. |
| S6.5 | Upgrade tests from every still-supported request, manifest, report, and CLI API version. | Compatible inputs succeed or receive the documented version error and migration action. |
| S6.6 | Security/resource tests against packaged release artifacts. | Path, symlink, provider, cancellation, cleanup, race, and limit contracts hold outside the checkout. |

**S6 gate:** all terminal acceptance criteria pass on every claimed release
target and no missing optional capability is represented as a pass.

### Phase S7 — Promote and maintain

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| S7.1 | Dated standalone-product status record with exact artifact identities and verification matrix. | Record distinguishes current claims from plans and unsupported targets. |
| S7.2 | README quick start switched to installed-product usage, with contributor `cargo run` usage retained separately. | First-time consumer path does not assume a clone. |
| S7.3 | CI/release gates made mandatory for resource audit, packaging, relocation, schemas, external consumer, determinism, and hygiene. | A deliberate fixture proves each gate detects its target regression. |
| S7.4 | Compatibility and deprecation ownership assigned. | Every public schema/API has an owner and supported-version window. |

**S7 gate:** the release is the documented current operating model, this plan
is marked complete, and subsequent changes are governed by the published
compatibility policy.

## 8. Terminal Acceptance Matrix

The program is complete only when all of the following pass against the exact
release candidate, not merely a repository debug binary:

| Gate | Required evidence |
| --- | --- |
| Relocation | Extract/install artifact, remove access to source checkout and Cargo source cache, change CWD, then run version, capabilities, templates, compose, generate, validate, and report. |
| Qualified composition | Discover the live catalog; default-generate, validate, report, and reproduce every qualified template/bundle with explicit unavailable accounting. |
| Caller pixels and attributes | Generate from external raw monochrome/RGB assets plus standard, private, binary, multi-valued, empty, and recursive Sequence overrides; verify hashes and resolved provenance. |
| Structural assembly | Assemble representative standard and unknown explicit-VR elements, private blocks, recursive Sequences, identities/references, native pixels, float/double-float values, and non-pixel bulk; verify no IOD claim. |
| Automation | Validate every JSON success/error/result schema, exit class, stdout/stderr rule, dry-run shape, and stable error code from a non-Rust harness. |
| Rust SDK | Build and test an external packaged-crate consumer using only `sdk`; exercise bytes, files, explicit asset root, cancellation, typed results, and typed errors. |
| Determinism | Compare two fresh roots across unrelated CWDs and supported parallelism; byte-stable files, UIDs, paths, plan hashes, manifests, and normalized outcomes match. |
| Existing behavior | Run `cargo test --locked --all-targets --no-default-features`, documented fresh-root profile workflows, applicable feature/backend matrices, and independent-evidence gates. |
| Packaging | `cargo package --locked`, archive extraction, checksums, license notices, resource manifest, version metadata, and claimed-target smoke tests pass. |
| Security/resources | Unsafe paths, symlinks, traversal, resource overruns, hash drift, provider substitution/crash/hang/flood, cancellation, cleanup, and destination races fail safely. |
| Documentation | Every public quick start and integration example runs from an installed artifact outside the repository; stale `cargo run`-only consumer guidance is absent. |
| Hygiene | `cargo fmt --check`, `git diff --check`, schema validation, no tracked generated DICOM/run artifacts, and clean release worktree pass. |

No gate may be waived by narrowing documentation after implementation. A
capability that cannot pass remains explicitly unavailable and is excluded from
the corresponding release claim.

## 9. Orchestration And Change Discipline

- Begin each phase by re-reading current executable behavior, schemas,
  registry/template inventories, this plan, and the newest applicable status
  record. Historical counts and file lists are not invariants.
- Keep at most one acceptance gate in a partially migrated state. Prefer
  compatibility adapters over simultaneous edits to every frontend.
- Add contract tests before or with each public behavior change. Test through
  the installed/packaged surface as soon as S1 makes that possible.
- Preserve user changes in a dirty worktree and stage files selectively.
- Commit each numbered logical unit separately with the repository's required
  `type(scope): subject` message and explanatory body. Verify each commit with
  `git log --oneline -3` before starting the next unit.
- Do not promote a phase based only on same-project validation. Retain the
  existing independent-evidence boundaries and record unavailable tools.
- When a design decision changes a contract fixed here, update this plan or add
  a superseding ADR in a dedicated documentation commit before implementing
  divergent behavior.

## 10. Completion Definition

This realignment is complete when a side project can treat the release as an
opaque deterministic DICOM generator: install it, discover capabilities,
submit qualified or structural requests, receive stable typed outcomes, locate
artifacts through the manifest, validate them according to their declared
evidence class, reproduce them, and upgrade them using published compatibility
rules—without cloning, entering, importing internal modules from, or otherwise
understanding this repository.
