# Unified Generation Spine Baseline Audit

**Recorded:** 2026-08-29

**Contract:** `docs/unified-generation-spine-plan.md`

This is the U0 migration inventory. Counts shown here are a dated audit
snapshot, not operating documentation invariants. The executable audit derives
current values from the registry, template catalog, generated manifests, and
source classification rules.

## Reproducible inventory commands

```sh
cargo run --locked -- list-cases
cargo run --locked -- list-cases --profile all
cargo run --locked -- report gaps --format json
cargo test --locked --test unified_generation_spine_audit
```

At this checkpoint the registry contains 178 implemented recipe identities:
161 valid DICOM recipes, 15 expected-invalid DICOM recipes, one bounded fuzz
qualification, and one EOT arithmetic qualification. The 161 valid recipes
comprise 154 native and seven explicit external-backend boundaries. These
figures are retained only to identify the reviewed baseline; tests calculate
their expectations from source data.

## Current paths

| Classification | Current path | Required removal or convergence |
| --- | --- | --- |
| Composition SC/classic | composition resolver -> `ResolvedInstancePlan` -> `Part10Materializer` -> composition transaction | Wrap in `CorpusPlan`; move transaction to `CorpusExecutor` (U1.5, U2.5). |
| Composition advanced defaults | composition -> `write_composition_default_artifacts` -> curated writer -> reopen -> `resolved_plan_from_curated_dataset` | Replace with neutral providers and remove reverse dependency (U5.6). |
| Curated native valid | generator family builder/writer -> file -> reopen/import -> delete -> `Part10Materializer` | Recipes must return plans before file creation; delete migration pass (U3-U7, U9.1). |
| Curated external construction | locked backend emits full Part 10 file -> checked import/publication | Preserve only as named external-provider import boundaries with exact tool/request/response evidence (U6.7, U7.2). |
| Negative | private valid source writer -> typed byte mutation -> isolated invalid publication | Obtain source from versioned plan-first recipe and execute through shared mutation stage (U8.1-U8.3). |
| Fuzz | private valid source writer -> bounded mutation session -> source/candidate cleanup -> qualification only | Obtain source from plan-first recipe and execute a payload-free `QualificationPlan` (U8.4-U8.5). |
| Stress | specialized generator writers and resource guard | Use ordinary valid plan providers with shared preflight and actual resource evidence (U7.4-U7.5). |
| Non-instance EOT qualification | direct qualification record | Represent as `QualificationPlan` and execute through the shared executor (U8.4). |

## Direct writer and ordering inventory

The allowlist is intentionally exhaustive at U0:

- `src/composition/materializer.rs` is the target ordinary valid Part 10
  writer.
- `src/generator.rs` contains the native curated direct writers, file-meta
  builders, manual family/source ordering, the
  `migrate_shared_plan_curated_files` post-write migration pass, and the
  composition-default compatibility entry point. Native uses are assigned to
  U3 through U7 and removed by U9.1-U9.3.
- `src/composition/curated.rs` contains the dataset-to-plan bridge. Its unit
  fixtures may exercise external import semantics, but production native uses
  are removed by U9.1 and the module is reduced to an explicitly named
  external import boundary or deleted.
- `src/composition/advanced_family.rs` imports the curated generator and is
  assigned to U5.6.
- `src/generation_backends/` owns locked external full-file construction. Each
  use is classified as an external plan-provider/import boundary by U6.7;
  frame-capable codec transforms move to typed encoded content by U7.2.
- `src/negative.rs` and `src/mutation.rs` are the only expected-invalid byte
  mutation boundary and move beneath shared execution in U8.
- test-only `write_to_file` calls may create deliberately mutated validation
  fixtures; production-source audits distinguish tests from writers.

No other production direct writer or publication path is accepted. Adding one
requires updating this inventory and assigning its removal before the relevant
phase gate can close.

## Baseline projection

Before family migration, private fresh-root runs capture and compare:

- selected and skipped registry rows for every profile;
- ordered file paths and logical memberships;
- UIDs, references, validation check names, expectations, and report axes;
- exact bytes for byte-stable cases; and
- decoded hashes and bounded metrics for semantic-stable cases.

Generated roots remain ignored and are never committed. The final dated status
record contains the exact commands, feature/runtime availability, and outcomes.

## Specialized evidence inventory

The executable audit joins each implemented recipe with registry standards
evidence and its generated validation projection. Template validation and
independent routes are derived from `templates/catalog.json` and
`templates/qualification-evidence.json`. Dated phase records remain the
qualification history for geometry/metadata, enhanced/WSI, derived/SR/RT,
waveform/document/mesh, codecs, stress, negative, fuzz, and interoperability.

Generic plan validation is additive. A migrated recipe is incomplete if its
previous specialized validation name, manifest expectation, report axis, or
independent route disappears.
