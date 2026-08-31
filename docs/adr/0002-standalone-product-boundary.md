# ADR 0002: Standalone product integration boundary

**Status:** accepted

**Date:** 2026-08-31

## Context

The project already has a shared plan-first executor beneath curated
generation and qualified composition. Those source-tree workflows are strong
generation evidence, but they are not yet a standalone product contract: their
defaults may depend on repository paths, the command output is primarily for
people, and the crate exposes internal modules without identifying a narrow
supported facade.

Side projects need a boundary that survives repository relocation and internal
refactoring. They also need arbitrary structural Part 10 construction when no
qualified composition template fits, without that expert route being mistaken
for IOD conformance or curated coverage.

## Decision

### Integration order

The versioned command-line interface is the primary supported integration. It
is language-neutral, subprocess-safe, schema-described, and exercised through
the installed release artifact. Only documented JSON request, result,
discovery, and error behavior is an automation compatibility surface. Human
output remains useful but is not a parsing contract.

The `dicom_test_suite::sdk` facade is the secondary supported integration for
Rust consumers. It exposes the same resource, request, result, manifest,
cancellation, provenance, and stable-error meanings as the CLI. Consumers do
not need planner, executor, recipe, materializer, or frontend-internal types.

Repository modules outside `sdk` remain source-compatible during the migration
but are not promoted as standalone product APIs. Any later visibility or
semver change follows the compatibility policy and published migration notes.

### Workflow and evidence classes

The product has exactly three file-producing workflow classes over the shared
plan-first executor:

1. **Curated generation** is selected by `generate`. It is authoritative only
   for registry cases and profile semantics, including explicit skipped and
   unavailable rows. Its manifest run kind is curated or the supported legacy
   curated shape.
2. **Qualified composition** is selected by `compose`. It is authoritative for
   catalog-listed templates, bundles, content slots, reference roles,
   validation routes, and qualification evidence. Its manifest uses
   `run.kind = "composition"` and contains no registry case or profile credit.
3. **Structural assembly** is selected by `assemble`. It serializes a bounded,
   caller-owned element tree and typed bulk declarations as deterministic
   DICOM Part 10. Its manifest uses `run.kind = "structural_assembly"`, records
   `iod_conformance = "not_assessed"`, and contains no template qualification
   or curated coverage credit.

All three workflows use the same artifact identities, caller-asset safety,
resource ceilings, deterministic planning, validation appropriate to their
declared evidence class, cleanup, hashing, and atomic no-overwrite publication
infrastructure. The manifest is the artifact-discovery authority; callers do
not infer output closure from directories.

The evidence classes are disjoint. A successful structural assembly cannot be
promoted to a qualified template or registry case without the corresponding
standards evidence, descriptor or recipe, specialized validation, and
independent-route accounting. Missing optional features, codecs, providers,
validators, or peers are unavailable outcomes and never implicit passes.

### Supported compatibility surface

The standalone compatibility commitment covers:

- documented CLI command syntax, JSON envelopes, exit classes, stable error
  codes, and stdout/stderr rules for a declared CLI API version;
- versioned request, result, manifest, report, catalog, and provider schemas
  within their published support windows;
- qualified template identities, versions, defaults, limitations, and
  determinism classifications;
- `dicom_test_suite::sdk`, including its public models and error taxonomy;
- immutable product-resource identities and integrity behavior;
- output publication, manifest-driven artifact discovery, and documented
  reproducibility semantics; and
- archive layout, checksums, licenses/notices, and target qualification claims
  for an exact release artifact.

Implementation details beneath those boundaries are internal. Additive public
changes and breaking changes are classified by the separate compatibility
policy before merge.

### Runtime and distribution boundary

Normal operation uses immutable first-party resources embedded in or installed
beside the artifact. It performs no runtime download and has no implicit
source-tree fallback. Caller assets are explicit inputs beneath a declared
asset root. Optional executables remain explicit, fingerprinted runtime
capabilities.

The first distribution contract is a target-specific checksummed native
archive. `cargo install`, package-manager recipes, containers, or hosted
services are additional channels only after they pass the same applicable
black-box contract; they are not substitutes for archive qualification.

### Non-goals

This decision does not establish:

- a network service, daemon, hosted API, or multi-tenant execution system;
- runtime downloading of resources, inputs, templates, codecs, or validators;
- medical/anatomical synthesis, patient-data intake, or a PHI-handling claim;
- automatic IOD inference or conformance for arbitrary structural assembly;
- arbitrary malformed-byte or post-serialization mutation through `assemble`;
- universal image/scientific-container decoding;
- portable OS-level sandboxing of untrusted provider executables;
- bundling every optional codec or validator into the base artifact;
- official DICOM certification from same-project validation; or
- a release claim for any operating-system/architecture target before the
  exact target artifact passes the terminal qualification matrix.

## Consequences

- External consumers can remain independent of repository layout and internal
  Rust modules.
- Every public result and failure needs a stable schema and versioned meaning.
- Resource embedding, relocation, packaging, and external-consumer tests are
  product requirements rather than deployment conveniences.
- Structural assembly expands caller control while preserving the stronger
  meaning of qualified composition and curated generation.
- CLI and SDK implementations must converge on shared public models and error
  classification instead of evolving separate semantics.

## Classification test

A proposed change is reviewed in this order:

1. Does it alter one of the supported CLI, schema, SDK, resource, template,
   manifest, archive, or determinism meanings? If yes, classify it under the
   compatibility policy.
2. Does it change registry-selected identity, profile, recipe, provider,
   availability, or evidence? If yes, it is a curated-generation change.
3. Does it change a qualified catalog descriptor, content slot, bundle,
   validation route, or qualification record? If yes, it is a qualified-
   composition change.
4. Does it change generic element, private creator, identity/reference, or
   typed-bulk structural behavior without an IOD claim? If yes, it is a
   structural-assembly change.
5. Otherwise it is internal, provided contract tests prove no public behavior
   or evidence meaning changed.

Changes may occupy more than one class and must satisfy every applicable gate.
Documentation wording cannot reclassify a failing supported capability as
internal or unavailable after implementation.

## Source-of-truth order

Executable behavior and schemas remain authoritative, followed by the registry,
a particular generated manifest, the transfer-syntax capability matrix, and
dated evidence records as specified by `AGENTS.md`. This ADR freezes intended
product boundaries; it does not override unimplemented current behavior.
