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
| R0 — freeze migration contract | Complete | R0.1, R0.2, R0.3, R0.4 | ADR 0003, the dated cost baseline, the exhaustive 801-path ownership inventory, and the seed-1 smoke parity manifest fix repository ownership, invalidated verification class, and the byte/normalized-semantic migration boundary. The R0 gate passes. |
| R1 — contain CI and local build cost | In progress | R1.1, R1.2 | Ordinary PR, `main` push, and manual Fast runs now select only formatting/JSON, warning-denied public compilation, named light contracts, and tiny smoke generation/validation. The intact broad graph is scheduled or manually selected, and package/archive/upload is restricted to an immutable release-candidate commit. R1.3-R1.6 and live remote acceptance remain. |
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

### R0.2 — dated development-cost baseline

**State:** complete

**Baseline revision:**
`65a296bbb489fcaaff22e38fa35036f0805ccab6`, code-equivalent to starting
revision `fbd0f76a36dc5726bb41602f44bff290588f560d` outside R0.1 documentation.

**Evidence:**
`docs/synth-dicom-gen-dcmview-cost-baseline-2026-09-01.md`

The baseline maps every current workflow job and recorded heavyweight target
to an owner, verification class, and R1/R2 acceptance item. A clean explicit
`CARGO_TARGET_DIR` all-target no-run build completed in 72.29 seconds, occupied
8,013,463,552 bytes, and linked 188 Cargo-reported harness artifacts, including
186 integration-test targets. The exact temporary target and log paths were
removed after measurement.

Public GitHub API data for authoritative run `33491521696` records 7,935
seconds (132m15s) of wall time through the isolated successful retry, 10,545
seconds (175m45s) of actual runner time including the failed JPEG 2000 attempt,
and 180 per-job rounded billable minutes. The run uploaded one 9,929,745-byte
artifact, ID `9798112659`, whose archive and ZIP identities remain bound to
historical candidate `69d3e5f8`. The run predates `f1d1727`'s removal of an
unrelated curated test from codec jobs, so its exact codec timings are
conservative historical evidence rather than a claim of a remote run at
`65a296b`.

No independent Fast PR, Corpus PR, Nightly, or release-candidate workflow is
present at R0.2. Subsystem and corpus-edit costs are not independently
measurable because every push, pull request, and manual dispatch selects the
same graph with no concurrency cancellation or path routing. No heavyweight
qualification or durable generation was performed for this baseline.

**Verification:** the documentation-only Fast PR check
`cargo test --locked --no-default-features --test standalone_docs` passed 4/4
tests in 0.20 seconds, and `git diff --check` passed with no output. This does
not qualify any generator, corpus, codec, provider, package, release, or target.

### R0.3 — exhaustive file/module ownership inventory

**State:** complete

**Baseline revision:**
`f640748b412151b4410dfb104685519cef2bde75`

**Evidence:**

- `docs/synth-dicom-gen-dcmview-file-ownership-2026-09-01.md`
- `product/migration-file-ownership-2026-09-01.json`

The machine-readable inventory explicitly enumerates all 799 paths tracked at
the exact baseline plus the two new R0.3 ownership artifacts. This status file
is the third task-owned file but already existed at the baseline, so it appears
exactly once. All 801 unique entries have one disposition, primary destination,
ownership domain, rationale, invalidated verification class, and migration
phase/slice. All 175 split entries additionally name concrete synth and corpus
outputs while preserving one primary disposition and destination.

Disposition totals are 283 `retain_synth`, 310 `move_corpus`, 175 `split`, zero
`retire`, and 33 `archive_history`. The zero retirement count is intentional:
embedded-corpus code is split or moved until supported replacements and parity
make deletion safe. Domain totals are 105 engine, 136 generic capability, 313
corpus definition, 20 viewer expectations, 33 historical evidence, 20
build/release, 26 documentation, 4 governance/legal, and 144 test
infrastructure paths.

The inventory distinguishes corpus registry/recipes and case-selection notes
from generic templates/providers/engine evidence; assigns viewer schemas and
expectations downstream; and preserves dated documents, hashes, old artifact
names, ADRs, and qualification claims as synth history. Ambiguous-looking
`CorpusPlan`, manifest/schema, recipe-family, conformance-adapter, lockfile, and
mixed-test decisions are resolved in the dated evidence document.

**Verification:** a deterministic Python comparison proved the inventory path
set equals both `git ls-files` at the R0.3 commit and the fixed baseline tree
plus the two new paths, with no duplicates, missing paths, or extras. The same
check validated required fields, enums, destination prefixes, split-output
shape, and non-empty values. `jq empty`, `git diff --check`, and the focused
documentation test passed. Exact command results are recorded in the R0.3
commit and task report. No generation, parity, package, release, external
repository, target, or R0-gate qualification is claimed.

### R0.4 — seed-1 smoke migration parity baseline

**State:** complete; the R0 gate passes

**Commit:** the commit introducing the parity manifest, with subject
`docs(migration): freeze smoke parity baseline` (resolve the exact object with
`git log --format='%H %s' -- docs/baselines/dcmview-smoke-parity-seed-1-2026-09-01.json`)

**Baseline revision:**
`71f083669d46d6029a1ec4176942a13d317c97bf`, code-equivalent to starting
revision `fbd0f76a36dc5726bb41602f44bff290588f560d` outside R0 documentation.
A mechanical `git diff --exit-code` over Cargo, build, source, case, template,
schema, standards-lock, and transfer-syntax inputs confirmed that equivalence.

**Evidence:**
`docs/baselines/dcmview-smoke-parity-seed-1-2026-09-01.json`

The versioned parity manifest records the exact `smoke`, seed `1`,
no-default-feature selection and all three stable cases. Each case binds its
recipe and template version, native provider, output path, byte SHA-256 and
size, byte-stable determinism, SOP Class and transfer syntax, modality, image
and pixel contract, deterministic UIDs, empty reference closure, viewer-neutral
capabilities and semantics, validation result, and standards-evidence count.
It also records product, Cargo/toolchain, monolithic resource, schema, registry,
template-catalog, corpus-plan, standards-lock, raw-manifest, canonical-manifest,
normalized-semantic, and raw-report identities.

The exact generated payload identities are:

```text
classic/sc/mono1_u8_explicit_le/instance.dcm
  926 bytes  76dc5208b139899fcb87bbf7ec9edf1a323000a91c4015de9ef8bde7bd344ecc
classic/sc/mono2_u8_explicit_le/instance.dcm
  926 bytes  fce766bcbb4b4aa79cfb3fa0c3b5e4ef888b11c0708fad713b9cde8d41ec6a15
classic/sc/rgb_planar0_explicit_le/instance.dcm
  938 bytes  33de9448509431fda27005cbf83c79977f1c3ebadb669ae1dedf1a225742f3c5
raw manifest.json
  83,680 bytes  6a6540ba8acc13afa5e76e35e46d246d77f46ffdc2e5dcce0497fb882ab684eb
normalized semantic projection
  18f154c38903677cadf4f955b0658ed2fd59162c44a970a9b15c5dc9905eabcd
```

R3 may deliberately change product/package/crate/binary identity and the
accepted version spelling. R4 may replace the monolithic product-resource,
corpus, schema, catalog, and plan-digest representation with independently
recomputable identity domains. Those exceptions do not permit payload or
semantic drift. The parity manifest's R6.4 comparison contract requires exact
file bytes and normalized run, selection, recipe/template/provider, DICOM,
pixel, UID, reference, expectation, validation, and standards semantics. Any
other difference requires a reviewed, versioned migration; unavailable
capability remains an explicit non-success and cannot be omitted or counted as
generated.

**Executed commands and measurements:** one task-specific root,
`/private/tmp/dts-r04.1rW7mL`, held the fresh explicit
`CARGO_TARGET_DIR`, two fresh output roots, the JSON report, and transient logs.
The first successful generation command included 28 seconds waiting for the
fresh target's initial compilation invocation to release its build lock; this
contention is retained in the exact elapsed observation rather than subtracted.

```text
CARGO_TARGET_DIR=/private/tmp/dts-r04.1rW7mL/target \
  cargo run --locked --no-default-features -- generate \
  --profile smoke --out /private/tmp/dts-r04.1rW7mL/smoke-a --seed 1
result: 3 files written; manifest written
elapsed: 29.83 seconds

CARGO_TARGET_DIR=/private/tmp/dts-r04.1rW7mL/target \
  cargo run --locked --no-default-features -- validate \
  /private/tmp/dts-r04.1rW7mL/smoke-a
result: 3 files checked; 0 validation failures
elapsed: 1.21 seconds

CARGO_TARGET_DIR=/private/tmp/dts-r04.1rW7mL/target \
  cargo run --locked --no-default-features -- report \
  /private/tmp/dts-r04.1rW7mL/smoke-a --format json
result: generated 3; skipped 0; blocked 0; planned 0; deprecated 0;
        3 passed validation rows; no unavailable reasons
elapsed: 1.31 seconds

CARGO_TARGET_DIR=/private/tmp/dts-r04.1rW7mL/target \
  cargo run --locked --no-default-features -- generate \
  --profile smoke --out /private/tmp/dts-r04.1rW7mL/smoke-b --seed 1
result: 3 files written; manifest written
elapsed: 1.81 seconds

diff -r /private/tmp/dts-r04.1rW7mL/smoke-a \
  /private/tmp/dts-r04.1rW7mL/smoke-b
result: passed with no differences
elapsed: 0.00 seconds
```

At measurement, the target directory occupied 1,918,328,832 bytes. Each
generated root occupied 98,304 allocated bytes; its four files contained
86,470 logical bytes. The raw report contained 93,929 bytes. A deterministic
Python comparison parsed the parity baseline and mechanically checked every
recorded run, count, identity, case, recipe/template/provider, payload,
semantic, validation, standards, report, and two-root value against the
generated manifest/files, repository inputs, registry, recipes, and report. It
passed with `cases=3 files=3 validation_failures=0 skipped=0 unavailable=0`.
`jq empty`, the focused documentation check, and `git diff --check` also
passed. The focused
`cargo test --locked --no-default-features --test standalone_docs` check passed
4/4 tests in 0.42 seconds. The exact temporary root was removed after
verification; no generated DICOM, ordinary manifest/report, build product, or
log was retained.

This is same-project generation and strict-validation evidence for the small
native smoke slice. It is not R6 migration parity, independent conformance,
viewer interoperability, codec/provider, package, release, or target-matrix
qualification.

### R1.1 — workflow concurrency and non-duplicated event ownership

**State:** complete; R1 remains in progress

**Commit:** the commit introducing this workflow contract, with subject
`fix(ci): cancel superseded workflow runs` (resolve the exact object with
`git log --format='%H %s' -- .github/workflows/ci.yml`)

**Owned files:**

- `.github/workflows/ci.yml`
- `tests/ci_release_gates.rs`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

**Acceptance evidence:** push ownership is restricted to `main`, while
`pull_request` and `workflow_dispatch` remain explicit workflow owners. A PR
run uses `ci-pr-<number>` so a newer synchronization of that PR supersedes its
older run. A push uses `ci-push-<ref>` and a manual dispatch uses
`ci-workflow_dispatch-<ref>`, so rapid repeats of the same event and ref
supersede older runs without a manual qualification canceling, or being
canceled by, an unrelated push. Because non-`main` branches no longer own a
push-triggered copy of this full graph, ordinary PR branches receive only the
pull-request workflow. Every concurrency group has `cancel-in-progress: true`.
The job graph and all upload/release assertions remain unchanged for R1.2 to
separate by verification class.

The existing `ci_release_gates` integration target contains a static contract
for the exact trigger and concurrency preamble. It fails if push ownership is
broadened, `main`, pull-request, or manual ownership is removed, the event-aware
key changes, cancellation is disabled, or duplicate trigger entries are added.
Its pre-existing assertions continue to protect mandatory release, package,
consumer, archive, and upload behavior.

**Verification:**

```text
cargo test --locked --no-default-features --test ci_release_gates
result: 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo fmt --all -- --check
result: passed with no output

workflow parse/inspection
result: the workflow preamble parsed successfully and the static contract
        confirmed exact main-push, PR, manual, group, and cancellation behavior

git diff --check
result: passed with no output
```

No remote workflow was created or mutated and two rapid GitHub updates were
not dispatched or observed for this item. Live superseded-run evidence remains
to be captured with the representative remote Fast PR evidence before the R1
gate is claimed. This item does not qualify a generator, corpus, codec,
provider, package, release artifact, or target.

### R1.2 — Fast PR and heavy qualification workflow classes

**State:** complete; R1 remains in progress

**Commit:** the commit introducing this workflow split, with subject
`fix(ci): separate fast and heavy qualification` (resolve the exact object with
`git log --format='%H %s' -- .github/workflows/qualification.yml`)

**Owned files:**

- `.github/workflows/ci.yml`
- `.github/workflows/qualification.yml`
- `tests/ci_release_gates.rs`
- `tests/project_artifacts.rs` (`ci_verifies_default_and_feature_gated_codec_paths` only)
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

**Selection and cadence:** `Fast PR` retains R1.1 ownership of pull requests,
`main` pushes, and manual dispatch, with event-aware superseded-run
cancellation. Its single 15-minute job performs formatting; committed
JSON/schema parsing; warning-denied no-default-feature library and binary
compilation; the named `schema_artifacts`, `compatibility_ownership`,
`standalone_docs`, and `ci_release_gates` targets; and only the three-file
`smoke` generate/validate operation. It does not install Python, prepare
highdicom, compile all targets or features, run WSI/stress/core/extended/full
profiles, exercise a codec matrix, package a crate, build an archive, upload an
artifact, or create a release build.

`Heavy qualification` has no push or pull-request owner. It runs on the
explicit weekly schedule or manual dispatch with superseded-run cancellation.
The native-provider, broad default, five in-process codec, and two
external-codec jobs were moved without narrowing their R1.3 codec case/profile
scope or R1.4 provider timing scope. Scheduled and manually selected `nightly`
runs retain that entire broad matrix but do not package, archive, or upload.
Manual `release-candidate` selection requires a lowercase 40-hex commit,
checks out that exact object in every job, prints the selected class/revision,
and proves `HEAD` equals it. Invalid, absent, or mutable-looking candidate
selection fails closed. Only that explicit class executes the retained
standalone contract, packaged-crate/SDK consumer, native archive, installed
consumer, adversarial archive, and single artifact-upload gates; the upload
name also carries the immutable revision.

The expanded `ci_release_gates` target parses both workflow classes as static
contracts. It proves the Fast trigger/concurrency header and all named Fast
inclusions and heavy exclusions, scheduled/manual-only heavy ownership,
immutable candidate validation, broad provider/default/codec/external-codec
availability, release-only package/archive/upload selection, and the existing
release regression-source markers.
The pre-existing `project_artifacts` workflow assertion now reads default and
codec evidence from the heavy workflow and independently rejects those same
heavy selections from Fast PR.

**Representative local Fast PR measurement:** an exact clean run used
`/private/tmp/dts-r12-fast.9dPAdz/target` as its explicit
`CARGO_TARGET_DIR` and `/private/tmp/dts-r12-fast.9dPAdz/smoke` as the fresh
output. The exact workflow-equivalent sequence was:

```text
cargo fmt --all -- --check
jq empty cases/registry.json schemas/*.json transfer-syntax/*.json standards.lock.json
RUSTFLAGS='-D warnings' cargo check --locked --no-default-features --lib --bins
cargo test --locked --no-default-features \
  --test schema_artifacts --test compatibility_ownership \
  --test standalone_docs --test ci_release_gates
cargo run --locked --no-default-features -- generate \
  --profile smoke --out <fresh-output> --seed 1
cargo run --locked --no-default-features -- validate <fresh-output>
```

The clean sequence completed in 71 seconds, below the 15-minute initial Fast
budget. The target occupied 2,315,911,168 allocated bytes and contained 3,471
files with 2,564,301,333 logical bytes. The smoke root occupied 98,304
allocated bytes and contained four artifacts with 86,470 logical bytes.
Generation wrote three files plus the manifest; validation checked three files
with zero failures. The exact temporary root was removed and its absence was
confirmed. This is a local development-cost and same-project tiny-smoke
observation, not remote runner/billable evidence or qualification of WSI,
stress, other profiles, codecs, providers, packaging, releases, independent
conformance, interoperability, or another target.

**Proportional verification:** both workflow files parsed as YAML; the focused
`ci_release_gates` run passed 3/3 tests; the exact Fast sequence above passed
73 schema tests, one compatibility-ownership test, four documentation tests,
three workflow-gate tests, generation, and validation. `cargo fmt --all --
--check` and `git diff --check` passed. The exact moved-workflow
`project_artifacts` assertion also passed 1/1. No heavyweight qualification
ran.

No remote workflow was dispatched, so actual GitHub wall/billable time,
scheduled execution, immutable candidate checkout, artifact upload identity,
and live R1.1 superseded-run cancellation remain open remote evidence. R1.3
through R1.6 also remain open; therefore neither the R1 gate nor terminal
Fast-development/heavy-qualification acceptance is claimed.

## Measurements

| Measurement | Baseline command/revision | R0 value | Terminal value | State |
| --- | --- | ---: | ---: | --- |
| CI wall time by verification class | Run `33491521696`; class projection documented in the dated baseline | 132m15s full observed interval; no class is independently routed | — | Baseline recorded |
| Billable runner time by verification class | Exact API job durations; failed attempt plus retry included | 175m45s actual; 180 per-job rounded minutes | — | Baseline recorded |
| Largest local target-directory size | `CARGO_TARGET_DIR=/private/tmp/dts-r02-target.xAApSK cargo test --locked --all-targets --no-default-features --no-run` | 8,013,463,552 bytes after 72.29s | — | Baseline recorded; exact directory removed |
| Integration-test target count | Cargo metadata plus top-level `tests/*.rs` at `65a296b` | 186 integration targets; 188 Cargo-reported harness executables | — | Baseline recorded |
| CI/generated artifact count and size | Actions API for run `33491521696` | 1 upload, ID `9798112659`, 9,929,745-byte ZIP; no uploaded corpus | — | Baseline recorded |
| Representative generator Fast PR | No independent class at `65a296b` | Not independently measurable; every PR selects the full graph | 71s clean local wall; 2,315,911,168-byte target; four 98,304-byte allocated smoke artifacts | Local R1.2 class passes initial budget; remote gate remains |
| Representative corpus PR | Repository does not yet exist; embedded corpus edit selects full graph | Not independently measurable | — | Explicit boundary recorded |
| Representative viewer PR | Viewer repository not in current scope | — | — | Not measured |
| Nightly and release-candidate cost | No separate Nightly/RC trigger; run `33491521696` is exact candidate evidence | Nightly not independently measurable; provider/default/release critical chain 123m53s | — | Explicit boundary recorded |

Before/after cost reduction cannot be claimed until R1/R2 record comparable
class-specific results and R9.6 repeats the terminal measurements. The R0.2
baseline is diagnostic evidence, not proof that a target budget has passed.

## Blockers and authority boundaries

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
| Smoke migration | In progress | The R0 parity baseline passes for the current embedded smoke slice; R6 repository generation and comparison have not run. |
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
