# `synth-dicom-gen` / `dcmview-test-corpus` migration status

**Recorded:** 2026-09-01

**Updated:** 2026-09-01

**Contract:** `docs/synth-dicom-gen-dcmview-corpus-separation-plan.md`

**Starting repository:** `dicom-test-suite`

**Exact starting revision:**
`fbd0f76a36dc5726bb41602f44bff290588f560d`

**Starting worktree state:** clean (`git status --short` produced no output)

## Status contract

This is the current dated execution record for phases R0-R9. The separation
plan's ordering, acceptance gates, verification classes, evidence boundaries,
and completion definition are authoritative. This record distinguishes
implemented behavior and executed evidence from proposed work. A row is
complete only when its acceptance condition has passed at the recorded commit;
an unrun command, unavailable target, missing runtime, or planned repository is
never an implied pass.

The immutable `dicom-test-suite 0.1.0` release-candidate evidence at
`69d3e5f8e045752b6e183781a7e13190a61430ff` remains historical evidence for
that exact candidate only. It is not inherited by a renamed product, moved
corpus, changed schema, changed identity domain, new repository, or new release
artifact.

## Current gate state

| Phase | State | Completed items | Current evidence or next gate |
| --- | --- | --- | --- |
| R0 — freeze migration contract | In progress | R0.1 | ADR 0003 fixes names, the `0.2.0` clean-rename decision, compatibility treatment, ownership, dependency direction, evidence boundaries, and verification invalidation. R0.2-R0.4 remain unmeasured or unimplemented. |
| R1 — contain CI and local build cost | Not started | None | Requires the accepted R0 contract and dated baseline. |
| R2 — reduce Rust test-linking amplification | Not started | None | Requires R1 routing and the R0 test-target baseline. |
| R3 — rename reusable product | Not started | None | Requires the R0 gate; no current file, package, crate, binary, archive, or environment spelling has been migrated. |
| R4 — split immutable resources and corpus definitions | Not started | None | Requires the accepted naming decision and sequential resource/schema migration. |
| R5 — add supported external corpus API | Not started | None | Requires the R4 resource and identity boundary. |
| R6 — establish smoke corpus repository | Not started | None | No external repository has been created; authority and destination are still required before out-of-workspace mutation. |
| R7 — migrate complete dcmview corpus | Not started | None | Requires R6 smoke parity and supported contracts. |
| R8 — decouple viewer development | Not started | None | Requires a qualified artifact key and complete-enough corpus ownership. |
| R9 — remove embedded corpus and qualify products | Not started | None | Terminal removal, documentation, measurement, and exact qualification have not run. |

## Completed phase-item evidence

### R0.1 — superseding boundary ADR

**State:** complete

**Commit:** the commit introducing this record, with subject
`docs(migration): freeze repository separation contract` (resolve the exact
object with `git log --format='%H %s' -- docs/adr/0003-synth-dicom-gen-dcmview-corpus-separation.md`)

**Owned files:**

- `docs/adr/0003-synth-dicom-gen-dcmview-corpus-separation.md`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

**Decision evidence:** ADR 0003 supersedes only the repository-boundary
assumptions of ADR 0002 and the completed standalone plan. It fixes
`synth-dicom-gen` as repository/product/package/crate distribution and binary,
`synth_dicom_gen` as the Rust path, and `dcmview-test-corpus` as the downstream
repository. It classifies crate, library, executable, schema, resource,
manifest, and recipe/template changes under the current compatibility policy;
sets the first renamed product release to `0.2.0`; and records ownership,
dependency, identity, evidence, verification-class, and non-goal boundaries.

**Inputs reviewed:**

- `AGENTS.md`
- `README.md`
- `SYSTEM_SPEC.md`
- `docs/synth-dicom-gen-dcmview-corpus-separation-plan.md`
- `docs/standalone-productization-plan.md`
- `docs/adr/0002-standalone-product-boundary.md`
- `docs/compatibility-policy.md`
- `docs/standalone-product-status-2026-08-31.md`
- `docs/rust-api-compatibility-audit-2026-08-31.md`
- `docs/product-resource-lookup-audit-2026-08-31.md`

**Verification:** documentation-only Fast PR checks passed:

```text
cargo test --locked --no-default-features --test standalone_docs
result: 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
elapsed: 38.38 seconds

git diff --check --cached
result: passed with no output
```

The focused test preserves the installed-product guide, documentation-map,
neutral-example, and README external-consumer contracts while the new ADR
supersedes only repository ownership. No generator, corpus, package, release,
or target qualification is claimed by R0.1.

## Measurements

R0.2 has not run. No value in this section is a performance baseline yet.

| Measurement | Baseline command/revision | R0 value | Terminal value | State |
| --- | --- | ---: | ---: | --- |
| CI wall time by verification class | Not recorded | — | — | Not measured |
| Billable runner time by verification class | Not recorded | — | — | Not measured |
| Largest local target-directory size | Not recorded | — | — | Not measured |
| Integration-test target count | Not recorded | — | — | Not measured; the plan's dated 186-target statement is diagnostic context only |
| CI/generated artifact count and size | Not recorded | — | — | Not measured |
| Representative generator Fast PR | Not recorded | — | — | Not measured |
| Representative corpus PR | Repository does not yet exist | — | — | Not measured |
| Representative viewer PR | Viewer repository not in current scope | — | — | Not measured |
| Nightly and release-candidate cost | Not recorded | — | — | Not measured |

Before/after cost reduction cannot be claimed until R0.2 records exact commands,
revisions, elapsed wall time, billable runner time, linked targets, build-tree
size, and artifacts, and R9.6 repeats the comparable terminal measurements.

## Blockers and authority boundaries

- R0.2-R0.4 remain required before the R0 gate can pass.
- The location, remote, and creation authority for `dcmview-test-corpus` have
  not been supplied. No external repository, remote, release, or other out-of-
  workspace state has been created or mutated.
- External-consumer inventory has not run. The clean rename remains the
  default; a temporary alias may be introduced only if R3.2 discovers a real
  consumer and records its tested support window.
- No generator or corpus release target is currently qualified under the new
  names. Existing macOS arm64 and Linux x86_64 evidence remains scoped to the
  immutable historical candidate named above.

## Terminal acceptance matrix

| Gate | State | Required terminal evidence or current blocker |
| --- | --- | --- |
| Repository boundary | Not run | Must prove no generator dependency on dcmview and no corpus use of unsupported modules or sibling paths. |
| Naming and compatibility | Not run | Current product still uses `dicom-test-suite`; renamed package, library, binary, archives, discovery, guides, and migrations do not exist. |
| External corpus contract | Not run | Versioned external definition loader and CLI/SDK generation contract are not implemented. |
| Identity separation | Not run | Engine, toolchain, template/provider, schema, corpus, and external-runtime identities are not independently projected. |
| Smoke migration | Not run | R0 parity baseline and R6 repository smoke generation have not run. |
| Complete migration | Not run | The current repository still owns the complete embedded corpus. |
| Fast development | Not run | R0 measurements and representative post-change PR measurements are absent. |
| Heavy qualification | Not run | New nightly/manual/release routing and exact applicable runs are absent. |
| Artifact consumption | Not run | No keyed downstream corpus artifact or default viewer-consumption workflow exists. |
| Packaging and release | Not run | Neither renamed repository has an independently qualified release procedure or exact candidate record. |
| Documentation | Not run | ADR 0003 records the intended boundary; current operating documents still describe the monolithic product. |
| Hygiene | Not run | Terminal clean-worktree, formatting, schema, diff, artifact, secret, and package-inventory checks have not run in both repositories. |

The migration is not complete until every R0-R9 gate and every row above
passes, both repositories are clean and independently usable, measured cost
reductions are recorded, and exact qualification evidence exists for every
claimed target and scope.
