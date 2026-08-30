# Unified Generation Spine Completion Status

**Recorded:** 2026-08-30

**Contract:** `docs/unified-generation-spine-plan.md`

**Status:** complete; all Program Acceptance Criteria satisfied

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
11. The final locked default regression, applicable feature/backend matrix,
    documented fresh-root workflows, reproducibility comparisons, schema
    checks, and repository-hygiene gates all pass.
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

| Gate | Command | Result |
| --- | --- | --- |
| Format | `cargo fmt --check` | Pass |
| Default regression | `cargo test --locked --all-targets --no-default-features` | Pass; every target completed, including maximal SC/stress/WSI parity and execution gates |
| Feature/backend regression | `cargo test --locked --all-features` for the exceptional execution/matrix/import/full-file, frame-codec, reproducibility, backend artifact/contract/discovery, legacy JPEG wrapper, and runtime-capability test targets | Pass; exercised native RLE, JPEG baseline, JPEG-LS, JPEG XL, JPEG 2000, deflate, Deflated Image Frame, OpenJPH HTJ2K, DCMTK legacy JPEG, and prepared highdicom/pydicom import paths |
| Fresh-root workflows | Documented `generate`, `validate`, and `report` commands for `smoke`, `core`, `extended`, `all`, `legacy`, `negative`, `fuzz`, and `stress`; `all --include-stress`; and `compose tests/fixtures/composition/valid/template-only.json` | Pass; strict validation reported zero failures, and both stress-inclusive manifests contained all seven approved reduced-scale qualifications |
| Reproducibility | Exact recursive comparisons for two fresh `smoke` and composition roots; `composition_parallel`, `composition_p8_qualification`, and `generate_reproducibility` | Pass; byte trees and canonical sequential/parallel outputs matched |
| Architecture and schemas | `unified_generation_spine_audit`, `unified_spine_boundaries`, and `schema_artifacts` | Pass; 5 architecture, 3 boundary, and 73 schema assertions |
| Repository hygiene | `git diff --check`, `git ls-files '*.dcm'`, generated-artifact audit, and clean-worktree check | Pass; no tracked generated DICOM payloads or pending changes |

Terminal verification exposed and closed acceptance defects rather than
weakening the gates: manifest publication resources now include the bounded
manifest allowance; contextual profile, negative, stress, and U3 compatibility
oracles validate canonical plan provenance separately from historical fields;
all seven approved stress qualifications are projected; `all --include-stress`
validation accepts only its explicit opt-in stress evidence; quantitative
recipes resolve every encoding policy; and the HTJ2K audit follows the promoted
executor codec boundary.

Unavailable capability remains explicit plan and manifest evidence and is
never converted into a pass. Feature/backend success above applies only to the
locally discovered and fingerprint-qualified runtimes exercised by the stated
matrix.
