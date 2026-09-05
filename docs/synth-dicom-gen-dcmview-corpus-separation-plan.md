# `synth-dicom-gen` and `dcmview-test-corpus` Separation Plan

**Status:** current authoritative execution plan

**Execution amendment:** 2026-09-05 — slice-level delivery and proportional verification

**Prepared:** 2026-09-01

**Predecessor:** `docs/standalone-productization-plan.md`

## 1. Goal

Separate the reusable DICOM generation product from the viewer-specific corpus
without weakening determinism, validation, evidence boundaries, or release
qualification. Rename this product and repository to `synth-dicom-gen`, then
move the dcmview corpus definition into a new `dcmview-test-corpus` repository
that consumes only supported `synth-dicom-gen` CLI or Rust SDK contracts.

The separation must also make ordinary development materially cheaper:

- corpus-definition and viewer changes must not rebuild or test the generator's
  internal qualification suite;
- ordinary pull requests must not run WSI, stress, full-profile, all-feature,
  packaged-crate, or native-release qualification unless their dependency
  surface requires it;
- superseded remote runs must be cancelled, and equivalent push and pull-request
  events must not perform the same full work twice;
- generated corpora and build products remain untracked and bounded; and
- the complete expensive matrix remains available as an explicit scheduled,
  manual, or release-candidate gate.

This plan supersedes the repository-boundary assumptions in the standalone
productization plan. It does not invalidate the exact `69d3e5f8` release-
candidate evidence, but that evidence applies only to that immutable candidate.

## 2. Current Baseline and Problem Statement

The standalone initiative completed its S0-S7 contract, but it left the
generator, its bundled corpus, and its qualification machinery in one package.
At the dated baseline:

- Cargo reports 186 integration-test targets; the count is a diagnostic
  snapshot, not a permanent inventory invariant;
- five in-process codec jobs and two external-codec jobs compile feature-
  distinct test artifacts in addition to the default configuration;
- the default suite serializes the entire harness to protect a small number of
  wall-clock provider contracts;
- codec jobs still provision the highdicom runtime and generate the complete
  `extended` profile, including feature-independent WSI work;
- package, external-consumer, release-archive, and archive-harness checks create
  additional build trees or release builds;
- `build.rs` embeds the case registry, every case recipe, templates, schemas,
  conformance configuration, backend files, and `Cargo.lock` into one resource
  identity; and
- the supported SDK exposes composition and assembly but not a caller-owned,
  registry-led corpus operation. An explicit resource root must be a byte-for-
  byte mirror of the embedded product resources and cannot define a downstream
  corpus.

These couplings make corpus edits rebuild the Rust product, broaden manifest
identity invalidation, multiply linked test binaries across features, and force
viewer development to pay generator-qualification costs.

## 3. Target Repository Boundary

### 3.1 `synth-dicom-gen`

The renamed repository owns reusable generation behavior:

- neutral `CorpusPlan`, bounded execution, Part 10 materialization, atomic
  publication, cancellation, resource ceilings, and common evidence types;
- qualified composition templates and generic typed providers;
- structural assembly and generic negative/mutation/fuzz primitives;
- codec and external-provider adapters with explicit capability discovery;
- generic DICOM parsing, validation, manifest, and provenance infrastructure;
- versioned CLI and Rust SDK contracts for loading and executing a caller-owned
  corpus definition;
- engine, security, compatibility, public-consumer, packaging, and release
  qualification tests; and
- product documentation that makes no dcmview behavior or profile claim.

It does not own dcmview case selection, viewer expectations, viewer regression
policy, or a dcmview release corpus.

### 3.2 `dcmview-test-corpus`

The new repository owns the product-specific test corpus:

- stable case IDs, profiles, selection policy, and the corpus registry;
- versioned corpus-definition documents that reference supported generator
  templates, providers, mutation primitives, or assembly content;
- standards notes required to explain the selected corpus cases;
- dcmview-specific expectations, known results, issue links, and compatibility
  report schemas;
- the pinned `synth-dicom-gen` dependency and required feature/runtime policy;
- a thin generation/validation/reporting entry point built only on supported
  generator interfaces;
- corpus-definition, selection, smoke-generation, migration-parity, and viewer
  integration tests; and
- CI artifact publication and retention policy for generated corpora.

Generated DICOM, ordinary manifests, reports, caches, build directories, and
private tool environments remain ignored and uncommitted.

### 3.3 Dependency direction

```text
dcmview-test-corpus definitions and viewer expectations
  -> versioned synth-dicom-gen CLI or SDK request
  -> synth-dicom-gen planning, providers, materialization, and validation
  -> manifest-bound generated corpus artifact
  -> dcmview tests and compatibility reporting
```

`synth-dicom-gen` production code, product resources, and current operating
claims must not depend on, import, test, or name `dcmview-test-corpus`.
Migration and historical documents may identify the downstream repository
explicitly. The corpus repository may depend on a released crate, an immutable
Git revision, or a checksummed native artifact. Its final CI must not require a
sibling checkout or path dependency.

## 4. Non-Negotiable Invariants

1. Valid, negative, fuzz, stress, media, protocol, same-project, and independent
   evidence remain distinguishable and cannot inflate one another.
2. Generated output stays plan-first, deterministic according to its declared
   class, bounded, validated, cleaned, and atomically published without
   overwriting an existing root.
3. Missing features, providers, codecs, validators, peers, and corpus
   capabilities remain explicit unavailable outcomes.
4. A downstream corpus definition is a versioned caller input, not a mutable
   replacement for immutable engine resources.
5. Manifests record separate engine, template/provider catalog, corpus
   definition, schema, toolchain, and external-runtime identities wherever
   those domains can change independently.
6. A corpus-only change must not change the engine identity. A toolchain-only
   change must not masquerade as a corpus-definition change.
7. The public CLI remains the primary language-neutral boundary. The Rust SDK
   is the supported in-process boundary. Downstream production code imports no
   internal planner, executor, recipe, materializer, or resource module.
8. Existing qualified generation bytes are preserved during migration unless a
   deliberately versioned recipe or template change documents otherwise.
9. No broad or expensive gate is removed. It is assigned to the narrowest
   dependency surface and execution cadence that still proves its contract.
10. Every completed logical unit is committed separately under the applicable
    repository's `AGENTS.md` policy.

## 5. Verification Classes and Budgets

At the initial baseline boundary, record the current wall time, billable runner time,
largest local target-directory size, and artifact count for each class. Later
gates compare against that baseline instead of relying on subjective speed.

| Class | Purpose | Required cadence | Initial budget target |
| --- | --- | --- | --- |
| Fast PR | Formatting, schemas, changed unit/domain tests, SDK compile, tiny smoke | Every pull request | Under 15 minutes wall time; no WSI, stress, full profile, Python backend, package, or release build |
| Subsystem | Owning domain plus affected CLI/SDK/resource contracts | When that subsystem changes | Named targets only; no unrelated feature matrix |
| Corpus PR | Definition/schema/selection checks and changed-case generation | Corpus changes | Generate only changed cases plus dependencies; no full corpus by default |
| Nightly | Broad default and applicable codec/provider coverage | Scheduled or manually requested | May be expensive; cancelled when superseded and retained with exact evidence |
| Release candidate | Complete terminal matrix on every claimed target | Explicit immutable candidate only | No fixed short budget; every expensive row runs once unless its dependency surface changes |

CI jobs must print elapsed time and relevant target/output sizes. CI build
profiles use no incremental compilation and minimal debug information unless a
specific diagnostic job requires otherwise. Local scripts use a task-specific,
explicit target directory and report its size before removal or reuse.

During implementation, run focused checks for the affected behavior. At slice
integration, inspect routing and run the mapped ordinary coverage once. Repeat
only checks invalidated by a relevant change, failure, or unresolved concern;
a status-only commit does not invalidate executable evidence. Documentation-only
changes need link/claim review and `git diff --check`; exercise documented commands
when their syntax or behavior changes, not when recording existing results.
Reuse accepted evidence when its inputs and dependency surface are unchanged.
This does not waive scheduled or exact release-candidate gates. Record unavailable
measurements explicitly; do not recreate completed baselines merely for bookkeeping.

## 6. Phased Execution

### Phase R0 - Freeze the migration contract

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R0.1 | Superseding ADR for the two-repository boundary, product rename, compatibility treatment, and corpus-definition ownership. | The ADR classifies the crate, library, binary, schema, resource, and manifest changes under the current compatibility policy. |
| R0.2 | Dated baseline of CI wall time, runner time, build-tree size, test-target count, and expensive test ownership. | Measurements identify commands and exact revisions without generating new durable artifacts. |
| R0.3 | File/module ownership inventory for engine, generic capabilities, corpus definition, viewer expectations, and historical evidence. | Every file to move, retain, split, retire, or archive has one destination and rationale. |
| R0.4 | Migration parity manifest for the initial smoke slice. | The three smoke cases name their expected recipe/template version, output paths, byte hashes, manifest semantics, and unavailable behavior. |

**R0 gate:** maintainers can decide where any changed file belongs and which
verification class it invalidates before code or history is moved.

### Phase R1 - Contain CI and local build cost

This phase precedes rename or API work so later changes do not repeat the
current development loop.

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R1.1 | Workflow concurrency groups with superseded-run cancellation and non-duplicated push/PR event ownership. | Two rapid updates leave only the newest applicable run active; a pull request does not receive two equivalent full workflows. |
| R1.2 | Fast PR workflow separated from nightly/manual/release qualification. | Ordinary documentation, SDK, CLI, or isolated engine changes run only their declared fast/subsystem gates. |
| R1.3 | Codec matrix restricted to feature-sensitive libraries, tests, and selected generated cases. | Codec jobs do not prepare highdicom or execute feature-independent WSI/quantitative cases; each manifest proves its requested codec case. |
| R1.4 | Provider timing contracts isolated from the ordinary test harness. | Strict timing/cancellation tests run serially in their provider job; the remaining default tests run with normal harness parallelism. |
| R1.5 | Build-storage controls and reporting. | CI uses non-incremental, low-debug test builds; jobs report build/output sizes and remain under the agreed ordinary-job disk budget. |
| R1.6 | Release build reuse. | Package, archive, installed consumers, and adversarial archive checks reuse the minimum number of compiled artifacts and do not rebuild an identical release binary. |

**R1 gate:** a representative fast PR completes within budget, performs no
heavy corpus work, and the full matrix remains separately invocable.

### Phase R2 - Reduce Rust test-linking amplification

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R2.1 | Test ownership manifest mapping every test to a domain and verification class. | Unowned, multiply owned, and accidentally heavy fast tests fail a metadata check. |
| R2.2 | Domain-grouped integration harnesses. | The dated 186-target baseline is reduced to at most 20 intentionally named integration binaries, without reducing behavioral assertions. |
| R2.3 | Explicit heavy-test entry points for byte parity, all-profile, WSI, and stress qualification. | Heavy tests are not selected by the fast PR gate and are selected by nightly/release commands. |
| R2.4 | Targeted change-to-test routing. | Representative fixture changes prove engine, codec, provider, schema, SDK, and corpus surfaces select the owning bundles. |

**R2 gate:** compiling the fast suite produces a bounded number of linked test
binaries and materially reduces clean and incremental disk use relative to R0.

### Phase R3 - Rename the reusable product

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R3.1 | Rename repository/product metadata to `synth-dicom-gen`, crate/package to `synth-dicom-gen`, Rust path to `synth_dicom_gen`, and primary binary to `synth-dicom-gen`. | Package metadata, discovery, help, archives, examples, and installed consumer tests consistently use the new name. |
| R3.2 | Apply the pre-1.0 compatibility decision. | Default action is product `0.2.0` with a clean rename because `0.1.0` was an unpublished candidate; any discovered external consumer receives a separately tested temporary alias. |
| R3.3 | Separate current operating documentation from immutable `dicom-test-suite` historical qualification records. | Historical hashes and artifact names remain unchanged; current guides and links use the new product name. |
| R3.4 | Rename environment variables and staging prefixes under a documented transition rule. | Discovery and migration tests cover every retained or removed spelling. |

**R3 gate:** a clean external consumer installs and uses `synth-dicom-gen`
without an old repository path, while historical evidence remains truthful.

### Phase R4 - Split immutable resources from caller-owned corpus definitions

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R4.1 | `EngineResources` abstraction for immutable schemas, templates, generic providers, and required small assets. | Engine integrity remains fail-closed and relocation-safe. |
| R4.2 | Versioned `CorpusDefinitionBundle` schema and typed loader for caller-owned registry, profiles, cases, and expected evidence. | Positive, adversarial, traversal, symlink, size, hash, version, and reference-closure fixtures pass. |
| R4.3 | Independent identity domains in discovery and manifests. | Changing one corpus document changes the corpus digest but not engine, toolchain, or template digests. |
| R4.4 | Remove corpus files and `Cargo.lock` from the monolithic runtime-resource digest where their meanings are represented by separate identities. | Existing provenance remains reconstructable and compatibility migration is explicit. |
| R4.5 | Reusable or lazy resource materialization. | A batch corpus run does not copy the complete immutable resource set once per case or request. |

**R4 gate:** an integrity-checked external corpus bundle can vary independently
of the installed generator without becoming an engine-resource override.

### Phase R5 - Add the supported external corpus API

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R5.1 | SDK `GenerateCorpusRequest` and typed outcome using file or bytes input, explicit asset root, seed, selectors, parallelism, dry-run, and cancellation. | An external crate generates by profile and case ID while importing only `synth_dicom_gen::sdk`. |
| R5.2 | CLI corpus input and selection contract. | `generate --corpus PATH` supports profile and explicit case-ID selection with versioned machine results and stable errors. |
| R5.3 | Batch planning/execution over one corpus bundle and resource context. | Multiple cases and their dependencies enter one deterministic plan and one atomic publication transaction. |
| R5.4 | Definition-driven manifest/report projection. | Outputs retain selected, generated, skipped, unavailable, validation, and definition identity without assuming an embedded dcmview registry. |
| R5.5 | Capability discovery for engine, template/provider, and loaded-corpus support. | A consumer can decide whether to submit a request without parsing docs or internal modules. |

**R5 gate:** a repository-independent consumer can define, select, generate,
validate, report, and reproduce a corpus entirely through supported contracts.

### Phase R6 - Establish `dcmview-test-corpus` with a smoke slice

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R6.1 | New repository with its own `AGENTS.md`, licenses, README, dependency pin, ignore policy, corpus schema fixtures, and CI. | The repository contains no generated DICOM or sibling-checkout assumption. |
| R6.2 | Move the three smoke case definitions and profile selection. | Definition validation passes and case identities remain stable. |
| R6.3 | Thin supported-interface runner. | A clean clone obtains the pinned generator, generates smoke, validates it, and emits manifest/report locations through typed results. |
| R6.4 | Migration parity. | Seed-1 smoke files are byte-identical and normalized manifest semantics match the R0 baseline, except for deliberately versioned product/resource-domain changes. |
| R6.5 | Initial dcmview compatibility result contract. | Viewer results attach to stable case IDs and cannot alter generator validation or standards evidence. |

**R6 gate:** the smallest useful dcmview corpus is generated and consumed from
the new repository without compiling or testing generator internals.

### Phase R7 - Migrate the complete dcmview corpus

Migration proceeds in reviewable slices: ordinary native valid cases,
multi-instance and derived relationships, optional codecs, external providers,
legacy, negative, fuzz, and stress. Do not move all files in one commit.

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R7.1 | Move registry/profile/definition documents and corpus-specific standards notes by slice. | Each slice validates, selects the expected closure, and preserves stable case identity. |
| R7.2 | Replace corpus-specific Rust planners with supported templates or generic provider parameters where possible. | Downstream code imports no internal generator module. |
| R7.3 | Promote genuinely reusable algorithms or validation primitives into the generator through versioned public capabilities. | Generic names and tests contain no dcmview policy; downstream definitions reference stable IDs. |
| R7.4 | Preserve negative, fuzz, stress, media, protocol, and independent-evidence isolation. | Separate runs and reports retain their existing semantic boundaries. |
| R7.5 | Slice-specific parity and availability evidence. | Byte-stable output matches or carries a versioned migration; semantic-stable output uses its declared comparison; missing runtimes remain unavailable. |

Execute R7.1–R7.5 as complete vertical slices grouped by reusable capability and
dependency closure, not as a new approval chain for each helper or case. A slice
includes import, reusable engine changes, tests/routing, availability/parity,
and necessary current documentation. Preserve dependency ordering within it.

Consolidate duplicated capture/availability/parity infrastructure into reusable
mechanisms with declarative slice contracts for selection, identities, file
closure, semantics and comparison rules. Make the first consolidation bounded
and useful to the next slice; do not build a speculative universal framework.
Keep historical receipt formats and artifacts intact. Historical tests should
use immutable source/definition fixtures rather than reconstructing every old
version by reversing the growing live corpus. Never commit generated DICOM or
ordinary run evidence as fixtures. Do not require rewriting old proofs before
new imports can proceed.

Caller definitions own patient values, names, recipe metadata and pixel patterns.
The engine validates reusable structural and semantic constraints through public
capabilities; matching a hardcoded historical tuple is not sufficient genericity.
Previously accepted bounded tuple support remains valid at its recorded scope,
but corpus-specific whitelists are migration debt to remove by the R7/R9 gates,
with qualified output bytes preserved or an explicit versioned migration.

**R7 gate:** every dcmview case is owned by the corpus repository, and the
generator repository contains only reusable capabilities and generic evidence.

### Phase R8 - Decouple viewer development from corpus generation

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R8.1 | Corpus artifact key derived from generator version/revision, corpus-definition digest, seed, features, and external-runtime identities. | Equal keys reproduce the declared byte- or semantic-stable corpus; unequal inputs cannot reuse an artifact silently. |
| R8.2 | Scheduled/manual full-corpus publication with bounded retention. | Artifacts include manifest, reports, checksums, and unavailable rows, but are not committed. |
| R8.3 | dcmview CI consumes an existing qualified artifact by default. | Ordinary viewer changes neither compile the generator nor regenerate the full corpus. |
| R8.4 | Changed-case and explicit refresh workflows. | Corpus-definition pull requests generate only changed cases/dependencies; maintainers can request a full refresh. |
| R8.5 | Failure triage identifies generator, corpus-definition, artifact, or viewer ownership. | A viewer failure does not automatically invalidate generator release evidence. |

**R8 gate:** ordinary dcmview development uses a manifest-bound cached corpus
and completes without generator qualification or full corpus generation.

### Phase R9 - Remove the embedded dcmview corpus and qualify both products

| ID | Deliverable | Acceptance |
| --- | --- | --- |
| R9.1 | Remove migrated dcmview registry/recipes/profile code, tests, and current operating claims from `synth-dicom-gen`. | Resource and package inventories contain no dcmview corpus ownership. |
| R9.2 | Reduce package contents to the supported generator product. | Public packages omit repository-only corpus tests and generated evidence while retaining required licenses, schemas, guides, and examples. |
| R9.3 | Update current README, system specification, generation guide, SDK/CLI guides, compatibility policy, changelog, and status records in both repositories. | Search finds no stale repository name, embedded-corpus claim, command, dependency, or CI ownership statement outside marked history. |
| R9.4 | Run exact generator release qualification once on each claimed target. | CLI/SDK, resources, composition, assembly, generic providers, codecs, security, packaging, relocation, and release artifacts pass. |
| R9.5 | Run exact dcmview corpus qualification once against the generator candidate. | All selected slices generate or report explicit unavailability, validate, reproduce, publish artifacts, and enter viewer testing with correct evidence separation. |
| R9.6 | Record measured cost reduction. | Final status compares R0 and terminal PR/nightly/release wall time, runner time, linked targets, build-tree size, and artifact size. |

**R9 gate:** the generator and corpus are independently versioned, buildable,
testable, releasable, and maintainable, and ordinary development satisfies the
agreed cost budgets.

## 7. Parallelization and Commit Discipline

Only work with disjoint files and contracts may run in parallel. Recommended
parallel groups are:

- R1 workflow routing, R1 build-profile/storage controls, and R0 ownership
  inventory after the baseline measurement is frozen;
- R2 domain harness conversions split by non-overlapping test families;
- R3 documentation/history classification and R4 schema design after the ADR
  fixes naming and version decisions;
- R7 migration slices whose case and provider closures do not overlap; and
- R8 artifact plumbing and viewer-result schema work after the artifact key is
  accepted.

The resource model, public corpus API, manifest identity changes, smoke parity
slice, and terminal releases remain sequential gates. At most one public
compatibility boundary is partially migrated at a time.

A compatibility boundary means a shared public schema, API, identity contract or
provider behavior. A helper, test, review or ledger entry is not automatically a
new compatibility boundary. Preserve the sequential gates above while allowing
disjoint work on already stable contracts in the listed parallel groups.

A completed task is a reviewable vertical migration slice or a necessary reusable
capability, including its tests, routing and necessary documentation. Aim for
2–4 coherent commits across the affected repositories per ordinary slice; this
is guidance, not permission to combine unrelated work or omit needed commits.
Do not create separate assignment, readiness, review or acceptance commits unless
an actual external approval boundary requires a durable artifact.

For each slice:

1. Name phase items, dependency closure and file ownership in agent messages
   before editing. Delegate bounded disjoint implementation where useful;
   serialize shared contracts and integrate shared files through one owner.
2. Review every agent result and integrate selectively. A separate review is
   useful evidence, not a mandatory additional task/commit for every helper.
3. Run focused verification during development and mapped coverage once at
   integration as specified in section 5. Preserve real evidence boundaries.
4. Commit coherent implementation with related tests, routing, current docs and
   the slice ledger entry. Use the applicable message policy and verify commits.
5. Report completed cases/families and phase gates, remaining gates, elapsed work,
   verification commands/results, relevant sizes, commits and genuine blockers.
   If an ordinary slice again needs bespoke infrastructure or mostly procedural
   work, adjust the implementation approach before repeating that pattern.

The single authoritative execution ledger is
`docs/migration-continuation-status-2026-09-05.md` in the generator repository.
Append one concise dated entry per completed slice or genuine blocker, preferably
in its implementation commit; identify that commit by its descriptive subject
when its hash cannot yet be known. Other repositories link to this ledger rather
than duplicate entries. Existing dated records remain immutable historical
evidence; do not rewrite them or keep updating inventories in AGENTS.md.

Authorization to execute this plan covers its necessary local implementation and
bounded verification. Do not repeatedly request permission for already authorized
native checks. Review helpers and authenticate inputs before native execution,
use fresh outputs, retain failures, and rerun only for a concrete invalidation or
corrected failure. External publication or other actions outside the user's
scope still require authorization. These execution rules supersede historical
per-helper approval instructions, without expanding any evidence claim.

## 8. Terminal Acceptance Matrix

The plan is complete only when all rows pass:

| Gate | Required evidence |
| --- | --- |
| Repository boundary | No generator dependency on dcmview; no corpus use of unsupported generator modules or sibling paths. |
| Naming and compatibility | New crate, Rust path, binary, archives, discovery, guides, and migrations consistently use `synth-dicom-gen`; historical evidence is unchanged. |
| External corpus contract | Versioned corpus definitions load safely and generate by profile/case ID through CLI and SDK. |
| Identity separation | Engine, toolchain, template/provider, schema, corpus, and external-runtime changes invalidate only their owned meanings. |
| Smoke migration | The three smoke cases pass byte/semantic and manifest parity under the documented migration normalization. |
| Complete migration | All dcmview valid, optional, legacy, negative, fuzz, stress, media, and protocol scopes have an owner and correct availability/evidence behavior. |
| Fast development | Representative generator, corpus, documentation, and viewer pull requests satisfy their declared wall-time and disk budgets without heavy gates. |
| Heavy qualification | Nightly/manual and immutable release workflows still execute every applicable WSI, stress, parity, codec, provider, package, relocation, and target gate. |
| Artifact consumption | dcmview CI consumes a correctly keyed qualified corpus without committing payloads or regenerating the full corpus by default. |
| Packaging and release | Both repositories have clean, independently reproducible release procedures and exact status records. |
| Documentation | Current operating docs describe the two-repository model; historical plans and hashes remain explicitly historical. |
| Hygiene | Both worktrees are clean; formatting, schema, diff, generated-artifact, secret, and package-inventory checks pass. |

No performance target may be met by deleting evidence or silently narrowing a
claim. When a gate remains too expensive, move it to the correct explicit
cadence, reduce duplicated setup/build work, or make its selection more
precise.

## 9. Completion Definition

This plan is complete when `synth-dicom-gen` is a viewer-neutral reusable
generation product, `dcmview-test-corpus` owns and reproducibly generates the
entire dcmview corpus through supported interfaces, and ordinary generator,
corpus, and viewer development no longer invokes unrelated heavyweight
qualification. Both repositories must have clean granular history, current
documentation, measured cost improvements, explicit unavailable behavior, and
passing terminal release evidence for their claimed targets and scopes.
