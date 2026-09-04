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
| R4 — split immutable resources and corpus definitions | Complete | R4.1, R4.2, R4.3, R4.4, R4.5 | At the accepted R4 checkpoint, EngineResources v2 owned 74 immutable members and its private lazy lease materialized a 254-file transitional physical closure once per shared context. Those checkpoint measurements remain historical; current schema additions and exact identities are recorded in the latest dated R5 entry below. Cases/Cargo remain excluded from authoritative engine identity, and exact legacy 240-member v1 provenance remains reconstructable. |
| R5 — add supported external corpus API | In progress | Input substrate, batch execution, reporting, accepted SDK/CLI; discovery slice pending review | SDK `ca571fb..aa7fcad` and CLI `aa7fcad..a18d149` are independently accepted. The dated R5.5 entry records destination-free supported inspection and capabilities3, pending review. Independent installed/cross-repository acceptance remains required. |
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
- `73a309a` — `fix(resources): bound explicit resource capture`
- `de25d9a` — `fix(build): reject unsafe embedded resource paths`
- `35c9456` — `test(resources): lock the transitional v1 oracle`

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

A post-completion adversarial review found that the first R4.1 implementation
used path metadata checks followed by `fs::read`: it captured bytes only after
verification but did not bound caller-controlled allocation and left metadata/
open/read rename races. Commits `73a309a` and `35c9456` supersede that reader
claim. On supported macOS and Linux, the constructor now opens the root once,
checks the pre-open and descriptor device/inode identity, resolves every child
with descriptor-relative `openat`, and uses `O_NOFOLLOW` on every component.
Final inputs also use `O_NONBLOCK`, must be regular files, and must match their
embedded expected lengths. The constructor checks every length and the exact
total before content allocation or reading, then limits each read to expected
length plus one and rechecks the open descriptor and root identity afterward.
Oversized and undersized inputs retain the established
`evidence.integrity.failed` classification; roots or components with invalid
types, FIFO/special files, and unstable paths fail with stable invalid-resource
codes. Exact-size sparse or otherwise altered content remains bounded and then
fails the complete digest check.

Build generation now validates each initial scan root as a real directory and
each fixed input as a regular file while rejecting any symlinked or
non-directory ancestor. Its testable validator is exercised with a symlinked
scan root, symlinked fixed input, symlinked ancestor, and non-directory scan
root. This is a trusted-checkout preflight rather than an atomic filesystem
snapshot: a separate local process with write access could still mutate a
build input after validation and before Rust evaluates `include_bytes!`.
Runtime integrity remains fail-closed through the exact oracle below, but
eliminating that trusted-build interval would require a descriptor-backed
build staging design outside R4.1. No caller-controlled runtime read retains a
known path-race or unbounded-read gap on the supported macOS/Linux targets.

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

The version, count, and digest are executable constants checked by
`verify_integrity`, not documentation-only measurements. Embedded verification
must satisfy all three before it can pass, and explicit verification first
requires that exact embedded oracle. R4.3/R4.4 therefore must update or replace
the v1 oracle deliberately when they project separate identity domains.

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

The source/test-only routing dry run selected the
`schema_resources__subsystem` ordinary bundle plus unconditional Fast coverage
and reported release-candidate packaging evidence as deferred. A combined dry
run also classified `build.rs` under the conservative `global-build` rule and
listed all configured ordinary commands plus their deferred classes. Per the
R4.1 review scope, that broad list was inspected but not executed; the exact
focused commands above were run instead. No schema or manifest identity split,
external `CorpusDefinitionBundle`, corpus/Cargo removal, lazy materialization,
heavy body, codec/provider runtime, external adapter, Nightly,
release-candidate body, remote operation, or release ran. R4.2-R4.5 and the
aggregate R4 gate remain open.

### R4.3 — first bounded identity-domain discovery slice

**State:** discovery/version/capabilities slice complete on 2026-09-02;
generated/composition/assembly/coverage/release manifest slices and aggregate
R4.3 remain in progress

**Commits:** `5a841cc`, `10ad60e`, `8d8e996`, `d6d52b2`, `97322b1`,
`ff0e7db`, `d052e5c`, and `dfee112`.

Version and capabilities producers now emit schema `2.0.0`. Their shared
identity projection has separately framed schema `1.0.0` domains for engine,
schema set, template catalog, provider catalog, toolchain, standards, and
execution. The canonical content-domain frame includes the identity-domain
schema version, identity version, domain, and each path-sorted member's logical
path, byte length, and SHA-256. The default inspection context emits an
explicitly absent corpus identity. A corpus identity can be projected only
from a successfully loaded `CorpusDefinitionBundle`; callers cannot fabricate
one from public identity fields. External-runtime identities remain empty
because capability declarations and environment-variable names are not
per-invocation executable fingerprints.

The installed default projection measured these exact content identities:

| Domain | Members | Bytes | SHA-256 |
| --- | ---: | ---: | --- |
| engine | 3 | 20,643 | `4268d9216842aaaca8e9ea1d3fd8e8538d7d02124deccf8cd17b63c180b86276` |
| schema set | 40 | 796,505 | `713c6d218810e6ea4a6ef62403fc0e94f5624b950e928257c452651afb661f46` |
| template catalog | 3 | 145,871 | `ecd3875e89fbcba17e4d183b524237212674fd3c8ff7f20dc20900589926f5da` |
| provider catalog | 16 | 229,788 | `a9241cab52ceee7332cb71a710e0fbb9680fcf6ca37e3111234bf317708cede5` |

The default no-feature aarch64 macOS build measured Cargo.lock SHA-256
`4aa4b6c94043fb2f236ec888ac9b253f2ff451b666464609f81f82aaac6d8a4d`,
toolchain SHA-256
`936108d136db532e6da0359975cfba3eb456e8cda12191cdf14fe0e6bc03c530`,
standards-lock SHA-256
`823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559`,
and execution SHA-256
`162374c6231ba42c2453b04446b5bdccec5f0f04d1261db5f714ad24240e6bdd`.
Toolchain and execution identities intentionally include target and enabled
features and therefore are build-context identities rather than cross-target
constants.

The v2 producer temporarily retains top-level `product_resources` so this
slice does not preempt the R4.4 removal boundary. The v2 schema makes that
locked compatibility field optional: the current producer emits the exact
240-resource, `dc61cc...` v1 snapshot, while R4.4 may stop emitting it without
changing v2 meaning. Migration context separately retains its origin so an
embedded versus explicit resource root remains observable while all content
digests remain relocation-stable. The two new v2 schemas are directly embedded
in the schema-set domain and excluded from the transitional 240-resource
oracle.

Capabilities advertises `result_schemas` as current producer versions: version
and capabilities each list only `2.0.0`. Its separate
`result_schema_validation` field lists `1.0.0` and `2.0.0` only where committed
JSON Schema consumer fixtures exercise both. This is not a v1 producer
selector and does not claim a typed Rust v1 reader or synthesize split
subdigests from a legacy monolithic document. The v1 schemas remain unchanged;
their fixtures validate without any `identity_domains` field.

Adversarial tests prove that an unknown installed path fails classification,
that every current installed member maps to one centralized domain, and that
embedded and relocated explicit resources have identical content domains with
only legacy origin differing. A second proof loads the committed corpus
fixture through the fail-closed R4.2 loader, changes an evidence document and
its verified descriptor, reloads it, and observes that only
`corpus_definition` changes; engine, schema, template, provider, toolchain,
runtime, standards, and execution identities remain equal.

Focused verification passed:

```text
cargo test --locked --no-default-features --lib identity::identity_domain_tests::
2 passed; 494 filtered

cargo test --locked --no-default-features --test cli_sdk__nonfast version_cli::
3 passed

cargo test --locked --no-default-features --test cli_sdk__nonfast capabilities_cli::
3 passed

cargo test --locked --no-default-features --test schema_resources__subsystem
86 passed

cargo test --locked --no-default-features --test schema_resources__fast schema_artifacts::committed_schema_files_compile
1 passed; 72 filtered

python3 scripts/check-test-ownership.py
passed: 22 targets; 264 groups; 1,396 entries; historical 20 integration targets, 186 sources, and 879 entries unchanged

cargo check --locked --no-default-features
cargo fmt --all -- --check
git diff --check
passed
```

The fail-closed route now owns `src/identity.rs` and `src/discovery.rs`
through the identity bundle instead of routing discovery through embedded
corpus. Each v2 version/capabilities schema path deliberately overlaps the
identity and generic schema rules, so a contract-only change selects the exact
two-entry identity module, bounded version and capabilities CLI filters, full
schema subsystem, and unconditional Fast coverage. Fourteen routing unit tests
bind both schema paths to that exact four-command selection; the routing
configuration SHA-256 is
`79a7246ad94642ed3e842bb483132ec655faefe8f85a5cee7e7ca15f70c4c8c7`.
An earlier unnecessarily broad CLI/SDK harness run exposed the pre-existing
nonsquare spatial hash mismatch; it is outside this slice and the focused
affected filters pass. No manifest schema or producer, R4.4 removal, R4.5
materialization, R5 generation API, feature/provider runtime, external adapter,
heavy, Nightly, release-candidate, remote, or release body ran.

### R4.3 — bounded curated-generation identity slice

**State:** curated-generation manifest/result slice complete on 2026-09-02;
composition, assembly, coverage, and release manifest slices plus aggregate
R4.3 remain in progress

**Commits:** `1a1200e`, `3041bc6`, `71e4c62`, `4959d56`, `0566717`,
`62decf0`, `ec342e2`, `242883c`, `ec529fc`, `1d59eed`, `7bec254`, and
`5e8bfa8`, `f80564b`, `507b69f`, and `fe9b369`.

The curated-generation producer now emits manifest `1.0.0` and generation
result `2.0.0`. The manifest's `identity_projection` is an honest projected
state containing independent engine, schema-set, template-catalog,
provider-catalog, toolchain, standards, and execution identities. Its corpus
state is `transitional_embedded_unverified` with a null identity for today's
embedded generation path. The pure internal projector accepts only a
successfully loaded `CorpusDefinitionBundle`; perturbing one verified corpus
evidence document changes only the corpus identity. No CLI or SDK corpus
selector was introduced ahead of R5.

External runtime identities are invocation evidence rather than capability
declarations. A runtime is projected only from successful typed provider,
codec, evidence-tool, or materialization-service evidence carrying an exact
executable SHA-256. The built-in smoke path correctly emits an empty runtime
list. Focused tests cover each of those four evidence branches and prove failed
or executable-free evidence is omitted; the identity projector independently
sorts, validates, and rejects duplicate or malformed runtime identities.

The producer still emits the exact transitional top-level resource identity
and the future-safe legacy provenance record: version `1.0.0`, origin
`embedded`, 240 members, and SHA-256
`dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`.
The two new schemas are directly included in the schema-set domain but
excluded from that frozen transitional oracle. The schema set is now 42
members, 803,698 bytes, SHA-256
`6466d33b404c6ade533daa3b6478dbbb2b2e3ea1d64fdeb80c3c4bd131036357`.
R4.4 may remove the optional top-level monolith without changing manifest
1.0's split identity meaning.

The frozen manifest `0.2.0` and `0.3.0` schema was not changed. One shared
fail-closed loader selects those versions, current curated `1.0.0`, composition
`0.4.0`/`0.5.0`, or structural-assembly `1.0.0` by explicit discriminator and
version before any library, CLI, or SDK validation/report semantics run.
Unknown kinds or versions fail, and curated `1.0.0` fails when its split
identity is absent or malformed. Real generated-root fixtures prove curated
`0.2.0`, `0.3.0`, and `1.0.0` through every CLI and SDK validate/report path;
legacy inputs are never assigned synthesized split identities. A committed
generation-result `1.0.0` fixture remains valid under its frozen schema;
capabilities separately advertises current generation producer `2.0.0` and
validated consumer versions `1.0.0` and `2.0.0`.

The package-version bump initially exposed an R3 regression in the locked
smoke bytes: the SC Implementation Class UID and DICOM Software Versions tag
were derived from the release version. Commit `71e4c62` freezes that legacy
DICOM implementation identity independently of package reporting. Before the
repair, the seed-1 hashes were `9d74d59a...`, `0c59e2f2...`, and
`61b29e8d...`; after the repair, the exact R0 identities are restored:

| Case | Bytes | SHA-256 |
| --- | ---: | --- |
| `classic/sc/mono1_u8_explicit_le` | 926 | `76dc5208b139899fcb87bbf7ec9edf1a323000a91c4015de9ef8bde7bd344ecc` |
| `classic/sc/mono2_u8_explicit_le` | 926 | `fce766bcbb4b4aa79cfb3fa0c3b5e4ef888b11c0708fad713b9cde8d41ec6a15` |
| `classic/sc/rgb_planar0_explicit_le` | 938 | `33de9448509431fda27005cbf83c79977f1c3ebadb669ae1dedf1a225742f3c5` |

### R3 output-version compatibility repair — 2026-09-02

The SC repair above exposed the same release-version coupling in the remaining
unchanged built-in byte-stable providers. The completed audit classifies all
162 implemented `byte_stable` cases by provider: mutation 15, classic 32,
encapsulated 2, enhanced 7, exceptional SC 1, metadata SC 7, presentation 4,
quantitative 5, registration 2, RT 6, SC 66, SR 3, stress CT 1, stress SC 4,
waveform 2, and WSI 5. A fail-closed source inventory now derives this set from
the registry and recipe mapping and rejects `PACKAGE_VERSION` or
`CARGO_PKG_VERSION` coupling in every byte-stable recipe and the built-in
composition default.

Commits `400e2c4`, `4b4a8d4`, `c120d89`, `f51f683`, `713de4b`, `6ee6b15`,
`e0afd29`, `90298f4`, `562356f`, `e7d9314`, `b8deab1`, and `f5aa968`
centralize `BYTE_STABLE_OUTPUT_VERSION = "0.1.0"` and apply it only to
output-bearing DICOM Implementation Class UID and Software Versions producers
and their paired validators. The earlier SC repair is `71e4c62`. Product,
discovery, manifest, built-in runtime, package, and release evidence continues
to use `PACKAGE_VERSION = "0.2.0"`. Five semantic-stable external cases remain
intentionally product-versioned: two `external.highdicom_sr_import_plan` cases
through `semantic_context` and three
`external.quantitative_import_plan` cases through `quantitative_context`.

The exact frozen Implementation Class UID is
`2.25.93442075376351194778596039619060852790`; the incorrect release-coupled
`0.2.0` derivation was
`2.25.191308153041538677130862129189427757183`. Representative native plan
locks cover every affected direct provider family:

| Provider family | Exact canonical plan SHA-256 |
| --- | --- |
| SC | `71792e2a52c1bb1b0ef483324922a4c7c7613d0ae9535f088f84601c33eec32a` |
| metadata SC | `ba18d5029e477f943d6765a9d706cef3c5f437a24c5346fa4f64c8adafaf2040` |
| classic | `9656dc07538da6542157492b28bbd1c5bb9f27a7b86d73e522440885aa8c6430` |
| enhanced | `8d970294e2143f6f55d6c134d5fea21366428faaf33c8ee368f48a2ab9e3dca7` |
| WSI | `be80772ebab7462896244117b6581caf50a541e0eb6aa9f030c97e3bd217b1ab` |
| registration | `7e3f825b6923734077281143600098003312c6acecf73a69828853a0ddb6c80c` |
| presentation | `c032ecbd1179acd98e356a5d4a036fb6fd33ca313c841d91dd8f381efaec647c` |
| waveform | `1848a8c90fee9191d66270822e9fb8fd4f950169182c79688f7ae6a56e8e717a` |
| encapsulated | `f46f37b1f643c4a9b53814bde2926aeb372f7ebad1e822ef174ec2426f624967` |
| quantitative | `002629859f739a4cbc4699a9f1550b095f29bc90e869cde201f3481ec00c0915` |
| SR | `5438c54932da7b4de3e733f385854e2538a350507fb7e9195097f1d6e32d6fc6` |
| RT | `e8210ca6e1c6864d259d1817c18c4102ec3db65e2b52cdc7c756a1426594c17b` |
| stress CT | `5b36d3cf3f930d803a07d118fe9023f3aa80eecd032fc75e6fe7d1c501be00b9` |
| stress SC | `6ad82a4b0c076e95037a30a69f85914c40792b6e2c536498f7d07bd6d8068f8f` |

Commit `409ee2d` additionally locks the standalone smoke corpus plan at
`2a18d78c956c2873755c30e989e90c900ece38bfd5ca87947565c19cfed8127c`
and its mono1, mono2, and RGB resolved plans at
`71792e2a52c1bb1b0ef483324922a4c7c7613d0ae9535f088f84601c33eec32a`,
`394035d0ae5aa616041b4c140d6e20b0861e5074efa3177059cd533e5c5060ef`,
and `aa32a98e6145116050f31d35739423619c84b096b65fa1d38e332396b888853e`,
respectively. The three exact payload hashes and
sizes remain those in the table above.

The all-profile R0 comparison was performed mechanically against recorded R0
revision `65a296bbb489fcaaff22e38fa35036f0805ccab6`, not inferred from a new
expected value. Pretty-serialized seed-7 plans at R0 and current were identical:
4,071,121 bytes and SHA-256
`c85d84dc77197042e42bbe0ea072128853c7c185df3dcce5ef57cc17754b0bae`.
Bounded ordinary generation emitted the same 153 paths and zero differing
payloads. Exactly two manifest-entry evidence classes changed: 59
`/pixel_data/codec/version` values changed from `0.1.0` to the required product
version `0.2.0` through the pre-existing `built_in_tool` runtime identity when
commit `86d0298` bumped the package, and one validation message changed in
`c120d89` to describe the byte-stable contract accurately. Commits `e88202f`
and `710f56f` therefore lock both the exact current projection digest
`5d7a02ef873833dba33e9feb56330eabad709215c25de7c6caf0aa61986ab21e`
and, after fail-closed normalization of exactly those 59 product-evidence
versions and one message, the restored R0-compatible digest
`a50de8b288b3543876e4e58bcc2b435f41b81e84201e78508f093e894b8f4c36`.
Smoke and legacy require zero normalizations and retain their exact historical
digests `798319444e6a0cd0b34607ebee9f4b2d88987e9c8cd0bb2e4a95480aa4f6a68e`
and `162112cb5b497bce5111a5f1a95d003f63b67ca444f37931b78f097fda86a864`.

Focused ordinary verification passed:

```text
cargo test --locked --no-default-features --test corpus_generation__subsystem unified_generation_spine_audit::
7 passed; includes the 162-case fail-closed inventory and 14-family exact plan assertions

cargo test --locked --no-default-features --test corpus_generation__subsystem curated_generate_integration::ordinary_generate_preserves_locked_curated_history_for_public_profiles -- --exact
1 passed; exact current and R0-compatible smoke/all/legacy projections

cargo test --locked --no-default-features --test schema_resources__subsystem standalone_generate_resources::generate_uses_embedded_resources_from_an_unrelated_working_directory -- --exact
1 passed; exact smoke corpus, resolved-plan, and three payload digests

cargo test --locked --no-default-features --test engine__subsystem corpus_plan::
22 passed

cargo test --locked --no-default-features --test composition__subsystem template_default_recipes::
4 passed

python3 scripts/check-test-ownership.py
passed: 22 targets; 264 groups; 1,400 entries; 20 integration targets, 186 sources, 882 entries

python3 -m unittest tests/test_test_ownership_checker.py tests/test_change_test_routing.py
20 passed

python3 scripts/check-spelling-transition.py
passed: 883 classified retained occurrences; SHA-256 6a56bdc8947beb4cddd1d4b250eee6f682c7cc15c221d57aff842a53357919fa

cargo fmt --all -- --check
git diff --check
passed
```

Every changed path was inspected through `route-changed-tests.py --dry-run`
before its focused command. The terminal routing metadata hashes are config
`9000f04a4318ace7ecb0e97de2d54c6d91bd70ec82ba16beda44b96cea05472e`
and ownership
`26dd5b5f0969f676a0be403fe5352d55200c94b43d603ef6d9754b460af299d8`.

One exploratory ordinary subsystem run after the relevant digest repair was
not acceptance evidence: `cargo test --locked --no-default-features --test
corpus_generation__subsystem -- --format terse` reported 65 passed and 27
failed. The failures are pre-existing R4.3 manifest-v1/schema/routing debt and
were not changed in this compatibility task:

| Exact failing test | Exact observed cause |
| --- | --- |
| `advanced_blending_presentation_state::advanced_blending_vertical_slice_is_byte_deterministic_and_closed` | Frozen schema rejects manifest `1.0.0` and unexpected `identity_projection`. |
| `blending_presentation_state::blending_presentation_state_vertical_slice_is_byte_deterministic_and_closed` | Frozen schema rejects manifest `1.0.0` and unexpected `identity_projection`. |
| `color_softcopy_presentation_state::color_softcopy_presentation_state_vertical_slice_is_byte_deterministic_and_closed` | Frozen schema rejects manifest `1.0.0` and unexpected `identity_projection`. |
| `ct_geometry::core_generates_two_series_in_one_study_and_frame_of_reference` | Frozen schema accepts only `0.2.0`/`0.3.0` and rejects `identity_projection`. |
| `curated_generate_integration::ordinary_generate_routes_curated_sc_through_the_shared_executor_only` | Static assertion still requires removed spelling `prepare_curated_sc_plan`. |
| `deformable_spatial_registration::deformable_registration_vertical_slice_is_byte_deterministic_and_closed` | Generated manifest fails the frozen manifest schema. |
| `enhanced_mr_temporal::enhanced_mr_temporal_position_vertical_slice_is_self_consistent` | Frozen schema rejects manifest `1.0.0` and unexpected `identity_projection`. |
| `enhanced_pet_multiframe::enhanced_pet_vertical_slice_is_deterministic_schema_valid_and_strictly_validated` | Generated manifest fails the frozen locked schema. |
| `enhanced_pet_multiframe::validator_rejects_tampered_enhanced_pet_manifest_contracts` | Schema-first validation rejects tampered orientation because `"Axial"` was expected before the intended semantic assertion. |
| `general_ecg_waveform::general_ecg_vertical_slice_is_byte_deterministic_and_closed` | Test expects manifest version `0.3.0`; producer emits `1.0.0`. |
| `metadata_empty_type2::empty_type2_vertical_slice_is_exact_byte_stable_and_reported` | Generated manifest fails the frozen manifest schema. |
| `metadata_empty_type2::validator_rejects_tampered_empty_type2_contract` | Schema-first validation rejects the tampered value because `0` was expected before semantic inspection. |
| `metadata_private_creators::private_creator_vertical_slice_is_exact_byte_stable_and_reported` | Generated manifest fails the frozen manifest schema. |
| `metadata_private_creators::schema_and_validator_require_private_metadata_and_block_count` | Schema-first validation rejects the tampered count because `3` was expected before semantic inspection. |
| `metadata_sequence_lengths::sequence_length_vertical_slice_is_exact_byte_stable_and_reported` | Generated manifest fails the frozen manifest schema. |
| `metadata_sequence_lengths::validator_rejects_tampered_sequence_length_contract` | Schema-first validation rejects the tampered item because `"Head"` was expected before semantic inspection. |
| `metadata_string_boundaries::string_boundary_vertical_slice_is_exact_byte_stable_and_reported` | Generated manifest fails the frozen manifest schema. |
| `metadata_timezone::timezone_boundaries_are_deterministic_strict_and_reported` | Generated timezone manifest fails the committed frozen schema. |
| `metadata_utf8::utf8_person_name_vertical_slice_is_exact_and_byte_stable` | Frozen schema rejects manifest `1.0.0` and unexpected `identity_projection`. |
| `nm_multiframe::nm_multiframe_vertical_slice_is_exact_byte_stable_and_reported` | Generated manifest fails the frozen manifest schema. |
| `nm_multiframe::validator_rejects_tampered_nm_dimension_contract` | Schema-first validation rejects the tampered dimension because `"0054,0010"` was expected before semantic inspection. |
| `pet_rescaled_activity::pet_rescaled_activity_vertical_slice_is_exact_byte_stable_and_reported` | Generated manifest fails the frozen manifest schema. |
| `pet_rescaled_activity::validator_rejects_tampered_pet_activity_contract` | Schema-first validation rejects the tampered units because `"BQML"` was expected before semantic inspection. |
| `spatial_registration::spatial_registration_vertical_slice_is_byte_deterministic_and_strictly_validated` | Generated manifest fails the frozen locked schema. |
| `twelve_lead_ecg_waveform::twelve_lead_ecg_vertical_slice_is_byte_deterministic_and_closed` | Frozen schema rejects manifest `1.0.0` and unexpected `identity_projection`. |
| `us_multiframe::us_multiframe_vertical_slice_is_exact_byte_stable_and_reported` | Generated manifest fails the frozen manifest schema. |
| `us_multiframe::validator_rejects_tampered_us_multiframe_contract` | Schema-first validation rejects the tampered region because `"ABDOMINAL"` was expected before semantic inspection. |

No Heavy byte-parity/all-profile/WSI/stress body, external provider adapter,
Nightly, release-candidate, or R7 terminal qualification ran. This repair is
ordinary R3/R0 compatibility evidence only and does not promote any deferred
R7 or terminal acceptance row.

#### 2026-09-02 curated manifest 1.0 ordinary-debt closure

Commits `263954e` through `1505f6a` close all 27 ordinary failures recorded
above without changing a production schema or manifest producer. A shared
test validator now selects the frozen curated `0.2.0`/`0.3.0` schema or the
referenced curated `1.0.0` schema graph, including the frozen legacy manifest
resource and version-result-v2 identity resource. The affected verticals now
validate the version actually emitted, and the generation routing assertion
names the supported `prepare_curated_plan_for_selection` seam.

The seven locked-field mutations that cannot satisfy manifest 1.0 now assert
the public schema-first `ManifestContract` rejection. A crate-internal-only
semantic seam calls the unchanged post-schema validator directly; its focused
test proves that the empty-Type-2, private-creator, sequence-length, NM, PET,
US, and enhanced-PET semantic guards still fail closed. This seam is not a
public API and does not bypass schema validation for CLI, library, or SDK
consumers.

Focused and terminal ordinary evidence passed:

```text
27 exact cargo test --locked --no-default-features --test
  corpus_generation__subsystem <recorded-test-name> -- --exact invocations
27 passed; 0 failed

cargo test --locked --no-default-features --lib
  tests::schema_locked_manifest_fields_retain_downstream_semantic_guards
  -- --exact
1 passed; seven schema-locked downstream guards exercised

cargo test --locked --no-default-features --test
  corpus_generation__subsystem -- --format terse
92 passed; 0 failed; 0 ignored

python3 scripts/check-test-ownership.py
passed: 22 targets; 265 groups; 1,401 entries; 20 integration targets,
187 integration sources, 882 integration entries

python3 tests/test_change_test_routing.py
15 passed

python3 scripts/check-spelling-transition.py
passed: 884 classified retained occurrences;
SHA-256 08a12f22493fa0cbed591f3178e063330fe73d945649fe10f18db79bcf4293e0

cargo fmt --all -- --check
git diff --check
passed
```

Every changed code, test, ownership, and spelling-inventory path was inspected
with `route-changed-tests.py --dry-run` before its focused ordinary checks.
The resulting routing config identity is
`c5aaca6491ecdeba43497eea7502532b810d8ccf6ecddf30df32f5ac13b429c9`;
the ownership identity is
`e01309baff27d13cddf9ac2271dffd62a02ade5423ed93cafefeac656b64bb43`.
No Heavy, ignored, feature-gated, external-provider, Nightly,
release-candidate, composition/assembly/coverage/release identity, R4.4, or R5
body ran or is claimed by this closure.

The subsequent R3 review found that eight crate-internal validator fixture
modules still constructed Software Versions with `PACKAGE_VERSION`, allowing
their paired expectations to pass against release-coupled fixture bytes even
though native producers were correctly frozen. Commit `8123f2b` changes the
color, advanced-blending, and blending presentation-state fixtures; general
and twelve-lead ECG fixtures; linked RT image and plan fixtures; and
second-generation RT radiation/set fixtures to
`BYTE_STABLE_OUTPUT_VERSION`. An exhaustive search now finds zero
`PACKAGE_VERSION` or `CARGO_PKG_VERSION` occurrences in
`src/validation_*_tests.rs`.

All 36 accept and mutation tests in the affected validator modules passed:

```text
cargo test --locked --no-default-features --lib validation::color_softcopy_presentation_state_tests::
8 passed

cargo test --locked --no-default-features --lib validation::advanced_blending_presentation_state_tests::
4 passed

cargo test --locked --no-default-features --lib validation::blending_presentation_state_tests::
3 passed

cargo test --locked --no-default-features --lib validation::general_ecg_tests::
3 passed

cargo test --locked --no-default-features --lib validation::twelve_lead_ecg_tests::
6 passed

cargo test --locked --no-default-features --lib validation::rt_image_tests::
4 passed

cargo test --locked --no-default-features --lib validation::rt_plan_tests::
4 passed

cargo test --locked --no-default-features --lib validation::rt_radiation_tests::
4 passed
```

Commit `5736d50` expands the fail-closed audit from recipe-only coverage to the
complete version-bearing output boundary: recursive recipes, composition
defaults, native curated plans and execution, curated manifest projection,
library manifest/product expectations, production validators, and every
crate-internal validator fixture. It additionally enumerates exact allowed
lines for product manifest and runtime identities in assembly, codecs,
composition, materialization, curated execution, and the library; the two
highdicom and three quantitative semantic-stable external cases are the only
allowed external DICOM Software Versions uses. Any new or moved occurrence
fails the test. The complete seven-test unified audit passed, and refreshed
ownership metadata passes with 22 targets, 264 groups, 1,400 entries, 20
integration targets, 186 integration sources, and 882 integration entries;
ownership SHA-256 is
`a13adc8112b28ab2bf27f42db4768a059d9b5d4743d23ce73ddd144820e71cea`.

This review remediation ran no Heavy, unrelated manifest-fixture, external
provider, Nightly, release-candidate, or R7 qualification body.

Routing review then found that changes to `src/validation.rs` and
`src/validation_*_tests.rs` selected corpus integration evidence but omitted
the crate-internal validators repaired above. Commit `b821f2c` adds the
`byte_stable_validation` ordinary bundle without adding an integration target.
It contains eight exact `cargo test --locked --no-default-features --lib
validation::...::` filters with locked list counts 4, 3, 8, 3, 4, 4, 4, and 6,
covering all 36 affected accept and mutation tests.

Dry runs for `src/validation.rs` and each of the eight repaired fixture paths
select both `byte_stable_validation` and the existing `corpus` bundle. The
immediate library commands contain no `--ignored`, feature, release, Nightly,
or Heavy selector; `explicit_heavy`, `future_external_corpus`, and
`release_candidate` remain explicitly deferred. The routing config identity is
`c5aaca6491ecdeba43497eea7502532b810d8ccf6ecddf30df32f5ac13b429c9`.
All eight exact routed filters passed again with 36/36 tests, and
`python3 -m unittest tests/test_change_test_routing.py` passed 15/15 router
tests. Ownership remains unchanged at SHA-256
`a13adc8112b28ab2bf27f42db4768a059d9b5d4743d23ce73ddd144820e71cea`.

This routing repair did not run the existing corpus integration commands,
because the scoped blocker was the missing library slice and the known 27
unrelated manifest-v1/schema/routing failures remain outside R3. No Heavy or
terminal qualification evidence was invoked or claimed.

Focused verification passed:

```text
python3 scripts/route-changed-tests.py --path schemas/manifest-v1.schema.json
3 identity tests; 3 capabilities tests; 3 version tests; 86 schema/resource tests passed

cargo test --locked --no-default-features --test cli_sdk__nonfast generate_cli::generate_machine_result_is_clean_typed_and_manifest_bounded -- --exact
1 passed; published result 2.0 and manifest 1.0 schema-valid

cargo test --locked --no-default-features --test schema_resources__fast schema_artifacts::committed_schema_files_compile -- --exact
1 passed; 72 filtered

cargo test --locked --no-default-features --test cli_sdk__nonfast sdk_facade::
8 passed; exact SDK validate/report readers cover curated 0.2/0.3/1.0 and rejection cases

cargo test --locked --no-default-features --test schema_resources__subsystem
86 passed; exact CLI validate/report readers cover curated 0.2/0.3/1.0 and rejection cases

cargo test --locked --no-default-features --test cli_sdk__nonfast validate_cli::
28 passed; schema-valid EOT fixtures reach the retained semantic guards

cargo test --locked --no-default-features --test cli_sdk__nonfast report_cli::
46 passed; handcrafted coverage fixtures are schema-bound before report semantics

cargo check --locked --no-default-features
cargo fmt --all -- --check
python3 scripts/check-test-ownership.py
python3 tests/test_change_test_routing.py
python3 scripts/check-spelling-transition.py
git diff --check
passed; ownership 22 targets, 264 groups, 1,398 entries; routing 14 tests;
spelling 883 reviewed occurrences
```

Both new schema paths overlap the identity and generic schema rules. Their
bounded route includes the three identity isolation tests, version and
capabilities producers, the 86-test schema/resource suite with live standalone
generation and legacy SDK readers, and unconditional Fast checks. No feature,
provider, external adapter, Heavy, Nightly, release-candidate, remote, R4.4,
R4.5, R5, or downstream manifest body ran. The exploratory fixture
incompatibilities were migrated without bypassing the loader: schema-valid
fixtures preserve reachable semantic assertions, while shapes forbidden by the
schema assert public schema-first rejection and retain their downstream guards
in crate-internal tests.

### R4.2 — versioned caller-owned corpus definition bundle

**State:** complete on 2026-09-02; aggregate R4 remains in progress

**Commits:** `ed3ec9a`, `fbf6990`, `329fe37`, `78354e6`, `78dd58d`,
`614024e`, `39a41c8`, `4908032`, `4706fa7`, `dca8122`, `262e955`,
and `af9d062`.

The new strict Draft 2020-12 contract has schema version `1.0.0` and the
post-rename identifier
`https://synth-dicom-gen.local/schemas/corpus-definition-bundle.schema.json`.
It describes profiles, the exact registry descriptor, implemented
case-to-recipe bindings, dependencies, expected evidence, and assets. Unknown
fields, duplicate keys including escaped collisions, BOMs, invalid UTF-8,
unsupported versions, unsafe and case-fold-colliding paths, missing/extra
content, and every configured size, count, JSON-depth, array, and string limit
fail closed. The manifest exact bytes and every declared file's exact size and
SHA-256 form a framed aggregate identity with no host-absolute path input.

On macOS and Linux, one `O_DIRECTORY|O_NOFOLLOW` root descriptor is held from
manifest capture through declared-file capture and terminal inventory. Every
component is opened descriptor-relatively with `openat` and `O_NOFOLLOW`;
final inputs additionally use `O_NONBLOCK`, must be regular with `nlink == 1`,
and retain device/inode/length while read under a bounded `take(limit + 1)`.
The descriptor-relative inventory streams one bounded directory entry at a
time, distinguishes `readdir` errors from EOF, rejects entry counts beyond the
declared inventory, and rejects symlinks, hardlink aliases, special files,
undeclared files, and undeclared directories including empty ones. It also
checks directory stability. A regression replaces the pathname after opening
the root and proves later reads remain on the held original descriptor. The
non-Unix fallback retains bounded reads and symlink/type checks but does not
claim the Unix descriptor-race proof.

Role roots are canonical: `cases/registry.json`, JSON below
`cases/recipes/`, evidence below `evidence/`, and assets below `assets/`.
Caller data cannot enter engine-owned schema, template, transfer-syntax,
conformance, provider/backend, security, product, lock, or engine-asset
namespaces. The trusted registry and recipe schemas plus existing registered
provider/rule/output-shape validation apply to captured bytes. Implemented
registry rows have exactly one matching recipe; non-implemented rows have
none. Dependencies bind exact recipe identities, are unique and acyclic.
Ordinary smoke/core/extended definitions cannot import legacy, opt-in stress,
negative, or fuzz recipes; legacy and stress cannot import one another or an
invalid scope. Negative and fuzz definitions may reference ordinary valid
sources or their own scope only: neither may import legacy, opt-in stress, or
the other invalid scope. The intentional ordinary-source direction remains
supported: the current corpus proves 16 negative or fuzz definitions
referencing ordinary valid source recipes. Profile membership equals the
registry; each profile has its exact
scope; `all` is only smoke/core/extended with optional stress. Local notes and
assets have complete, non-orphan closure. Unavailable runtimes remain
unavailable definition input rather than implied passes.

The committed deterministic assembler copies source registry, recipe, and
note bytes into a fresh dedicated root without normalization. The full-current
fixture proves 191 registry rows, 178 implemented recipes, eight profiles,
zero external assets, and 45 local-note references resolving to 34 unique
evidence files. The 34th note,
`phase-3-integer-parametric-map-provider.md`, is referenced only by planned
case `derived/parametric-map/integer_ct_derived_explicit_le`; it is still
declared and verified although planned cases correctly own no recipe. The
result has 214 files and 1,754,298 captured bytes. Its exact manifest SHA-256
is `905d36bc93c7ae10ae5011304b25a647c4b792852e143bd2017e2aacd1574de8`
and framed corpus SHA-256 is
`571fa23fd392dd557ccdbe2db527698eaedc7078d86543efc68dfffc877411f7`.
The canonical frame includes role, logical record ID, logical path, decimal
size, and file SHA-256 for every path-sorted declared record.
Relocation preserves both; a whitespace-only manifest byte change changes
both.

R4.2 adds no SDK method, CLI option, generation request/result, discovery
field, generated-manifest field, or execution route. The public implementation
module is explicitly unsupported migration surface, has no crate-root type
re-export, and compatibility ownership advertises no live SDK/CLI API before
R5. Embedded behavior is unchanged. Direct inclusion of the new corpus schema
is excluded from the transitional engine scan, so the R4.1 oracle remains
exactly 240 resources and SHA-256
`dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`.

Focused verification passed:

```text
cargo test --locked --no-default-features --lib corpus_definition::tests::
19 passed; 0 failed; 475 filtered; 8.99s test

cargo test --locked --no-default-features --lib corpus_definition::tests:: -- --list
exactly 19 entries

cargo test --locked --no-default-features --test schema_resources__fast schema_artifacts::committed_schema_files_are_parseable_json_schema_documents -- --exact
1 passed; 72 filtered

cargo test --locked --no-default-features --test schema_resources__subsystem engine_resources::embedded_resources_cover_engine_families_and_transitional_membership -- --exact
1 passed; 85 filtered; locked 240/dc61 oracle unchanged

cargo check --locked --no-default-features --lib
cargo fmt --all -- --check
git diff --check
passed

python3 scripts/check-test-ownership.py
passed: 22 targets; 263 groups; 1,394 entries; historical 20 integration targets, 186 sources, and 879 entries unchanged

python3 -m unittest tests/test_change_test_routing.py
13 passed

python3 scripts/check-spelling-transition.py
passed: 870 classified retained occurrences; SHA-256 ecff895f74165daa5d9d72ee145ee746f7f799a6028f12916fd7c010dfaa44b1
```

The routing dry run selects the exact 19-entry library filter
`corpus_definition::tests::`, schema subsystem, and unconditional Fast
coverage. A standalone `python3 -m py_compile` attempt was unavailable because
system Python tried to write sandbox-denied user cache state; the assembler
itself ran successfully in the full-current tests and its exact temporary
roots were removed. No broad all-target, heavy, feature/provider runtime,
external adapter, Nightly, release-candidate, remote, release, R4.3 identity,
R4.4 removal, R4.5 materialization, or R5 API body ran. R4.3-R4.5 and the
aggregate R4 gate remain open.

### R4.3 — bounded composition identity slice

**State:** composition manifest/result slice complete on 2026-09-02;
assembly, coverage, and release manifest slices plus aggregate R4.3 remain in
progress

**Commits:** `23fbb3d`, `63ce3f3`, `cacaf6e`, `62b9158`, `e2eb787`,
`60dccf6`, `11d77e5`, `8127b33`, `0d48a2e`, `9fa9be5`, `2b9a8ec`,
`e6f8824`, `4016ff2`, `ba5e3c6`, `bdd972f`, and `cb1668c`.
Review remediation is recorded by `937d045`, `9fd275c`, `80e9d85`, and
`3b73b49`; provider-cadence remediation is `2b034dc`, `def139b`, and
`64ee40c`.

The composition producer now emits manifest `1.0.0` and result `2.0.0`.
Before the producer change, exact validation-only fixtures froze real manifest
`0.4.0`, manifest `0.5.0`, and result `1.0.0` documents. The shared
schema/version-aware reader retains CLI and SDK validate/report support for
composition manifests `0.4.0`, `0.5.0`, and `1.0.0`; it rejects unknown
versions and rejects a malformed or missing `1.0.0` identity projection before
composition semantics. The current result producer is `2.0.0`, while
capabilities separately advertises schema-validation support for result
`1.0.0` and `2.0.0` and manifest-reader support for `0.4.0`, `0.5.0`, and
`1.0.0`. Both additive discovery fields remain optional in capabilities v2,
and a frozen pre-field capabilities-v2 document remains valid.

Composition verifies `EngineResources` and computes the installed base
identity before executing. The terminal manifest projects independent engine,
schema-set, template-catalog, provider-catalog, toolchain, standards, and
execution identities. Its corpus state is honestly
`transitional_embedded_unverified` with a null identity; no external corpus
selector or R5 execution route was added. A caller-selected custom template
catalog changes `run.template_catalog_sha256` but not the installed template
catalog identity. Successful provider execution projects exactly one
`generation_provider` runtime with its executable SHA-256; built-in execution
projects no external runtime, and failed provider execution publishes no
manifest. The shared manifest-contract semantic guard rejects duplicate
runtime IDs even when their fingerprints differ, closing the gap that JSON
Schema `uniqueItems` cannot express.

The transitional top-level resource identity and legacy provenance remain
exactly version `1.0.0`, origin `embedded`, 240 members, and SHA-256
`dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`.
The new schemas are directly classified in the schema-set identity and
excluded from that frozen oracle. After the EOF-only formatting correction,
the installed schema-set identity is exactly 44 members, 826,625 bytes,
SHA-256
`29f9c67a96302911a4ba38013deb0cafb8c6205eed53801f816e58728ebecba1`.

Exact equality after removing only `identity_projection` and resetting the
version to `0.5.0` proves that manifest `1.0.0` preserves the frozen `0.5.0`
DICOM hashes and paths, plan and ordering, bundles, assets, references,
validation, unavailable rows, and publication semantics. Custom-catalog byte
parity and provider provenance tests also pass. Focused evidence completed:

```text
composition manifest fixtures: 1 passed (0.4/0.5/result-v1 validation-only)
composition manifest identity/schema tests: 3 passed
composition public API identity/parity tests: 4 passed
composition CLI result-v2 and 0.4/0.5/1.0 reader tests: 2 passed
SDK composition result and 0.4/0.5/1.0 reader tests: 2 passed
composition CLI and SDK invalid-reader matrices: 2 passed; both validate and report reject unknown version, missing identity, malformed digest, and duplicate runtime ID
capabilities live/additive fixture tests: 2 passed
schema compilation and transitional exclusion checks: 2 passed
identity-domain isolation/evidence module: 3 passed
python3 scripts/check-test-ownership.py: 22 targets, 265 groups, 1,409 entries
python3 -m unittest tests.test_change_test_routing: 17 passed
cargo test --locked --no-default-features --test release_ci__fast: 11 passed
cargo fmt --all -- --check; git diff --check: passed
```

The new composition schemas route through live composition producers, SDK
readers, identity discovery, generic schema checks, and unconditional Fast
coverage. The full ordinary composition harness passes with 84 passed and five
ignored. Three are the pre-existing prepared-backend qualifications; the two
structured-report semantic bodies are now also explicit ignored external
qualifications. Their bodies and assertions remain intact and are separately
invocable with exact `--ignored` test filters, but they were unavailable and
were not run because the locked
`generation-backends/highdicom-pydicom/.venv/bin/python` executable is absent.
If invoked without it, fail-closed acquisition reports that exact missing
runtime; mutex recovery prevents one failed invocation from changing a later
failure into a poison error. Routing classifies changes to that source as
deferred `native_provider_contract` evidence. This is not independent external
provider evidence, and this slice did not install or run that backend. No
feature matrix, external provider qualification, Heavy, Nightly,
release-candidate, remote, assembly/coverage/release projection, R4.4, R4.5,
or R5 body ran.

The `native-provider-contract` workflow now prepares the exact locked
highdicom project, exports its prepared interpreter, and then invokes each of
the two structured-report bodies with an exact `--ignored --exact` command.
The fail-closed `PROVIDER_IGNORED_TESTS` inventory contains all nine scheduled
provider qualifications, discovers both structured-report functions in their
own source, and statically proves preparation/export precede invocation. Those
two commands were inspected but not executed locally because the required
external backend is absent; their next evidence comes only from the scheduled
provider job. Compatibility ownership also assigns the new manifest-v1 and
result-v2 schemas exactly once to the qualified-composition contract while the
frozen predecessor schemas retain the same owner and advertised reader window.

### R4.3 — bounded structural-assembly identity slice

**State:** structural-assembly manifest/result slice complete on 2026-09-02;
coverage and release projections plus aggregate R4.3 remain in progress.

Commits `4db06eb` through `225740e` freeze genuine assembly v1 evidence,
separate its deterministic UID namespace from mutable resource membership,
introduce assembly manifest and result `2.0.0`, and retain exact `1.0.0`
validation/report compatibility. The current manifest projects engine,
schema-set, installed template catalog, provider catalog, toolchain, standards,
and execution identities before assembly execution. Native assembly records no
external runtime fingerprints and records corpus definition as
`transitional_embedded_unverified` with a null identity. Caller requests and
assets remain run inputs: perturbing request content changes its request hash
without changing any installed identity domain.

The transitional monolith remains explicit and unchanged at 240 members and
`dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`.
That same digest is now a named private compatibility salt for all five assembly
UID roles and callsites, so later R4.4 membership removal cannot perturb
assembly bytes.
The direct schema-set identity deliberately includes the two new schemas and
crossed from 44 files / 826,625 bytes /
`29f9c67a96302911a4ba38013deb0cafb8c6205eed53801f816e58728ebecba1` to
46 files / 839,053 bytes /
`0983a809348e6bd4c1df52ec579a321fa34118609ffd51d4a8895aee176dcca0`.

Frozen provenance uses the tracked seed-5 request fixture SHA-256
`29a0da03979c74631959851cc103cdcb9114a703de3f2b658ef714efd062f664`.
The committed pretty fixture is 62,483 bytes and SHA-256
`f0e1d6db70594718121e7d35740288575bc0e66a8df27f78396878e02998df0e`;
it intentionally has one final LF beyond the historical producer's 62,482 raw
bytes and SHA-256
`8804bd9c950a624d6bc21c62cce1aa1f36192c324b92b48ed0bf8e3d2be56c26`.
These identities are recorded separately. The frozen result fixture is
SHA-256 `61d00bebf41d876d5b8a60541c25ef9aa774a103c9ab799fa49e82e87d92f6d7`.
The v2-to-v1 manifest comparison removes only `identity_projection` and resets
the manifest version. Result comparison resets the two contract versions and
only the volatile output-root and manifest paths.

Exact seed-5 byte evidence remains: corpus plan
`d4ebd5a7ca2081375022b0d9bf8726d5e8f508afe59e1c1ad7d68d65f5ebda45`,
resolved plan
`b7505f0aa740d280f4be80041aa8d7b15830ea931954bd870ff24658c14c05e9`,
and a 774-byte DICOM with SHA-256
`7da898187a4f13054d48268660770f86c6939c082a5731d8f3737a5a855c7ba7`.
The exact study, series, SOP, frame-of-reference, and implementation identities
remain asserted, including implementation version `DICOMTS010`.

The shared manifest loader now accepts structural-assembly manifests `1.0.0`
and `2.0.0` before every CLI/library/SDK validate and report path. Version 2
rejects unknown versions, missing identity projection, malformed digests, and
duplicate runtime IDs with different fingerprints; version 1 remains a frozen
legacy-only reader and never synthesizes identities. Capabilities advertises
manifest/result `2.0.0` producers separately from `1.0.0`/`2.0.0` validation
windows. Both new capability fields are optional so the frozen pre-field
capabilities-v2 document remains valid. Each public schema has exactly one
compatibility owner, and changes to either new schema route the assembly,
identity, SDK, and generic schema bundles.
The canonical seed-5 request, frozen manifest-v1, and frozen result-v1 fixtures
also route assembly byte/plan/CLI parity together with schema and SDK readers.
Changes to the shared manifest-contract loader route assembly semantics as well
as its schema and SDK consumers.

Bounded ordinary verification after the slice:

- `assembly__subsystem`: 25 passed in 3.23 seconds (3.29 seconds wall);
- identity-domain library route: 3 passed in 0.72 seconds (0.79 seconds wall);
- SDK facade route: 12 passed in 2.82 seconds (2.89 seconds wall);
- capabilities route: 4 passed in 1.28 seconds (1.35 seconds wall);
- version route: 3 passed in 1.12 seconds (1.18 seconds wall);
- schema/resources route: 86 passed in 6.28 seconds (6.35 seconds wall);
- ownership inventory: 22 targets, 265 groups, and 1,414 entries passed;
- spelling inventory: 920 retained occurrences, SHA-256
  `91566a24eb3bd68af2bb2e0fa24a11fece70ef4cb26ad4e0acfa4e98cfbfa126`.

No provider, codec-feature, external-corpus, Heavy, Nightly,
release-candidate, coverage/release projection, R4.4, R4.5, or R5 body ran.

### R4.3 — bounded report identity-preservation slice

**State:** report projection complete on 2026-09-02; release projection and
aggregate R4.3 remain in progress.

Commits `1bc204f` through `e7bf0e6` freeze genuine legacy report bytes, add
strict current report contracts, branch only on the already validated source
manifest version, and copy the source `identity_projection` exactly. Curated
manifest `1.0.0` now derives coverage report `1.0.0`; composition manifest
`1.0.0` derives composition report `1.0.0`; and structural-assembly manifest
`2.0.0` derives structural report `2.0.0`. Curated `0.2.0`/`0.3.0`, composition
`0.4.0`/`0.5.0`, and assembly `1.0.0` readers retain report `0.1.0`, `0.1.0`,
and `1.0.0` respectively and never synthesize identity from
`product_resources`. The generic report-result envelope and coverage-gap
report remain unchanged.

The frozen raw JSON identities are: smoke seed-1 coverage report 93,929 bytes /
`c18f4bbd8807eb9d108c07a59035f97ff75771aa3fe962f395153fd81d1a8b0a`,
composition report 920 bytes /
`974b39b687907e73b4b8a147f21364c21cd8517490704295fc2056505ab23415`,
and structural-assembly report 635 bytes /
`9dcac66e9a6ab1f0d5a4f7f5940f6d94c8a56510dbc8e26bc5878d7867416203`.
Current-to-legacy parity removes only `identity_projection` and resets the
report version; the smoke comparison reproduces the exact R0 report bytes.

All three current schemas reject unknown versions, missing identity,
malformed digests, and duplicate runtime IDs with different fingerprints.
The curated family now proves all four mutations through both public CLI
`validate`/`report` commands and SDK `validate`/`report` methods; composition
and assembly retain their equivalent public matrices.
The two domain report producers now have explicit overlapping change routes:
`src/composition/validation.rs` selects composition plus the focused report
bundle, and `src/assembly/validation.rs` selects assembly plus that report
bundle. The curated producer remains conservatively covered by the existing
`src/lib.rs` all-ordinary route. Routing fixtures fail closed if either overlap
is removed.
The dedicated composition report legacy/current schemas establish the
previously absent public contract and have exactly one compatibility owner.
Capabilities advertise current producers separately from report validation
windows: coverage and composition `0.1.0`/`1.0.0`, assembly
`1.0.0`/`2.0.0`. Additive fields remain optional for frozen capabilities-v2
documents. The direct schema-set identity is now 50 files / 848,317 bytes /
`be3665f9a7a70182fde9c70bda7bdcada660a3d3c19545be1102a0a88a8ca98b`;
the transitional oracle remains exactly 240 members /
`dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`.

Bounded ordinary verification after the slice:

- report-contract unit route: 3 passed in 0.58 seconds;
- report CLI route: 48 passed in 16.69 seconds;
- SDK facade route: 12 passed in 3.34 seconds;
- composition subsystem: 84 passed and 5 explicitly ignored provider bodies
  in 14.29 seconds;
- assembly subsystem: 25 passed in 3.11 seconds (20.31 seconds wall including
  final rebuild);
- schema/resources route: 86 passed in 6.51 seconds;
- capabilities route: 4 passed in 1.30 seconds;
- identity-domain route: 3 passed in 0.78 seconds;
- compatibility ownership: 1 passed;
- test ownership: 22 targets, 266 groups, and 1,419 entries passed;
- spelling inventory: 944 retained occurrences, SHA-256
  `8c59a643a5719e4690c9150cdfd632014e7996c6b8e2ad4b639d0e0cf4c88618`.

The first full assembly run exposed one stale test binding to the frozen v1
report schema (24 passed, 1 failed); commit `7db4f29` migrated that assertion
to the strict v2 schema and the exact focused body passed. The final full
assembly rerun above passed. No ignored provider body, codec feature,
external corpus, Heavy, Nightly, release-candidate, R4.4,
R4.5, or R5 body ran.

### R4.3 — bounded release/package identity slice

Release manifest producer `2.0.0` copies the exact installed
`identity_domains` value from version result `2.0.0`; the build rejects a
capabilities result whose projection, product version, target, or feature set
does not agree. The verifier retains the frozen `1.0.0` reader and adds a
fail-closed `2.0.0` reader. Current v2 verification requires strict nested
schemas, exact three-way domain equality, valid source revision/dirty
provenance, unique runtime IDs, and bound inventory paths, sizes, and hashes.
The deferred RC harness dispatches v1 and v2 schemas by version and registers
the v2 version/capabilities references; its target was compile-checked without
running the archive body. Cargo, jq, and tar are release-process
evidence, not invocation runtimes, so the current producer emits `[]`.

Optional `legacy_product_resources` is emitted only while both discovery
documents carry the same value. It is not authoritative, so R4.4 can remove
all three legacy copies without changing v2 domain meaning. The transitional
oracle remains 240 members /
`dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`.
The direct schema set is 51 files / 850,644 bytes /
`365bf67eea8b161434f85e892cd49e273642e127a9af9fa77a57d3bfe71d9220`.
The frozen v1 schema is 1,836 bytes /
`ba3a2b51c2b77b6223119cf19782a7e0232a43ef62d98639b278c6e3db1a00e3`;
v2 is 2,251 bytes /
`b2a6bb595b3bd603af421ae685f4d1151604b0e96838ab7519e43817b1cc177d`.

No historical archive or raw release-manifest v1 payload is tracked or
available locally. The 639-byte v1 fixture (SHA-256
`59bb754e8ddd7142ecde3c5f2621362b4cccac8fc040a4fdcafc1f54d6e47de0`)
is deterministic synthetic validation evidence, not historical or RC
evidence. The immutable v1 schema bytes are the genuine preserved public
contract. Retrieving or constructing an archive was excluded from this slice.

The Fast adversarial matrix rejects product, target, feature, source,
top-level domain, nested-domain, duplicate-runtime, legacy presence/value,
and inventory drift. Positive cases prove all three legacy fields may be
absent and one unique actual runtime may be present while retaining exact
three-way identity equality.

Bounded ordinary evidence: release Fast suite 14 passed in 1.94 seconds;
schema/resources 86 passed in 6.44 seconds; deferred RC harness compile-check
passed in 0.32 seconds; routing 22 passed in 2.20 seconds; ownership passed for
22 targets, 266 groups, and 1,422 entries;
spelling passed for 944 occurrences at
`8c59a643a5719e4690c9150cdfd632014e7996c6b8e2ad4b639d0e0cf4c88618`.
Actual archive construction, Cargo packaging, installed-consumer relocation,
target binary equality, RC, publication, Heavy, Nightly, and external-provider
bodies did not run.

### R4.4 — separated runtime-resource identity

R4.4 implementation commits:

- `617670a` — `feat(resources): separate current engine identity`
- `2d9989e` — `refactor(identity): isolate toolchain from engine resources`
- `d75b763` — `feat(discovery): omit legacy resource identity from v2`
- `423aac6` — `fix(manifests): preserve legacy resource provenance`
- `2d3f619` — `chore(tests): route separated identity evidence`
- `4c0a20b` — `fix(resources): include all current schemas in v2`
- `38b8556` — `test(reports): use current coverage contract reader`
- `7ee8a16` — `chore(tests): refresh report fixture ownership`
- `9474faf` — `test(xray): migrate current manifest reader assertions`
- `80561d8` — `fix(tests): keep manifest helper harness-local`
- `ea1be6d` — `fix(tests): freeze historical direct-plan UID seed`
- `d57a7f2` — `chore(tests): refresh byte-oracle ownership`
- `99787c8` — `chore(spelling): classify legacy schema fixture ID`

The authoritative `EngineResources` identity is now version `2.0.0`, 74
members, 1,251,116 bytes, and SHA-256
`a54f1c1e897162dfaca6c3bc9264b45d2e2ddc77258fe3c6263f7a285a675c17`.
It excludes all `cases/**` paths and `Cargo.lock`, and includes all 51 current
schemas, including the 14 schemas that had previously been embedded directly
by their owners outside the transitional table. Those 14 paths now join the
build-generated table exactly once; the schema-set projection consequently
remains 51 files / 850,644 bytes /
`365bf67eea8b161434f85e892cd49e273642e127a9af9fa77a57d3bfe71d9220`.

Physical embedding, snapshots, explicit-root capture, and package membership
temporarily remain a 254-file compatibility closure because R5 still owns the
default curated/composition/report corpus reader migration. A generated
legacy-path inventory reconstructs and verifies exactly the original resource
identity version `1.0.0`, 240 members, and SHA-256
`dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`.
Every explicit root is checked against that full legacy closure before the
current v2 identity is accepted, so excluded corpus or Cargo bytes cannot act
as an engine override. Curated, composition, and assembly manifests that still
require `product_resources` receive only this named compatibility identity;
the assembly UID salt remains `dc61cc...`. Current version/capabilities v2
documents omit their optional monolithic field, and release v2 therefore uses
the already-supported all-legacy-absent shape. Split identities remain
authoritative.

`Cargo.lock` is read separately for toolchain identity. A bounded isolation
test proves a Cargo-only perturbation changes toolchain plus reconstructed
legacy provenance and no engine, schema, template, provider, corpus,
standards, execution, or runtime identity. Verified caller bundle perturbation
continues to change only corpus identity. The default embedded generation path
remains honestly `transitional_embedded_unverified` with null corpus identity;
R5, not R4.4, owns the supported external-corpus execution route.

Focused ordinary evidence after the membership settled:

- `cargo test --locked --no-default-features --test schema_resources__subsystem
  engine_resources::`: 7 passed; explicit relocation, tamper, symlink,
  bounded-read, complete snapshot, direct-schema, and v1/v2 oracle checks.
- `cargo test --locked --no-default-features --lib
  identity::identity_domain_tests::`: 4 passed, including corpus-only and
  Cargo-only isolation.
- version CLI 3 passed; capabilities CLI 4 passed; SDK facade 12 passed;
  release Fast manifest readers 2 passed.
- assembly subsystem 25 passed; composition subsystem 84 passed and 5
  explicitly ignored provider qualification bodies remained unrun; generation
  CLI 9 passed and 1 explicit Heavy body remained ignored.
- schema/resources subsystem 86 passed; corpus-generation subsystem 92 passed.
  The first authoritative broad route exposed 10 stale vertical-slice tests
  that validated current coverage-report 1.0 output against the frozen 0.1
  schema. `38b8556` routes those exact tests through the shared version-aware
  SDK reader; all 10 exact tests and the full 92-test subsystem then passed.
- The continuing ordinary route exposed four XA/XRF tests with the same stale
  frozen-schema assumption. Their current manifest/report assertions now use
  the shared version-aware readers; schema-invalid const mutations prove
  schema-first rejection and still-schema-valid mutations reach their original
  semantic guards. Four exact tests passed. The route also exposed two direct
  waveform/document byte tests whose local helper incorrectly coupled its
  Implementation Class UID to package version `0.2.0`; production bytes were
  unchanged. `ea1be6d` binds that helper to the historical `0.1.0` output
  version, retains all four immutable hashes, and extends the static
  product-version coupling guard. The two exact byte tests, guard, and full
  136-test engine subsystem passed.
- The final fail-closed ordinary route over `c0f888e..d57a7f2` passed every
  selected command. It selected `all-ordinary`, assembly, composition,
  identity, provider, routing-contract, schema, and exact changed-test bundles
  under routing configuration SHA-256
  `43e2c812edb319ecfa8acffb076816c18e5d0811be89325ba7778bd41b3db577`;
  the router reported the Heavy, codec-feature, external-corpus,
  native-provider, Nightly, release-candidate, and unrouted-library classes as
  deferred rather than executing them.
- `cargo check --locked --no-default-features`, `cargo fmt --all -- --check`,
  `git diff --check`, 22 routing unit tests, and the 1,423-entry ownership
  inventory passed. Spelling transition remained 945 classified occurrences
  at SHA-256
  `e5f5585a41c7003530c62019bff6cb6a8196dc345bd676ce618225a4ae5b9e3e`.

No Heavy, Nightly, release-candidate, archive/package construction, external
provider, remote, release, R4.5, or R5 body ran. `cases/**` and `Cargo.lock`
remain packaged deliberately even though they are no longer members of the
authoritative engine digest; their physical/default-reader removal belongs to
later ordered phases.

### R4.5 — reusable lazy resource materialization

R4.5 commits:

- `4296013` — `feat(resources): cache private materialization leases`
- `b542247` — `refactor(resources): reuse leases across engine operations`
- `c2d930a` — `fix(resources): coordinate retryable cache construction`
- `cb5e6ca` — `fix(cli): materialize default resources only when needed`
- `c317470` — `chore(tests): route resource cache evidence`
- `f943049` — `test(resources): prove batch lease reuse and byte parity`

The public `EngineResources::snapshot()` contract remains a fresh, caller-owned
tree: a path returned to external code is never reused internally, avoiding a
same-user mutation race. Generation, validation, reporting, discovery,
conformance, and composition instead use a crate-private lease shared by clones
of one `EngineResources` handle. The cache has explicit empty/building/ready
states guarded by a mutex and condition variable. Integrity is checked before
first construction; waiters observe only a fully written, recaptured tree; an
RAII pending-directory guard removes partial output; failed construction resets
to empty and wakes waiters; and cached acquisition revalidates every physical
byte before returning. Separate embedded or explicit handles never share a
cache. The materialized root survives while either a resource handle or lease
owns it and is removed when the final owner drops, so there is no process-global
temporary-directory leak.

The pre-change bounded warm measurement at `779362a` acquired three sequential
snapshots from one handle. Each snapshot contained the full transitional 254
files / 2,664,374 logical bytes. The three runs therefore created three roots,
wrote 762 files / 7,993,122 bytes, and took 401,844, 388,472, and 390,135
microseconds (median 390,135). The equivalent post-change private-lease runs
created one root, wrote 254 files / 2,664,374 bytes, and took 182,545, 183,330,
and 183,717 microseconds (median 183,330). This is a 66.67% reduction in files
and bytes written and a 53.01% reduction in median wall time for this bounded
three-acquisition workload. Revalidation deliberately still reads the complete
tree on reuse; the measurement does not claim elimination of integrity I/O.

Seven focused cache tests prove zero materialization for identity/direct-byte
access, one copy across sequential and concurrent clone acquisition, isolated
independent handles, last-owner cleanup, retry after injected write failure,
fail-closed mutation detection, and one shared tree across a three-case smoke
generation followed by validation, report construction, and composition. That
batch test retains the exact R0 payload hashes
`76dc5208b139899fcb87bbf7ec9edf1a323000a91c4015de9ef8bde7bd344ecc`,
`fce766bcbb4b4aa79cfb3fa0c3b5e4ef888b11c0708fad713b9cde8d41ec6a15`,
and `33de9448509431fda27005cbf83c79977f1c3ebadb669ae1dedf1a225742f3c5`.
The seven existing public resource integrity/relocation tests also pass.

The CLI no longer creates an unconditional snapshot before dispatch: commands
using engine-owned internal paths use the private lease, while commands that
must expose a path lazily create one fresh public snapshot. The same refactor
surfaced and corrected a pre-existing discrepancy where CLI `validate` checked
an explicit resource root at startup but then validated with embedded schemas;
it now uses the existing resources-aware SDK validation path. All 29 validate
CLI tests pass, including valid explicit-root and tampered-root boundaries.

Routing selects the seven-entry private cache suite plus schema/resources for
`src/engine_resources.rs`. The 1,431-entry ownership inventory and 22 routing
tests pass. `cargo check --locked --no-default-features`, formatting, spelling,
and range diff checks pass. Heavy, Nightly, release-candidate, provider,
external, R5, and later-phase bodies did not run.

### R4.5 review correction — 2026-09-03

Commits `8f9cafa`, `cc90259`, and `4640e7e` correct the report-gaps
explicit-path regression and record its ordinary test ownership. The CLI now
tracks omitted registry/standards arguments separately from explicitly supplied
paths. Omitted defaults use lazy embedded resources; explicitly supplied
`cases/registry.json` and `standards.lock.json` remain caller-relative. The
pre-existing compose default-path behavior is unchanged.

`report_gaps_preserves_explicit_default_spelled_caller_paths` runs from a fresh,
unrelated temporary directory. Four argument-presence combinations compare exact
registry and standards hashes against distinct valid caller bytes or embedded
bytes as appropriate. After deleting both caller inputs, each explicit
default-spelled path independently fails rather than falling back.

Verification: the route dry-run for `src/main.rs`,
`tests/coverage_gaps_cli.rs`, and ownership metadata selected the conservative
ordinary route plus the focused CLI and routing contracts. The authorized
bounded command `cargo test --locked --no-default-features --test
cli_sdk__nonfast coverage_gaps_cli::` passed 6 tests, 0 failed, 142 filtered,
in 2.13s (2.30s compilation). The 22 routing tests, 1,432-entry ownership check
(902 integration entries), spelling check (945 retained occurrences), formatting,
and diff checks passed. Ownership metadata was regenerated after the added
missing-file assertions changed its entry digest. A mistaken unittest discovery
under `scripts` selected zero tests; the recorded 22-test result is the corrected
discovery under `tests/test_change_test_routing.py`.

No broad ordinary, Heavy, Nightly, provider, external, package, or release-candidate
qualification was run for this correction, and R5 implementation did not begin.
Earlier copy-cost measurements remain unchanged. Private cache reuse evidence
does not claim atomic protection against hostile same-user writers after
validation.

### R5.1 input substrate — 2026-09-03

This bounded slice establishes inspection inputs, not completion of R5.1 or
the R5 gate. `load_descriptor_file(path, root)` and
`load_descriptor_bytes(bytes, root)`, with corresponding limits variants, reuse
the existing strict `CorpusDefinitionBundle` capture and closure validation.
These methods remain on the unsupported migration module, not the supported
SDK surface. No generation request, CLI option, planning, execution, or output
publication is introduced.

The root is explicit and supplies every declared registry, recipe, evidence,
and asset. It is never inferred from a descriptor parent, current repository,
or sibling checkout. A relative path is explicitly caller-relative; descriptor
files require a parent component (`./selected.json` is valid, a bare filename
is rejected as ambiguous). Empty locations and parent traversal fail. Unix
file/member relative locations resolve from one held current-directory anchor;
every ancestor uses no-follow directory traversal and final files reuse bounded,
regular-file capture. Symlinked ancestors, including platform convenience
aliases, are not accepted by these new explicit-input methods. The historical
`load(root)` entry point retains its prior location behavior. Non-Unix code
retains its documented weaker pathname-based race boundary and is not newly
qualified here.

The member root remains a dedicated closure, not an arbitrary working tree.
An optional canonical `corpus-definition.json` must equal the selected bytes;
a conflict is a closure error, not an alternate selection or fallback. An
external descriptor may be outside this root. A differently named descriptor
inside the member root remains an undeclared file and is rejected. Captured
identity includes the actual descriptor bytes but excludes its host filename
and location. Loading creates no output, and returned captured bytes remain
immutable when source files later change.

Schema `1.0.0` is unchanged. Its fixed eight profiles, exact scope isolation,
`all` union, registry/recipe bindings, reserved engine namespaces, limits,
hashes, sizes, and reference closure are not generalized. Missing roots or
files retain `io.read.failed`; malformed bytes use `request.json.invalid`,
unsupported versions `request.version.unsupported`, limits
`resource.limit.exceeded`, mismatched members `evidence.integrity.failed`, and
unsafe/ambiguous/conflicting locations `resource.document.invalid`.

Implementation commits are `eb689f0` and `51603fe`; focused evidence is
`deb89c8`. The 23-test loader suite passed in 12.17s after 22.95s compilation.
It covers equivalent file/bytes/root inputs, relocation, no output, post-load
mutation, missing and conflicting locations, oversized bytes, custom-limit
overflow, invalid/versioned descriptors, traversal and symlink ancestry.
The full-current fixture remains exactly 214 files / 1,754,298 bytes, manifest
SHA-256 `905d36bc93c7ae10ae5011304b25a647c4b792852e143bd2017e2aacd1574de8`
and corpus SHA-256
`571fa23fd392dd557ccdbe2db527698eaedc7078d86543efc68dfffc877411f7`
for all three loading forms.

Route dry-runs selected the loader suite, schema/resource subsystem, and
unconditional Fast contracts. The resource subsystem passed 86 tests in 7.72s;
release Fast passed 14 in 2.11s. The initial schema Fast run passed 72 and failed
one: `committed_schema_files_compile` lacked frozen report references introduced
in R4.3. Its authorized test-only repair registers the committed schema
inventory without changing schema bytes or production behavior. This is prior
static-test registration debt, not a loader regression. Repair commit
`e010b80` passes all 73 schema Fast tests in 1.67s (1.25s compilation);
the resource subsystem rerun passes all 86 in 7.35s. Metadata commit
`f32f560` records 1,436 owned entries and the exact 23-test loader filter.
The 22 routing tests, ownership, 945-occurrence spelling inventory, formatting,
Cargo check (13.42s), and range diff checks pass. Exact verification commands:

- `cargo test --locked --no-default-features --lib corpus_definition::tests::`
- `cargo test --locked --no-default-features --test schema_resources__fast`
- `cargo test --locked --no-default-features --test schema_resources__subsystem`
- `cargo test --locked --no-default-features --test schema_resources__subsystem --test schema_resources__fast --test release_ci__fast` (initial run stopped at the recorded schema failure)
- `cargo check --locked --no-default-features`
- `python3 -m unittest discover -s tests -p test_change_test_routing.py`
- `python3 scripts/check-test-ownership.py`
- `python3 scripts/check-spelling-transition.py`
- `cargo fmt --all -- --check`
- `git diff --check 04a85fe..HEAD`

Heavy, Nightly,
provider, package/release-candidate, external repository, and later R5 execution
work did not run.

### R5 internal planning-input separation — 2026-09-03

Commits `fa5c9c6`, `52f70de`, `1cb0ef6`, and `b3698d6` add only an
internal planning bridge. `CapturedCuratedPlanningContext` accepts a verified
bundle and `EngineResources`; it parses registry/recipe bytes from captured
memory and never reopens or infers a caller corpus directory. Installed
templates, codec policy, standards, and external-provider source paths remain
bound to the private engine lease. Capability evaluation uses the verified
installed matrix, not caller data or executable discovery. The context and
returned private plan wrapper retain the lease through later consumption.

Recipe catalog assembly shares existing schema, shape, registry binding,
dependency, provider, template compatibility, and planning-order checks. The
legacy path entry point still checks every installed template's default recipe.
External subsets are not required to supply unrelated engine default recipes;
their own references and template compatibility remain mandatory. No schema,
recipe, template, DICOM, version, UID salt, or oracle changes were made. The
frozen eight-profile bundle contract remains unchanged.

Four focused tests pass (5.89s; 19.72s compilation on the final focused run):

- The full verified bundle produces exact serialized legacy/new smoke and
  `derived/registration/spatial_ct_pair` dependency plans, including UIDs,
  order, bindings, and projections. The entire caller source root is deleted
  before planning. No plan normalization is used.
- A three-case smoke subset plans without unrelated default recipes and has
  exactly equal planned artifacts and byte bindings to full-bundle smoke.
  The legacy catalog still rejects the same subset's missing default closure.
- Descriptor metadata changes only the retained corpus identity; planned
  smoke output and installed matrix remain equal. An unavailable highdicom
  parametric-map request has the same explicit unavailable plan as legacy,
  without discovering or invoking its runtime.
- A schema-valid unknown installed template is rejected by planning;
  unclosed dependency declarations and a reserved engine matrix namespace
  are rejected by bundle loading. A preliminary test fixture used an invalid
  template-ID spelling and correctly failed at schema validation; its spelling
  was corrected to reach the intended compatibility guard, without relaxing
  any validation.

Dropping the context and resource handle leaves the returned plan wrapper's
engine files alive. Dropping the final wrapper removes the exact private root.
This proves ownership/lifetime, not hostile same-user atomic filesystem
protection. Planning is still internal and output-free; these tests do not
establish the later supported SDK/CLI execution contract or independent
external conformance.

The inspected and executed ordinary route was
`python3 scripts/route-changed-tests.py --path src/curated_plan.rs --path
src/recipes/loader.rs --path src/runtime_capabilities.rs`. It exited successfully
with the exact four-test captured planner, SDK facade (12 passed, 3.89s),
corpus-generation subsystem, corpus-plan contract, and CLI schema contract
(4 passed, 0.06s). Quiet summary reruns of
`cargo test --locked --no-default-features --test corpus_generation__subsystem --quiet`
and `cargo test --locked --no-default-features --test engine__subsystem corpus_plan:: --quiet`
passed 92/92 in 30.75s and 22/22 in 0.85s, respectively, with zero ignored tests.
`cargo check --locked --no-default-features` passed in 0.06s warm (initial
implementation check 13.28s). Unconditional Fast was separately exercised with
`cargo test --locked --no-default-features --test schema_resources__fast --test
release_ci__fast`: schema Fast 73 passed in 3.11s and release Fast 14 in 3.91s.
The router now maps library fixtures to their declared exact source filter,
rather than treating every library test file as the corpus loader. All four
affected code/test paths select the new filter; an absent filter fails closed.
Routing tests pass 23, ownership records 1,440 entries / 268 groups, spelling
retains 945 exact occurrences, and formatting/diff checks pass. Heavy, Nightly,
provider qualification, package/release-candidate, and external-consumer bodies
did not run. No batch execution, SDK, CLI, or later R5 slice was implemented.

### R5 internal batch execution and external manifest 2.0 — 2026-09-03

This is the bounded internal R5.3/necessary R5.4 compatibility slice, not a
supported external-corpus API or completion of R5. The sequential implementation
range begins after `14dce37`: `ee0631a`, `f5bc6dc`, `95dd991` establish the
schema/reader; `f7f2b66`, `1a7685d` inventory it and close dependency evidence;
`f9e76bc` implements the private transaction; `3eb1c9c`, `93a5fb6`, `c33b83e`
own routing, frozen references and isolated test sources; `8246d68`, `ad2d25f`
retain complete captured case definitions and their exact metadata. No recipe,
provider protocol, public request/result, or existing embedded manifest1 producer
changed.

`run_captured_corpus` retains a verified bundle, EngineResources and the private
planning lease through one CorpusExecutor transaction. Profile selection follows
the captured fixed bundle1 profile definitions; explicit IDs must be nonempty,
unique, known and in the declared profile scope. Dependency closure is separate
from direct selection. Every selected registry status remains explicit; only
implemented recipe-bearing rows enter executable planning. Zero parallelism,
unknown profiles and invalid stress scope fail before publication. One caller
parallelism and cancellation token govern the batch. Dry-run returns `Planned`
without creating output; planned-only or unavailable-only execution returns
`NoExecutableCases` with validation/publication `not_run`, not an empty success.
Mixed runnable/non-runnable selection publishes through the normal transaction.

External manifest `2.0.0` contains verified corpus identity, exact captured
`case_definition` rows (all blockers, requirements and standards facts), sorted
dependency edges and explicit direct/dependency outcome ownership. The reader
checks schema/version, identity, runtime uniqueness, case ID/status/profile
coherence, graph closure/acyclicity/reachability, nonempty direct and generated
publication, file ownership and unique qualification ownership. It proves
internal document consistency, not correspondence to an original bundle that
was not supplied; the producer copies the verified bundle graph and rows.
Primary non-generated reason codes retain captured skip/blocker codes where
present, without case-ID-derived roadmap phases. Current native invocation
runtime evidence is empty, not a declaration of installed executables.

Library validation accepts the tested manifest2 output. All report entry points
and SDK manifest loading explicitly reject it as not yet supported; they cannot
silently read the embedded registry. The later report slice can use the complete
captured rows. Existing 0.x/1.x curated, composition and assembly readers remain
unchanged. No supported external-corpus CLI or SDK execution was introduced.

Final contract identities at this slice:

- `schemas/manifest-v2.schema.json`: 5,642 bytes,
  SHA-256 `63e22fe905d3849856b749101f216e0c13e204bd9cc5699e783386e018096ed6`.
- Current engine v2: 75 members / 1,256,758 bytes,
  SHA-256 `742cb5d3409219873fb8f5a6cf4b5d652e9ac131f9cc85c32cb60dc3e2d7fc5b`.
- Schema domain: 52 members / 856,286 bytes,
  SHA-256 `f4ec9e1df1230fc0168ed8763e0f9de1f7ceeb46e6b3d8c98a018ae29b2e0afc`.
- Physical transitional capture: 255 files / 2,670,016 logical bytes. Named
  legacy v1 remains exactly 240 / `dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`.
- Verified full bundle remains 214 files / 1,754,298 bytes, descriptor
  `905d36bc93c7ae10ae5011304b25a647c4b792852e143bd2017e2aacd1574de8`, corpus
  `571fa23fd392dd557ccdbe2db527698eaedc7078d86543efc68dfffc877411f7`.

Route dry-runs preceded focused ordinary verification. The new runner filter
`cargo test --locked --no-default-features --lib corpus_generation::captured_runner_tests::`
passes 5/5 in 4.76s after an 18.40s compile. It proves three exact R0 native smoke
SHA-256 values, sequential/parallel payload equality, matched-parallelism legacy
plan equality, deleted source-root and unrelated-CWD independence, spatial
dependency closure, mixed planned/native rows, unavailable-only nonpublication,
dry-run, invalid selection, cancellation, projection refusal, cleanup and
existing-root preservation. The contract filter
`cargo test --locked --no-default-features --lib manifest_contract::external_manifest_contract_tests::`
passes 7/7 in 2.08s after the final resolver correction. Current schema Fast
passes 73/73 in 1.54s; identity-domain tests pass 4/4 in 1.31s. SDK facade tests
pass 12/12 in 3.32s; report CLI tests pass 48/48 in 18.51s, including legacy
reader/byte-normalization evidence. These are same-project ordinary checks,
not independent conformance or provider qualification.

Final ordinary resource subsystem passes 86/86 in 6.92s; release Fast passes
14/14 in 1.88s. `cargo check --locked --no-default-features` passes in 10.99s;
format and exact `git diff --check 14dce37..HEAD` pass. Routing fixtures pass
24/24; ownership checks report 22 targets / 270 groups / 1,452 entries. Reviewed
retained spelling inventory is 955 matches, SHA-256
`59b3e0c69369b2cd47921175e0691c5d62d2dc773b293168d972dcdf61182d0c`.
The 9 references in the new schema and 2 in the shared reader retain frozen
contract identities, not aliases for product-controlled inputs.

Intermediate failures were repaired rather than omitted: planned rows initially
reached the recipe-only planner; positive contract fixtures initially assumed
the frozen empty manifests contained files; the strict production lookup audit
initially saw test-only compiled repository paths; adding the case-definition
reference initially exposed a missing generic-reader resolver. The final
implementation filters recipe planning without dropping ledger rows, uses real
bounded smoke evidence, isolates test sources, and registers the frozen schema
in both reader paths. Earlier historical measurements above are unchanged.

Heavy, Nightly, RC/package/archive, all-profile, stress, WSI, fuzz execution,
feature-specific codec and external-provider bodies were not run. No external
repository or runtime tool was created, installed or invoked. The Python bundle
builder is an ordinary fixture preparer only. R5 remains open for definition-led
reporting, supported SDK/CLI surfaces, discovery and SDK-only external-consumer
proof; this internal slice makes none of those claims.

### R5 manifest dependency-policy remediation — 2026-09-03

Review of `14dce37..6b30dda` rejected the standalone reader's dependency
policy: graph closure alone allowed a generated ordinary row to depend on a
planned row or isolated-profile evidence. This follow-up requires both edge
endpoints to be implemented recipe-bearing definitions and mirrors the verified
bundle's ordinary/legacy/stress/negative/fuzz isolation rules. Ordinary
dependencies remain permitted for nonordinary owners. The check applies to all
edges, including edges between directly selected rows; direct-selection scope
checking is not used as a substitute for edge checking.

The existing seven-test contract group now contains all 25 profile-family edge
pairings, planned-target rejection, direct-to-direct scope/status mutations and
a positive implemented dependency. Nonimplemented primary reasons must match
the producer's exact captured precedence: skip reason, otherwise first blocker,
otherwise `case_<status>`. Positive tests retain skip-over-blocker, first-blocker
and no-blocker fallback behavior; contradictory reasons fail. Initial test
fixture attempts exposed frozen registry constraints on implemented roadmap/
blockers and blocker-code enums; fixtures were corrected without relaxing the
schema. No manifest schema bytes, contract versions, resource/corpus identities,
producer behavior, supported API, or report boundary changed.

Implementation `9145fce` and ownership `9a4a0ac` pass the exact routed contract
filter (7/7, 2.23s; 21.30s compile) and captured-runner filter (5/5, 4.97s).
`python3 -m unittest discover -s tests -p test_change_test_routing.py` passes
24/24 in 2.434s; ownership remains 22 targets / 270 groups / 1,452 entries.
Spelling (955 matches, unchanged digest), formatting and diff checks pass.
The dry-run was inspected for `src/manifest_contract.rs` before the focused
ordinary checks. No Heavy, feature/provider, RC or external execution ran; no
later R5 report/SDK implementation began.

### R5.4 definition-driven raw reporting — 2026-09-03

The preceding internal runner/manifest boundary `14dce37..f568ef9` received
independent acceptance before this sequential slice began. The new range after
`f568ef9` contains `beac74b` (report schema/current identity inventory), `d094ddb`
(pure projector, semantic validator and raw dispatch), `6420662` (old CLI
envelope rejection), `0d51f24` (SDK rejection classification), `c524a7e`
(isolated metadata/runner evidence), `e81b736` (routing/ownership), and `7fe3f4c`
(generation-guide reporting boundary).

External `coverage_report_schema_version = "2.0.0"` has
`report_kind = "external_corpus"`. It preserves the complete source manifest as
a JSON Value, including every definition, selector, ledger edge, identity,
file, qualification and recorded validation field; it does not claim to retain
the original JSON whitespace. Top-level identity is copied exactly. Generic
case dimensions use captured definitions; artifact dimensions use emitted file
metadata only. Profile, modality, SOP Class, transfer syntax, determinism and
provider groups carry sorted membership and counts. Logical-case, direct,
dependency, file and qualification counts are distinct. Unknown caller case
names require no built-in case-ID inference.

The semantic reader first validates the source manifest2, then reconstructs the
entire report and rejects changed summaries, dimensions, identities, source
ledger, assessment claims or versions. Report creation is explicitly
`manifest_projection`, with report-level validation and independent conformance
`not_assessed`; it neither reopens payloads nor upgrades or discards recorded
source results. Markdown identifies profile, selector, verified corpus digest,
per-case profile membership, outcome and reason, and points to retained full
JSON evidence. The raw library/CLI dispatch occurs before private snapshot or
legacy registry reads. No old generated/skipped coverage-row helper is used.

The frozen report-result1 machine envelope cannot represent the new report
kind. `report <root> --format json --cli-api 1.0.0` therefore returns stable
`request.version.unsupported`, exit 2 and a schema-valid error envelope, with no
stdout. Raw JSON/Markdown are available. SDK report2 likewise returns the
unsupported-version code; external manifest SDK validation remains unsupported.
No new supported generation request, public corpus selector, report envelope,
ReportKind or external-consumer API was added. Old curated/composition/assembly
reports and the coverage-gap report retain their existing contracts.

New exact resource endpoint (all prior dated measurements remain historical):

- Report2 schema: 2,641 bytes,
  SHA-256 `0f0b3945a575563c6d45d4acb5b29917b86a14748b4762b47cdafeb93cfc0eda`.
- Current engine v2: 76 members / 1,259,399 bytes,
  SHA-256 `e8cb8ba376e03d9d4d1451c44a70b3609c0ada11baab16bb96b012b8e6e6cd77`.
- Schema domain: 53 members / 858,927 bytes,
  SHA-256 `a562c02e31550c0bac2d2f14b2c7af49b772af1368f19109a0e1b9174a229299`.
- Physical capture: 256 files / 2,672,657 logical bytes. The exact named
  legacy240/dc61 reconstruction and verified214-file corpus identity above
  remain unchanged.

Route dry-runs preceded the bounded ordinary checks. Exact new report filter
`cargo test --locked --no-default-features --lib corpus_report::captured_report_tests::`
passes 3/3 in 4.04s (21.06s compile); frozen/current report-contract filter
passes 3/3 in 0.92s. Captured runner passes 5/5 in 11.93s and identity-domain
tests 4/4 in 3.64s during overlapping local ordinary checks. New tests cover
deleted definition/output source trees, complete evidence equality, spatial
dependency and mixed planned/unavailable selection, synthetic multi-file case
counts, unknown case names, and metadata-only negative/fuzz/stress grouping.
The report validator rejects changed totals, dimensions, identities, source
status and assessment claims.

The exact CLI regression
`report_cli::external_report_raw_formats_preserve_evidence_and_reject_old_envelope`
passes 1/1 in 2.38s. It moves a schema-valid synthetic manifest2 metadata fixture
over real smoke file evidence to an unrelated working directory, deletes its
payload files, verifies raw JSON/Markdown and rejects the frozen machine
envelope. This is reporting evidence, not a claim that the synthetic metadata
fixture was produced by the external runner; the separate runner tests prove
that execution boundary. Full ordinary report CLI passes 49/49 in 24.93s,
including all previous 48 entries; SDK facade passes 12/12 in 4.38s.
Schema Fast passes 73/73, release Fast 14/14, resource subsystem 86/86 (7.14s).
`cargo check --locked --no-default-features` passes in 12.92s; formatting and
exact range diff checks pass. Routing passes 25/25; ownership is 22 targets /
271 groups / 1,456 entries, including 903 integration entries. Spelling remains
955 with unchanged digest. The new schema uses current contract references.

Intermediate errors were resolved without weakening evidence: the initial
unsupported envelope text classified as an internal invariant error; it now
uses the existing unsupported-version category. The runner's old report-error
prose assertion was updated to the stable SDK code. Adding one CLI entry
required the fail-closed integration inventory increment from 902 to 903.

**Dated evidence correction:** the earlier internal-runner section's blanket
statement that fuzz execution did not run was too broad. Its full ordinary
report CLI 48-entry suite included
`report_command_isolates_bounded_fuzz_qualification`, which executes a bounded
ordinary fuzz generation/report test. The current 49-entry suite retains and
passes that same entry. Neither run invoked the ignored Heavy dispatcher or
terminal target qualification. The new isolated-profile grouping checks use
metadata only. Earlier text is preserved as historical, with this correction
authoritative for the actual bodies executed. No Heavy/Nightly, feature-specific
codec, external provider, package/archive/RC or external repository work ran in
this reporting slice. No later SDK/CLI implementation began.

### R5 reporting accepted boundary and duplicate-load cleanup — 2026-09-03

Independent review accepted `f568ef9..ea26b52` with no P1/P2 findings. The
bounded P3 follow-up passes the already validated manifest by reference into
the sole-caller private registry report helper. Curated/composition/assembly
report dispatch previously loaded and schema-validated the same manifest twice;
it now performs that load once. External early dispatch, public signatures,
error mapping, resource lease lifetime, schema bytes and report projection
semantics are unchanged. This is a structural reduction in duplicate work,
not a claimed wall-time speedup. No SDK implementation began.

The `src/lib.rs` route dry-run preceded focused ordinary checks:
`report_contract::report_contract_tests::` passes 3/3 in 0.94s (20.57s compile),
and `cli_sdk__nonfast report_cli::` passes 49/49 in 18.89s (21.94s compile),
including the exact raw external JSON/Markdown and rejected envelope regression.
As corrected above, this ordinary suite includes bounded fuzz reporting, not
the deferred Heavy qualification. Ownership remains 22/271/1,456;
spelling, formatting and diff checks pass. No Heavy/provider/RC work ran.

### R5 supported SDK corpus facade — 2026-09-03

Predecessor reporting `f568ef9..ea26b52` and bounded duplicate-load cleanup
`ea26b52..ca571fb` were independently accepted. The SDK slice starts from clean
`ca571fb`; its implementation/evidence/documentation range is
`ca571fb..e78c89a`, pending independent review. Granular commits:

- `6ad1003`: explicit corpus requests/outcomes, manifest2/report2 SDK readers,
  and structural execution error mapping.
- `5f05cfe`: six facade-only consumer tests, direct nested-error evidence,
  and replacement of the former SDK unsupported-reader expectations.
- `50f80e1`: bounded SDK/runner routing and exact ownership inventory.
- `f7cd58b`: preserve typed planner resource-limit causes and direct evidence.
- `e78c89a`: SDK workflow, accessor-version and compatibility documentation.

Both request constructors require the selected descriptor (file or bytes), a
separate explicit member/asset root, output root, and Profile/CaseIds selector.
Capture happens at invocation, not request construction; a subsequent source
mutation before invocation fails integrity checks. Seed, parallelism, dry-run,
and the existing cancellation token feed the runner once. The tested file
descriptor is outside the member root and no canonical descriptor is required.
No ambient root or corpus fallback is added.

Published wraps the validated manifest2 and typed file count/bytes/plan hash.
Planned and NoExecutableCases have no manifest path, publication/validation
NotRun, and complete captured selection/dependency/definition/reason/identity
evidence. Typed preview dispositions include ready, unavailable, planned,
skipped, blocked and deprecated; generated is rejected as a preview invariant.
SDK evidence accessor version `1.0.0` is not a standalone JSON document schema
and exports no internal plan. Loader codes and typed planner/executor errors
remain structural; nested cancellation is distinguished from cleanup failure,
which returns `io.cleanup.failed` rather than claiming successful cleanup.

Exact `route-changed-tests.py --dry-run` inspection preceded the six-path
ordinary route for `src/sdk.rs`, `src/sdk/corpus.rs`, `src/corpus_generation.rs`,
`tests/sdk_corpus.rs`, `tests/captured_corpus_generation.rs`, and
`tests/captured_corpus_report.rs`. The route was executed without deferred work:

| Bounded ordinary command (all Cargo tests locked, no-default-features) | Result | Body time |
| --- | --- | --- |
| `--lib corpus_generation::captured_runner_tests::` | 6/6 | 4.64s; final typed-planner rerun 4.75s |
| `--lib corpus_report::captured_report_tests::` | 3/3 | 3.89s |
| `--lib manifest_contract::external_manifest_contract_tests::` | 7/7 | 2.10s |
| `--test cli_sdk__nonfast sdk_corpus::` | 6/6 | 20.15s; final rerun 21.53s |
| `--test cli_sdk__nonfast sdk_facade::` | Existing 12/12 | 3.22s |
| `--test schema_resources__subsystem cli_contract_schema::` | 4/4 | 0.06s |
| `--test release_ci__fast --test schema_resources__fast` | 14/14 + 73/73 | 1.88s + 1.55s |
| `--test cli_sdk__nonfast report_cli::external_report_raw_formats_preserve_evidence_and_reject_old_envelope -- --exact` | 1/1 | 2.52s |

Final `cargo check --locked --no-default-features` passes in 12.07s. Routing
fixtures pass 25/25 in 2.46s; ownership passes at 22 targets, 272 groups and
1,463 entries (188 integration sources, 909 integration entries). Spelling
remains 955 matches / `59b3e0c69369b2cd47921175e0691c5d62d2dc773b293168d972dcdf61182d0c`;
format and range-diff checks pass.

File/bytes smoke at seed1 and parallelism2 produces equal manifest bytes,
equal payload bytes, three emitted files and 2,790 payload bytes. The existing
runner's R0 smoke and exact-plan parity assertions still pass. SDK evidence
also covers actual dependency closure and multi-file geometry, mixed planned
and generated rows, uncompiled codec unavailability, source deletion followed
by output relocation and validate/report, unrelated CWD, invalid selectors,
zero parallelism, precancellation, preserved existing-output sentinel,
missing/unsafe/oversized/changed input, and public manifest version/identity/
duplicate-runtime rejection. Skipped/blocked/deprecated cases use explicitly
synthetic schema-valid metadata only, not expensive qualification bodies.
The initial fixture failures were test assumptions: macOS `/var` symlink
ancestry, incorrect smoke byte/count expectations, and an unavailable test
hashing dependency; fixes used a real temporary ancestor, observed payload
inventory, an actual geometry multi-file case, and existing fixture tooling.
No production safety checks or byte oracles were relaxed.

No schemas or resource memberships changed: current engine remains 76 members,
1,259,399 bytes / `e8cb8ba376e03d9d4d1451c44a70b3609c0ada11baab16bb96b012b8e6e6cd77`;
schema set remains 53 / 858,927 bytes /
`a562c02e31550c0bac2d2f14b2c7af49b772af1368f19109a0e1b9174a229299`.
Legacy 240/dc61 reconstruction and verified bundle214/571fa23 identity remain
unchanged. These are same-workspace ordinary consumer tests, not installed
package or independently qualified external-consumer evidence. No CLI corpus
input, discovery, report-result2 envelope, public whole-preview JSON schema,
external runtime discovery/execution, Heavy, Nightly, feature matrix, package,
release-candidate, external repository or release work ran. Earlier dated SDK
unsupported statements describe their then-current slices; this section
supersedes them only for the supported facade, not the CLI envelope boundary.

### R5 SDK review corrections — 2026-09-03

Independent review rejected `ca571fb..e443b15` for a P2 SDK-guide example
and P3 adversarial-test precision defect; the prior passing checks did not
exercise those exact conditions. Commit `bc3a298` fixes only those boundaries:
the file example uses `./definition.json`, explicitly documents the required
parent component, and preserves the loader's rejection of bare filenames.
The existing isolated-CWD subprocess now executes the exact documented
descriptor/member/output spellings, seed1/parallelism2, generation, validation,
and report2; bare `definition.json` fails `resource.document.invalid` without
creating its output. No global CWD mutation or loader behavior change occurs.
The malformed-digest test now mutates the existing
`identity_projection.engine.engine_sha256` via a checked pointer, rather than
adding an unknown `sha256` property. Both public SDK reader paths reject it.
Search found no other current `GenerateCorpusRequest::from_file` examples;
composition's distinct parent-inference contract remains unchanged.

After the docs/test route dry-run, `cargo test --locked --no-default-features
--test cli_sdk__nonfast sdk_corpus::` passes 6/6 in 21.04s (1.99s compile).
The exact isolated example regression, adding
`sdk_corpus::sdk_corpus_works_from_unrelated_cwd -- --exact`, passes 1/1 in
9.44s. Ownership stays 22 targets / 272 groups / 1,463 entries; spelling
remains 955 with the unchanged digest above. Formatting and diff checks pass.
No production code, schemas, identities, discovery, CLI corpus input,
Heavy/provider/package/release evidence changed or ran. The corrected slice
awaits independent re-review.

### R5.2 external corpus CLI and result contracts — 2026-09-03

Independent review accepted SDK range `ca571fb..aa7fcad`. This next sequential
slice starts at clean `aa7fcad` and records implementation through `573187a`,
pending independent review. No R5.5 discovery implementation began.

| Commit | Coherent unit |
| --- | --- |
| `d0871e4` | External-only generation-result3/report-result2 schemas, compatibility ownership, current resource/schema inventories |
| `48d2b10` | SDK-only external generation dispatch, typed SDK error bridge, external report2 envelope |
| `4e25e77` | Five bounded CLI consumer/error/state tests and migrated external report-envelope regression |
| `487d79d` | Fail-closed live CLI/SDK/report routing and exact test ownership |
| `acd7698` | One frozen profile-schema URI spelling record and reviewed snapshot |
| `909ace7` | Explicit CLI API mode selects JSON when format is omitted; exact preview regression |
| `a8659f0` | Current command/result/evidence documentation |
| `573187a` | Exact documented CLI validation after source removal |

`generate --corpus ./definition.json --asset-root corpus-members --profile
smoke --out generated/cli --seed 1 --parallelism 2 --format json` invokes only
the supported SDK for execution. Required member root and explicit descriptor
parent remain independent. The pre-dispatch scan respects option-value slots,
so an output value spelled `--corpus` does not accidentally select the new
route. Formats, options and numeric syntax are validated before resource
construction/corpus capture or publication. SDK errors retain their registered
codes, exit class, retry flag and meaning without Display-string parsing.

External generation uses generation-result `3.0.0` in CLI API1, while embedded
generation2 stays unchanged. Published, planned and no-executable results
retain exact ledger/definitions/identity/scope/plan facts and explicit logical
case versus emitted-file counts. Empty dry-run stays planned; no-executable
has no ready rows/artifact IDs. Nonpublication has null manifest path, zero
emitted bytes/files and validation/publication not_run. No internal plan is
serialized. Report JSON/Markdown stays lossless; only external machine reports
use report-result `2.0.0` with a strict report2 reference, leaving report1 for
legacy kinds. CLI signal cancellation is not implemented or claimed.

All affected paths were dry-run routed first. Build/main remain conservative
all-ordinary fallbacks in the router; this explicitly assigned bounded slice
ran the focused ordinary evidence below, not the broad fallback or deferred
classes. New result schemas additionally select their owning live CLI, SDK
and report tests as well as schema coverage.

| Exact bounded check (Cargo tests use `--locked --no-default-features`) | Result | Body time |
| --- | --- | --- |
| `--test cli_sdk__nonfast external_corpus_cli::` | 5/5 | 20.91s |
| `--test cli_sdk__nonfast report_cli::` | 49/49 | 20.68s |
| `--test cli_sdk__nonfast sdk_corpus::` | 6/6 | 21.28s |
| `--test cli_sdk__nonfast sdk_facade::` | 12/12 | 3.20s |
| `--test cli_sdk__nonfast generate_cli::generate_command_writes_smoke_part10_files_and_manifest -- --exact` | 1/1 | 1.85s |
| `--test schema_resources__subsystem` | 86/86 | 10.95s |
| `--lib engine_resources::snapshot_cache_tests::` | 7/7 | 2.36s |
| `--lib identity_domain_tests::` | 4/4 | 1.33s |
| `--test schema_resources__fast` | 73/73 | 1.63s |
| `--test release_ci__fast` | 14/14 | 1.92s |
| Exact external CLI planned/no-execution test after explicit API-mode correction | 1/1 | 20.12s |
| Exact external CLI profile/SDK-parity test including documented validate/report commands | 1/1 | 8.13s |

The 49-test ordinary report suite includes its existing bounded fuzz reporting
body, not deferred Heavy fuzz or target qualification. Initial Fast failures
correctly identified the missing retained-URI entry and then a snapshot digest
that omitted the reviewed owner/reason; both were corrected, not suppressed.
Final spelling is 956 matches /
`d695b8a5cdce36c11352a07f55f11df30a0b45b3c38234d2a3d7aef56597f5b9`.
Routing fixtures pass 26/26 in 2.53s; ownership passes 22 targets, 273 groups,
1,468 entries (189 integration sources, 914 integration entries). Final cargo
check passes in 0.25s; format/diff checks pass.

Exact current resource changes are only the two schemas:

- generation-result3: 7,015 bytes /
  `eee219e1fcc403f811416631e228c88b88b555df8310845b7384712602846dfd`;
- report-result2: 681 bytes /
  `3822642a4d4189523255eb4310b91cf06c7362f1d45fc7a84b0550425dda9f0a`;
- current engine: 78 members / 1,267,095 bytes /
  `668dd330fe4bc80c66910c45fdd86510b76ced2fdd4694dcbfb18f4a04497209`;
- schema set: 55 members / 866,623 bytes /
  `084e17c32fa2c500478f3cd89a96cb758b17750eb8a82f42f50eec2b1b2cea69`;
- physical private snapshot: 258 files / 2,680,353 bytes. Legacy240/dc61 and
  verified bundle214/571fa23 identities are unchanged.

The documented fresh caller-relative command produces three smoke files /
2,790 payload bytes identical to SDK generation at matched seed/parallelism,
with equal manifest bytes. Tests also prove dependency counts, schema-bound
state constraints, source deletion before CLI validation/reporting, invalid
format/options/IDs/profile/parallelism, missing/unsupported/oversized/tampered
descriptor input, preserved existing destination, and exact typed error
envelopes. Frozen embedded reader/result fixtures remain covered.
Capabilities2 is deliberately not changed: its frozen validation inventory
cannot advertise generation3. Loaded-corpus and complete result discovery
await capabilities3 in R5.5. No package/installed-consumer, Heavy, Nightly,
codec feature matrix, external provider execution, release-candidate,
external repository or release qualification ran; terminal acceptance remains
open.

### 2026-09-04 — R5.5 supported loaded-corpus discovery (pending review)

Independent review accepted preceding CLI range `aa7fcad..a18d149`; the SDK
range `ca571fb..aa7fcad` remains accepted. This bounded discovery slice begins
at `a18d149`. Functional/evidence commits are `7a453bc`, `205ed7e`, `5cda053`,
`798cac4`, `e2b2cda`, `ca64c0b`, `3a99e60`, `b1dc5b6`, `bfdae8d`, and
`f0b5f71`; documentation/status follow separately. No later phase began.

`InspectCorpusRequest` accepts descriptor file or bytes with an explicit member
root, optional selection, seed and parallelism. Inspection captures at call
time. Metadata-only results explicitly have no assessment; typed case status,
profile IDs, exact definitions, provider/requirement/blocker facts and verified
identity remain available. A selected assessment shares the exact lease-owning
preparation used by generation, without a fake destination, filesystem output
probe, service construction or provider execution. It preserves direct and
dependency dispositions/reasons, seed, parallelism, artifact IDs and plan hash.
Ready is not generated, and validation/publication remain not-run. Cooperative
cancellation is checked before/after bounded capture and planning.

`capabilities_with_corpus` projects that one capture into capabilities3; top
and nested corpus identities agree without reloading inputs. CLI
`capabilities --corpus ./definition.json --asset-root corpus-members --format json`
is metadata-only. Adding `--profile smoke --seed 1 --parallelism 2` assesses
the selected scope. Repeated case IDs and stress remain scope-bound; planning
options require a profile. CLI errors retain SDK codes structurally. No CLI
signal cancellation is claimed. SDK evidence-accessor version1 is not a
standalone JSON document or an exposed private plan.

Provider discovery is supplied by `qualified_templates`, `transfer_syntaxes`,
`optional_runtimes` and the new `provider_support` declarations from the
installed locked backend catalog. Native compiled support is distinct from
unassessed external declarations; all runtime assessment is explicitly not
performed. Loaded definitions preserve their exact provider/requirements facts,
and selected readiness/unavailability comes from the same planner as execution,
not registry implemented status. Empty-PATH CLI inspection is identical to the
ordinary result; no ambient provider or executable discovery is required.

Capabilities3 advertises external generation3, manifest2, report2 and bundle1,
and retains predecessor validation windows. Frozen capabilities1/2 and
version2 schemas are unchanged. The existing release2 contract strictly binds
capabilities2, so the default discovery change necessarily introduces release3
with capabilities3; release2 remains frozen and valid. The verifier retains
1/2/3 with exact 2→2 and 3→3 pairing, domain/product/target/features/source,
inventory, runtime uniqueness, and optional legacy parity checks. Tests use
synthetic manifests only: the v2 compatibility projection is explicitly not a
historical or qualified archive. Deferred archive schema dispatch and installed
black-box expectations are updated but their bodies were not run.

Routing dry-runs were inspected for the changed SDK/runner, discovery, CLI,
schema and release paths before focused checks. Capabilities3 selects live
external CLI/SDK, identity, schema and release evidence; release3 selects
schema/release evidence. The conservative main/build all-ordinary fallback was
not executed; the authorized bounded affected suites below were used instead.
All Cargo test rows use `cargo test --locked --no-default-features`:

| Exact test arguments | Result | Test wall time |
| --- | --- | --- |
| `--lib corpus_generation::captured_runner_tests::` | 6/6 | 4.68s |
| `--test cli_sdk__nonfast sdk_corpus::` | 7/7 | 35.72s |
| `--test cli_sdk__nonfast sdk_corpus::inspection_is_destination_free_and_agrees_with_generation_planning -- --exact` after input-error additions | 1/1 | 28.48s |
| `--test cli_sdk__nonfast capabilities_cli::` | 4/4 | 1.66s |
| `--test cli_sdk__nonfast external_corpus_cli::` | 6/6 | 24.69s |
| `--test cli_sdk__nonfast sdk_facade::` | 12/12 | 3.53s |
| `--test cli_sdk__nonfast version_cli::` | 3/3 | 1.14s |
| `--test cli_sdk__nonfast report_cli::` | 49/49 | 20.08s |
| `--test schema_resources__subsystem` | 86/86 | 6.90s |
| `--test schema_resources__fast` | 73/73 | 2.04s |
| `--test release_ci__fast` | 14/14 | 2.12s |
| `--lib identity::identity_domain_tests::` | 4/4 | 1.39s |
| `--lib engine_resources::snapshot_cache_tests::` | 7/7 | 2.14s |

The report suite includes its existing ordinary bounded fuzz-report test, not
Heavy qualification. The inspection comparisons cover smoke, dependency,
planned and unavailable selections without heavy generation. Existing external
CLI smoke remains SDK-byte-identical. Initial failures were a test's wrong
digest key, moved JSON-schema options in new test helpers, a rustfmt-sensitive
static assertion, and routing assertions that omitted the deliberately added
overlap; all were repaired and rerun, with no evidence body removed.

Current schemas/resources (these supersede earlier dated inventories):

- capabilities3: 20,696 bytes /
  `d79781d45f2482ec8a9a46524b5ac0010303c874b4324761c8f4db6e13d7174a`;
- release3: 2,611 bytes /
  `b08de95b56bc595e05d69238fd54672eef2a61e68978894f1bb705b0074ad78c`;
- engine: 80 members / 1,290,402 bytes /
  `76e335c57a4b6f9aeefda2cf56b2bfea83231440d9d69e1e78ab4d81d1ce0740`;
- schema set: 57 members / 889,930 bytes /
  `a27fc5915c974c51a142b8bc4772a5fbc10a0077196e8a58e96a4a7cba378a83`;
- physical snapshot: 260 files / 2,703,660 bytes. Legacy240/dc61 and verified
  bundle214/571fa23 identities remain unchanged.

Routing fixtures pass 26/26 in 3.316s. Ownership passes 22 targets, 273 groups,
1,470 entries (189 integration sources, 916 integration entries). Spelling
passes 957 matches /
`a211351385574ab085b0b660de6c0588c914ebc6ba7823735c59f6dfe8a843da`;
the sole new retained occurrence is the frozen case-registry schema reference.
Cargo check passes in 13.76s; formatting and diff checks pass. No Heavy,
Nightly, codec feature matrix, external provider, package/archive build,
installed-consumer/RC body, external repository or release publication ran.
Independent review and the supported installed/cross-repository consumer proof
remain required; R5 and terminal acceptance are not declared complete.

### 2026-09-04 — R5.5 deferred harness compile correction

Independent review rejected `a18d149..d5cf159` for one compile blocker in the
touched deferred release harness. Its schema-registration loop discarded the
owning `with_resource` return value. Commit `3b823f1` retains the returned
options. The earlier ordinary/static checks passed but did not compile this
deferred target; they were insufficient evidence for its buildability. The
previous dated test results remain historical ordinary evidence, not release
execution evidence.

The `tests/release_archive.rs` route dry-run still correctly defers its bodies
to release-candidate qualification. Explicit compile-only
`cargo check --locked --no-default-features --test release_ci__nonfast` passes
in 0.41s; no test body or archive build ran. `sh -n` passes for
`scripts/build-release-archive.sh`, `scripts/validate-release-manifest.sh`, and
`scripts/verify-release-archive.sh`. Python AST-only parsing passes for
`tests/black_box_cli_consumer.py`, `scripts/check-test-ownership.py`, and
`tests/test_change_test_routing.py`, without running consumer/qualification
bodies. Ownership remains 22 targets / 273 groups / 1,470 entries; formatting
and diff checks pass. Resource identities and generated behavior are unchanged.
This narrow correction awaits independent re-review; no later phase began.

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
| External corpus contract | In progress | R4.2 proves the bounded verified definition loader; accepted R5 SDK/CLI slices expose explicit-root selection/generation without unsupported execution imports. Supported loaded-corpus discovery is implemented pending review; independent installed/cross-repository consumer acceptance remains required before this terminal row passes. |
| Identity separation | In progress | R4.3 independently projects engine, toolchain, template, provider, schema, standards, execution, verified loaded-corpus, and invocation runtime identities through v2 discovery and current generation, composition, assembly, report, and release contracts. R4.4 removes `cases/**` and `Cargo.lock` from the authoritative v2 engine digest while retaining exact named v1 reconstruction for required compatibility fields. Embedded paths do not fabricate corpus or runtime identities. The supported R5 external-corpus generation route and later physical/default-reader migration remain before the terminal cross-repository row can pass. |
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
