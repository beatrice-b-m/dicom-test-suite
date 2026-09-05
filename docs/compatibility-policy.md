# Standalone product compatibility policy

**Effective:** 2026-08-31

**Updated:** 2026-09-02

**Applies to:** the supported black-box CLI, `synth_dicom_gen::sdk`, versioned
schemas and catalogs, qualified template behavior, product resources, and
release archives under the ADR 0003 rename decision

## 1. Version domains

The product uses independent versions because changing one contract does not
necessarily change every other contract.

| Domain | Current pre-release line | Identity and compatibility boundary |
| --- | --- | --- |
| Product and crate | `0.2.x` | Cargo package and release archive; follows Semantic Versioning, including the pre-1.0 rules below. The unpublished `0.1.0` candidate remains historical evidence only. |
| CLI API | `1.0.0` | JSON envelopes, command result objects, error codes, exit classes, and stdout/stderr rules. Human output is excluded. |
| Composition request | `0.1.0` | `composition_spec_schema_version` and its accepted document semantics. |
| Structural-assembly request | `1.0.0` when introduced | `assembly_request_schema_version` and its accepted document semantics. |
| Curated manifest | `1.0.0` (reader retains `0.2.0` and `0.3.0`) | `manifest_schema_version` for registry-led runs. Version `1.0.0` adds split identity projection; older readers remain legacy-only and never synthesize split identities. |
| External corpus SDK | Evidence accessors `1.0.0`; bundle `1.0.0`, manifest/report `2.0.0` | `GenerateCorpusRequest` requires explicit member root and selector. Published, Planned, and NoExecutableCases are distinct; previews have no manifest or standalone JSON schema. Lossless preview ledger/identity fields use manifest2 meanings with preview-only ready. SDK and CLI generation/validate/report support external2. Caller-defined classic CT dispatch is bound to the complete documented capability tuple, not case/recipe names, order, or output path. |
| External corpus CLI results | Generation `3.0.0`; report `2.0.0` | External-only CLI API1 results preserve explicit nonpublication, full selection/identity evidence and strict report2 payloads. Embedded generation2 and legacy report1 schemas/producers remain unchanged. Capabilities3 advertises loaded-corpus assessment and complete version windows; frozen capabilities1/2 are not reinterpreted. |
| Capability discovery | `3.0.0` producer; 1/2/3 schema compatibility | Optional loaded-corpus inspection retains one verified identity and destination-free selected planning facts; registry status and installed declarations never imply execution success. |
| Release manifest | `3.0.0` producer; 1/2/3 verifier | Release2 embeds capabilities2; release3 embeds capabilities3, both with version2 and exact domain equality. Frozen predecessor schemas remain unchanged. Contract tests do not qualify a release archive. |
| Composition manifest | `1.0.0` (reader retains `0.4.0` and `0.5.0`) | `manifest_schema_version` plus `run.kind = "composition"`. Version `1.0.0` adds split identity projection; legacy readers remain validation/report compatible without synthesizing split identities. |
| Composition result | `2.0.0` (schema validation retains `1.0.0`) | `composition_result_schema_version`; version `2.0.0` binds published and dry-run outcomes to the composition manifest `1.0.0` contract while preserving their typed shape. |
| Structural-assembly manifest | `2.0.0` (reader retains `1.0.0`) | `manifest_schema_version` plus `run.kind = "structural_assembly"`. Version `2.0.0` adds split identity projection while retaining the exact no-IOD-claim semantics; the legacy reader never synthesizes split identities. |
| Structural-assembly result | `2.0.0` (schema validation retains `1.0.0`) | `assembly_result_schema_version`; version `2.0.0` binds published and dry-run outcomes to the structural-assembly manifest `2.0.0` contract without changing their typed execution shape. |
| Coverage report | `1.1.0` for curated manifest1; `2.0.0` for external manifest2 | Readers retain `0.1.0` and `1.0.0`. Legacy curated manifests still produce `0.1.0`; report-result1 remains the envelope for curated reports. |
| Template catalog | `0.1.0` | `template_catalog_schema_version`; each descriptor also has an independent template ID/version. |
| Case registry | `0.2.0` | Registry document shape; case recipe identity and determinism change through `recipe_version`. |
| Composition provider | `1.0.0` | Request/response protocol used by caller-selected content providers. |
| Generation backend | `0.1.0` | Locked external generation-backend request/response protocol. |
| Product resources | `1.0.0` | Immutable embedded or explicit resource-set inventory and hashes. |

The version/capability discovery response is the machine authority for the
versions supported by a particular executable. This table records the policy
baseline and must not be used to infer runtime availability.

The unreleased product `0.2.0` source candidate adds coverage schema `1.1.0`
for supported selected-core reporting. It preserves every previously accepted row
and permits explicitly non-generated nonsquare rows to retain null artifact
observations. Generated rows retain the original strict requirements. Frozen
coverage schemas `0.1.0` and `1.0.0` are unchanged. This is an independently
versioned additive schema capability, not a product release or replacement of
the historical executable. Source revision, binary hash, and current schema
and engine identities distinguish any later reporting candidate.

Frozen manifest/discovery identity schemas still bind legacy provenance to
the historical resource digest. A future product-version bump changes
Cargo.lock and backend-lock provenance and therefore requires an explicit
versioned identity migration before the R9 release gate. This remains a
blocker, not permission to hide changed bytes behind historical hashes or
edit frozen schemas. No qualified release is claimed here.

## 2. Compatibility rules

Every public schema version uses `MAJOR.MINOR.PATCH`.

- A **patch** may clarify documentation, tighten an implementation to the
  already-declared schema, fix rejection of input that the same version
  promises to accept, or add qualification evidence without changing output
  identity. It must not add a required field or alter an accepted value's
  meaning.
- A **minor** may add optional request fields with unchanged defaults, optional
  result/manifest fields, new enum/error/capability values, a new qualified
  template, or a new command that does not alter existing commands. Consumers
  must ignore unknown object properties and unknown append-only values where
  the schema and SDK type explicitly permit extension.
- A **major** is required to remove or rename a supported field or command,
  reject previously accepted conforming input, change a default that affects
  identity or bytes, reuse an existing error/value with a different meaning,
  change stdout/stderr or exit-class behavior, or change a qualified
  template's accepted content or deterministic output incompatibly.

Product/crate releases follow Semantic Versioning. Before product `1.0.0`, a
breaking change to the supported CLI/SDK surface increments the product minor
version and resets its patch version; patch releases remain backward
compatible within that minor line. Product `1.0.0` is prohibited until the
standalone plan's terminal acceptance matrix passes for an exact release
candidate. After `1.0.0`, ordinary semantic-version major/minor/patch meanings
apply.

An independent schema or protocol major change does not mechanically require a
product major change when the product continues to support the prior major for
its published window. Dropping that prior supported major is a product-breaking
change.

## 3. Testable change classification

The following examples are normative acceptance cases.

| Proposed change | Classification | Required action |
| --- | --- | --- |
| Add an optional result field that old consumers may ignore. | Additive | Increment that schema's minor version; validate old and new fixtures. |
| Add a namespaced error code without changing existing mappings. | Additive | Increment CLI API minor; append to the registry and golden tests. |
| Add a qualified template with a new stable ID. | Additive | Increment catalog minor and template inventory; qualify defaults and declared routes. |
| Correct an implementation that emitted a field contrary to its schema. | Compatible fix | Increment product patch; retain schema version if accepted meaning is unchanged. |
| Rename a JSON field or command flag. | Breaking | Introduce a new schema/CLI major or keep an adapter for the old spelling. |
| Change an error code's meaning or move it to another exit class. | Breaking | Introduce a new CLI API major; never recycle the old code. |
| Change a template default that changes byte-stable output. | Breaking template behavior | Increment template version and record migration; retain the prior version for its support window or cross the applicable catalog/product boundary. |
| Change a curated recipe's deterministic bytes. | Versioned recipe change | Increment `recipe_version`, preserve case identity, and refresh proportional qualification evidence. |
| Add a transfer syntax to a template while retaining existing defaults. | Additive qualified capability | Increment template/catalog minor and qualify the new syntax; absence remains explicit in older artifacts. |
| Make a previously optional request field required. | Breaking | New request-schema major with a documented migration. |
| Tighten rejection of traversal that was always prohibited. | Compatible security fix | Product patch; add adversarial regression without a schema-major change. |
| Stop accepting a documented path form that was previously valid. | Breaking | New request/CLI major unless an adapter preserves the old form. |
| Refactor planners or materializers with identical public outcomes. | Internal | No public version change; contract and determinism tests must prove equivalence. |

A review must name the row or rule used. If none applies, the change is treated
as public until a dedicated compatibility decision establishes otherwise.

For external classic CT bundles, compatibility is the conjunctive tuple
documented in the generation and SDK guides: `rust_native` registry provider,
no feature/codec requirements, `native.classic_plan`, and matching
`classic/ct@1.0.0`/`content.native_pixels`/`algorithm.classic_ct`/CT projection
artifacts with typed parameters and explicit outputs. Partial tuples are
invalid. `planning_order` remains required and unique but is not dispatch.
This rule does not extend to `native.stress_ct_plan` or other classic/VL
families and supplies no independent-conformance, viewer, package, or release
evidence.

## 4. Version negotiation and rejection

Requests carry their own exact schema version. The product:

1. accepts a supported version and reports that same request version in the
   outcome and manifest;
2. may upgrade an older supported minor/patch internally only when the canonical
   interpretation and recorded provenance are unchanged;
3. rejects an unsupported major before planning with a stable
   `request.version.unsupported` error, the requested version, the supported
   versions, and a migration-document identifier; and
4. never silently treats an unknown major as the newest version.

Results and errors always carry the CLI API version used to encode them.
Consumers select only an advertised supported version. No content negotiation
is inferred from `--help`, a human version banner, or package version alone.

Manifests and reports are read according to their embedded schema versions.
When a supported older document is upgraded for internal processing, the
original bytes and version remain the input identity; validation reports the
reader/upgrade path. Unsupported versions receive a stable version error and a
migration action rather than a generic parse failure.

Every curated `validate` and `report` entry point uses the same fail-closed
contract loader. It accepts exactly manifest `0.2.0`, `0.3.0`, and `1.0.0`;
version `1.0.0` must contain a schema-valid split identity projection. The same
loader dispatches composition manifests `0.4.0`, `0.5.0`, and `1.0.0` and
structural-assembly manifests `1.0.0` and `2.0.0` before their semantic
validation or reporting begins. Current split-identity versions reject missing,
malformed, or duplicate runtime identities; frozen predecessors remain
legacy-only inputs and never receive inferred identity domains.

## 5. Support windows and deprecation

Until a superseding release policy is published, the standalone product
supports:

- the current CLI API major and the immediately preceding major for at least
  one product minor release after a replacement becomes available;
- every request, manifest, and report version advertised by `capabilities` for
  that exact product release;
- the current provider-protocol major and any older major required by a live
  qualified template or packaged example; and
- the current `sdk` surface for the product semantic-version line that
  introduced it.

Deprecation is additive. It requires a discovery-visible replacement and
migration document before removal. A deprecated contract remains fully tested
and may not change meaning during its support window. Removal occurs only at
the version boundary required by Section 2.

Pre-standalone source-tree human output and public internal Rust modules do not
gain a new support window merely because they remain callable during migration.
Existing report JSON remains unchanged until a separately announced CLI API
wrapper version; CLI API `1.0.0` provides that wrapper only when report callers
pass `--cli-api 1.0.0`. Omitting the selector preserves the raw report object,
and migration tests compare the wrapped `result.report` to the raw object.

## 6. Upgrade evidence

Every compatibility-affecting change supplies, as applicable:

- positive fixtures for the new version;
- adversarial fixtures for invalid or unsafe inputs;
- old-version fixtures that still succeed through the documented path;
- exact stable error and migration context for unsupported versions;
- normalized result and manifest comparisons;
- template/recipe determinism evidence when identities or bytes can change;
- packaged external-consumer tests, not only in-crate unit tests; and
- changelog and migration entries naming both old and new versions.

A release is not compatible merely because deserialization succeeds. Field
meaning, default identity, evidence class, unavailable accounting, artifact
closure, exit behavior, and determinism must also satisfy their assertions.

## 7. Error and enum evolution

Stable error codes are namespaced and append-only within a CLI API major. They
are never deleted, reused, or assigned a new exit class in that major. SDK
errors and extensible public enums are `#[non_exhaustive]`; consumers must keep
an unknown/future branch. JSON schemas use explicit extension rules rather than
accidentally accepting arbitrary misspelled request fields.

Availability enums distinguish supported, feature-gated, runtime-unavailable,
provider-unavailable, validator-unavailable, peer-unavailable, planned, and
unsupported states as applicable. Adding a state is additive only where the
consumer contract already requires unknown-value handling.

## 8. Determinism and evidence changes

Any change to byte-stable output requires a recipe or template version even if
the product version also changes. Semantic-stable codec output retains its
declared decoded or bounded numeric comparison and must not be upgraded to a
byte-stable claim. Changes to validation, conformance, or qualification wording
cannot merge curated, qualified-composition, structural-assembly, independent,
negative, fuzz, stress, media, or protocol evidence classes.

Unavailable capability may become available only with the required
qualification evidence and a discovery-visible identity. A missing runtime is
not a pass, and a release claim cannot be recovered by narrowing documentation
after a gate fails.

## 9. Ownership and review

`product/compatibility-owners.json` assigns one accountable maintainer role and
an explicit support window to every public JSON schema and supported API
surface. Its coverage test fails when a schema is unowned, multiply owned, or
when the CLI, SDK, workflow, external-evidence, or native-release surface loses
an owner. Role owners review compatibility classification, fixtures, migration
notes, deprecation timing, and release evidence for their contract; approval
does not permit shortening the support windows in Section 5.

Human CLI formatting and legacy internal Rust modules remain outside the
standalone compatibility surface as defined above. Their exclusion is explicit
and must not be used to exclude a machine schema or supported `sdk` facade item
from the ownership registry.
