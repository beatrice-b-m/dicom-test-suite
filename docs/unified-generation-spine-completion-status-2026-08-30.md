# Unified Generation Spine Completion Status

**Recorded:** 2026-08-30

**Contract:** `docs/unified-generation-spine-plan.md`

**Status:** implementation complete; terminal clean-worktree verification in progress

## Promoted architecture

Both public frontends resolve immutable, versioned `CorpusPlan` values and call
the same bounded `CorpusExecutor::execute` transaction. The executor owns the
artifact DAG, providers and codecs, `Part10Materializer`, typed mutations and
qualifications, validation and evidence, private cleanup, manifest projection,
and atomic no-overwrite promotion. Curated and composition projectors preserve
their distinct public meanings.

The migration-era generator tree, native dataset read-back conversion,
post-write rematerialization pass, advanced-default scratch generation,
composition-to-curated dependency, family writer dispatch, and duplicate Part
10/file-meta helpers are absent. Full-file DICOM import remains only at named,
planned external construction or locked-tool boundaries. Their request,
runtime, output, resource, semantic, and strict validation evidence remains
explicit.

## Acceptance evidence

1. `write_generation_run` and composition execution both call
   `CorpusExecutor::execute` with frontend-specific typed projectors.
2. The strict recipe loader and executable audits join every implemented
   registry `recipe_id`/`recipe_version` exactly once and retain explicit
   unavailable outcomes for missing features or runtimes.
3. Production-source audits prove the retired dataset-to-plan symbol and files
   are absent; native recipes return plans without output paths or pre-plan
   file creation.
4. Shared recipes, content factories, identity/reference resolution, encoding,
   validation, and materialization serve both planners; composition has no
   curated-generator import.
5. The only production DICOM writers are the shared materializer and executor
   normalization of qualified external Part 10 imports.
6. Curated migration, schema, report, qualification, WSI closure, and
   profile-specific tests preserve case IDs, selections, unavailable rows,
   specialized checks, independent routes, and determinism semantics.
7. Full-catalog composition qualification covers defaults, bundles, external
   imports, root validation, reports, sequential/parallel determinism,
   resources, and provider evidence.
8. Negative artifacts execute typed ordered mutations from private planned
   sources; fuzz and EOT use `QualificationPlan`, and fuzz publishes no payload.
9. Modular schema-validated recipe documents hold static differences; every
   algorithmic provider is named in its binding and returns plans.
10. `unified_generation_spine_audit` and `unified_spine_boundaries` derive
    writer classification, bridge absence, dependency direction, recipe and
    template completeness, and validation/evidence attachment.
11. The final clean-worktree command matrix is recorded below after execution.
12. Operating guides, architecture, taxonomy, documentation map, and this
    dated record describe the promoted spine. Generated run roots remain
    ignored and uncommitted.

Generated curated file records contain `corpus_plan_sha256` and valid DICOM
records also contain `resolved_plan_sha256`. Composition manifests contain the
corpus hash in `run` and the resolved plan hash per entry. External imported
content is marked separately from native plan construction so validation does
not misrepresent an observation as a reconstructible native plan.

## Focused integration evidence

The following commands passed while closing U9 implementation:

```sh
cargo check --locked --all-targets --no-default-features
cargo test --locked --no-default-features \
  --test composition_advanced_plan_first \
  --test composition_sr_plan_first \
  --test composition_quantitative_plan_first \
  --test composition_curated_migration \
  --test unified_generation_spine_audit \
  --test unified_spine_boundaries -- --test-threads=1
cargo test --locked --no-default-features \
  --test curated_external_import_plan --test u4_classic_qualification \
  -- --test-threads=1
cargo test --locked --no-default-features \
  --test composition_p8_qualification --test composition_schema \
  --test schema_artifacts -- --test-threads=1
```

The prepared highdicom/pydicom runtime is Python 3.12.12. `cjxl`,
`ojph_compress`, and `dcmcjpeg` are locally available, so their corresponding
feature/backend gates are applicable to the terminal matrix.

## Terminal verification matrix

This table is intentionally pending until the documentation state containing
this record is committed and the commands can run from a clean worktree.

| Gate | Command | Result |
| --- | --- | --- |
| Format | `cargo fmt --check` | Pending |
| Default regression | `cargo test --locked --all-targets --no-default-features` | Pending |
| Feature/backend regression | Applicable codec and external backend gates | Pending |
| Fresh-root workflows | Profile generation, validation, report, and composition | Pending |
| Reproducibility | Two-run curated and composition comparisons | Pending |
| Repository hygiene | Architecture audits, schemas, `git diff --check`, generated-artifact audit | Pending |

Unavailable capability is not converted into a pass. Any runtime that fails
discovery or fingerprint qualification will be recorded with its exact
explicit-unavailable evidence in the finalized table.
