# ADR 0003: Separate `synth-dicom-gen` from `dcmview-test-corpus`

**Status:** accepted

**Date:** 2026-09-01

**Supersedes:** the repository-boundary assumptions of
`docs/adr/0002-standalone-product-boundary.md` and
`docs/standalone-productization-plan.md`

## Context

The standalone product work proved that the current `dicom-test-suite` release
candidate can operate through relocatable CLI and Rust SDK contracts. It did
not separate reusable generation behavior from the bundled dcmview-oriented
case registry, profiles, recipes, expectations, and qualification machinery.
Those inputs are embedded in one product-resource identity and changes to them
invalidate generator builds and tests even when the engine is unchanged.

The exact `69d3e5f8e045752b6e183781a7e13190a61430ff` release-candidate
qualification remains valid for that immutable `dicom-test-suite 0.1.0`
candidate and its named artifacts. It is historical evidence, not evidence for
the renamed product, a changed corpus definition, or a future artifact.

This decision applies the compatibility rules in
`docs/compatibility-policy.md`. It preserves ADR 0002's CLI-primary,
SDK-secondary integration order and its separation of curated generation,
qualified composition, and structural assembly evidence. It replaces ADR
0002's assumption that a reusable product owns an embedded curated corpus.

## Decision

### Fixed names and release line

The reusable product and its repository are named `synth-dicom-gen`. Its Cargo
package and crate distribution name is `synth-dicom-gen`, its Rust library path
is `synth_dicom_gen`, its primary executable is `synth-dicom-gen`, and its
target archives use the `synth-dicom-gen-<version>-<target>` stem.

The downstream repository is named `dcmview-test-corpus`. It owns the dcmview
corpus definition and viewer expectations. Generated corpus payloads are
artifacts, not repository content or a third product repository.

The first renamed product release is `synth-dicom-gen 0.2.0`. The `0.1.0`
candidate was not published as a supported release, so the default migration
is a clean rename: no `dicom-test-suite` Cargo package, Rust path, executable,
archive, or environment-variable alias ships by default. R3 must inventory
external consumers before removing the old source-tree spelling. If a real
external consumer is found, any temporary alias requires an explicit support
window, migration guidance, and packaged external-consumer tests; discovery
must make the alias visible. Historical artifact names and hashes are never
renamed.

### Compatibility classification

The `0.2.0` product version is not permission to reuse an independent schema,
protocol, resource, or manifest version with a changed meaning. Each boundary
is classified and versioned separately:

| Boundary | Classification and required treatment |
| --- | --- |
| Cargo package/crate | Renaming `dicom-test-suite` to `synth-dicom-gen` is a breaking pre-1.0 product change. Increment the product minor version to `0.2.0`; use no compatibility package unless the consumer inventory requires and tests one. |
| Rust library | Renaming `dicom_test_suite` to `synth_dicom_gen` breaks the supported SDK import path. `0.2.0` is the required product boundary. The supported surface remains `synth_dicom_gen::sdk`; downstream production code may not replace it with currently public internal modules. Any temporary Rust alias follows the same explicit adapter rule. |
| Executable and distribution | Renaming the executable and archives is a breaking product/distribution change covered by `0.2.0`. CLI subcommand, flag, JSON envelope, error-code, exit-class, and stdout/stderr meanings retain their independent CLI API version rules. Renaming or removing one of those supported meanings requires a new CLI API major or a tested adapter. Human formatting is not a compatibility surface. |
| Corpus-definition schema | `CorpusDefinitionBundle` is a new caller-input contract and starts at its own declared `1.0.0`. Additive fields require a schema minor; renamed/removed fields, newly required fields, changed defaults, or rejection of previously conforming input require a schema major or an adapter. The exact accepted input bytes and schema version contribute to corpus identity. |
| Existing request and result schemas | Adding an optional external-corpus input while retaining old behavior is additive. Removing embedded-corpus defaults, changing selection meaning, or changing an accepted request/result meaning is breaking and requires the applicable schema or CLI API major unless a supported adapter preserves it. Product `0.2.0` alone does not satisfy this rule. |
| Engine resources | Splitting `ProductResources` into immutable engine, schema, and template/provider domains changes inventory and digest meaning. The public SDK type rename is product-breaking at `0.2.0`; any resource document whose existing fields change meaning requires its own major. Explicit engine resources remain immutable integrity-checked product input and cannot be used as a mutable corpus override. |
| Manifest | Separating engine, template/provider catalog, corpus definition, schema, toolchain, and external-runtime identities changes manifest structure and provenance meaning and therefore requires the applicable manifest-schema major. Readers retain supported historical versions for their published windows. Migration parity compares normalized semantics without rewriting old manifests or hashes. |
| Recipe/template output | A byte-stable output change still requires a recipe or template version and proportional determinism evidence. A product, schema, or repository rename cannot absorb such a change. Semantic-stable output retains its declared semantic comparison and cannot be promoted to byte-stable evidence. |

The old embedded curated-generation route may remain temporarily during the
migration, but R9 removal must cross every compatibility boundary it actually
changes. Deprecation is additive and cannot shorten the support windows in the
compatibility policy.

### Ownership and dependency direction

`synth-dicom-gen` owns reusable, viewer-neutral behavior: bounded plan-first
execution, Part 10 writing, atomic publication, generic validation and
provenance, immutable engine resources, qualified templates and generic
providers, generic mutation/fuzz/negative primitives, codecs and external-tool
adapters, capability discovery, and the supported CLI and SDK corpus-loading
contracts. It owns product, security, packaging, public-consumer, and release
qualification for those meanings.

`dcmview-test-corpus` owns stable case IDs, registries, profiles, selection,
dcmview corpus definitions, corpus-specific standards notes, expectations,
known viewer results, issue links, compatibility-report schemas, dependency and
runtime policy, migration parity, and publication/retention of generated
corpus artifacts.

The only production dependency direction is:

```text
dcmview-test-corpus definition and expectations
  -> versioned synth-dicom-gen CLI or synth_dicom_gen::sdk request
  -> reusable planning, providers, materialization, and validation
  -> manifest-bound corpus artifact
  -> dcmview tests and compatibility reporting
```

The generator must not import, test, name, or derive a current operating claim
from dcmview corpus content. Historical and migration records may name the
downstream repository. The corpus may consume a released crate, an immutable
Git revision, or a checksummed native artifact, but its terminal workflows may
not require a sibling checkout, path dependency, or unsupported generator
module. Caller-owned corpus input is data validated against a versioned
contract, never replacement engine resources.

### Identity and evidence boundaries

Manifests and discovery separate identities for the engine, schema,
template/provider catalog, corpus definition, toolchain, and applicable
external runtimes. A corpus-only change changes corpus identity without
changing engine identity; a toolchain-only change cannot masquerade as a
corpus change. Artifact keys bind every input needed for the declared byte- or
semantic-stability class.

Valid, legacy, negative, fuzz, stress, media, and protocol scopes remain
separate. Qualified composition and structural assembly do not acquire curated
case credit. Built-in generation and validation remain same-project evidence;
independent conformance and interoperability require their pinned external
routes. Missing features, providers, codecs, validators, peers, or runtimes
remain explicit unavailable results. Reduced stress qualification does not
prove full-scale behavior, and payload-free fuzz qualification does not retain
or promote generated candidates.

Existing qualified bytes are preserved during migration unless a deliberately
versioned recipe or template change records the new identity and required
evidence. Repository movement, normalization, or performance targets cannot be
used to waive a parity failure or silently narrow a claim.

### Verification-class invalidation

Every change names all classes it invalidates. The narrowest applicable class
runs first; broader scheduled or candidate evidence remains required when the
changed surface reaches it.

| Changed surface | Minimum immediate class | Additional invalidation |
| --- | --- | --- |
| Documentation only | Fast PR documentation/schema-link checks | Release candidate when a current release procedure, compatibility promise, target claim, or artifact inventory changes. |
| Engine, writer, validation, security, or public SDK/CLI behavior | Owning Subsystem plus Fast PR | Nightly broad defaults; Release candidate for any release claim or supported contract change. |
| Corpus definition, profile, selection, expectation, or corpus standards note | Corpus PR for changed cases plus dependency closure | Nightly/full corpus when shared selection or closure changes; corpus Release candidate for claimed scopes. It does not invalidate engine identity by itself. |
| Template, generic provider, codec, or external adapter | Owning Subsystem with selected capability cases | Applicable Nightly matrix and both release candidates when the downstream corpus consumes that capability. |
| Schema, identity projection, manifest, artifact key, or resource inventory | Owning Subsystem plus affected external-consumer and adversarial fixtures | Nightly and Release candidate because provenance, compatibility, or reuse meaning changes. |
| Toolchain, target, feature set, or external runtime | Owning Subsystem or applicable capability job | Exact affected target/runtime Release candidate; no claim transfers from another target or runtime. |

Fast PR excludes WSI, stress, full-profile, Python-backend, package, and release
builds. Corpus PR generates changed cases and dependencies, not the full
corpus. Heavy evidence is moved to the plan's named nightly, manual, or exact
release-candidate cadence; it is not deleted, weakened, or inferred from a
cheaper class.

### Non-goals

This decision does not establish:

- a network service, hosted API, multi-tenant executor, or runtime downloading;
- medical or anatomical synthesis, patient-data intake, or a PHI-handling
  claim;
- a general plug-in system or arbitrary executable provider trust;
- an engine-resource override mechanism for downstream corpus policy;
- automatic IOD conformance for structural assembly or independent DICOM
  certification from same-project validation;
- committed generated DICOM, manifests, reports, caches, build trees, or
  external runtime environments;
- Windows or any other target claim before exact native terminal evidence
  exists for that target;
- preservation of source paths or unsupported internal Rust modules as public
  APIs; or
- performance savings obtained by deleting evidence, weakening limits,
  broadening unavailable outcomes, or silently narrowing release claims.

## Consequences

- Corpus changes can be reviewed, generated, and tested without rebuilding the
  engine's internal qualification inventory.
- The generator gains a versioned external-corpus contract and more precise
  identity domains, increasing schema, adversarial, compatibility, and public-
  consumer testing obligations.
- The downstream corpus becomes independently versioned and must pin a
  supported generator artifact or dependency without repository-layout
  coupling.
- The current monolithic product remains the migration source until parity and
  terminal gates pass; this ADR does not itself prove that separation.
- Historical `dicom-test-suite` evidence remains immutable and explicitly
  scoped, while all new product claims require new `synth-dicom-gen` evidence.

## Classification test

Review a proposed change in this order:

1. Which repository owns its policy and implementation under this ADR?
2. Does it alter a supported crate, Rust SDK, executable, CLI, schema,
   resource, manifest, archive, template/recipe, identity, or determinism
   meaning? Apply every matching compatibility rule above.
3. Which Fast PR, Subsystem, Corpus PR, Nightly, or Release candidate class is
   invalidated?
4. Does it alter valid, negative, fuzz, stress, media, protocol, same-project,
   or independent evidence meaning? Preserve the affected boundary and run its
   gate.
5. Does it change qualified bytes or semantic output? Require the declared
   parity comparison or a deliberate versioned migration.

A change may belong to multiple rows. Documentation cannot reclassify a
failing supported capability as internal, unavailable, or out of scope after
implementation.

## Source-of-truth order

Until migration completes, executable behavior and schemas remain
authoritative, followed by `cases/registry.json`, a particular generated
manifest, `transfer-syntax/capability-matrix.json`, and the dated evidence
records as defined by `AGENTS.md`. After separation, the generator executable
and its schemas govern generator behavior; the versioned corpus bundle governs
dcmview case identity, profiles, and selection; and a generated manifest
governs what a particular run emitted or skipped. This ADR fixes intended
ownership and compatibility treatment but does not claim unimplemented
behavior.
