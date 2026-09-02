# `synth-dicom-gen` / `dcmview-test-corpus` migration status

**Recorded:** 2026-09-01

**Updated:** 2026-09-02

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
| R1 — contain CI and local build cost | Complete | R1.1, R1.2, R1.3, R1.4, R1.5, R1.6 | A disposable draft-PR probe proved superseded-run cancellation and single-event ownership. Replacement Fast run `33581809536` passed in 123 seconds of job time with only the declared Fast work, a 739,602,432-byte target, four smoke artifacts occupying 122,880 allocated bytes, and the 4-GiB ceiling enforced. The broad matrix remains separately scheduled/manually invocable. The R1 gate passes. |
| R2 — reduce Rust test-linking amplification | In progress | R2.1 | A machine-readable inventory assigns all 188 Cargo harness targets and 1,375 direct Rust test entries to exactly one domain and verification class. The Fast metadata checker fails closed on target/entry drift, duplicate or missing ownership, unsupported generated-test attributes, and heavy or ignored Fast assignments. Harness consolidation, explicit heavy entry points, targeted routing, and the R2 gate remain. |
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

### R1.3 — feature-sensitive codec qualification

**State:** complete; R1 remains in progress

**Commit:** the commit introducing selected codec qualification, with subject
`fix(ci): bound codec qualification to selected cases` (resolve the exact
object with `git log --format='%H %s' -- .github/workflows/qualification.yml`)

**Owned files:**

- `.github/workflows/qualification.yml`
- `src/main.rs`
- `src/lib.rs`
- `tests/generate_cli.rs`
- `tests/ci_release_gates.rs`
- `tests/project_artifacts.rs` (the existing workflow assertion only)
- `docs/generation-guide.md`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

The additive embedded-corpus CLI contract keeps `--profile` required and
accepts repeatable `--case-id`. Requested IDs must be known, unique, and
members of the named profile; errors are deterministic machine failures and
occur before publication. Selection uses the existing
`CuratedScSelection::CaseIds`, including its sorted set semantics and recipe
dependency closure, then follows the same plan-first executor, validation,
private staging, and atomic publication path as profile generation. Omitting
`--case-id` leaves the prior profile-wide CLI and SDK behavior unchanged. This
is deliberately not the R4/R5 external `CorpusDefinitionBundle` contract.

Each codec job asserts that every requested case appears in the generated
manifest either as a file or as an explicit skipped/unavailable case. A
missing external executable can therefore prove only unavailability, never a
pass. The nightly/manual/release-candidate heavy workflow retains all five
in-process features and both external-command features, but their jobs no
longer install uv/Python or prepare highdicom, compile `--all-targets`, or
generate the full `extended` profile. Feature-independent WSI and quantitative
generation are absent from these jobs.

Representative selections are:

| Feature | Requested cases |
| --- | --- |
| `jpeg` | `classic/sc/rgb_planar0_jpeg_baseline_8bit` |
| `charls` | `classic/sc/mono2_u8_jpeg_ls_lossless` |
| `jpegxl` | `classic/sc/rgb_planar0_jpegxl_lossless`; `classic/sc/rgb_jpegxl_lossy` |
| `jpeg2000` | `classic/sc/mono2_u16_jpeg2000_lossless` |
| `deflate` | `classic/sc/mono2_u8_deflated_explicit_le`; `derived/seg/binary_multiframe_deflated_image_frame` |
| `htj2k_openjph` | `classic/sc/mono2_u16_htj2k_lossless`; `classic/sc/mono2_u16_htj2k_lossy` |
| `legacy_jpeg_dcmtk` | `classic/sc/mono2_u16_jpeg_lossless_process_14`; `classic/sc/mono2_u16_jpeg_lossless_sv1` |

**Representative local measurement:** a clean `jpeg` build and selected run
used `/private/tmp/dts-r13-codec.rRz1Ir/target` and requested only
`classic/sc/rgb_planar0_jpeg_baseline_8bit`. Compilation plus generation took
41.93 seconds; cached validation took 1.32 seconds and checked one file with
zero failures. The manifest contained one generated requested case and 125
explicit profile-scope skipped rows, occupied 299,008 allocated bytes with the
single DICOM plus manifest, and the isolated target occupied 1,960,050,688
allocated bytes. Compared with R0's all-target no-run observation, the target
used 75.5% less allocated space (1,960,050,688 versus 8,013,463,552 bytes) and
the selected output retained one payload instead of generating unrelated WSI,
quantitative, or full-profile payloads. These local numbers are development-
cost evidence, not remote billable time or another codec/target qualification.

**Verification:** the two focused CLI selection tests passed, covering sorted
output, manifest evidence, unknown IDs, duplicates, profile incompatibility,
stable machine errors, and fail-before-publication. The existing workflow
artifact assertion passed. Focused workflow, feature-sensitive codec, schema,
formatting, and diff checks are recorded in the task commit report. The local
`jpeg` corpus generated one file, validated with zero failures, and its
manifest carried the requested transfer syntax and case ID. No highdicom,
Python, WSI, quantitative, stress, full profile, external executable,
independent conformance, package, release, or remote workflow qualification
ran. OpenJPH, DCMTK `dcmcjpeg`, and `cjxl` runtime availability were not claimed
by the local run.

### R1.4 — isolated native-provider timing and heavyweight qualifications

**State:** complete; R1 remains in progress

**Commit:** the commit isolating provider-owned tests, with subject
`test(provider): isolate serial provider qualifications` (resolve the exact
object with `git log --format='%H %s' -- .github/workflows/qualification.yml`)

**Owned files:**

- `.github/workflows/qualification.yml`
- `src/generation_backends/process.rs`
- `tests/composition_curated_migration.rs`
- `tests/composition_quantitative.rs`
- `tests/ci_release_gates.rs`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

The default all-target command retains its full test target selection but no
longer sets a global `RUST_TEST_THREADS=1`. Seven exact provider qualifications
are ignored by default with an explicit `R1.4 native-provider-contract` reason
and are invoked individually by the scheduled/manual/release-candidate native
provider job under `RUST_TEST_THREADS=1`, `--ignored`, and `--exact`:

1. `generation_backends::process::tests::fake_backend_timeout_is_enforced`
2. `generation_backends::process::tests::fake_backend_cancellation_interrupts_fingerprinting_promptly`
3. `generation_backends::process::tests::fake_backend_cancellation_kills_and_reaps_a_spawned_process_tree_promptly`
4. `generation_backends::process::tests::fake_backend_inherited_pipe_timeout_is_enforced`
5. `migrated_curated_recipes_record_shared_plan_materialization`
6. `quantitative_default_bundles_are_closed_provenanced_and_reproducible`
7. `caller_segmentation_and_parametric_values_round_trip_at_fixed_shape`

The two ignored re-entrant-process entries in the library harness are marked
separately as subprocess fixtures, not qualification evidence. Static CI
regressions require exactly seven provider-owned ignore reasons, exactly two
fixture reasons, no unreasoned ignore marker in these sources, one explicit
serial command per provider test, and no default-job test-thread override.
This keeps default discovery and the provider command inventory synchronized.
The 30-millisecond timeout, 15-second cancellation ceilings, 8-second inherited
pipe ceiling, and other existing resource/timing ceilings were not relaxed.

**Verification and local runtime boundary:** all four exact ignored process
commands passed serially on macOS arm64. Their harness execution times were
3.00 seconds, 0.05 seconds, 0.07 seconds, and 0.57 seconds respectively. Test
discovery found all seven named entries. The three prepared-backend tests were
not executed because
`generation-backends/highdicom-pydicom/.venv/bin/python` was absent; this is an
explicit local runtime blocker, not an unavailable-to-pass conversion or
provider qualification. Their workflow commands remain exact and the heavy job
prepares the locked backend before execution. No Python environment was
created, and no WSI, quantitative, stress, full-profile, package, release,
external-codec, or remote qualification ran locally.

**Representative local cost measurement:** after one clean compile into the
isolated `/private/tmp/dts-r14-provider.FqqaUu/target`, the default library
bundle ran 475 discovered tests with 469 passed and six ignored. Normal harness
parallelism took 8.46 seconds wall time; the same cached bundle under
`RUST_TEST_THREADS=1` took 33.34 seconds, a 74.6% wall-time reduction for this
representative bundle. The isolated target occupied 1,125,044 KiB and its exact
temporary root was removed. This is local development-cost evidence, not
remote runner/billable evidence or a broad default, provider, corpus, target,
or release qualification.

### R1.5 — build-storage controls and cost reporting

**State:** complete; R1 remains in progress

**Commit:** the commit introducing this storage contract, with subject
`fix(ci): bound and report build storage` (resolve the exact object with
`git log --format='%H %s' -- scripts/report-ci-cost.sh`)

**Owned files:**

- `Cargo.toml`
- `.github/workflows/ci.yml`
- `.github/workflows/qualification.yml`
- `scripts/report-ci-cost.sh`
- `tests/ci_release_gates.rs`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

The committed development and test profiles disable incremental compilation
and set debug information to zero. Every Fast or heavy workflow job also sets
`CARGO_INCREMENTAL=0`, the development/test debug controls, and a unique
absolute `CARGO_TARGET_DIR` under the runner-provided `RUNNER_TEMP`; each job
exports that path through `GITHUB_ENV` before Cargo work because the `runner`
expression context is unavailable while GitHub evaluates a job-level `env`
map. The release-candidate job
additionally sets release debug information to zero. Package extraction and
native archive construction consume that exact target root instead of an
implicit repository `target/` path.

Every logical job has one final `if: always()` cost step. The shared reporter
prints elapsed build-work seconds, the exact target root and allocated bytes,
relevant output allocated bytes and file count, the configured byte ceiling,
and whether enforcement applies. Missing outputs report zero; paths are
passed as quoted arguments; unsigned and exact-number-range checks reject
unsafe arithmetic; and the target argument must exactly equal
`CARGO_TARGET_DIR`. The reporter never removes or broadly searches for build
evidence. A Fast, selection, provider, or codec job enforces its ceiling only
after all preceding job work succeeds, while a failed job still reports its
partial evidence. Nightly and release-candidate jobs retain their larger
explicit ceilings as measurements rather than short-budget gates.

The R0 7.46-GiB all-target tree establishes the upper comparison boundary.
The agreed class ceilings are:

| Class/job | Ceiling | Enforcement |
| --- | ---: | --- |
| Fast PR | 4,294,967,296 bytes (4 GiB) | Fail an otherwise-successful job above the ceiling |
| Selection | 1,073,741,824 bytes (1 GiB) | Fail an otherwise-successful job above the ceiling |
| Provider and codec subsystem | 6,442,450,944 bytes (6 GiB) | Fail an otherwise-successful job above the ceiling; 19.6% below R0 |
| Nightly broad default | 12,884,901,888 bytes (12 GiB) | Always report; do not discard broad evidence to meet an ordinary ceiling |
| Release candidate | 17,179,869,184 bytes (16 GiB) | Always report; do not discard terminal evidence to meet an ordinary ceiling |

**Representative clean Fast measurement:** the exact R1.2 Fast-equivalent
sequence ran with the new profiles and explicit
`/private/tmp/dts-r15-fast.xciWHT/target`, generated only
`/private/tmp/dts-r15-fast.xciWHT/smoke`, and completed successfully in 56
seconds. The reporter recorded 870,412,288 allocated target bytes, 1,872
target files, and four smoke artifacts occupying 98,304 allocated bytes. This
is 62.4% less target storage than the comparable pre-R1.5 R1.2 Fast
measurement (2,315,911,168 bytes) and 89.1% below the differently scoped R0
all-target observation (8,013,463,552 bytes). The exact temporary root was
removed after reporting and confirmed absent. The 56-second local result is
below the 15-minute Fast wall budget; it is not remote runner or billable-time
evidence.

**Verification:** shell syntax and spaced-path, missing-path, overflow, and
over-budget fixtures passed. Both workflow files parsed as YAML. The clean
Fast sequence passed formatting, committed JSON parsing, warning-denied public
compilation, 73 schema tests, one compatibility-ownership test, four
documentation tests, six workflow/storage tests, smoke generation, and strict
validation of three files with zero failures. `cargo metadata --locked
--no-deps`, `cargo fmt --all -- --check`, and `git diff --check` passed. The
packaged-crate inventory includes the executable reporter through the existing
`scripts/**` package include. No WSI, stress, full-profile, codec, provider,
Python, package build, archive build, release, external-runtime, independent
conformance, interoperability, or remote qualification ran.

The local evidence alone did not close the gate; the corrected live probe
below supplies the required remote cancellation, Fast-routing, and storage
measurement. Terminal multi-repository Fast-development and heavy-
qualification acceptance remain later-phase claims.

**Remote workflow-evaluation regression (2026-09-01):** the first authorized
live probe at immutable revision
`6af35c1ed0f96389a7387dab3c867571efaec9ff` exposed a workflow-definition
defect before any runner was allocated. Push runs
[`33581314881`](https://github.com/beatrice-b-m/dicom-test-suite/actions/runs/33581314881)
(`CI`) and
[`33581315565`](https://github.com/beatrice-b-m/dicom-test-suite/actions/runs/33581315565)
(`.github/workflows/qualification.yml`) both completed with conclusion
`failure` and an empty `jobs` array. The invalid definitions used
`${{ runner.temp }}` in job-level `env`; GitHub permits the `runner` context
only after a job is running. The repair moves every unique target assignment
into the job's initial shell step via `$RUNNER_TEMP` and `$GITHUB_ENV`, and the
static gate now rejects `runner` expressions outside step scope and requires
each exact target export before Cargo work. A fresh local Fast-equivalent run
with the repaired export contract passed in 57 build-work seconds: its exact
target occupied 870,436,864 allocated bytes, and its four-file smoke output
occupied 98,304 allocated bytes with zero validation failures. The focused six
workflow/storage tests, workflow YAML parsing, formatting, and diff checks also
passed. The local temporary root was removed after measurement. These failed
remote runs are diagnostic evidence only and are not counted as passing jobs.

**Corrected live R1 gate probe (2026-09-02):** with explicit user authority, a
disposable draft pull request received two rapid synchronize updates. Fast run
[`33581782550`](https://github.com/beatrice-b-m/dicom-test-suite/actions/runs/33581782550)
started for head `65aa444fc643da1947ed1d3b3fd7f29361bac47d` at
02:03:39Z and completed `cancelled` at 02:04:18Z after the second update.
Replacement Fast run
[`33581809536`](https://github.com/beatrice-b-m/dicom-test-suite/actions/runs/33581809536)
ran for head `5d1564a02b4ddbfbec4ca99274d24bc3ea1575dd` from
02:04:21Z through 02:06:24Z and completed `success`. Both repaired-head runs
had event `pull_request`; neither head scheduled a push-equivalent CI run or a
qualification workflow. The two earlier push-event parser failures above are
bound only to the invalid pre-repair head and allocated zero jobs.

The successful job passed formatting and JSON schemas, warnings-denied public
library/CLI compilation, the named light contract/domain tests, generation of
the three-file seed-1 smoke corpus, and strict validation with zero failures.
It performed no WSI, stress, full-profile, Python-backend, codec, provider,
package, archive, or release work. The reporter measured 116 build-work
seconds, 739,602,432 allocated target bytes, 122,880 allocated output bytes,
and four output artifacts, and enforced the 4,294,967,296-byte Fast ceiling.
The GitHub job interval was 123 seconds including setup and cleanup, below the
15-minute gate. The draft pull request was closed without merge and its remote
probe branch was deleted after evidence capture. The permanent implementation
is commit `066357f`; the two probe-only commits are not part of product
history. This closes the R1 gate without claiming a Nightly, release-candidate,
external-runtime, conformance, interoperability, or target-matrix run.

### R1.6 — release build and archive reuse

**State:** complete; R1 remains in progress

**Commit:** the commit establishing immutable RC artifact reuse, with subject
`fix(release): reuse immutable candidate artifacts` (resolve the exact object
with `git log --format='%H %s' -- .github/workflows/qualification.yml`)

**Owned files:**

- `.github/workflows/qualification.yml`
- `scripts/build-release-archive.sh`
- `tests/release_archive.rs`
- `tests/ci_release_gates.rs`
- `tests/release_process.rs`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

The release-candidate job retains exactly one `cargo package --locked` and one
package extraction for the packaged SDK consumer. It then compiles the
target-specific optimized binary exactly once, records its absolute path,
SHA-256, target, and immutable selected revision, constructs exactly one
archive, verifies and extracts that archive once, and proves that the installed
binary is byte-identical to the compiled candidate. Black-box, caller-content,
qualified-catalog, structural-catalog, and upgrade consumers all use that one
installed binary and extraction. The archive harness receives the same binary
and archive identities through the job environment; it does not repackage the
Cargo test binary when a candidate is supplied. Upload remains after the
archive harness, and the artifact name includes the immutable selected
revision.

The archive builder now fails closed when an override is relative, lacks a
64-hex expected SHA-256, differs from that hash, lacks an expected source
revision or target, or disagrees with the checked-out revision or requested
target. The archive harness similarly requires either none or all six candidate
bindings, requires absolute paths, rehashes both inputs, compares target and
manifest revision, and confirms the extracted executable hash. Its checksum
and payload-tampering fixtures copy the supplied archive, so adversarial checks
do not construct an unqualified substitute. The no-environment local source
test still constructs one archive around Cargo's test binary and passed.

**Before/after construction count:** before R1.6, the RC workflow made one
optimized binary/archive for installed consumers, then the integration harness
made a second archive around its debug test binary. After R1.6, the RC workflow
performs one optimized release build, one archive construction, and one archive
extraction; all installed consumers and the harness share those exact
artifacts. Static CI regressions count the package, release-build, archive,
extraction, and upload commands, prove the hash/revision/target dataflow, and
require upload ordering after qualification. No package, archive, or release
evidence was removed.

**Focused local verification and measurement:** shell syntax, formatting, six
CI release/storage tests, four release-process tests, and the complete
release-archive harness passed. The ordinary no-prebuilt harness took 36.54
seconds wall time (35.75 seconds in the test). A fresh isolated macOS arm64
candidate run at revision `333355d8b3ccf8ff693d43235b62b18b3772cfa1`
performed one release build and one archive construction, then passed the
supplied-candidate archive harness in 11.01 seconds; the complete isolated
compile/archive/test sequence took 118 seconds. The 25,256,816-byte binary had
SHA-256 `8607165d05c5790a5d42664c1239c7de2a55acfbabef80d14a1676125d0ebcbd`.
The 9,031,179-byte archive had SHA-256
`e6675709b136b8a87c5c4bc0564da6544558c0bb978a805aae364b181586eea7`.
The isolated target occupied 1,117,933,568 allocated bytes. These are local
reuse and identity measurements for a dirty-worktree test candidate, not
promoted release or remote runner evidence.

No broad profile, WSI, stress, provider, codec, external-runtime, independent
conformance, interoperability, or target matrix ran. R1.6's single-construction
and identity contract is established by the focused local candidate and static
workflow gates; R1 does not require a release-candidate execution. The live
probe above closes the phase gate only for the representative Fast class.
Terminal Fast-development and heavy-qualification rows remain open until the
later repository split and exact terminal runs.

### R2.1 — exhaustive Rust test ownership metadata

**State:** complete; R2 remains in progress

**Commit:** the commit introducing this ownership contract, with subject
`test(ci): require explicit test ownership metadata` (resolve the exact object
with `git log --format='%H %s' -- product/test-ownership.json`)

**Owned files:**

- `product/test-ownership.json`
- `scripts/check-test-ownership.py`
- `tests/test_test_ownership_checker.py`
- `.github/workflows/ci.yml`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

The manifest inventories all 186 integration targets plus the library and
binary harnesses reported by locked Cargo metadata. Every top-level integration
source, including a zero-entry harness if one exists, and every `src/**/*.rs`
file containing a direct `#[test]` attribute has one source group. Each group
owns the exact entry-name list and a digest of the discovered test
segments, and assigns every entry one domain, verification class, and ordinary
or heavy cost tier. Target keys and source groups are unique, so duplicate
ownership is not representable without failing the checker.

The accepted inventory contains 188 targets, 262 entry groups, and 1,375 test
entries. Target classes are 4 Fast, 149 Subsystem, 26 Nightly, and 9 Release
candidate; entry classes are 84 Fast, 1,151 Subsystem, 128 Nightly, and 12
Release candidate. Entry ownership by domain is 24 assembly, 151 CLI/SDK, 88
codec, 96 composition, 81 conformance/interoperability, 487 corpus generation,
160 engine, 30 provider, 25 release/CI, 160 schema/resource, and 73
standards/validation tests.

The six exact heavyweight entries frozen by R0.2 remain explicitly named in
their groups. Stress, WSI, full-file, and ignored-provider groups are
conservatively marked heavy; no such group is Fast. Fast entry names carrying
heavy-looking terms require a non-empty cost exemption. The three current
exemptions are limited to static workflow/script inspection, static JSON Schema
validation, and static documentation inspection; none executes the heavy work
its strings describe. Process launches, sleeps, broad Cargo flags, full-profile
selectors, and release-archive commands are also detected Fast cost markers.
Fast assignment fails if a group is heavy or contains an ignored test even if
an exemption is present.

The checker derives the Cargo target set without compiling, rediscovers all
direct Rust test attributes, compares exact targets, sources, names, counts,
and digests, validates enum and target references, and rejects missing,
duplicate, stale, conflicting, or accidentally heavy Fast ownership. Async or
macro-generated test attributes are deliberately unsupported and fail closed
until the checker is extended; no such attribute exists in the accepted
inventory. This is a static ownership/routing contract, not evidence that the
1,375 tests executed or passed.

The Fast workflow runs the checker and its five negative/positive fixtures
before Rust compilation. Fixtures prove the current inventory, exact six-entry
heavy baseline, workflow hook, unowned target, duplicate target, entry drift,
duplicate source ownership, heavy Fast assignment, and missing Fast exemption
boundaries. No Rust integration target was added, no harness was consolidated,
and no existing test assertion or test selection was changed.

**Measurement and verification:** the checker completed in 0.35 seconds wall
(0.29 user, 0.04 system), and the five fixtures completed in 2.23 seconds wall
(2.184 seconds in `unittest`). The JSON manifest is 225,408 logical bytes and
224 KiB allocated; checker and fixture sources are 18,357 and 3,257 logical
bytes. `python3 scripts/check-test-ownership.py`, `python3 -m unittest
tests/test_test_ownership_checker.py`, Python bytecode compilation in an exact
disposable cache, workflow YAML parsing, `jq empty`, locked no-dependency Cargo
metadata, `cargo fmt --all -- --check`, and `git diff --check` passed. The
disposable bytecode cache was removed by exact path and confirmed absent. No
corpus output, package, archive, heavyweight test, qualification workflow, or
external state was created. The existing six-test `ci_release_gates` target
also passed in an explicit fresh target to protect the changed Fast workflow;
that focused compile/test took 26 seconds and occupied 567,877,632 allocated
target bytes. Its cost was reported before the exact temporary root was
removed and confirmed absent. This focused static target is not a broad Rust
suite or qualification run.

R2.2 still owns harness consolidation and the at-most-20 binary gate. R2.3
still owns explicit heavy entry points, and R2.4 owns change-to-test routing.
Therefore neither the R2 gate nor a reduction in linked harness count is
claimed by R2.1.

### R2.2 harness-conversion slice — assembly, engine, provider, and standards

**State:** slice complete; R2.2 remains in progress

**Commit:** the commit introducing this conversion slice, with subject
`test(harnesses): group planning and provider domains` (resolve the exact
object with `git log --format='%H %s' -- tests/harnesses/engine__subsystem.rs`)

**Owned files:**

- `tests/harnesses/assembly__nightly.rs`
- `tests/harnesses/assembly__subsystem.rs`
- `tests/harnesses/engine__nightly.rs`
- `tests/harnesses/engine__subsystem.rs`
- `tests/harnesses/provider__subsystem.rs`
- `tests/harnesses/standards_validation__subsystem.rs`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

This slice is derived mechanically from
`/private/tmp/r2.2-proposed-partition-fecc6bf.json`, SHA-256
`f99fc5d0930bffd7838772b69f45129bb718e71e5d6b4ba1ce8309561df625b8`.
The artifact declares schema `r2.2-proposed-partition/v3`, source revision
`fecc6bf99f908153b77a712edb0deb6e87441159`, R2.1 ownership SHA-256
`3befc2d9a9cbe634c959f368988ee4385fc3f32aad67bd22cca6a2849db63637`,
and entry-contract SHA-256
`87adfb84d24b5160beb27cba648f51ca1594a272608d48b82976ed6f42919d0e`.
It proposes the complete 186-source/20-harness partition; this commit owns only
the following disjoint portion:

| Harness | Included sources | Existing test entries |
| --- | ---: | ---: |
| `assembly__nightly` | 1 | 2 |
| `assembly__subsystem` | 4 | 22 |
| `engine__nightly` | 6 | 19 |
| `engine__subsystem` | 30 | 137 |
| `provider__subsystem` | 7 | 30 |
| `standards_validation__subsystem` | 1 | 1 |
| **Slice total** | **49** | **211** |

Each harness includes every assigned top-level integration source exactly once
with stable `#[path = "../<source>.rs"] mod <source_stem>;` declarations. The
49 source files remain in place and byte-untouched; no test assertion, helper,
ignore marker, evidence boundary, or selection changed. Module isolation keeps
source-local helper names from colliding across the grouped harness.

**Static verification:** the partition artifact's SHA-256 was recomputed before
generation. A deterministic comparison proved exact ordered membership for all
six harnesses, 49 unique source paths with no within- or cross-slice duplicate,
matching source-stem module identifiers, and an existing file for every path.
All six new files end with a newline, contain no trailing whitespace, and pass
the scoped diff check. Nothing was staged during the parallel implementation
step.

The sources include existing `CARGO_BIN_EXE_dicom-test-suite`,
`CARGO_MANIFEST_DIR`, and source-relative `include_str!` uses. Their behavior
must be proven by the central Cargo target declaration and compile/list gate;
this slice deliberately does not edit `Cargo.toml`, workflows, R2.1 ownership
metadata/checker, or routing. The other 14 proposed harnesses, removal of the
186 implicit top-level targets, exact compiled test-entry parity, linked-binary
count, and clean size/time comparison are shared sequential boundaries still
pending. Accordingly this slice does not mark R2.2 complete and makes no R2
gate or cost-reduction claim.

### R2.2 harness-conversion slice — corpus generation and codecs

**State:** slice complete; R2.2 remains in progress

**Commit:** the commit introducing this conversion slice, with subject
`test(harnesses): group corpus and codec domains` (resolve the exact object
with `git log --format='%H %s' -- tests/harnesses/corpus_generation__subsystem.rs`)

**Owned files:**

- `tests/harnesses/corpus_generation__nightly.rs`
- `tests/harnesses/corpus_generation__subsystem.rs`
- `tests/harnesses/codec__nightly.rs`
- `tests/harnesses/codec__subsystem.rs`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

This disjoint slice uses the same complete partition artifact as the preceding
R2.2 slice: `/private/tmp/r2.2-proposed-partition-fecc6bf.json`, SHA-256
`f99fc5d0930bffd7838772b69f45129bb718e71e5d6b4ba1ce8309561df625b8`.
The artifact is bound to source revision
`fecc6bf99f908153b77a712edb0deb6e87441159`, R2.1 ownership SHA-256
`3befc2d9a9cbe634c959f368988ee4385fc3f32aad67bd22cca6a2849db63637`,
and entry-contract SHA-256
`87adfb84d24b5160beb27cba648f51ca1594a272608d48b82976ed6f42919d0e`.
This commit owns only the following four-harness portion:

| Harness | Included sources | Existing test entries |
| --- | ---: | ---: |
| `corpus_generation__nightly` | 14 | 49 |
| `corpus_generation__subsystem` | 36 | 102 |
| `codec__nightly` | 1 | 3 |
| `codec__subsystem` | 7 | 42 |
| **Slice total** | **58** | **196** |

Every assigned source appears exactly once in artifact order through a stable
`#[path = "../<source>.rs"] mod <source_stem>;` declaration. All 58 paths are
unique and exist, and every path stem equals its module identifier. The source
files remain byte-untouched, so this slice changes no assertion, helper, ignore
marker, qualification scope, or evidence claim.

**Static verification:** the partition hash was recomputed before generation.
An exact ordered comparison checked all four harnesses against the artifact,
proved 58 total and 58 unique source paths, matched all 196 existing entry
assignments, parsed every path/module pair, and found no missing file. Scoped
Rust formatting, trailing-whitespace, final-newline, and diff checks passed.
The parallel implementation step left the files unstaged for selective serial
integration.

The stable paths preserve deliberate hazards for the later compile gate. The
nightly corpus harness contains one feature-gated source, one compile-time
include source, and four `CARGO_BIN_EXE_dicom-test-suite` consumers. The
subsystem corpus harness contains two sources with nested `#[path]` modules,
three compile-time include sources, 25 binary-environment consumers, and seven
`CARGO_MANIFEST_DIR` consumers. The subsystem codec harness contains two
feature-gated sources, three binary-environment consumers, and one manifest-
directory consumer; the nightly codec harness has no recorded path, include,
environment, or crate-attribute hazard. None of these four harnesses owns a
recursion-limit source.

This slice deliberately does not edit central `Cargo.toml` test registration,
workflows, the R2.1 ownership manifest/checker, or routing. Until the shared
Cargo boundary disables implicit discovery, registers all 20 proposed
harnesses, compiles and lists their exact entries, updates selectors, and
measures linked binaries and build cost, these new files are not independently
discoverable Cargo targets. Therefore R2.2 remains in progress; this slice
makes no compile-pass, at-most-20-binary, R2 gate, or cost-reduction claim.

## Measurements

| Measurement | Baseline command/revision | R0 value | Terminal value | State |
| --- | --- | ---: | ---: | --- |
| CI wall time by verification class | Run `33491521696`; class projection documented in the dated baseline | 132m15s full observed interval; no class is independently routed | — | Baseline recorded |
| Billable runner time by verification class | Exact API job durations; failed attempt plus retry included | 175m45s actual; 180 per-job rounded minutes | — | Baseline recorded |
| Largest local target-directory size | `CARGO_TARGET_DIR=/private/tmp/dts-r02-target.xAApSK cargo test --locked --all-targets --no-default-features --no-run` | 8,013,463,552 bytes after 72.29s | — | Baseline recorded; exact directory removed |
| Integration-test target count | Cargo metadata plus top-level `tests/*.rs` at `65a296b` | 186 integration targets; 188 Cargo-reported harness executables | — | Baseline recorded |
| CI/generated artifact count and size | Actions API for run `33491521696` | 1 upload, ID `9798112659`, 9,929,745-byte ZIP; no uploaded corpus | — | Baseline recorded |
| Representative generator Fast PR | No independent class at `65a296b` | Not independently measurable; every PR selects the full graph | Remote run `33581809536`: 123s job interval, 116 build-work seconds, 739,602,432-byte target, four smoke artifacts occupying 122,880 allocated bytes | R1 gate passes 4-GiB and 15-minute budgets; target is 68.1% smaller than the pre-R1.5 Fast measurement and 90.8% smaller than the differently scoped R0 all-target tree |
| Representative corpus PR | Repository does not yet exist; embedded corpus edit selects full graph | Not independently measurable | — | Explicit boundary recorded |
| Representative viewer PR | Viewer repository not in current scope | — | — | Not measured |
| Nightly and release-candidate cost | No separate Nightly/RC trigger; run `33491521696` is exact candidate evidence | Nightly not independently measurable; provider/default/release critical chain 123m53s | — | Explicit boundary recorded |

Before/after cost reduction cannot be claimed until R1/R2 record comparable
class-specific results and R9.6 repeats the terminal measurements. The R0.2
baseline is diagnostic evidence, not proof that a target budget has passed.

## Blockers and authority boundaries

- The location, remote, and creation authority for `dcmview-test-corpus` have
  not been supplied. No persistent corpus repository, release, or remote has
  been created. The authorized disposable R1 probe was closed and its branch
  deleted after evidence capture.
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
