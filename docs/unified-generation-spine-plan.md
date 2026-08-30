# Unified Generation Spine Migration Plan

**Status:** implementation complete; terminal verification recorded in the dated completion status

**Prepared:** 2026-08-29

**Goal:** make one plan-first generation architecture the source of every
retained DICOM artifact while preserving registry-selected curated coverage,
caller-authored composition, specialized qualification evidence, deterministic
output, and profile isolation.

This document is the execution contract for the migration. It supersedes the
architectural follow-up implied by the completed arbitrary-composition plan; it
does not change current executable behavior merely by being committed.

## 1. Target Outcome

The repository shall have one neutral generation spine:

```text
run request
  -> frontend selection and input validation
  -> recipe/template resolution
  -> CorpusPlan
       -> ordered artifact graph
       -> ResolvedInstancePlan for each valid DICOM instance
       -> MutationPlan for each expected-invalid derivative
       -> QualificationPlan for payload-free evidence
       -> validation and evidence obligations
  -> one bounded executor
       -> content/provider resolution
       -> transfer-syntax encoding
       -> Part 10 materialization
       -> generic and specialized validation
       -> evidence collection
  -> frontend-specific manifest/report projection
  -> atomic publication
```

Two public frontends remain:

1. `generate --profile ...` selects curated cases from
   `cases/registry.json`, resolves their versioned case recipes, and projects
   stable `case_id`, profile, skipped-case, stress, negative, fuzz, and
   qualification evidence.
2. `compose --spec ...` resolves caller intent through the qualified template
   catalog and projects template, instance, asset, provider, and composition
   evidence without claiming curated coverage.

The distinction is evidence semantics, not object construction. Both frontends
must produce the same internal `CorpusPlan` and invoke the same executor.

At completion:

- every implemented valid registry case, including `legacy` and reduced-scale
  `stress`, is planned before any DICOM file is written;
- native curated recipes do not create a DICOM object or file and then import
  it back into a resolved plan;
- composition defaults do not call the curated generator to obtain DICOM
  artifacts;
- every registry `recipe_id` and `recipe_version` resolves through a complete,
  unique recipe binding;
- declarative case differences live in modular recipe documents where the
  model can express them, with narrow typed Rust plan providers for genuinely
  algorithmic behavior;
- valid source instances for negative and fuzz work are obtained from the same
  plan-first recipe layer;
- the generic writer, codec adapters, validation executor, resource accounting,
  and publication transaction are shared; and
- curated specialized oracles and independent evidence remain additive and
  are never replaced by same-project generic validation.

## 2. Current-State Review

This section records the reviewed architecture as of the prepared date. Counts
are an audit snapshot, not documentation invariants; future agents must derive
current inventory from the registry and executable.

### 2.1 What is already shared

The composition subsystem provides strong registry-independent primitives:

- typed tags, VRs, values, Sequences, empty/remove operations, and private
  creators;
- deterministic identity allocation;
- attribute precedence and protection;
- typed native, encapsulated, float, waveform, document, and mesh content;
- logical references, dependency bundles, and resolved UID closure;
- canonical `ResolvedInstancePlan` hashing;
- `Part10Materializer`;
- generic plan/manifest validation;
- safe staged content, bounded providers, cancellation, parallelism, and atomic
  publication; and
- qualified template descriptors for every currently implemented valid SOP
  Class represented by `templates/inventory.json`.

A fresh default-feature-disabled `generate --profile all` audit emitted 158
files across 141 logical case IDs, and every emitted file recorded the passed
`curated_composition_plan` check. This proves that the shared resolved-plan
materializer is production infrastructure for ordinary valid output.

### 2.2 What is not yet shared

The source of curated intent remains the original generator:

- `write_generation_run` reads the registry and invokes
  `generator::write_supported_cases` directly.
- `write_supported_cases` owns profile-special routing, hard-coded dependency
  order, family dispatch, provider invocation, generation, and source
  registration.
- `src/generator.rs` remains a large mixed-responsibility module containing
  recipe data, IOD construction, codecs, validation, manifest construction
  inputs, stress execution, and migration adapters.
- static recipe differences are spread across the registry, Rust constants,
  family-specific structs, template declarations, and specialized tests.
- the registry names `recipe_id` and `recipe_version`, but those fields do not
  resolve through a single explicit recipe catalog.

The current broad migration often occurs after legacy construction:

```text
curated recipe builds/writes DICOM
  -> reopen the file
  -> convert its dataset with resolved_plan_from_curated_dataset
  -> remove the first file
  -> rematerialize with Part10Materializer
```

This verifies byte preservation and creates a shared final writer, but the
resolved plan is not yet the source of the object. The bridge also discards
useful construction provenance by labeling imported values as instance
overrides.

Complex composition defaults currently have the reverse dependency:
`composition::advanced_family` calls
`generator::write_composition_default_artifacts`, reopens curated artifacts,
and adapts them back into composition plans. This makes the supposedly neutral
composition layer depend on the curated frontend.

### 2.3 Public contracts that are correctly separate

The following separation must remain:

- registry case IDs, profiles, status, requirements, provider identity,
  roadmap blockers, and standards evidence;
- composition template IDs, caller instances, assets, and provider inputs;
- curated manifest and coverage-report semantics;
- composition manifest and report semantics;
- valid, legacy, stress, negative, fuzz, media, and protocol evidence
  boundaries; and
- specialized same-project and independent validator routes.

Unifying these public meanings would inflate coverage or weaken evidence. The
migration instead unifies the internal plan and executor beneath them.

### 2.4 Inventory classes reviewed

The registry currently includes ordinary valid images, enhanced/WSI objects,
derived and quantitative graphs, SR, RT, waveform, documents, meshes, codec
variants, external-backend cases, legacy encoding, expected-invalid mutations,
reduced stress cases, and a payload-free fuzz qualification. The migration must
therefore model more than a single SOP Instance:

- one logical case may emit several instances;
- a case may depend on instances emitted by another case;
- one recipe may exercise byte-level encoding choices not represented by an
  ordinary attribute plan;
- external providers may own clinically structured object construction or
  compression;
- negative output is intentionally not a valid resolved instance; and
- fuzz/media/protocol qualifications may emit no DICOM instance at all.

## 3. Non-Negotiable Invariants

Every phase preserves these contracts.

1. `cases/registry.json` remains authoritative for curated case identity,
   status, profiles, requirements, provider, standards evidence, and blockers.
2. `templates/catalog.json` remains authoritative for caller-visible qualified
   object contracts. Template coverage does not create registry coverage.
3. Existing CLI syntax and public Rust composition APIs remain compatible
   unless an explicit versioned change is separately approved.
4. Curated manifest `case_id`, profile membership, skipped rows, qualifications,
   expectations, validation, reference projections, and report meaning remain
   backward compatible.
5. Byte-stable cases retain exact output under the same recorded inputs.
   Semantic-stable cases retain their decoded hashes, bounded metrics, and
   semantic contracts.
6. Every generated valid or source DICOM instance is synthetic, non-PHI, and
   sets `Synthetic Data (0008,001C)` to `YES`.
7. Existing route-specific validation and independent evidence remain required.
   A generic shared-plan pass is never a substitute.
8. `all` remains the union of `smoke`, `core`, and `extended`; it does not gain
   `legacy`, `negative`, `fuzz`, or implicit stress membership.
9. Negative outputs remain isolated and retain exact source/mutation hashes,
   byte ranges, failure layers, and acceptable outcomes.
10. Fuzz remains bounded and payload-free. Stress remains reduced-scale and
    explicit about what it does not prove.
11. Missing feature, codec, provider, external backend, or validator capability
    remains an explicit unavailable/skipped result.
12. External content and providers remain path-safe, hash-bound,
    resource-bounded, network-disabled at the protocol layer, cancellable, and
    privately staged.
13. Publication remains no-overwrite, transactional, and atomically promoted
    where the platform contract currently guarantees it.
14. Generated payloads, ordinary manifests/reports, caches, source artifacts,
    and private keys are never committed.
15. Case counts are derived by executable inventory tests and reports, never
    copied into code or documentation as permanent invariants.

## 4. Target Internal Architecture

Exact names may change, but each responsibility and dependency boundary below
must exist.

### 4.1 Frontends

`CuratedFrontend` owns only:

- registry/schema loading and semantic validation;
- profile selection and feature/runtime requirement evaluation;
- lookup of `recipe_id` plus `recipe_version`;
- construction of a curated planning request; and
- curated manifest/report projection.

`CompositionFrontend` owns only:

- composition-spec/schema loading;
- template lookup and caller-input validation;
- construction of a composition planning request; and
- composition manifest/report projection.

Neither frontend writes DICOM, invokes a codec directly, or owns publication.

### 4.2 Recipe and template distinction

A template describes a qualified DICOM object contract: SOP Class, modules,
attribute policies, structural parameters, content slots, reference roles,
transfer syntaxes, defaults, validation routes, and standards evidence.

A case recipe describes a test scenario using one or more templates: exact
parameters, attributes, content pattern/provider, identities, topology,
encoding choice, expected compatibility axes, specialized validator rules, and
manifest expectations.

Many case recipes may use one template. A recipe may emit a bundle or series.
The registry references recipes; callers reference templates.

### 4.3 Versioned recipe binding

Add a committed schema such as `schemas/case-recipe.schema.json` and modular
documents under a structure such as:

```text
cases/recipes/
  classic/
  enhanced/
  derived/
  geometry/
  metadata/
  non-image/
  vl/
  stress/
  negative/
  qualifications/
```

Static recipes should be data-first. A recipe document should be able to name:

- `recipe_id` and `recipe_version`;
- recipe kind;
- one or more logical instances;
- template ID and version per valid instance;
- transfer syntax and feature/runtime requirements;
- typed parameters and attribute operations;
- content pattern, fixture, provider, or codec request;
- identity-sharing and reference relationships;
- output path role and deterministic ordering;
- specialized validation rule IDs;
- expected manifest projection rule IDs;
- declared determinism and compatibility stressors; and
- optional typed plan-provider ID for algorithmic behavior.

Do not duplicate registry-owned status, profiles, standards evidence, or
provider availability in recipe documents. Loader validation must compare the
binding with the registry and template catalog and reject identity drift.

Typed Rust `PlanProvider` implementations remain appropriate for algorithms
such as large deterministic pixel streams, WSI tiling, geometry series,
codec-specific frame construction, and source-derived graphs. A provider must
return plans and evidence obligations; it must not publish files. Its binding
and parameters remain explicit in the recipe document.

### 4.4 Corpus plan

Introduce a run-neutral plan above `ResolvedInstancePlan`. Conceptually:

```rust
struct CorpusPlan {
    schema_version: Version,
    seed: u64,
    artifacts: Vec<PlannedArtifact>,
    dependencies: Vec<ArtifactDependency>,
    unavailable: Vec<UnavailableCapability>,
    publication: PublicationPlan,
}

enum PlannedArtifact {
    Dicom(PlannedDicomArtifact),
    Mutation(PlannedMutationArtifact),
    Qualification(PlannedQualification),
    Auxiliary(PlannedAuxiliaryArtifact),
}

struct PlannedDicomArtifact {
    logical_id: String,
    case_binding: Option<CaseBinding>,
    instance: ResolvedInstancePlan,
    output: OutputPlan,
    encoding: EncodingPlan,
    validation: ValidationPlan,
    evidence: EvidencePlan,
}
```

Required properties:

- canonical serialization and hashing;
- stable topological artifact order independent of parallel execution;
- explicit requested versus dependency provenance;
- explicit file and frame relationships;
- no filesystem paths except validated output-relative paths and staged asset
  handles;
- checked resource estimates before materialization;
- typed unavailable capability results;
- no frontend-specific manifest JSON embedded in the plan; and
- plan schema versioning sufficient to audit recipe migrations.

### 4.5 Recipe planner interface

The recipe lookup must be complete and keyed by the registry's versioned
identity. A suitable responsibility boundary is:

```rust
trait CasePlanner {
    fn identity(&self) -> RecipeIdentity;
    fn plan(
        &self,
        request: &CuratedCaseRequest,
        context: &PlanningContext,
    ) -> Result<PlannedCase, PlanningError>;
}
```

`PlanningContext` exposes deterministic identities, capabilities, qualified
templates, already planned dependency handles, content-pattern factories, and
standards-lock identity. It does not expose an output directory or a generic
file writer.

The composition resolver implements the same lower planning interfaces and
returns a `CorpusPlan`; it need not pretend that caller input is a curated
`CasePlanner`.

### 4.6 Artifact graph and dependencies

Replace generation-order coupling with an explicit DAG. The graph must model:

- multi-instance series and studies;
- source/derived case dependencies;
- bundle membership and requested/dependency provenance;
- frame-scoped references;
- shared Study, Series, Frame of Reference, specimen, and concatenation
  identities;
- generated-source requirements for negative mutations;
- external-provider inputs and outputs; and
- auxiliary evidence derived from a published DICOM artifact.

Topological sorting must be deterministic. Parallel workers may execute only
ready nodes and must not change output order, UIDs, hashes, or manifest order.

### 4.7 Content and pattern providers

Extract reusable, deterministic factories for:

- native integer, one-bit, float, and color pixels;
- palette, overlay, LUT, ICC, padding, and metadata boundary payloads;
- geometry and temporal frame vectors;
- WSI tiles, pyramids, optical paths, and concatenations;
- waveforms, PDF, STL, SR values, and RT grids;
- large/repeated stress streams; and
- encoded frames and codec requests.

Factories return typed content plans or staged asset handles plus semantic
metadata. They do not write DICOM. Composition defaults and curated recipes
must call the same factories when they represent the same qualified domain.

### 4.8 Encoding and materialization

`Part10Materializer` remains the only ordinary valid DICOM writer. Extend its
typed encoding inputs rather than retaining case-specific file writers for:

- sequence/item defined versus undefined length policy;
- native versus encapsulated content;
- Basic and Extended Offset Table policy;
- fragmentation policy;
- dataset deflate;
- Deflated Image Frame;
- implementation identity; and
- deterministic preamble/file-meta policy.

Transfer-syntax backends consume frames/content and return an encoded content
plan plus backend evidence. External full-file transforms may remain only as a
temporary, explicitly inventoried adapter when a locked tool cannot expose
frames. They must be removed or reduced to a typed import boundary before the
corresponding migration lane closes.

Importing an already constructed DICOM dataset is acceptable only at a named
external-backend boundary where independent library construction is itself
part of the case's evidence. Native Rust case recipes may not use that escape
hatch at completion.

### 4.9 Validation and evidence

Introduce a registry of stable validation rule IDs. A `ValidationPlan` names:

- shared Part 10 checks;
- shared data-element, identity, content, and reference checks;
- template/IOD rules;
- curated case-specific semantic oracles;
- codec round-trip or lossy-metric checks;
- stress, negative, or fuzz obligations; and
- independent conformance routes.

The executor runs these rules against the exact planned and materialized
artifacts. Manifest projectors consume typed validation results. Recipe code
must stop constructing ad hoc manifest validation JSON.

### 4.10 Evidence projections

Create an internal typed `RunEvidence` assembled from the corpus plan and
execution results. Keep two public projectors:

- `CuratedManifestProjector` preserves `manifest.schema.json`, registry
  joins, skipped cases, qualifications, and case-specific expectation blocks.
- `CompositionManifestProjector` preserves
  `composition-manifest.schema.json`, template instances, bundles, assets,
  provenance, and composition publication metadata.

The projectors may share file, identity, reference, content, validation, tool,
and publication substructures internally. They must not infer facts by
reopening output that were already known during planning.

### 4.11 Executor and publication

One `CorpusExecutor` owns:

- private staging and permissions;
- cancellation;
- bounded parallel scheduling;
- provider and codec invocation;
- content streaming and hashing;
- DICOM materialization;
- mutation and qualification execution;
- validation and evidence collection;
- aggregate resource accounting;
- manifest writing through a supplied projector; and
- cleanup and atomic no-replace promotion.

The existing `generate` and `compose` transactional implementations converge
onto this executor. Frontends supply policies and projectors, not separate
publication loops.

### 4.12 Dependency direction

The terminal module dependency must be acyclic:

```text
core plan/types
  <- templates, recipe schema, content factories, codecs, validators
  <- curated planner and composition planner
  <- shared executor
  <- CLI/API frontends and manifest/report projectors
```

The neutral core must not import `generator`, CLI argument types, registry
reporting, or composition-spec parsing. Composition modules must not import the
curated generator. Curated planners may use qualified template and plan APIs,
but not composition CLI/spec types.

## 5. Migration Classification

| Class | Current examples | Target treatment |
| --- | --- | --- |
| Declarative single-instance | SC pixel matrix, metadata boundaries, simple classic images | Recipe document selects a template, pattern/content factory, transfer syntax, attributes, and validation rules. |
| Multi-instance geometry | CT sorting, tilt, spacing, multislice MR | Typed series plan provider returns several template instances and shared identity/geometry relationships. |
| Enhanced/WSI | Enhanced CT/MR/PET, concatenation, tiled/sparse WSI, pyramids | Typed frame/functional-group/tiling providers return a bundle plan directly. |
| Derived graph | SEG, registration, presentation states, RWVM | Recipe DAG names source roles; planner materializes embedded references from graph identities. |
| Semantic document graph | SR and RT | Schema-bounded semantic plan providers plus explicit source and evidence roles. |
| Typed bulk | float pixels, waveform, PDF, STL | Shared typed content slots and validators; recipe documents own exact bounded parameters. |
| Codec variants | RLE, JPEG families, JPEG 2000, HTJ2K, JPEG XL, deflate | Transfer-syntax plan and backend registry; recipe variation should not duplicate IOD construction. |
| External generation backend | highdicom/pydicom outputs | Explicit external plan provider/import boundary with request/response/tool identity, followed by shared evidence and publication. |
| Legacy valid encoding | Explicit VR Big Endian, legacy JPEG | Valid plan with explicit encoding backend; remains outside `all`. |
| Stress | many instances/frames, large bulk, deep Sequences, WSI pyramid | Same valid plan providers with reduced-scale resource qualification attached. |
| Negative | malformed meta, VR, lengths, identity, pixels, encapsulation | Plan a valid private source, then apply a typed deterministic `MutationPlan`; never pass invalid output to ordinary materialization validation. |
| Fuzz | bounded parser seed qualification | Resolve private valid sources through recipes, run `QualificationPlan`, retain no candidate payload. |
| Non-instance qualification | EOT arithmetic fixture | `QualificationPlan`, not a fake DICOM plan. |
| Media/protocol | DICOMDIR and transaction evidence | Remain downstream consumers of generated roots and outside instance planning. |

## 6. Phased Execution Plan

Each task ID is one logical change or a short explicitly scoped commit series.
Follow `AGENTS.md` for selective staging, commit format, verification, and
source-of-truth policy.

### Phase U0 — Freeze the migration contract

**Purpose:** prevent a structural refactor from silently changing coverage or
evidence.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U0.1 | Architecture decision record adopting one neutral `CorpusPlan` and executor with two evidence frontends. | None |
| U0.2 | Executable audit that derives registry-to-recipe, registry-to-template, generated-file-to-plan, and specialized-validator coverage without hard-coded counts. | U0.1 |
| U0.3 | Baseline projection comparing current profile selection, skipped rows, file ordering, references, validation names, and report axes. | U0.2 |
| U0.4 | Inventory every direct DICOM writer, read-back bridge, composition-to-generator dependency, external full-file backend, and case-specific publication path. | U0.1 |
| U0.5 | Document the allowed temporary exceptions and assign each to a later removal task. | U0.4 |

**Gate U0**

- The audit covers every implemented registry row and every qualified template.
- Every valid emitted file is classified by its current planning/writing path.
- Every specialized profile and non-instance qualification is classified.
- Byte- and semantic-stability baselines are reproducible from commands, not
  committed DICOM payloads.

### Phase U1 — Establish neutral contracts

**Purpose:** add the destination architecture without changing output.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U1.1 | Versioned `CorpusPlan`, artifact, dependency, output, encoding, validation, and evidence types with canonical hashing. | U0 gate |
| U1.2 | `CasePlanner`, `PlanProvider`, content-factory, validation-rule, and manifest-projector interfaces. | U1.1 |
| U1.3 | Case-recipe schema, modular loader, strict unknown-field rejection, and positive/negative fixtures. | U1.2 |
| U1.4 | Registry-to-recipe completeness, uniqueness, version, provider, template, and requirement cross-checks. | U1.3 |
| U1.5 | Adapt composition resolution to return `CorpusPlan` without changing its public manifest or output. | U1.1-U1.2 |
| U1.6 | Add dependency-boundary tests that forbid neutral-core imports from curated/frontend modules and forbid composition-to-generator imports. | U1.2 |

**Gate U1**

- Plans serialize and hash deterministically.
- A no-op adapter can project existing composition behavior through
  `CorpusPlan` with byte-identical output and identical manifest semantics.
- Registry binding errors fail before staging.
- No current case has silently changed generation path yet.

### Phase U2 — Build the shared executor and evidence model

**Purpose:** converge transactionality before migrating families.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U2.1 | `CorpusExecutor` with deterministic DAG scheduling, cancellation, resource accounting, and typed results. | U1 gate |
| U2.2 | Shared content/provider/codec execution contracts and staged asset registry. | U2.1 |
| U2.3 | Shared materialization dispatch for valid DICOM, mutation, qualification, and auxiliary artifacts. | U2.1-U2.2 |
| U2.4 | Typed `RunEvidence` and adapters that reproduce current composition and curated manifest inputs. | U2.1 |
| U2.5 | Shared staging, cleanup, manifest-write, and atomic publication transaction used first by composition. | U2.3-U2.4 |
| U2.6 | Failure-injection tests for cancellation, provider/codec failure, validation failure, manifest failure, cleanup failure, and destination race. | U2.5 |

**Gate U2**

- Composition uses the shared executor end to end.
- Its CLI, Rust API, manifests, reports, hashes, parallel determinism, resource
  envelopes, cancellation, and security tests remain unchanged in meaning.
- The curated frontend can invoke the executor through a compatibility adapter,
  but native case migration has not yet been claimed.

### Phase U3 — Migrate Secondary Capture and metadata plan-first

**Purpose:** remove the read-back bridge for the broadest recipe matrix and
prove declarative case variation.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U3.1 | Modular recipe documents for ordinary SC native/color/palette/padding/shape variants. | U2 gate |
| U3.2 | Shared pixel/content factories and transfer-syntax parameters used by composition defaults and SC recipes. | U3.1 |
| U3.3 | Metadata recipe documents/providers for charset, temporal, Type 2, private creator, value boundary, sequence-length, ICC, LUT, overlay, and nonsquare cases. | U3.1 |
| U3.4 | Encoding-plan support for sequence length, fragmentation, BOT/EOT, padding, and preamble/file-meta variations required by valid SC cases. | U3.2-U3.3 |
| U3.5 | Direct curated `CorpusPlan` generation and shared execution for the migrated cases; remove their post-write import/rematerialization. | U3.2-U3.4 |
| U3.6 | Exact before/after byte, manifest, validation, report, feature-gate, and reproducibility qualification. | U3.5 |

**Gate U3**

- No migrated native recipe builds or reopens a DICOM dataset before planning.
- Static variants are visibly configured in recipe documents rather than a
  monolithic Rust constant table where expressible.
- Codec-specific cases reuse the same object plan and vary through encoding.
- Smoke output and all migrated byte-stable artifacts remain byte-identical.

### Phase U4 — Migrate classic images and geometry

**Purpose:** establish modular modality and series planning.

Parallel lanes may own CT/geometry, MR/CR, DX/mammography, US/NM/PET, VL, and
XA/XRF after shared classic interfaces land.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U4.1 | Neutral common Patient/Study/Series/Equipment/Image and classic pixel/module plan providers. | U3 gate |
| U4.2 | CT and geometry series recipe providers, including ordering, tilt, spacing, duplicate/empty Instance Number, and shared Frame of Reference. | U4.1 |
| U4.3 | MR and CR recipe providers, including multislice geometry, overlay, Modality LUT, and VOI LUT. | U4.1 |
| U4.4 | DX and mammography recipe providers with detector, shutter, presentation/processing, and MONOCHROME1 semantics. | U4.1 |
| U4.5 | US, NM, and PET recipe providers with frame vectors, acquisition, isotope, correction, and rescale semantics. | U4.1 |
| U4.6 | VL, XA, and XRF recipe providers with color/acquisition and projection semantics. | U4.1 |
| U4.7 | Remove corresponding legacy dataset builders, migration mapping arms, and duplicate file-meta/write helpers. | U4.2-U4.6 |
| U4.8 | Cross-family byte, semantic, validation, independent-route, manifest, and report regression. | U4.7 |

**Gate U4**

- Every ordinary classic, geometry, and single-frame VL curated case is
  plan-first.
- Composition and curated cases share neutral module/content factories without
  importing each other's frontend types.
- Central dispatch contains no classic family element construction.

### Phase U5 — Migrate enhanced, WSI, and reference graphs

**Purpose:** replace generated-file source ordering with an explicit plan DAG.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U5.1 | Shared enhanced functional-group, dimension, temporal, and concatenation plan providers. | U4 gate |
| U5.2 | Enhanced CT/MR/PET recipes and stress-frame variants planned directly. | U5.1 |
| U5.3 | WSI tiled-full, tiled-sparse, optical-path, pyramid, and reduced stress providers planned directly. | U5.1 |
| U5.4 | Deterministic artifact DAG and identity/reference resolution for cross-case and bundle sources. | U5.1 |
| U5.5 | Spatial/deformable registration and presentation-state recipes planned from explicit source roles. | U5.4 |
| U5.6 | Remove `write_composition_default_artifacts` and replace advanced composition defaults with shared plan providers. | U5.2-U5.5 |
| U5.7 | Remove generated-file readback as the source of planned reference identity. | U5.4-U5.6 |
| U5.8 | Reproducibility, parallel scheduling, reference closure, byte equality, and independent-route qualification. | U5.7 |

**Gate U5**

- There is no composition-to-generator dependency.
- Enhanced/WSI/default bundles and curated cases originate from the same
  neutral providers.
- Artifact order is a deterministic graph result, not a sequence of dispatcher
  branches.

### Phase U6 — Migrate derived, quantitative, and non-image families

**Purpose:** make the entire ordinary valid P6 catalog plan-first.

Parallel lanes may own quantitative, SR, RT, waveform, and document/mesh
families after shared graph and typed-bulk interfaces land.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U6.1 | Shared typed-bulk and semantic-parameter providers for integer/float pixels, waveform, document, mesh, SR, and RT content. | U5 gate |
| U6.2 | SEG, Parametric Map, and RWVM recipes with exact source-frame roles and content validators. | U6.1 |
| U6.3 | Basic, Comprehensive, Comprehensive 3D, TID 1500, and Key Object recipes with schema-bounded semantic plans. | U6.1 |
| U6.4 | RT Structure Set, Dose, Plan, Image, Radiation, and Radiation Set graph recipes. | U6.1 |
| U6.5 | Twelve-lead and General ECG recipes with typed multiplex-group sample plans. | U6.1 |
| U6.6 | Encapsulated PDF and STL recipes with bounded format validators. | U6.1 |
| U6.7 | External highdicom/pydicom plan-provider/import boundary with exact request, tool, dependency, output, and semantic evidence. | U6.2-U6.3 |
| U6.8 | Remove corresponding native post-write imports and family-specific publication code. | U6.2-U6.7 |
| U6.9 | Catalog-wide default, caller-content, bundle, curated, external-backend, validation, and report qualification. | U6.8 |

**Gate U6**

- Every ordinary `smoke`, `core`, and `extended` valid DICOM case is
  plan-first, whether available or explicitly unavailable in the active build.
- External imports are named boundaries; native recipes cannot use them.
- Specialized semantic and independent evidence is unchanged.

### Phase U7 — Complete codecs, legacy, and stress

**Purpose:** make valid exceptional encodings and resource cases use the same
spine.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U7.1 | Complete transfer-syntax backend registry for all implemented native, feature-gated, and external codecs. | U6 gate |
| U7.2 | Replace remaining full-file codec transforms where feasible with encoded-frame/content plans; document any unavoidable locked-tool boundary. | U7.1 |
| U7.3 | Plan-first Explicit VR Big Endian and legacy JPEG cases with exact feature/tool evidence. | U7.1-U7.2 |
| U7.4 | Plan-first many-instance, many-frame, large-bulk, deep-sequence, long-value, encapsulated, and WSI stress cases. | U7.1 |
| U7.5 | Shared resource preflight and qualification projection from planned versus actual execution. | U7.4 |
| U7.6 | Feature-specific generation, decode, semantic-stability, legacy isolation, and reduced-stress qualification. | U7.3-U7.5 |

**Gate U7**

- Every implemented valid DICOM registry case, including `legacy` and
  `stress`, enters the shared executor as a plan.
- Transfer-syntax variation does not duplicate IOD construction.
- Full-scale unavailability remains explicit.

### Phase U8 — Integrate negative, fuzz, and non-instance qualifications

**Purpose:** make specialized robustness work consume the same valid-source
planning layer without weakening its isolation.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U8.1 | Typed `MutationPlan` model for source identity, ordered edits, ranges, output hash, failure layers, and acceptable outcomes. | U7 gate |
| U8.2 | Negative recipes select private valid source plans by versioned recipe identity rather than invoking writer helpers. | U8.1 |
| U8.3 | Shared executor mutation stage and expected-invalid evidence projector. | U8.1-U8.2 |
| U8.4 | `QualificationPlan` for payload-free fuzz and EOT arithmetic/substrate evidence. | U8.1 |
| U8.5 | Fuzz source planning, bounded execution, candidate cleanup, and no-payload publication through the shared executor. | U8.4 |
| U8.6 | Profile-isolation, mutation-chain, parser-outcome, resource-bound, cleanup, and reproducibility qualification. | U8.3-U8.5 |

**Gate U8**

- Negative and fuzz do not maintain private duplicate valid-object builders.
- Invalid output never enters ordinary valid-DICOM validation or conformance
  inputs.
- Fuzz retains no generated source or candidate payload.
- Media and protocol remain downstream workflows, not forced into instance
  composition.

### Phase U9 — Remove compatibility architecture and promote the spine

**Purpose:** ensure the migration is structural, not another permanent layer.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| U9.1 | Delete native uses of `resolved_plan_from_curated_dataset` and the post-write `migrate_shared_plan_curated_files` pass. | U8 gate |
| U9.2 | Delete obsolete family dataset builders, duplicated DICOM/file-meta writers, manual dispatch stages, and ad hoc source registry ordering. | U9.1 |
| U9.3 | Reduce `src/generator.rs` to curated frontend/orchestration compatibility or replace it with modular planner/provider modules. | U9.2 |
| U9.4 | Make both `generate` and `compose` invoke the same executor and publication transaction directly. | U9.2 |
| U9.5 | Finalize typed manifest projectors and eliminate recipe-authored manifest JSON where a typed result exists. | U9.3-U9.4 |
| U9.6 | Update README, generation/composition guides, system architecture, taxonomy, documentation map, and dated completion status. | U9.5 |
| U9.7 | Run the full verification matrix and executable architectural audits on a clean worktree. | U9.6 |

**Gate U9**

- Dependency-boundary tests prove the terminal module graph.
- No native valid recipe writes DICOM before returning its plan.
- No composition default invokes curated generation.
- No post-write plan-import migration pass exists.
- Every implemented registry recipe has exactly one binding and every binding
  is reachable or explicitly unavailable.
- The central dispatcher performs selection and DAG submission only; it has no
  IOD-family construction or hand-maintained generation order.
- Public outputs and evidence satisfy the Program Acceptance Criteria below.

## 7. Verification Matrix

Every family migration runs focused tests and the following proportional gates.
The orchestrator records commands and outcomes in commits or the final dated
status document.

### 7.1 Always-required checks

```sh
cargo fmt --check
cargo test --locked --all-targets --no-default-features
git diff --check
```

Exercise the documented commands on fresh output roots:

```sh
cargo run --locked -- generate --profile smoke --out <fresh> --seed 1
cargo run --locked -- generate --profile core --out <fresh> --seed 1
cargo run --locked -- generate --profile all --out <fresh> --seed 1
cargo run --locked -- validate <root>
cargo run --locked -- report <root> --format json
cargo run --locked -- compose --spec <fixture> --out <fresh> --seed 1
```

### 7.2 Architecture audits

Tests must derive and enforce:

- every implemented registry row resolves to one recipe binding;
- recipe ID/version exactly match the registry;
- every valid DICOM recipe resolves to qualified template/plan semantics or an
  explicitly documented non-template encoding provider;
- every specialized validator/evidence route remains attached;
- every generated valid file records its corpus-plan and instance-plan hashes;
- no native recipe uses a DICOM import adapter;
- no composition module imports the curated frontend;
- no direct DICOM writer exists outside the shared materializer and explicitly
  allowlisted external/negative boundaries;
- no undeclared output is published; and
- two-run ordering, identities, references, hashes, and manifests are stable.

### 7.3 Profile matrix

Verify independently:

- `smoke`, `core`, `extended`, and `all`;
- `legacy`;
- `stress` and `all --include-stress` selection semantics;
- `negative` expected-invalid isolation;
- `fuzz` payload-free qualification; and
- planned/feature/backend-unavailable skipped rows.

### 7.4 Feature and backend matrix

Run each applicable focused gate with the same generation and validation
features:

- `jpeg`;
- `charls`;
- `jpegxl`, including locked lossy command evidence;
- `jpeg2000`;
- `deflate`;
- `htj2k_openjph`;
- `legacy_jpeg_dcmtk`;
- native RLE; and
- the locked highdicom/pydicom backend.

Unavailable local runtimes may remain explicit, but code paths must have CI or
recorded pinned-environment evidence before their migration gate closes.

### 7.5 Evidence equivalence

For each migrated case compare before and after:

- selection and skipped reason;
- ordered output paths and logical membership;
- exact bytes for `byte_stable`;
- decoded hashes and bounded metrics for `semantic_stable`;
- UIDs and reference closure;
- manifest expectations and validation rows;
- generated report axes and grouping;
- independent conformance route and accepted-finding policy; and
- external backend/codec fingerprints and arguments.

Do not compare only a final SHA-256. A manifest/report/evidence regression is a
failed migration even when DICOM bytes match.

## 8. Parallelization and Integration Rules

The supervising orchestrator owns phase gates, central interfaces, schemas,
and final integration. Sub-agents receive bounded family or infrastructure
lanes only after their dependency contracts land.

Safe parallel lanes include:

- recipe schema and loader tests;
- corpus-plan/DAG types;
- executor/resource/publication work;
- validation/evidence rule registry;
- manifest projector internals;
- disjoint SC/metadata recipe groups;
- disjoint classic modality families;
- enhanced versus WSI providers;
- quantitative, SR, RT, waveform, and document/mesh families;
- codec backends; and
- negative versus fuzz planning.

Unsafe concurrent ownership includes:

- multiple agents editing the same central schema block;
- multiple agents editing monolithic `src/generator.rs` before extraction;
- family work before common plan/provider contracts are committed;
- deleting compatibility bridges while another lane still depends on them;
- broad generated-document rewrites during active descriptor migrations; and
- concurrent changes to manifest semantics without one integration owner.

Integration procedure:

1. Land contracts and tests before family implementations.
2. Give each agent disjoint files/modules and explicit acceptance commands.
3. Require each lane to preserve its current specialized tests and add direct
   plan-first evidence.
4. Integrate in dependency order, one logical commit at a time.
5. Run focused tests after each lane and the default all-target gate after each
   phase.
6. Re-run architecture audits after every bridge deletion.
7. Do not mark a phase complete while temporary adapters assigned to that
   phase remain.

## 9. Risk Controls

### Byte stability

Plan-first construction can change attribute ordering, string padding,
Sequence encoding, fragments, file meta, or implementation identity. Add the
required encoding policy to the neutral plan rather than preserving a second
writer. Use pre-migration generated roots only as private test oracles; do not
commit DICOM payloads.

### Hidden recipe semantics

Some manifest expectations are assembled far from their dataset builders.
Migrate construction, specialized validation, and projection as one traced
case lane. The U0 audit must map all three before implementation begins.

### Over-declarative design

Do not create a JSON workflow language. Static scenario choices belong in
recipe documents; loops, large streams, geometry algorithms, codecs, and
complex graphs belong in typed plan providers with schema-bounded parameters.

### Evidence weakening

The shared executor may make generic validation easier, but it must not erase
case-specific or independent checks. Evidence-equivalence tests are required
before deleting old code.

### Circular dependencies

The current composition-to-generator default path must not be replaced by a
generator-to-composition frontend dependency. Both must depend on neutral plan
providers and content factories.

### External backend semantics

An external backend may intentionally contribute independent construction
behavior. Preserve its identity and semantic evidence through an explicit
provider/import boundary; do not disguise it as native plan construction.

### Long-running migration

Every phase leaves the repository releasable. Compatibility adapters are
allowed only when inventoried, tested, assigned to a removal task, and absent
by U9. Avoid a flag day rewrite.

## 10. Program Acceptance Criteria

The program is complete only when all of the following are demonstrably true:

1. `generate` and `compose` resolve to the same versioned `CorpusPlan` model and
   execute through the same bounded transaction/publication engine.
2. Every implemented valid DICOM registry case—including ordinary, legacy,
   stress, native, feature-gated, and external-backend cases—has one complete
   versioned recipe binding and a plan-first execution path when available.
3. Every native valid recipe returns its plans before file creation. No native
   recipe uses `resolved_plan_from_curated_dataset`, a temporary generated
   DICOM file, or an equivalent read-back bridge.
4. Composition defaults and curated recipes share neutral template/module,
   content, identity, reference, codec, and validation providers. Composition
   has no dependency on the curated generator.
5. The post-write curated migration pass and obsolete duplicate DICOM writers
   are removed.
6. Curated selection, profiles, case IDs, skipped/unavailable reporting,
   manifests, reports, specialized validators, independent evidence, and
   determinism retain their documented meaning.
7. Composition specifications, public Rust APIs, template descriptions,
   manifests, reports, security/resource bounds, provider contracts,
   cancellation, and deterministic parallel behavior retain their documented
   meaning.
8. Negative output is derived from plan-first valid sources through typed
   mutation plans; fuzz uses plan-first private sources and publishes no
   payload; stress remains reduced-scale and opt-in.
9. Recipe differences that are static and schema-expressible are stored in
   modular recipe documents. Algorithmic providers are typed, bounded, and
   referenced explicitly rather than hidden in central dispatch.
10. Architecture audits find no unclassified direct writer, read-back bridge,
    reverse frontend dependency, unbound registry recipe, unreachable recipe,
    or missing validation/evidence route.
11. The full default regression, every applicable feature/backend gate,
    documented fresh-root workflows, reproducibility comparisons, schema
    checks, and `git diff --check` pass.
12. Current operating documentation and a dated completion record describe the
    unified spine honestly, generated artifacts remain untracked, every
    logical change is committed according to `AGENTS.md`, and the worktree is
    clean.

A partial family migration, continued native read-back bridge, shared writer
without shared planning, passing DICOM bytes with weakened evidence, or a new
parallel executor is not completion.
