# ADR 0002: Adopt one plan-first generation spine

**Status:** accepted

**Date:** 2026-08-29

## Context

The composition platform already supplies typed resolved-instance plans, a
shared Part 10 materializer, bounded providers, deterministic identities, and
transactional publication. Curated generation nevertheless still constructs
and writes objects in `generator`, reopens selected files, imports their
datasets into composition plans, removes the originals, and rematerializes
them. Advanced composition defaults have the inverse dependency: they invoke
curated generation and import its output.

That arrangement shares a final writer but not construction, scheduling,
resource accounting, evidence, or publication. It cannot represent
multi-instance dependencies, expected-invalid derivatives, or payload-free
qualifications as one auditable run plan.

## Decision

Adopt the architecture and migration contract in
`docs/unified-generation-spine-plan.md`.

Both public frontends resolve a versioned, run-neutral `CorpusPlan` and submit
it to one bounded `CorpusExecutor`. The plan contains an ordered artifact DAG
whose valid DICOM nodes contain `ResolvedInstancePlan`; mutation,
qualification, and auxiliary nodes are separately typed. The executor owns
materialization, providers/codecs, validation, evidence collection, resource
accounting, staging, cleanup, and atomic no-replace publication.

The frontends remain intentionally distinct above that boundary:

- curated generation owns registry selection, versioned case-recipe lookup,
  capability decisions, and curated manifest/report projection;
- composition owns caller-spec and qualified-template validation plus
  composition manifest/report projection.

Registry identity and coverage do not flow into composition. Template
qualification does not imply registry coverage. Same-project generic checks
do not replace specialized or independent evidence.

Static scenario differences are committed as schema-validated modular recipe
documents. Typed Rust plan providers are retained only for bounded algorithms,
large streams, graph construction, codec work, and external construction
boundaries. A provider returns plans and evidence obligations and cannot
publish files.

The terminal dependency direction is:

```text
core plan/types
  <- templates, recipes, content, codecs, validators
  <- curated planner and composition planner
  <- shared executor
  <- CLI/API frontends and evidence projectors
```

## Invariants

The fifteen non-negotiable invariants in the migration plan are part of this
decision. In particular, output and evidence compatibility, profile
isolation, synthetic-data marking, explicit unavailability, provider safety,
determinism, and transactional publication are release gates rather than
refactoring preferences.

## Migration discipline

Temporary adapters are permitted only when the executable architecture audit
classifies them and assigns a removal task. A phase is not complete while an
adapter assigned to that phase remains. U9 requires deletion of all native
read-back bridges, composition-to-generator dependencies, duplicated native
writers, manual generation ordering, and frontend-specific publication loops.

## Consequences

The migration must proceed in dependency order and keep the repository
releasable after each granular commit. The plan and executor contracts land
before family migrations. Family work retains its specialized validation and
evidence as it moves. Final completion is determined only by the Program
Acceptance Criteria, not by intermediate reuse of the shared writer.
