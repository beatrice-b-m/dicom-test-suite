# Arbitrary DICOM Composition Implementation Plan

**Status:** proposed execution plan; no composition capability is implemented by
this document

**Prepared:** 2026-08-28

**Goal:** add a deterministic, standards-aware composition surface that accepts
caller-supplied DICOM attributes and content while preserving the curated test
suite, its evidence boundaries, and its built-in synthetic non-PHI defaults.

## 1. Target Outcome

The completed project supports two related but distinct workflows:

1. `generate --profile ...` continues to emit committed, registry-selected test
   recipes with exact case expectations and qualification evidence.
2. `compose --spec ...` resolves a versioned caller-authored specification
   through a supported IOD template, supplies documented defaults for omitted
   attributes and content, writes deterministic DICOM Part 10 instances, and
   records the resolved inputs and validation evidence in a manifest.

The composition engine is shared infrastructure, not a parallel serializer.
Curated recipes migrate onto the same attribute, content, transfer-syntax,
identity, reference, Part 10, and generic-validation primitives while retaining
their stronger case-specific oracles.

At program completion:

- every currently implemented, valid DICOM SOP Class in the registry has a
  documented composition template or template bundle;
- negative, fuzz, media, protocol, and non-instance qualifications remain
  outside composition-template coverage;
- every template can produce a deterministic valid default output without
  caller-supplied attributes or content, including a deterministic dependency
  bundle when the primary object requires referenced sources;
- callers can replace the template's applicable pixel or other bulk-content
  slots and can set arbitrary valid standard and private attributes;
- template descriptions expose defaults, required and conditional attributes,
  protected fields, content slots, reference roles, and supported transfer
  syntaxes in machine-readable and human-readable forms; and
- the project never claims that structurally serializing an unknown SOP Class
  establishes IOD conformance.

## 2. Scope And Non-Goals

### 2.1 In scope

- A versioned JSON composition specification and JSON Schema.
- A versioned, standards-evidenced IOD template catalog.
- Typed standard and private attribute values, including nested Sequences.
- Deterministic defaults and deterministic UID/reference resolution.
- Native, encapsulated, float, and double-float pixel-content slots where the
  selected IOD permits them.
- General bulk-content slots for waveform, document, mesh, and other existing
  generated object families.
- Safe local-file content inputs and a later bounded provider protocol for
  dynamic or high-volume content.
- Composition-specific manifests, validation, reports, CLI documentation, and
  a public Rust API.
- Incremental migration of existing recipes onto shared composition primitives.

### 2.2 Out of scope

- Domain-specific image, anatomy, annotation, or training-data generation.
- Synthetic PHI or PHI-like content generation, classification, or policy.
- Committing caller input assets or generated DICOM payloads.
- Treating caller content as a new registry case.
- An embedded workflow language for arbitrary loops, branching, or data
  science pipelines; external callers expand their own job specifications.
- Automatic conformance claims for SOP Classes without a qualified template.
- Network fetching of content or dependencies during generation.
- Weakening the isolation of valid, negative, fuzz, stress, media, or protocol
  evidence.

Caller-supplied content is opaque input. The repository owns safe packaging,
deterministic provenance, and declared validation; it does not generate or
interpret domain-specific content.

## 3. Non-Negotiable Invariants

All phases preserve these contracts:

1. Existing `generate` commands, profile membership, registry meaning, and
   skipped-capability reporting remain backward compatible unless a separately
   versioned public change is explicitly approved.
2. `cases/registry.json` remains the source of truth for curated cases.
   Composition templates live in a separate catalog and never inflate case
   coverage counts.
3. Every template is backed by the pinned standards knowledge base or an
   official-source note under the existing standards policy.
4. Template qualification distinguishes Part 10 validity, generic data-element
   validity, IOD/template validity, pixel or bulk-content validity, and
   independent validation.
5. Built-in defaults remain deterministic, synthetic, and non-PHI and set
   `Synthetic Data (0008,001C)` according to project policy.
6. No generated DICOM, caller content, ordinary manifest, or ordinary report is
   committed.
7. File Meta Information, SOP Class identity, generated UIDs, Pixel Data, and
   pixel-shape attributes are controlled through typed fields. Conflicting raw
   attribute overrides fail before output publication.
8. Unknown private elements require a valid private creator and explicit VR.
   A supplied VR for a known standard tag must match the pinned dictionary.
9. Output remains staged, bounded, validated, and atomically promoted. Safe
   path, symlink, file-count, per-file-size, and total-size checks apply to
   every external content source and provider result.
10. Missing template, codec, provider, or independent-validator capability is
    explicit. It never becomes an implied pass.

## 4. Target Architecture

### 4.1 Resolution pipeline

```text
CompositionSpec
  -> schema and path validation
  -> template and bundle resolution
  -> deterministic identity/reference allocation
  -> AttributePlan + ContentPlan + TransferSyntaxPlan
  -> condition and protected-field validation
  -> ResolvedInstancePlan
  -> Part 10 materialization
  -> generic and template-specific validation
  -> manifest and report projection
  -> atomic output promotion
```

The resolved plan is canonical and hashable. It is the boundary between input
parsing and DICOM writing, and it is also the source for expected manifest
metadata and generic validation.

### 4.2 Core types

The exact Rust names may change during implementation, but the responsibilities
must remain explicit:

- `CompositionSpec`: versioned caller input.
- `InstanceSpec`: caller intent for one logical output object or bundle root.
- `TemplateId` and `TemplateVersion`: stable template identity.
- `TemplateDescriptor`: SOP Class, modules, constraints, defaults, content
  slots, reference slots, supported transfer syntaxes, and evidence.
- `AttributePlan`: typed standard and private elements after precedence and
  condition resolution.
- `AttributeValue`: VR-correct primitive, multi-valued, binary, Sequence, empty,
  or absent representation.
- `ContentPlan`: default, local-file, inline-small-fixture, or provider content.
- `PixelPlan`: integer, one-bit, float32, float64, native, or encoded-frame
  specialization.
- `ReferencePlan`: logical instance IDs and frame roles before UID resolution.
- `IdentityPlan`: deterministic Study, Series, SOP, Frame of Reference, and
  other UID roles.
- `ResolvedInstancePlan`: immutable, validated, serializer-ready plan.
- `CompositionRunManifest`: input, template, asset, resolved-plan, generated
  output, and validation provenance.

### 4.3 Attribute precedence and protection

Resolution uses this order:

1. template defaults;
2. composition-level patient, study, series, and equipment defaults;
3. instance attributes;
4. derived structural values from identities, references, and content shape.

Layer 4 is authoritative. A caller cannot use generic attributes to contradict
the selected SOP Class, transfer syntax, generated UIDs, Rows, Columns, Number
of Frames, sample representation, or bulk-content element. The caller instead
uses the corresponding typed identity, pixel, content, or reference field.

Supported attribute operations are `set`, `empty`, and `remove`. `remove`
fails for Type 1, Type 2, satisfied Type 1C/2C, and protected elements. Sequence
items are recursively typed. Standard tags may be addressed by normalized tag;
keyword aliases are a CLI convenience and normalize to tags before hashing.

### 4.4 Content inputs

Initial `PixelPlan` variants:

- `default`: deterministic template-owned pixels;
- `raw`: local frame bytes plus explicit dimensions, sample type, byte order,
  bits, photometric interpretation, and planar organization;
- `image`: a small, explicitly supported set of lossless input containers with
  derived shape and channel metadata; and
- `encoded_frames`: advanced encapsulated-frame input with declared transfer
  syntax and independent decode requirements.

`ContentPlan` later adds waveform samples, encapsulated document bytes, mesh
bytes, and other qualified bulk slots. Large local inputs are hashed and copied
or streamed through private staging; they are not embedded in the JSON spec.

### 4.5 Template catalog

The template catalog is separate from `cases/registry.json`, for example:

```text
templates/
  catalog.json
  classic/
  enhanced/
  derived/
  vl/
  non-image/
```

Each descriptor records:

- template ID, version, IOD name, SOP Class UID, default modality, and artifact
  kind;
- supported transfer syntaxes and feature/runtime requirements;
- required, conditional, optional, defaulted, derived, and protected
  attributes;
- supported content and reference slots;
- default dependency-bundle behavior;
- validation rule IDs and independent-validator routes; and
- standards evidence and determinism classification.

`templates list` and `templates describe` read this catalog. Human-facing
reference documentation is rendered from the same descriptors so exposed-tag
documentation cannot silently drift from execution.

### 4.6 Manifest and reporting

The manifest remains backward compatible for curated runs and gains an
explicit composition branch. Composition entries include:

- `run.kind = composition`;
- composition schema version and exact input-spec SHA-256;
- logical `instance_id`, template ID, and template version, distinct from
  curated `case_id`;
- resolved-plan SHA-256;
- input asset relative path, size, SHA-256, kind, and content slot;
- whether every value came from a template default, run default, instance
  override, derived structural field, or provider;
- deterministic identities and closed references;
- output hashes and frame or bulk-content hashes;
- generic, template, codec/provider, and optional independent validation; and
- unavailable capability records with stable reason codes.

Reports distinguish curated coverage from composition output. A custom run
summarizes templates, SOP Classes, content sources, transfer syntaxes,
validation, and determinism but does not add registry coverage.

## 5. Execution Graph And Parallelization Rules

### 5.1 Phase dependency graph

```text
P0 contracts
  -> P1 shared plan engine
      -> P2 Secondary Capture vertical slice
          -> P3 classic image templates
          -> P4 curated recipe migration
          -> P5 enhanced and reference graph
              -> P6 derived and non-image content
                  -> P7 providers, scale, and public API
                      -> P8 full qualification and promotion
```

P3 and P4 overlap after the first shared classic modules are stable. Within P3,
P5, and P6, modality/object-family lanes run in parallel after the phase's
common interfaces land. P7 provider and streaming work can begin after P2's
content contract is stable, but it cannot be promoted until P6 defines all
bulk-content slot kinds.

### 5.2 Safe parallel work

Parallel agents should own disjoint modules, schemas, templates, or test files.
Do not run concurrent edits against the current monolithic `src/generator.rs`,
`src/lib.rs`, or `src/validation.rs`. Land the relevant extraction/interface
commit first, then delegate work in new family-specific files.

Recommended parallel ownership boundaries:

- composition schemas and schema tests;
- core typed plan and resolver;
- safe content staging and hashing;
- manifest/report schema and projection;
- individual IOD family templates and builders;
- individual validation adapters;
- CLI and generated documentation;
- independent conformance routes.

Each parallel lane uses its own worktree or branch. The orchestrator integrates
in dependency order, runs the shared regression gate after each merge, and
never asks two agents to own the same central dispatch or schema section.

### 5.3 Task and commit size

Every task ID below is one reviewable logical unit unless its acceptance list
explicitly requires a short series of commits. Follow `AGENTS.md`: stage files
selectively; use `type(scope): subject`; include a body explaining the invariant
or design decision; never amend or force-push; and confirm every task with
`git log --oneline -3`.

An agent does not combine implementation, unrelated refactoring, another
family's templates, and broad documentation cleanup in one commit. Tests that
establish the same behavior normally travel with the implementation commit;
large fixture/schema additions may use a preceding coherent contract commit.

## 6. Phased Tasks

### Phase P0 — Freeze contracts and evidence boundaries

**Purpose:** agree on terminology, schemas, backward compatibility, and the
supported-template claim before restructuring generator code.

| Task | Deliverable | Depends on | Parallel after |
| --- | --- | --- | --- |
| P0.1 | Architecture decision record defining `generate` versus `compose`, registry versus template catalog, and qualification vocabulary. | None | Immediately |
| P0.2 | `composition-spec` JSON Schema with typed attributes, identities, references, local content sources, and strict unknown-field rejection. | P0.1 | P0.1 |
| P0.3 | `template-catalog` JSON Schema with standards evidence, attribute policies, content slots, references, defaults, and requirements. | P0.1 | P0.1 |
| P0.4 | Backward-compatible manifest design for `run.kind` and composition entries. | P0.1 | P0.1 |
| P0.5 | Security/resource policy for local inputs, staging, symlinks, size budgets, provider execution, and network prohibition. | P0.1 | P0.1 |
| P0.6 | Initial catalog inventory mapping every currently implemented valid SOP Class to a planned template family and qualification owner. | P0.3 | P0.3 |
| P0.7 | Schema compile/positive/negative fixtures and path-safety tests. | P0.2-P0.5 | Contracts stable |

**Gate P0**

- Schemas compile and reject unknown fields, unsafe paths, conflicting identity
  forms, malformed tags/VRs, and inline large bulk data.
- The manifest design represents curated and composition runs without changing
  existing generated manifests.
- The inventory names every currently implemented valid DICOM SOP Class and
  excludes negative/fuzz/media/protocol qualification rows.
- Standards evidence requirements for templates are documented.

### Phase P1 — Build the shared plan engine

**Purpose:** create stable, testable interfaces before exposing a CLI.

| Task | Deliverable | Depends on | Parallel after |
| --- | --- | --- | --- |
| P1.1 | Typed tag, VR, primitive, multi-value, empty, remove, private-creator, and recursive Sequence model. | P0.2 | P0 gate |
| P1.2 | Template descriptor loader, catalog uniqueness checks, version resolution, and requirement evaluation. | P0.3 | P0 gate |
| P1.3 | Deterministic identity allocator keyed by standards lock, template version, run seed, logical instance ID, role, and index. | P0.1 | P0 gate |
| P1.4 | Logical reference graph with cycle policy, dependency closure, source roles, frame roles, and UID materialization. | P1.3 | P1.3 |
| P1.5 | Attribute precedence, conditional-rule evaluation, protected-field detection, and canonical resolved-plan hashing. | P1.1-P1.3 | Core types stable |
| P1.6 | Local content-source resolver with safe paths, regular-file/symlink checks, hashing, bounds, and staging. | P0.5 | P0 gate |
| P1.7 | Generic native pixel-shape and byte-length planner for 1/8/16/32-bit integer, float32, float64, RGB/YBR/palette, planar, and multi-frame layouts. | P1.1, P1.6 | Attribute/content interfaces stable |
| P1.8 | Generic Part 10 materializer consuming only `ResolvedInstancePlan`. | P1.5, P1.7 | Resolver stable |
| P1.9 | Composition manifest assembler and generic validator projection from the same resolved plan. | P0.4, P1.5 | Manifest contract stable |

**Gate P1**

- Unit tests cover canonical hashing, precedence, protected collisions, private
  creators, nested Sequences, deterministic identities, references, path
  safety, and pixel-length arithmetic.
- The Part 10 writer has no dependency on registry case IDs.
- The same resolved plan drives writing, manifest expectations, and generic
  validation.
- No public CLI command is advertised yet.

### Phase P2 — Secondary Capture end-to-end proof

**Purpose:** prove defaults, custom tags, custom pixels, manifests, reports, and
validation through one fully supported public vertical slice.

| Task | Deliverable | Depends on | Parallel after |
| --- | --- | --- | --- |
| P2.1 | Secondary Capture template descriptors for monochrome and RGB native defaults. | P1 gate | P1 gate |
| P2.2 | Deterministic default pixel providers and default patient/study/series/equipment modules. | P1.7 | P1.7 |
| P2.3 | Raw native pixel source for single and multi-frame inputs with exact frame hashes. | P1.6-P1.8 | P1 gate |
| P2.4 | Standard/private attribute overrides, empty/remove operations, and nested Sequences in an SC output. | P1.1, P1.5, P2.1 | Template stable |
| P2.5 | `templates list`, `templates describe`, and machine-readable descriptor output. | P1.2, P2.1 | Catalog stable |
| P2.6 | `compose --spec --out --seed`, dry-run resolution, transactional publication, and concise output summary. | P1.8-P1.9, P2.1-P2.4 | Engine stable |
| P2.7 | `validate` and `report` support for a composition root without projecting registry coverage. | P1.9, P2.6 | Manifest entries stable |
| P2.8 | End-to-end CLI, reproducibility, schema, invalid-input, path-safety, and independent IOD tests plus first user guide. | P2.1-P2.7 | Feature complete |

**Gate P2**

- A spec naming only an SC template produces deterministic valid default DICOM.
- External raw monochrome and RGB frames round-trip exactly.
- Standard, private, empty, multi-valued, and Sequence attributes round-trip.
- Contradictory Rows, SOP Class, UID, transfer syntax, or Pixel Data overrides
  fail before the output root is promoted.
- Two identical runs produce identical byte-stable output and canonical
  manifests except for intentionally excluded environment paths.
- The exact documented commands pass on fresh output paths.

### Phase P3 — Classic image template breadth

**Purpose:** prove the engine across modality-specific requirements rather than
remaining a Secondary Capture wrapper.

Land P3.1 and P3.2 first; then P3.3-P3.7 are parallel family lanes.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| P3.1 | Shared Patient, Study, Series, Frame of Reference, Equipment, General Image, and content-date/time module builders with typed defaults. | P2 gate |
| P3.2 | Shared classic image, geometry, display transform, detector, acquisition, and pixel module plans with template conditions. | P3.1 |
| P3.3 | CT, MR, and CR templates with default and caller-supplied native pixels. | P3.2 |
| P3.4 | DX and mammography For Presentation/For Processing templates, including their distinct required/default display and detector semantics. | P3.2 |
| P3.5 | Ultrasound single/multi-frame, Nuclear Medicine, and PET templates. | P3.2 |
| P3.6 | VL Photographic, Endoscopic, and Microscopic templates. | P3.2 |
| P3.7 | XA and XRF templates plus supported native and existing codec-gated transfer syntaxes. | P3.2 |
| P3.8 | Cross-family template-reference documentation and template-specific independent conformance routes. | P3.3-P3.7 |

Each family lane includes descriptor evidence, defaults, content-slot validation,
manifest fields, generic plus template validation, list/describe output, CLI
tests, two-run reproducibility, reporting, and documentation.

**Gate P3**

- Every implemented classic-image and single/multi-frame VL SOP Class has a
  qualified template.
- Template defaults pass the existing internal validator and the pinned
  independent route appropriate to the family.
- Raw caller pixels are checked against each IOD's permitted pixel model and
  transfer syntax.
- All family descriptors document protected, derived, conditional, and
  caller-settable attributes.

### Phase P4 — Migrate curated classic recipes

**Purpose:** prove the composition engine is shared production infrastructure.

P4 lanes may overlap P3 after the corresponding family template is qualified.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| P4.1 | Adapt SC curated recipes to build resolved plans while preserving case-specific expectations. | P2 gate |
| P4.2 | Adapt CT/MR/CR curated recipes by family. | P3.3 |
| P4.3 | Adapt DX/mammography curated recipes by family. | P3.4 |
| P4.4 | Adapt US/NM/PET curated recipes by family. | P3.5 |
| P4.5 | Adapt VL/XA/XRF curated recipes by family. | P3.6-P3.7 |
| P4.6 | Remove superseded duplicate element-writing helpers only after all callers migrate. | P4.1-P4.5 |

**Gate P4**

- Existing byte-stable cases retain exact hashes for identical recorded inputs,
  or a deliberate recipe-version change documents every unavoidable delta.
- Existing semantic-stable cases retain decoded hashes and semantic contracts.
- Registry selection, profiles, reports, skipped rows, and independent
  conformance evidence are unchanged in meaning.
- Central generator dispatch becomes a registry of recipe implementations, not
  another list of inlined dataset builders.

### Phase P5 — Enhanced images, bundles, and reference graphs

**Purpose:** support multi-frame functional groups and deterministic dependency
closure.

After P5.1-P5.3 land, P5.4-P5.7 run in parallel.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| P5.1 | Shared Functional Groups, Dimension Organization/Index, concatenation, temporal, and per-frame plan types. | P3 gate |
| P5.2 | Bundle resolver that can create deterministic default source instances and close references when caller sources are omitted. | P1.4, P3 gate |
| P5.3 | Manifest/report representation for bundles, source provenance, frame references, and dependency closure. | P5.2 |
| P5.4 | Enhanced CT and Enhanced MR templates. | P5.1-P5.3 |
| P5.5 | Enhanced PET and other currently implemented enhanced image templates. | P5.1-P5.3 |
| P5.6 | WSI tiled full/sparse, multiple optical paths, and pyramid templates. | P5.1-P5.3 |
| P5.7 | Registration and presentation-state templates over explicit or default source bundles. | P5.2-P5.3 |
| P5.8 | Migrate corresponding curated recipes onto shared plans and retain exact specialized oracles. | P5.4-P5.7 |

**Gate P5**

- A caller can compose enhanced and WSI objects with default or supplied frame
  content.
- Functional-group, dimension, concatenation, tiling, and frame-reference
  inconsistencies fail before publication.
- Default bundles generate deterministic source closure without being counted
  as curated registry cases.
- Existing enhanced, WSI, registration, and presentation-state cases preserve
  their qualified semantics.

### Phase P6 — Derived, quantitative, RT, waveform, and encapsulated content

**Purpose:** cover every remaining currently implemented valid DICOM instance
family with appropriate non-pixel content slots and reference rules.

P6.1 is shared. P6.2-P6.6 are parallel lanes.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| P6.1 | General `BulkDataPlan` and typed slot traits for pixels, waveform samples, encapsulated documents, meshes, and backend-produced payloads. | P5 gate |
| P6.2 | SEG, Parametric Map, and Real World Value Mapping templates and default source bundles. | P6.1 |
| P6.3 | Basic Text, Comprehensive, Comprehensive 3D, TID 1500, and Key Object templates with content-tree/reference parameters rather than arbitrary untyped trees. | P6.1 |
| P6.4 | RT Structure Set, Dose, Plan, Image, Radiation, and Radiation Set templates with closed default graph. | P6.1 |
| P6.5 | Twelve-lead and General ECG waveform templates with default or supplied waveform content. | P6.1 |
| P6.6 | Encapsulated PDF and STL templates with default or supplied document/mesh content. | P6.1 |
| P6.7 | Migrate all corresponding curated recipes while retaining their specialized validation and independent routes. | P6.2-P6.6 |

**Gate P6**

- Every currently implemented valid DICOM SOP Class has a catalog template or
  template bundle.
- Every non-pixel bulk slot has exact size/hash provenance and an appropriate
  independent semantic validator.
- Derived and RT default bundles have complete, deterministic references.
- Template APIs expose safe high-level parameters for structured semantics and
  typed attribute overrides for additional valid elements.

### Phase P7 — Provider API, streaming, codecs, and large corpora

**Purpose:** make composition practical for external producers and large jobs
without weakening safety or determinism.

| Task | Deliverable | Depends on | Parallel after |
| --- | --- | --- | --- |
| P7.1 | Public Rust composition API over the same schema, resolver, and writer used by the CLI. | P2 gate | P2 gate |
| P7.2 | Versioned external content-provider request/response contract with preallocated identities, declared slots, hashes, bounds, timeouts, cleared environment, and no implicit network. | P6.1 | Bulk slot contract stable |
| P7.3 | Streaming or spill-to-disk native Pixel Data and bulk writing with bounded memory. | P2.3 | Content contract stable |
| P7.4 | Deterministic bounded file-level parallelism with stable output ordering and manifest canonicalization. | P5.2, P7.3 | Bundle and streaming stable |
| P7.5 | Existing codec integration for caller-supplied native frames, with feature/runtime availability and semantic-stable evidence. | P3.7 | Pixel plan stable |
| P7.6 | Resource envelopes, cancellation cleanup, provider crash/hang handling, and large-corpus qualification. | P7.2-P7.5 | Components stable |
| P7.7 | External integration guide with CLI, Rust API, provider, provenance, and reproducibility examples using neutral synthetic fixtures. | P7.1-P7.6 | APIs stable |

**Gate P7**

- External callers can integrate by file-backed CLI spec, Rust API, or bounded
  provider without maintaining a fork of the repository.
- Large corpora do not require holding the full corpus in memory; large single
  content values obey a documented bounded-memory path.
- Parallel and sequential generation produce the same canonical identities,
  ordering, and output hashes where determinism permits.
- Provider failures cannot publish a partial corpus or undeclared file.

### Phase P8 — Full qualification and promotion

**Purpose:** close documentation, compatibility, conformance, migration, and
scope claims before advertising arbitrary composition as supported.

| Task | Deliverable | Depends on |
| --- | --- | --- |
| P8.1 | Catalog audit against all implemented valid registry SOP Classes and template bundles. | P6-P7 gates |
| P8.2 | Full default-template generation, strict validation, reports, and independent conformance with all optional runtimes explicitly accounted for. | P8.1 |
| P8.3 | Two-run reproducibility for every byte-stable template and semantic comparison for every semantic-stable template. | P8.1 |
| P8.4 | Adversarial input suite for paths, symlinks, malformed VR/VM, conditional attributes, protected collisions, references, pixel sizes, providers, and resource limits. | P7 gate |
| P8.5 | README, generation guide, composition guide, corpus-consumption guide, taxonomy/source-of-truth text, and status record updated to current behavior. | P8.1-P8.4 |
| P8.6 | Remove temporary compatibility shims and record remaining planned/unavailable composition gaps with stable blockers. | P8.1-P8.5 |

**Gate P8 / program completion**

- All acceptance criteria in Section 8 pass.
- Current operating documentation distinguishes curated generation from custom
  composition and makes no claim beyond qualified catalog templates.
- No implemented valid registry SOP Class lacks a qualified template.
- No generated payload, caller asset, ordinary manifest/report, cache, or
  private runtime is staged in git.
- The complete default regression, exact documented composition workflows,
  feature-specific tests, independent validation, and `git diff --check` pass.

## 7. Verification Matrix

Every task runs focused tests. Every integration gate runs at least:

```sh
cargo test --locked --all-targets --no-default-features
git diff --check
```

Composition-schema and catalog work also runs all schema artifact tests.
CLI/documentation phases exercise the exact documented commands against new
output roots, including at minimum:

```sh
cargo run --locked -- templates list
cargo run --locked -- templates describe TEMPLATE_ID --format json
cargo run --locked -- compose \
  --spec /path/to/spec.json --out generated/composition-seed-1 --seed 1
cargo run --locked -- validate generated/composition-seed-1
cargo run --locked -- report \
  generated/composition-seed-1 --format markdown
```

These commands are proposed until P2 is implemented and must not be added to
current operating guides as usable before that gate.

Feature-gated codec tasks run their exact feature sets for generation and
validation. Template-family promotion runs the pinned independent IOD, pixel,
template, entity, or payload adapters appropriate to that family and records
tool identity. Same-project generation and validation are never described as
independent conformance.

Reproducibility tests compare:

- exact files and canonical manifest entries for `byte_stable` templates;
- decoded frame/bulk hashes, resolved plans, references, and bounded semantic
  metrics for `semantic_stable` templates; and
- absence of host-specific absolute paths from deterministic identity inputs.

## 8. Program Acceptance Criteria

The goal is achieved only when all of the following are true:

1. `generate --profile ...` remains a curated registry workflow and passes its
   complete existing regression and evidence gates.
2. `compose --spec ...` is a documented, versioned, transactional workflow.
3. A template-only spec produces valid deterministic defaults.
4. Every currently implemented valid registry SOP Class is represented by a
   qualified template or deterministic template bundle.
5. Standard, private, empty, multi-valued, and nested Sequence attributes
   round-trip through typed overrides.
6. Caller content round-trips according to its declared native, encoded,
   float, waveform, document, mesh, or other slot contract.
7. Structural conflicts are rejected before publication.
8. Template descriptions expose every supported default, attribute policy,
   content slot, reference role, transfer syntax, requirement, and limitation.
9. Composition manifests bind the exact spec, resolved plan, inputs, outputs,
   identities, references, hashes, provider/tool identities, and validation.
10. Composition reports never inflate or alter curated case coverage.
11. Default dependency bundles close every required reference without caller
    input and remain deterministic.
12. Local-file, provider, path, symlink, resource, timeout, cleanup, and partial
    publication protections are tested.
13. Large-corpus generation has bounded memory and deterministic ordering.
14. Built-in defaults remain synthetic and non-PHI; the project contains no
    domain-specific content generator.
15. Current user-facing documentation and dated promotion evidence accurately
    describe implemented and unavailable scope.

Near-completion is not completion. A Secondary Capture-only interface, raw
element serializer, unqualified unknown-SOP mode, partially migrated second
generator stack, or undocumented set of template conditions does not satisfy
this goal.

## 9. Orchestrator Goal Prompt

Use the following prompt as the supervising orchestrator's goal condition:

```text
Implement the arbitrary DICOM composition program defined in
docs/arbitrary-dicom-composition-plan.md to its Phase P8 completion gate.

This goal turns the current curated test-case generator into a shared,
standards-aware composition platform while preserving the existing curated
suite and its evidence boundaries.

Before acting, read AGENTS.md and the complete implementation plan. Treat the
plan's phases, dependencies, parallelization rules, invariants, verification
matrix, and acceptance criteria as the detailed execution contract. Resolve
current-state questions using the repository's documented source-of-truth
order.

The task is done only when the Phase P8 gate and every Program Acceptance
Criterion in the plan are demonstrably satisfied, all required implementation
and qualification evidence is committed in the required granular history, the
full prescribed verification passes, and the worktree is clean. A partial
vertical slice or subset of supported object families is not completion.

Begin with Phase P0 and continue through the dependency graph until that
terminal condition is reached or a genuine explicit blocker must be reported.
```
