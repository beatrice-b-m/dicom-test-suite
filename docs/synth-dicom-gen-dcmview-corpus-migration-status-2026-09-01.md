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
| R2 — reduce Rust test-linking amplification | Complete | R2.1, R2.2, R2.3, R2.4 | The fail-closed inventory maps 186 integration sources and all 879 integration entries into exactly 20 explicit harnesses. Six R0-measured heavy bodies have exact ignored qualification entry points, while deterministic change routing now selects bounded Fast/subsystem evidence and reports every deferred class without executing it. The aggregate R2 target-count, cost, heavy-isolation, and routing gates pass. |
| R3 — rename reusable product | Complete | R3.1, R3.2, R3.3, R3.4 | Product, package, crate, library, sole binary, archives, discovery, package metadata, current operating guides, product-controlled environment, and production scratch paths use `synth-dicom-gen` / `synth_dicom_gen` at the breaking pre-1.0 product boundary `0.2.0`. Immutable dated evidence retains its exact old candidate identity, and 12 qualified-adapter variables retain provenance-bound spellings pending external requalification. The external-consumer audit found no supported `0.1.0` product consumer requiring an alias. A clean side project compiled and exercised only `synth_dicom_gen::sdk` from the extracted, verified `synth-dicom-gen-0.2.0.crate`, without the old repository path. The aggregate R3 gate passes. |
| R4 — split immutable resources and corpus definitions | In progress | R4.1 | `EngineResources` now owns the embedded and explicit immutable engine-resource boundary without an old public alias. The serialized monolithic identity and its corpus/Cargo membership remain intentionally transitional until R4.3/R4.4. R4.2 externalizes the corpus definition next. |
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

### R2.2 harness-conversion slice — composition and conformance

**State:** slice complete; R2.2 remains in progress

**Commit:** the commit introducing this conversion slice, with subject
`test(harnesses): group composition and conformance domains` (resolve the exact
object with `git log --format='%H %s' -- tests/harnesses/composition__subsystem.rs`)

**Owned files:**

- `tests/harnesses/composition__nightly.rs`
- `tests/harnesses/composition__subsystem.rs`
- `tests/harnesses/conformance_interoperability__nightly.rs`
- `tests/harnesses/conformance_interoperability__subsystem.rs`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

This disjoint slice uses the complete frozen partition artifact
`/private/tmp/r2.2-proposed-partition-fecc6bf.json`, SHA-256
`f99fc5d0930bffd7838772b69f45129bb718e71e5d6b4ba1ce8309561df625b8`.
The artifact declares schema `r2.2-proposed-partition/v3`, source revision
`fecc6bf99f908153b77a712edb0deb6e87441159`, R2.1 ownership SHA-256
`3befc2d9a9cbe634c959f368988ee4385fc3f32aad67bd22cca6a2849db63637`,
and complete entry-contract SHA-256
`87adfb84d24b5160beb27cba648f51ca1594a272608d48b82976ed6f42919d0e`.
This commit owns only the following four-harness portion:

| Harness | Included sources | Existing test entries | Entry-contract SHA-256 |
| --- | ---: | ---: | --- |
| `composition__nightly` | 2 | 4 | `184dc2787d7ee41ad11af77a4f83a65ff0a071f2b0ff893a79a42a62e5bcd98b` |
| `composition__subsystem` | 28 | 84 | `233583b7250d6afacac8f4251b550babc7be1d139108ba16c125646057248a7d` |
| `conformance_interoperability__nightly` | 1 | 3 | `596545df29e846e7d24551cea9b0bc6b847f46dd70d0ca0e4e69294f7ac20dcd` |
| `conformance_interoperability__subsystem` | 17 | 45 | `0e4fb65ae6f1d1a1fefe90a500873ac9b7c6033c1e6dd3a2a27d21b9151146c9` |
| **Slice total** | **48** | **136** | — |

Each assigned source appears exactly once in artifact order through a stable
`#[path = "../<source>.rs"] mod <source_stem>;` declaration. All 48 paths are
unique and exist, every source stem matches its module identifier, and the
original top-level integration sources remain byte-untouched. This slice
therefore changes no assertion, ignore marker, provider timing, external-tool
availability behavior, conformance/interoperability boundary, or evidence
claim.

**Static verification:** the partition SHA-256 was recomputed before
generation. Deterministic exact-text and union comparisons proved ordered
membership for all four files, 48 total and 48 unique source paths, and all
136 existing entry assignments. Harness-only formatting with child traversal
disabled, final-newline, trailing-whitespace, tracked diff, and untracked
no-index diff checks passed. The parallel implementation step left the files
unstaged for this selective serial commit.

The frozen hazard inventory remains explicit for the central compile gate.
`composition__nightly` has one nested-path source.
`composition__subsystem` has five binary-environment consumers, three
compile-time include consumers, two Unix crate-attribute sources, and two
nested-path sources. `conformance_interoperability__nightly` has no recorded
path, include, environment, or crate-attribute hazard. The subsystem
conformance/interoperability harness has ten binary-environment consumers, two
manifest-directory consumers, one compile-time include consumer, ten Unix
crate-attribute sources, and two nested-path sources. None of these harnesses
contains a recursion-limit source. Module nesting also changes fully qualified
test names, so exact provider and workflow selectors must be updated only at
the shared routing boundary.

This slice deliberately does not edit `Cargo.toml`, workflows, the R2.1
ownership manifest/checker, or routing. The central sequential boundary must
disable the 186 implicit top-level targets, register all 20 proposed harnesses,
compile and list every expected entry, prove feature and platform attributes,
update exact selectors, and measure linked binaries and build cost. Until that
gate passes, these files are not independently discoverable Cargo targets and
R2.2 remains in progress. This slice makes no compile-pass,
at-most-20-binary, R2 gate, or cost-reduction claim.

### R2.2 harness-conversion slice — CLI, release, and schema resources

**State:** slice complete; R2.2 remains in progress

**Commit:** the commit introducing this conversion slice, with subject
`test(harnesses): group CLI release and schema domains` (resolve the exact
object with `git log --format='%H %s' -- tests/harnesses/cli_sdk__nonfast.rs`)

**Owned files:**

- `tests/harnesses/cli_sdk__nonfast.rs`
- `tests/harnesses/release_ci__fast.rs`
- `tests/harnesses/release_ci__nonfast.rs`
- `tests/harnesses/schema_resources__fast.rs`
- `tests/harnesses/schema_resources__release_candidate.rs`
- `tests/harnesses/schema_resources__subsystem.rs`
- `docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md`

This final disjoint slice uses the same complete partition artifact,
`/private/tmp/r2.2-proposed-partition-fecc6bf.json`, SHA-256
`f99fc5d0930bffd7838772b69f45129bb718e71e5d6b4ba1ce8309561df625b8`,
bound to R2.1 ownership SHA-256
`3befc2d9a9cbe634c959f368988ee4385fc3f32aad67bd22cca6a2849db63637`
and entry-contract SHA-256
`87adfb84d24b5160beb27cba648f51ca1594a272608d48b82976ed6f42919d0e`.

| Harness | Included sources | Existing test entries | Entry-contract SHA-256 |
| --- | ---: | ---: | --- |
| `cli_sdk__nonfast` | 12 | 151 | `24dd4802ac6aba7fc513c2aa58cbf7521b4652bf5b76b8c1bebe51ea63e3ddc0` |
| `release_ci__fast` | 3 | 11 | `c87d17ac666a4eb84ed03135204273a3b90f4c53dc67cc1826674c4e6986fa6d` |
| `release_ci__nonfast` | 9 | 14 | `6766e3189c1bb0d57400ae15bd1507322fd8b56713abd6bab87b68742c8bf3bd` |
| `schema_resources__fast` | 1 | 73 | `b405f6cfe3ea730095dfac849f199a204c74ca538ad4b913431d62cbb122b74e` |
| `schema_resources__release_candidate` | 1 | 1 | `9ebcddec6b7d1ec20b4d5a534124d96fd9b5191b70f9cd1a643141d0a79036c8` |
| `schema_resources__subsystem` | 5 | 86 | `b52f46e62afc06f4fdcd711953f5f4472be7b706c573757afe2ec3521df8fb38` |
| **Slice total** | **31** | **336** | — |

Each assigned source appears once in artifact order through a stable path
module. The 31 original files remain byte-untouched. Crate-level recursion
limits are repeated at the CLI and schema harness roots; the retained inner
attributes on `report_cli.rs` and `schema_artifacts.rs` are explicitly allowed
as module attributes so warnings-denied compilation can verify the grouping
without changing those source contracts.

**Static verification:** a deterministic artifact comparison proved exact
ordered membership for all six files, 31 unique source paths, matching module
stems, and all 336 entry assignments. All referenced paths exist and the
harness files pass the scoped whitespace and diff checks. Across the four
committed conversion slices, the proposed union is now exactly 20 harnesses,
186 unique source files, and all 879 integration entries; no source or
assertion has moved or changed.

The central sequential boundary still must disable implicit Cargo discovery,
register the 20 roots, remap R2.1 ownership, update exact workflow selectors,
compile/list every expected entry, and measure the final linked-binary count,
time, and storage. Until that atomic change passes, the six files are not
discoverable Cargo test targets and R2.2 remains in progress. This slice makes
no compile-pass, target-count, cost-reduction, or R2-gate claim.

### R2.2 atomic harness integration

**State:** complete; R2 remains in progress

**Commit:** the commit introducing this atomic compatibility boundary, with
subject `test(harnesses): integrate explicit domain targets` (resolve the exact
object with `git log --format='%H %s' -- Cargo.toml`)

The integration consumes
`/private/tmp/r2.2-proposed-partition-fecc6bf.json`, SHA-256
`f99fc5d0930bffd7838772b69f45129bb718e71e5d6b4ba1ce8309561df625b8`,
bound to source revision `fecc6bf99f908153b77a712edb0deb6e87441159`,
R2.1 ownership SHA-256
`3befc2d9a9cbe634c959f368988ee4385fc3f32aad67bd22cca6a2849db63637`,
and entry-contract SHA-256
`87adfb84d24b5160beb27cba648f51ca1594a272608d48b82976ed6f42919d0e`.
`autotests = false` disables the 186 implicit integration crates and Cargo
registers exactly the artifact's 20 roots. The ownership checker reports 22
owned test build targets including lib/bin, 262 source groups, and 1,375 total
entries; the integration subset remains exactly 186 sources and 879 entries.
Mixed-class `__nonfast` roots retain entry-level ownership and cannot acquire a
Fast assignment. Named-suffix class drift, missing or duplicate source
membership, entry digest drift, and the 20/186/879 cardinality contract all
fail closed.

Live workflow selectors were translated rather than broadened. Fast runs only
`schema_resources__fast` and `release_ci__fast`. Provider, codec, validation,
SDK/archive, and release-candidate selections name both their new harness and
source-module prefix; the default Nightly suite intentionally retains its
broad all-target evidence. R2.3 heavy-prefix and R2.4 change-routing work was
not pulled forward.

Three harness-root support shims were required for source files that previously
relied on being Cargo crate roots. `tests/sr_rt_evidence.rs` and
`tests/protocol_baseline.rs` now import the one harness-root support module so
product and test code share nominal types; `release_ci__nonfast` re-exports the
typed-bulk product modules expected by its path-included projection. These are
compile-only wiring changes: no test function, assertion, source membership,
or product `src/**` file changed.

Verification and measurements:

- `python3 scripts/check-test-ownership.py` passed with 22 targets, 262
  groups, and 1,375 entries; six checker fixture tests passed.
- YAML parsing, `cargo metadata --locked --no-deps --format-version 1`,
  `cargo fmt --all -- --check`, and `git diff --check` passed. Metadata exposed
  exactly 20 explicit integration targets.
- A warnings-denied clean Fast no-run build completed in 25.74 seconds and
  occupied 580,317,184 target bytes. All six focused
  `ci_release_gates::` tests then passed.
- A clean
  `cargo test --locked --all-targets --no-default-features --no-run` completed
  in 33.87 seconds (`user 149.82`, `sys 10.16`), produced 22 executable lines
  and 1,101 target files, and occupied 1,259,962,368 target bytes. Its one
  8,192-byte allocated log artifact was reported before cleanup.
- Default list mode discovered 841 entries. All-features no-run/list mode was
  restricted to the five harnesses owning the 38 feature-gated entries; its
  compile completed in 25.80 seconds. The default/all-features configuration
  union matched all 879 expected module-qualified entries exactly once, with
  zero missing, unexpected, duplicate-owned, or wrong-harness entries.
- Ignored list mode retained exactly seven provider-owned entries and two
  subprocess fixtures. No ignored or heavy test body ran, and no broad
  qualification was invoked.

Compared with the same no-default all-target R0 baseline of 8,013,463,552
bytes, the clean R2.2 tree is 6,753,501,184 bytes (84.28%) smaller. The three
exact temporary build roots and the list-output root were measured before
removal and then removed. This closes R2.2's at-most-20 integration-binary,
entry-parity, selector-compatibility, and measured-link-cost acceptance
boundary. R2.3, R2.4, and the aggregate R2 gate remain open.

### R2.3 — explicit heavy qualification entry points

**State:** complete on 2026-09-02; R2 remains in progress

**Commit:** the atomic commit with subject
`test(qualification): isolate explicit heavy entry points` (resolve the exact
object with `git log --format='%H %s' -- scripts/run-heavy-qualification.sh`)

The six exact R0-measured heavyweight functions are now ignored by ordinary
broad/default harness execution with the common reason `R2.3 explicit heavy
qualification; run through scripts/run-heavy-qualification.sh`. No assertion,
source membership, or harness root moved, and the integration inventory remains
20 targets, 186 sources, and 879 entries. The ownership checker requires the
exact six source/function assignments, the exact ignore reason, and their
post-attribute entry digests:

| Primary entry point | Harness and exact module-qualified function | Ownership entry digest |
| --- | --- | --- |
| Byte parity | `corpus_generation__nightly case_recipe_catalog::data_first_sc_and_metadata_values_and_hashes_match_current_generator_bytes` | `1c5997e98ac7f858a85464388b92f01370372ceaa7d2246145a4c94e310cd6a7` |
| All-profile | `cli_sdk__nonfast generate_cli::generate_command_writes_all_profile_union_and_skips_planned_cases` | `4798e28c764d10de70957b13b5f6dca9b7a21e11bae95635de1de4b0ff0c8f` |
| WSI ordinary | `engine__nightly wsi_direct_plan::ordinary_wsi_direct_plans_match_fresh_seed_one_bytes_and_manifest_facts` | `4dd7971502e9748a95ad7d76688b73c1203a89400d7a6e5aa14c4e9913ebaa45` |
| WSI stress | `corpus_generation__nightly wsi_pyramid::stress_profile_emits_complete_three_instance_wsi_pyramid` | `e668db30959a9a3aedd4785aa74c9fb567379d0d651a35e96c6ae68af537acb3` |
| Stress projection | `corpus_generation__nightly curated_stress_manifest::typed_stress_projection_matches_frozen_file_values_and_resources` | `708bb20e44d9970c5ce1473e821bbb228f1b36942f81e7137fff90d3a054e57d` |
| Stress execution | `corpus_generation__nightly curated_stress_sc_integration::all_stress_sc_cases_execute_through_private_streaming_services` | `1faef91a82a2a4090a73854e008c3d3fc245202a777e1df97877645c5d511ae3` |

`scripts/run-heavy-qualification.sh` exposes `byte-parity`, `all-profile`,
`wsi`, `stress`, and `all`. The four primary classes are a disjoint 1/1/2/2
partition; `all` is their six-command union, so semantically overlapping WSI
stress evidence is not duplicated. Byte parity includes ordinary, stress, and
legacy scope, all-profile includes opt-in stress, and WSI includes ordinary
and reduced-stress evidence. Before executing a heavy body, every command runs
an exact ignored list preflight and requires one discovered match, closing
Cargo's otherwise-successful zero-match behavior. The 2,527-byte dispatcher
has SHA-256
`1904f04698a9eaf224ba79359ae41e40f915d8c17ee808368e8a80c97e0792cb`;
the 193,361-byte ownership manifest has SHA-256
`7a9d032b7179b6ab1994fe672bbaa15b078b09e66b0101febed542ba7eb1e7c6`.

The Nightly/default job first runs the ordinary locked all-target suite, then
invokes `scripts/run-heavy-qualification.sh all` exactly once. A release-
candidate run depends on that default job and its standalone package/archive
job contains no dispatcher or heavy function, so it inherits rather than
repeats the six bodies. Fast statically rejects the dispatcher, Nightly and
mixed non-Fast harnesses, ignored selection, broad profiles, and every exact
heavy function. Provider, codec, package, archive, consumer, external-runtime,
conformance, and interoperability cadence remains unchanged.

Verification deliberately did not execute a heavy body. `sh -n` passed; dry
runs of all four primary classes and `all` produced the expected disjoint
1/1/2/2 sets and six unique locked, no-default-feature, ignored, exact
commands. YAML parsing, `python3 scripts/check-test-ownership.py`, and all six
ownership fixtures passed. The focused static
`ci_release_gates::heavy_workflow_retains_nightly_matrix_and_immutable_release_gate`
test passed. A fresh target at `/private/tmp/dts-r23-target.bO0W0w` compiled
that focused gate in 23.44 seconds wall (65.91 user, 4.72 system); exact list
mode then resolved every heavy selector to one test. After the three owning
harnesses were linked, the target occupied 715,896 KiB (733,077,504 allocated
bytes) across 993 files. Formatting and diff hygiene passed, and the exact
temporary target was removed after measurement.

The R0 bodies' recorded timings remain 683.69, 681.02, 688.21, 686.18, 691.07,
and 685.37 seconds (4,115.54 seconds total); they were not rerun or substituted
with list evidence. No ordinary broad suite, Nightly workflow, release-
candidate, corpus generation, provider, codec, package/archive, external
runtime, independent conformance, interoperability, remote workflow, or
terminal target matrix ran. R2.3's explicit-entry and cadence acceptance
passes; R2.4 routing, the aggregate R2 gate, and terminal heavy-qualification
evidence remain open.

### R2.4 — fail-closed targeted ordinary routing

**State:** complete on 2026-09-02; aggregate R2 gate complete

**Commit:** the atomic commit with subject
`test(routing): add fail-closed change selection` (resolve the exact object
with `git log --format='%H %s' -- product/change-test-routing.json`)

The repository now owns a structured routing contract in
`product/change-test-routing.json` and an argv-only dispatcher in
`scripts/route-changed-tests.py`. Exact paths and directory or filename
prefixes select deterministic bundles for representative engine, codec,
provider, schema, SDK, and current embedded-corpus changes. Repeated and
overlapping paths are unioned and deduplicated; a whole integration target
subsumes its module filters, and the two unconditional Fast targets subsume
their routed modules. Top-level Rust test sources derive their one target,
module, verification class, cost tier, heavy state, and ignore state from
`product/test-ownership.json`. Unknown executable, code, or data paths fail
closed. The only ignored paths are a hard-coded non-executable governance
allowlist, so config drift cannot hide a new executable surface.

Pull requests use the immutable merge-base/head pair, pushes use before/head,
and checkout fetches full history. Rename and copy records route both sides,
while deletion paths remain evidence-bearing. Revisions must be lowercase
40-hex objects and Git name-status tokens are validated before selection.
Manual runs and a zero before-object select the conservative configured
ordinary union without invoking Git; a positive fixture proves that zero-base
path produces all seven deferral records. Fast always runs
`schema_resources__fast` and `release_ci__fast` first, then invokes the router
exactly once in the same bounded target directory. A real empty diff may add
no routed command, but a missing selection mode is an error.

Immediate commands are locked and no-default-feature only. They contain no
all-target, feature, ignored, release, release-candidate, heavy, or provider-
qualification selector. The global fallback contains 60 unique commands: 46
ownership-safe integration target/module selections across 11 target names
and 14 explicit library prefixes. Every library prefix is configured rather
than inferred from a source path, resolves to a nonzero live no-default list,
and is bound to the owning entry names and count. This finite fallback does
not claim exhaustive ordinary library coverage; `unrouted_lib_groups` remains
explicitly deferred. Its other explicit deferrals are the codec feature
matrix, R2.3 heavy entries, future external-corpus evidence, native-provider
qualification, Nightly, and release-candidate evidence. None is executed by
the router.

Some representative bundles deliberately include ordinary dependency
evidence assigned to another R2.1 domain: engine changes include executor
library groups owned by corpus-generation and codec, provider changes include
ordinary corpus-generation library groups, SDK changes include a schema
contract module, and embedded-corpus changes include an engine planning
module. These dependencies are explicit structured commands, not accidental
broad-target leakage. `generation_backends::process::tests::` is the sole
restricted heavy-cost source exception: default list/execution selects its
four ordinary tests while Rust's default ignore behavior leaves its six
provider timing entries for `native_provider_contract` qualification. The
exception cannot be configured for any other source.

The config is 10,761 bytes with SHA-256
`bc1e377bbe4cf49d41eb12f637fbb3ce1efe0e222417b577587fa4960eaff626`;
the 22,299-byte dispatcher has SHA-256
`5514ddde65fd448b55b2a80062ff6e2d9de9477bf291d872a370b398544a1e7e`.
The ownership manifest has SHA-256
`d6e7c4812e3274420539605ccbaca199d80a83160c4f1ab30fcccd938fa91889`.
Regeneration changed only the `tests/ci_release_gates.rs` entry digest: target,
source, entry, domain, class, cost, heavy, and ignored inventories remain 22
targets, 262 groups, and 1,375 entries with otherwise identical assignments.
All 831 tracked paths route or match the explicit governance ignore inventory;
the Cargo package list contains 830 paths and includes the config, dispatcher,
and fixtures under the existing `product/**`, `scripts/**`, and `tests/**`
rules.

Proportional verification passed: 13 routing fixtures, six ownership-checker
fixtures, the ownership checker, and all six focused `ci_release_gates::`
tests; Python compile, JSON and workflow YAML parsing, Cargo metadata, package
inventory, Rust formatting, and diff hygiene also passed. The fixtures bind
the six representative exact command/deferred sets, overlap and whole/module
subsumption, unconditional-Fast coverage, docs-only behavior, unknown and
unowned source rejection, rename/copy/deletion parsing, malformed statuses,
immutable revisions, zero-base fallback, config/ignore/command injection
rejection, tracked-surface coverage, provider ignored isolation, package
contents, and the workflow contract.

A clean Fast-equivalent local measurement used the exact temporary target
`/private/tmp/dts-r24-fast-target/target`, ran the two unconditional Fast
harnesses and the selected engine ordinary bundle, and completed in 68.57
seconds wall (`user 184.02`, `sys 13.74`). The target occupied 705,949,696
bytes and linked four test binaries plus one product binary; the additional
library test binary is the fixture's no-default `--lib -- --list` proof. The
one retained log artifact occupied 36,864 allocated bytes, and the existing
4,294,967,296-byte Fast disk budget passed. Relative to the comparable R0
8,013,463,552-byte all-target tree, this bounded target is 7,307,513,856 bytes
(91.19%) smaller. It is 33,652,736 bytes below the differently scoped remote
R1 Fast target; compared with the compile-only R2.2 Fast target it adds
125,632,512 bytes for list-proven library and selected engine evidence. The
exact temporary target and routing/list/cache outputs were measured before
removal.

An independent warm-invalidation audit then rebuilt the same two
unconditional Fast harnesses and selected engine route in the exact temporary
target `/private/tmp/dts-r2-warm-target.esIeLZ/target`. The initial run took
31.57 plus 35.33 seconds and occupied 705,957,888 allocated bytes across 994
files. After changing only the modification time of `src/planning.rs`, the
same commands took 23.78 plus 35.04 seconds and occupied 706,031,616 allocated
bytes across the same 994 files. The invalidation therefore added 73,728 bytes
(0.0104%) and produced zero files under `target/debug/incremental`; both the
committed development/test profiles and the measurement environment disable
incremental compilation. The R0 record did not retain a separate warm-tree
capture, so the comparison deliberately uses its smaller clean
8,013,463,552-byte tree as the conservative denominator: the post-invalidation
R2 tree is still 7,307,431,936 bytes (91.19%) smaller. No source content or
tracked worktree state changed, and the exact temporary root was removed after
measurement.

No heavy, ignored, provider, feature-gated codec, broad all-target, Nightly,
release-candidate, package build, corpus-generation body, release/archive,
external runtime, independent conformance, interoperability, or remote
workflow body ran. The embedded-corpus rule remains temporary until the
separate corpus repository exists; it neither imports an internal generator
module nor accesses a sibling path. R2.1 ownership, R2.2 harness consolidation,
R2.3 explicit heavy isolation, and this bounded routing/cost acceptance now
close the aggregate R2 gate. Terminal qualification evidence remains owned by
the later qualification phases and is not implied by this routing proof.

### R3.2 — clean pre-1.0 compatibility boundary

**State:** complete on 2026-09-02; aggregate R3 subsequently completed through
R3.3 and R3.4

**Commits:** `86d0298` (`feat(product): establish the 0.2.0 rename boundary`),
`67efbe1` (`fix(release): derive packaged candidate version`), and the
documentation commit containing this section

**Audit evidence:** local Git/history/cache/project searches plus exact public
GitHub and crates.io queries recorded below

R3.2 applies ADR 0003's default breaking pre-1.0 decision. The Cargo package,
lock entry, discovery result, executable banner, release naming, and packaged
consumer now derive from `synth-dicom-gen 0.2.0`; no old package, Rust path,
executable, archive, or environment alias was added. Changing `Cargo.lock`
changed its SHA-256 from
`6b924968e704a002780829a784f5f6c453766bcff4b5b3531f9626378be6086c`
to `4aa4b6c94043fb2f236ec888ac9b253f2ff451b666464609f81f82aaac6d8a4d`,
and the native provider dependency identity in
`generation-backends.lock.json` was rebound to the latter exact bytes.

The consumer audit found no local sibling consumer, cached crate, crates.io
crate, GitHub tag, GitHub release, fork, dependency-graph repository/package,
or supported `0.1.0` product consumer. The exact crates.io endpoints returned
HTTP 404 and both name searches returned zero results. The public GitHub
repository reported zero forks/network members/stars/watchers/open issues;
its release, tag, and fork lists were empty, and its dependency graph reported
zero repositories and packages.

An exact old-URL commit search did find 18 commits in five TomeVault plugin
repositories. Three representative commits were inspected at patch level:
they copy repository agent guidance and record the old repository metadata,
but contain no Cargo dependency, SDK import, executable/archive invocation, or
`0.1.0` product-version binding. They are documented repository-metadata
mirrors, not supported product consumers; shipping a product alias would not
update their copied metadata. GitHub's documented old-URL redirect and those
mirrors remain a post-authorization remote-rename check, not evidence claimed
as already executed.

The exact local inventory used `git remote -v`, `git tag --list`,
`git for-each-ref refs/tags refs/remotes`, full-history `git log -S'cargo
publish'` and release-pattern searches, exact `git grep` publication and
dependency patterns, Cargo registry index/cache searches, and an `rg` search
under `/Users/beatrice/AgentFiles/projects`. Public read-only queries covered
[`beatrice-b-m/dicom-test-suite`](https://github.com/beatrice-b-m/dicom-test-suite),
its API release/tag/fork endpoints, its
[dependency graph](https://github.com/beatrice-b-m/dicom-test-suite/network/dependents),
the exact crates.io API entries and searches for both product names, and
GitHub repository/commit searches. The inspected TomeVault evidence is bound
to Cursor commits
[`9424888`](https://github.com/tomevault-io/cursor-plugins/commit/9424888ee1c04cb1bbce2847362e5cb293da2847),
[`7474a5c`](https://github.com/tomevault-io/cursor-plugins/commit/7474a5ce30c7ffb5376864ff5c5e1f9c0379c3a2),
and
[`a932c00`](https://github.com/tomevault-io/cursor-plugins/commit/a932c00f2ec93d41ff0549d9476ed5c227776347).

The audit limitation is explicit: authenticated GitHub code search was
unavailable because the configured token was invalid and the unauthenticated
API returned HTTP 401. Private, unindexed, out-of-root local, and arbitrary
undiscoverable Git consumers cannot be ruled out. A later discovered product
consumer requires a new compatibility decision and a tested, discovery-visible
support window; it does not retroactively justify an untested alias now.

Proportional verification passed. Locked Cargo metadata reported one package
named `synth-dicom-gen` at `0.2.0`, library `synth_dicom_gen`, and binary
`synth-dicom-gen`. The three exact `version_cli` tests and three exact
`generation_backend_artifacts` tests passed. The built executable emitted
machine and human version `0.2.0`, and the embedded resource inventory bound
the new Cargo lock hash. `cargo package --locked --offline --no-verify
--allow-dirty` packaged 830 files, 14.6 MiB uncompressed and 2.4 MiB
compressed as `synth-dicom-gen 0.2.0`.

The qualification workflow initially retained a literal
`synth-dicom-gen-0.1.0.crate` path. R3.2 repaired that coupling by requiring a
single locked Cargo metadata package, deriving `PACKAGE_STEM`, and using it for
both the crate and extracted SDK-consumer root. YAML parsing, the ownership
checker, single-file Rust formatting, and the focused
`ci_release_gates::heavy_workflow_retains_nightly_matrix_and_immutable_release_gate`
regression passed; the regression fails if the old literal returns or any
metadata-derived path is removed. The first bounded target occupied 631,192
KiB after version/provider/package checks; the isolated CI regression target
occupied 554,976 KiB. Both exact temporary roots were removed.

`git diff --check` passed. Repository-wide `cargo fmt --all -- --check`
remains unavailable as clean evidence because it reports import-order and
wrapping drift introduced by the preceding R3.1 rename across files outside
this item's ownership. R3.2 did not rewrite those unrelated sources; this is
an explicit hygiene blocker for the next owning repair, not an implied pass.
No heavyweight body, provider timing qualification, codec matrix, broad
all-target suite, Nightly, release-candidate, external runtime, conformance,
interoperability, remote rename, remote workflow, or release ran.

### R3.4 — environment and staging spelling transition

**State:** complete on 2026-09-02 after adversarial review and remediation

**Commits:** `b232a06` (`refactor(build): rename compile-time product
variables`), `2e114d7` (`refactor(runtime): rename product-controlled
environment`), `4b698d0` (`refactor(release): rename product environment
contract`), `ea7ba22` (`refactor(staging): rename product temporary paths`),
and `f86f60a` (`test(identity): enforce the spelling transition`). The focused
`24d1085` version-assertion repair and separate R3.2 provider-lock repair
`73a6ec9` preceded these slices and are not counted as R3.4 behavior. Review
remediation is recorded by `875d71f` (`fix(identity): finish scratch spelling
transition`), `103719c` (`test(identity): require explicit legacy spelling
review`), and the following status commit.

The clean `0.2.0` transition now uses `SYNTH_DICOM_GEN_*` for all 26
product-controlled compile-time, runtime, test-consumer, SDK, CI, and release
environment spellings inventoried at this boundary. No old-name environment
alias is consumed or emitted. The M6 generated-file qualification selector is
now `SYNTH_DICOM_GEN_M6_SEGMENTATION_FIXTURE`. Transaction staging, release
construction and verification scratch roots, CI corpus roots, optional-codec
temporary directories, composition streaming files, conformance work roots,
and media staging use `synth-dicom-gen` prefixes; 14 removed prefix families
are locked to their replacements.

`product/spelling-transition-2026-09-02.json` is the machine-readable
transition contract. `scripts/check-spelling-transition.py` scans every
tracked non-Markdown text file outside its self-referential checker fixtures,
rejects every removed environment or path spelling, requires every replacement
to remain discoverable, and permits retained spellings only through 507 exact
path, token, class, count, owner, and reason records. Independent detectors
reject unapproved old-name environment access and old-name production
path-building even if a summary snapshot is regenerated. `--bootstrap` prints
a review-only record proposal and never mutates or authorizes the inventory.
The accepted snapshot contains 871 occurrences with
SHA-256
`086c5823091c9d8a50271401ca6c35613d618ef6590c59b94d918922e681525c`:
188 DICOM payload identifiers, 271 payload/schema compatibility fixtures or
historical evidence occurrences, 127 locked Python module/backend identities,
143 qualified-adapter environment occurrences, and 142 qualified-adapter or
internal-test-fixture labels. Four focused Python regressions prove an
arbitrary `DTS_NEW_PRODUCT_ROOT` access and `dts-new-product-staging` path stay
rejected after ordinary summary regeneration, enforce exact retained counts,
and prove bootstrap does not mutate the approved inventory. The checker and
its tests run in the unconditional Fast workflow and route through the Fast
contract bundle.

The 12 retained qualified-adapter variables remain unchanged because their
executable fingerprints, environment fingerprints, dependency locks, and
external qualification evidence bind those exact interfaces. This includes
the native generation backend, highdicom, dicom-validator, WSI reconstruction,
LittleCMS, and PixelMed. Adversarial review established that the M6
segmentation-fixture selector is product-controlled rather than an external
adapter, so code, tests, inventory, and current documentation now use
`SYNTH_DICOM_GEN_M6_SEGMENTATION_FIXTURE`. Renaming the 12 adapter
interfaces remains explicitly unavailable until their scheduled external
qualification is rerun; R3.4 does not infer equivalent evidence under a new
name. DICOM payload/manufacturer/device/private-creator values, compatibility
reader/schema identities, security fixture DNS, and locked Python
package/module/backend identities likewise remain intentionally unchanged.

The initial proportional verification remains recorded above. Remediation
verification additionally passed: private-cache Python syntax compilation;
all four spelling-checker adversarial tests; all 17 change-routing unit tests;
the exact 871-occurrence checker and 507-record allowlist; the R2 ownership
checker at 22 targets, 262 entry groups, and 1,375 entries; JSON parsing;
Fast-workflow YAML parsing; `cargo fmt --all -- --check`; and
`git diff --check`. Focused default-feature-independent Rust evidence passed
15 codec unit tests, the composition streaming regression, five media-runner
staging/cleanup tests, and the exact Fast workflow/spelling assertion in 1.29
seconds. Routing maps the checker, inventory, and adversarial test to the
unconditional Fast contract and defers release-candidate evidence explicitly.

No heavyweight body, provider timing qualification, feature-specific codec
body, broad all-target suite, Nightly, release-candidate body, external
runtime, conformance tool, interoperability adapter, remote workflow, remote
rename, or release ran. The remediation changes only scratch/interface names
and checker evidence; it does not claim optional-codec, provider, independent
conformance, Nightly, or release qualification. R3.4 and the aggregate R3 gate
pass at this bounded evidence level.

### R3.3 — current documentation and historical evidence boundary

**State:** complete on 2026-09-02; aggregate R3 is complete after R3.4

**Commits:** `2548da8` (`docs(guides): migrate installed product usage`),
`b6678cb` (`docs(adapters): distinguish retained qualified spellings`), and
the following governance/status commit that records this evidence.

The pre-edit audit classified 46 Markdown surfaces: 16 current true-brand
operating guides, seven current adapter or R3.4-boundary guides, 22 immutable
dated historical/evidence documents, and this mixed active migration record.
The 23 current surfaces now use the `synth-dicom-gen` executable, repository,
archive, and `synth_dicom_gen` SDK identity at product version `0.2.0`.
Product-controlled environment and temporary-path examples use the exact
`SYNTH_DICOM_GEN_*` and `synth-dicom-gen` mappings. The packaged installation,
automation, examples, release, SDK, assembly, composition, compatibility, and
consumption guides no longer instruct consumers to invoke or link the old
product. The installation guide explicitly scopes the sole remaining current-
guide old-name mention to the immutable `0.1.0` historical candidate and says
that no renamed `0.2.0` archive is yet qualified; no release evidence was
inherited across the identity change.

All 22 immutable historical/evidence documents were left byte-for-byte
untouched. Their old artifact names, hashes, URLs, commands, and qualified
candidate claims remain evidence for their exact revisions and targets only.
ADR 0003 retains its intentional old/new contrast, and DICOM payload,
manufacturer, device, private-creator, and `DTS_*` fixture identifiers were not
treated as product branding. The active generation guide now enumerates the
12 retained qualified-adapter environment variables and labels the locked
`dts_highdicom_backend`, `dts_dicom_validator_adapter`,
`dts_wsi_reconstruction`, and `dts-wsi-reconstruct` identities as adapter
provenance rather than product aliases. Renaming those external interfaces
remains unavailable until their independent qualification is rerun.

The spelling inventory has no pending current-documentation exception. Before
the subsequent R3.4 audit correction, its post-R3.3 retained snapshot contained
884 occurrences with SHA-256
`997a86933b6e326a26751b1ec48d018b49e166972fe40af0d74109ddb7a4b0d6`:
188 DICOM payload identifiers, 271 payload/schema compatibility fixtures or
historical evidence occurrences, 127 locked Python module/backend identities,
145 qualified-adapter environment occurrences, and 153 qualified-adapter or
test-fixture labels. That snapshot is not terminal R3.4 evidence and must be
rebound by the owning remediation after removing the incorrectly retained M6
selector. A focused documentation regression rejects old binary,
archive, repository, and Rust-crate instructions in all 23 current surfaces,
allows the single explicitly scoped installation-history label, and requires
historical candidate and ADR sentinels to remain present.

Proportional verification passed: the exact spelling checker and snapshot;
the ownership checker at 22 targets, 262 entry groups, and 1,375 entries after
the documentation regression was folded into an existing test entry;
`cargo fmt --all -- --check`;
`git diff --check`; all four installed-documentation tests; the P7 composition
documentation test; focused README, corpus-consumption, external-codec, UV
conformance, release-process, and composition-boundary assertions. The
documentation regression passed as part of all four installed-
documentation tests. No heavyweight body, provider or external-runtime
qualification, feature matrix, broad all-target suite, Nightly, release-
candidate body, remote operation, or release ran.

### Final R3 gate — packaged external consumer

**State:** passed on 2026-09-02

The initial exact `cargo package --locked` attempt tried to access the package
index and was interrupted after 97.95 seconds when sandbox DNS was
unavailable; it is recorded as an unavailable attempt, not a pass. The bounded
offline retry set `CARGO_TARGET_DIR` to
`/private/tmp/sdg-r3-consumer.ygbBfY/target`, disabled incremental compilation
and test debug information, and ran `cargo package --locked --offline`.
Package construction and Cargo's package verification passed in 23.99 seconds,
covering 833 packaged files (14.8 MiB uncompressed and 2.4 MiB compressed).

The resulting `synth-dicom-gen-0.2.0.crate` was 2,500,992 bytes with SHA-256
`9bc4cdaa357ce6f87a8dcce61d2f554d45f7a795a5c5f09fbb93017e65f79fe6`.
It was extracted once into
`/private/tmp/sdg-r3-consumer.ygbBfY/package/synth-dicom-gen-0.2.0`, with all
833 package files present. With `SYNTH_DICOM_GEN_SDK_PACKAGE_ROOT` bound to
that extracted package, the exact command
`cargo test --locked --no-default-features --test release_ci__nonfast
sdk_external_consumer::` compiled and ran the clean side project through only
the supported `synth_dicom_gen::sdk` facade. Exactly one test passed, 13 were
filtered, in 47.54 seconds total with 29.86 seconds reported for the test
body. The side project used the extracted package path rather than this
repository checkout, closing the R3 no-old-repository-path gate.

The isolated target occupied 957,332 KiB (980,307,968 allocated bytes). The
exact temporary root was removed after measurement and the Git worktree
remained clean. No broad, heavyweight, provider, external-runtime, Nightly,
release-candidate, remote, or release body ran; this package-and-consumer proof
does not qualify a `0.2.0` release artifact or target.

### R4.1 — immutable `EngineResources` boundary

**State:** complete on 2026-09-02; aggregate R4 remains in progress

**Commits:**

- `262df91` — `refactor(resources): introduce EngineResources`
- `46d8e96` — `test(resources): qualify the EngineResources boundary`
- `d77f808` — `fix(resources): capture explicit resources immutably`

The public `ProductResources` implementation and module were replaced by
`EngineResources`, and the build-generated table is now
`EMBEDDED_ENGINE_RESOURCES` in `embedded_engine_resources.rs`. Generation,
composition, discovery, conformance, assembly, the SDK, and the CLI consume the
new abstraction. A source audit rejects the old module, type, and module path;
no compatibility alias was introduced.

The embedded set covers schemas, templates, standards and capability locks,
generic conformance and generation-provider resources, the security fixture
lock, the CLI error registry, and the small color-profile asset. Build-time
discovery rejects symbolic links and non-regular fixed inputs and tracks both
directories and files for rebuilds. Logical lookup rejects empty, absolute,
backslash, parent-traversal, and unknown paths. An explicit root rejects root,
directory, and file symbolic links and non-regular components, verifies the
complete set before returning a handle, and captures the verified bytes
immutably. Later filesystem mutation therefore cannot cross the constructor
integrity boundary or race snapshot materialization. Embedded operation and
the captured explicit operation are independent of the checkout location.

R4.1 deliberately preserves the existing machine-field spelling
`product_resources`, resource-set version `1.0.0`, and the pre-R4 identity of
240 resources with SHA-256
`dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`.
An archived build of exact pre-R4 revision `e061108` produced that same
version, count, and digest. `ENGINE_RESOURCE_SET_MEMBERSHIP` explicitly reports
`TransitionalMonolithic`, and regression coverage requires both
`cases/registry.json` and `Cargo.lock` to remain visible in this temporary set.
Those corpus-definition and package-membership removals belong to R4.3/R4.4;
the preserved serialized field and identity are compatibility evidence, not a
claim that separation is already complete.

Focused proportional verification passed after the final hardening change:

```text
cargo test --locked --no-default-features --test schema_resources__subsystem
result: 86 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo test --locked --no-default-features --test cli_sdk__nonfast sdk_facade::
result: 7 passed; 0 failed; 0 ignored; 0 measured; 131 filtered out

cargo test --locked --no-default-features --test cli_sdk__nonfast version_cli::
result: 3 passed; 0 failed; 0 ignored; 0 measured; 135 filtered out

cargo test --locked --no-default-features --test composition__subsystem standalone_compose_resources::
result: 1 passed; 0 failed; 0 ignored; 0 measured; 83 filtered out

cargo test --locked --no-default-features --test release_ci__fast ci_release_gates::heavy_workflow_retains_nightly_matrix_and_immutable_release_gate -- --exact
result: 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out

cargo check --locked --no-default-features --lib --bins
result: passed

python3 scripts/check-test-ownership.py
result: passed; 22 targets; 262 entry groups; 1,375 entries

python3 scripts/check-spelling-transition.py
result: passed; 870 retained occurrences; snapshot SHA-256 ecff895f74165daa5d9d72ee145ee746f7f799a6028f12916fd7c010dfaa44b1

python3 -m unittest tests/test_change_test_routing.py tests/test_spelling_transition.py
result: 17 passed

cargo fmt --all -- --check
git diff --check
result: passed
```

The routing dry run selected only the `schema_resources__subsystem` ordinary
bundle plus unconditional Fast coverage and reported release-candidate
packaging evidence as deferred. No schema or manifest identity split,
external `CorpusDefinitionBundle`, corpus/Cargo removal, lazy materialization,
heavy body, codec/provider runtime, external adapter, Nightly,
release-candidate body, remote operation, or release ran. R4.2-R4.5 and the
aggregate R4 gate remain open.

## Measurements

| Measurement | Baseline command/revision | R0 value | Terminal value | State |
| --- | --- | ---: | ---: | --- |
| CI wall time by verification class | Run `33491521696`; class projection documented in the dated baseline | 132m15s full observed interval; no class is independently routed | — | Baseline recorded |
| Billable runner time by verification class | Exact API job durations; failed attempt plus retry included | 175m45s actual; 180 per-job rounded minutes | — | Baseline recorded |
| Largest local target-directory size | `CARGO_TARGET_DIR=/private/tmp/dts-r02-target.xAApSK cargo test --locked --all-targets --no-default-features --no-run` | 8,013,463,552 bytes after 72.29s | R2.2 comparable clean run: 1,259,962,368 bytes after 33.87s | 6,753,501,184 bytes / 84.28% smaller; exact directory removed |
| Integration-test target count | Cargo metadata plus top-level `tests/*.rs` at `65a296b` | 186 integration targets; 188 Cargo-reported harness executables | 20 explicit integration targets; 22 executables including lib/bin | R2.2 target-count gate passes |
| CI/generated artifact count and size | Actions API for run `33491521696` | 1 upload, ID `9798112659`, 9,929,745-byte ZIP; no uploaded corpus | — | Baseline recorded |
| Representative generator Fast PR | No independent class at `65a296b` | Not independently measurable; every PR selects the full graph | Remote run `33581809536`: 123s job interval, 116 build-work seconds, 739,602,432-byte target, four smoke artifacts occupying 122,880 allocated bytes | R1 gate passes 4-GiB and 15-minute budgets; target is 68.1% smaller than the pre-R1.5 Fast measurement and 90.8% smaller than the differently scoped R0 all-target tree |
| R2.4 bounded Fast plus selected engine route | R0 clean no-default all-target target at `65a296b` | 8,013,463,552 target bytes; 188 Cargo-reported harness executables | 68.57s wall; 705,949,696 target bytes; four test binaries plus one product binary; one 36,864-byte allocated log artifact | 7,307,513,856 bytes / 91.19% smaller; 4-GiB Fast disk budget passes; exact temporary target removed |
| R2 warm invalidation, Fast plus selected engine route | Same R0 clean tree, conservatively used because R0 retained no separate warm-tree capture | 8,013,463,552 clean target bytes | Initial 66.90s and 705,957,888 bytes; post-touch 58.82s and 706,031,616 bytes; 994 files; zero incremental-cache files | Warm invalidation added 73,728 bytes / 0.0104%; terminal tree remains 7,307,431,936 bytes / 91.19% smaller than the R0 clean lower bound; exact temporary target removed |
| Representative corpus PR | Repository does not yet exist; embedded corpus edit selects full graph | Not independently measurable | — | Explicit boundary recorded |
| Representative viewer PR | Viewer repository not in current scope | — | — | Not measured |
| Nightly and release-candidate cost | No separate Nightly/RC trigger; run `33491521696` is exact candidate evidence | Nightly not independently measurable; provider/default/release critical chain 123m53s | — | Explicit boundary recorded |

R2.2 now has a comparable local linking-cost reduction. CI class-specific
terminal costs still require the later routing and R9.6 measurements; the R0.2
baseline alone is not proof that those budgets have passed.

## Blockers and authority boundaries

- The location, remote, and creation authority for `dcmview-test-corpus` have
  not been supplied. No persistent corpus repository, release, or remote has
  been created. The authorized disposable R1 probe was closed and its branch
  deleted after evidence capture.
- The R3.2 external-consumer inventory is complete and supports the clean
  `0.2.0` rename without a product alias. Public evidence found only five
  TomeVault repository-metadata mirrors, not a supported package, SDK,
  executable, or archive consumer. Authenticated GitHub code search was
  unavailable, so private and unindexed consumers remain explicitly outside
  the proven search scope; a later real product consumer requires a new tested
  compatibility decision rather than an inferred alias.
- No generator or corpus release target is currently qualified under the new
  names. Existing macOS arm64 and Linux x86_64 evidence remains scoped to the
  immutable historical candidate named above.

## Terminal acceptance matrix

| Gate | State | Required terminal evidence or current blocker |
| --- | --- | --- |
| Repository boundary | Not run | Must prove no generator dependency on dcmview and no corpus use of unsupported modules or sibling paths. |
| Naming and compatibility | Passed | R3.1-R3.4 prove package `synth-dicom-gen` `0.2.0`, library `synth_dicom_gen`, the sole binary and version-derived archive/discovery identities, clean no-alias product environment, renamed production scratch paths, dual readers only at the four approved compatibility boundaries, current operating guides under the new identity, immutable historical evidence under the old identity, and 12 explicitly evidence-bound retained adapter environments. The exact 2,500,992-byte packaged crate (`9bc4cdaa357ce6f87a8dcce61d2f554d45f7a795a5c5f09fbb93017e65f79fe6`) passed Cargo verification and a clean extracted-package SDK consumer without an old repository path. A qualified `0.2.0` release remains correctly separate in the packaging-and-release row. |
| External corpus contract | Not run | Versioned external definition loader and CLI/SDK generation contract are not implemented. |
| Identity separation | Not run | Engine, toolchain, template/provider, schema, corpus, and external-runtime identities are not independently projected. |
| Smoke migration | In progress | The R0 parity baseline passes for the current embedded smoke slice; R6 repository generation and comparison have not run. |
| Complete migration | Not run | The current repository still owns the complete embedded corpus. |
| Fast development | In progress | R1-R2 record the representative 123-second generator Fast PR, 739,602,432-byte target, bounded smoke artifacts, 20 integration harnesses, 84.28% comparable clean-tree size reduction, 91.19% bounded Fast-route reduction, and warm-invalidation evidence while preserving separately invocable heavy coverage. Representative corpus and viewer PR measurements remain absent, and R9.6 still owns terminal verification-class cost measurements, so this terminal row does not yet pass. |
| Heavy qualification | In progress | Nightly/manual/release routing now selects the six explicit heavy entries once, but no exact terminal Nightly or release-candidate run has executed at the separated-repository boundary. |
| Artifact consumption | Not run | No keyed downstream corpus artifact or default viewer-consumption workflow exists. |
| Packaging and release | Not run | Neither renamed repository has an independently qualified release procedure or exact candidate record. |
| Documentation | In progress | R3.3 passes the generator naming boundary: 23 current operating surfaces use the renamed product, 22 immutable historical/evidence documents retain exact old identities, and provenance-bound adapter spellings are explicit. Terminal cross-repository operating, migration, and qualification documentation requires the later R4-R9 separation and does not yet pass. |
| Hygiene | Not run | Terminal clean-worktree, formatting, schema, diff, artifact, secret, and package-inventory checks have not run in both repositories. |

The migration is not complete until every R0-R9 gate and every row above
passes, both repositories are clean and independently usable, measured cost
reductions are recorded, and exact qualification evidence exists for every
claimed target and scope.
