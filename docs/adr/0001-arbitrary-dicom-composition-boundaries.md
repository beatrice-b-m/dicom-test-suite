# ADR 0001: Arbitrary DICOM Composition Boundaries

**Status:** accepted

**Date:** 2026-08-28

## Context

The project currently exposes a curated, registry-selected corpus generator.
The arbitrary-composition program adds caller-authored DICOM instances without
changing what a registry case means or weakening the qualification evidence
attached to curated recipes. Both workflows need to share serialization,
identity, content, reference, and generic-validation infrastructure.

This decision freezes the public terminology and evidence boundaries before
the generator is restructured.

## Decision

### Workflows

`generate --profile ...` is the **curated generation workflow**. It selects
committed recipes from `cases/registry.json`, preserves profile semantics, and
emits `case_id`-scoped expectations and qualification evidence. Registry
`implemented` means that a curated recipe exists; build features, external
runtimes, and provider availability still determine whether a selected case is
generated or explicitly skipped in one run.

`compose --spec ...` is the **composition workflow**. It resolves a versioned,
caller-authored specification through a qualified template or deterministic
template bundle. Composition outputs use logical `instance_id` and
`template_id` identities. They are not registry cases and never change case
counts, profile membership, coverage-gap status, or curated qualification.

The workflows share a resolved-plan boundary and the Part 10, attribute,
content, transfer-syntax, identity, reference, staging, and generic-validation
primitives beneath it. Curated recipes retain stronger case-specific oracles
after migration onto those primitives.

### Catalogs and claims

`cases/registry.json` remains authoritative for curated case identity, status,
profiles, requirements, provider, standards evidence, and blockers.

`templates/catalog.json` is authoritative for composition-template identity,
version, SOP Class, attribute policy, content and reference slots, defaults,
requirements, validation routes, standards evidence, and determinism. Template
coverage is audited against distinct implemented valid DICOM SOP Classes in the
registry, but templates never become registry rows.

A template is **qualified** only when its descriptor passes catalog validation,
its deterministic default output or bundle passes generation-time checks and
strict internal validation, its required independent route is accounted for,
and its qualification evidence is recorded. Structural serialization without
a qualified template is not IOD conformance. Unknown SOP Classes have no
fallback composition mode.

### Qualification vocabulary

Composition evidence is reported as separate layers:

- **Part 10 validity:** file preamble, file meta, transfer syntax, and
  file/dataset identity are consistent.
- **Generic data-element validity:** tags, VRs, values, private creators,
  Sequences, and protected-field rules are satisfied.
- **Template validity:** required and conditional attribute policy, permitted
  content shape, reference roles, and template-specific semantics are
  satisfied.
- **Content validity:** native or encoded pixels and other bulk slots satisfy
  their declared byte, frame, sample, document, waveform, mesh, or backend
  contracts.
- **Independent validation:** a pinned external adapter evaluated the output.
  Missing tools or unsupported routes remain explicit capability records.

Built-in generation and validation are same-project evidence. They are never
described as independent conformance. A successful parser open is not a
substitute for IOD or content validation.

### Backward compatibility

Existing curated manifests remain valid and byte-for-byte unchanged until a
separately versioned manifest change is deliberately introduced. The manifest
schema initially treats an absent `run.kind` as the legacy curated shape; new
curated manifests may later emit `kind = "curated"` only with regression
evidence. Composition manifests emit `run.kind = "composition"` and a distinct
composition entry branch.

The meanings of `all`, `legacy`, `negative`, `fuzz`, and opt-in `stress` do not
change. Negative, fuzz, media, protocol, runtime-qualification, and
non-instance artifacts are outside template coverage. Missing templates,
codecs, providers, or independent validators are represented as unavailable
capabilities, never implied passes.

### Determinism and data policy

Built-in defaults are deterministic, synthetic, non-PHI, and set Synthetic
Data `(0008,001C)` according to project policy. Caller assets are opaque input:
the project records their bounded provenance but does not classify or generate
domain-specific content.

The canonical, hashable `ResolvedInstancePlan` is the only serializer input and
also supplies manifest expectations and generic validation. Host-specific
absolute paths are excluded from identity and resolved-plan hashes.

## Consequences

- Composition can grow without inflating curated coverage or creating a second
  serializer stack.
- Every supported SOP Class claim is traceable to a catalog descriptor and an
  explicit qualification route.
- Curated migrations can be reviewed family by family while preserving exact
  or semantic-stable recipe evidence.
- Schema, security, and manifest contracts must be completed before a public
  composition CLI is advertised.

## Source-of-truth order

When artifacts disagree, use executable command behavior and emitted schemas,
then `cases/registry.json`, a particular run manifest, the transfer-syntax
capability matrix, and finally dated status or plan documents, as required by
`AGENTS.md`.
