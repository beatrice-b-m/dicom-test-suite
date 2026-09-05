# `synth-dicom-gen` / `dcmview-test-corpus` migration status

**Recorded:** 2026-09-01

**Updated:** 2026-09-04

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
| R5 — add supported external corpus API | Complete | R5.1–R5.5 and isolated-source consumer gate independently accepted | SDK `ca571fb..aa7fcad`, CLI `aa7fcad..a18d149`, discovery `a18d149..5da18e8`, and the 2026-09-04 source-removed consumer proof at exact `232b9de` are independently accepted. This closes R5, not package/release qualification or R6+ terminal cross-repository evidence. |
| R6 — establish smoke corpus repository | Complete | R6.1–R6.5 independently accepted | The approved local repository at `/Users/beatrice/AgentFiles/projects/dcmview-test-corpus` owns its foundation, exact offline pin, imported smoke definitions, supported runner, full R0 smoke parity, viewer-result contract, and locally executed CI configuration. The R6 gate passes without compiling/testing generator internals. Hosted delivery, viewer execution and terminal release/CI evidence remain separate and unqualified. |
| R7 — migrate complete dcmview corpus | In progress | First ten native SC core and metadata3 slices imported/parity accepted | Exact source-pinned content0.2 definitions and isolated proof at corpus `c07b9c1` pass core10 profile/IDs/repeat plus smoke, complete file parity, strict validation and report2. Metadata3 content0.3.0 failed the canonical evidence-path gate; corrected0.3.1 at `54edb77` preserves all note/recipe/registry bytes, exact loading passed at `ca561cb`, and the separately approved two-run parity proof at `ce04867` preserves all three payloads, metadata and standards evidence. Original baselines and failed evidence remain authenticated. Remaining ordinary native, relationship, codec, provider and isolated special scopes are still required; R7.2/R7.3 genericity debt and embedded-copy terminal removal remain open. |
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

### R5 isolated immutable-source SDK/CLI consumer proof — 2026-09-04

Predecessor discovery range `a18d149..5da18e8` is independently accepted,
including compile-only remediation of the deferred release harness. This
supersedes the earlier dated pending-review wording without changing its
historical measurements. Proof implementation commits are `bc8e6a9` (harness,
SDK-only fixture, static checks), `3dd9fed` (explicit-boundary routing),
`60bd201` (scope documentation), `6c0b117` (preserved dependency resolution),
`95e7aba` (host-scoped offline metadata), `2558146` (actual reader assertions),
and `232b9de` (same pinned compiler for both builds). No generator behavior,
schema, recipe, or DICOM oracle changed.

The successful command was:

```sh
python3 scripts/prove-isolated-corpus-consumer.py --revision 232b9de41f97ee95abe1ecc40b6b8b70ebeeea5f --artifacts /private/tmp/synth-dicom-gen-r5-consumer-20260904-proof4 --retain /Users/beatrice/AgentFiles/projects/dicom-test-suite/generated/r5-isolated-consumer-20260904-proof4
```

The starting revision was clean and committed. Its immutable git archive is
18,073,600 bytes, SHA-256
`e63b7ce5380e6f0c7519b754bd0e8f5acbefebc3217f992ad61a07002244e78a`.
The receipt SHA-256 is
`b4e39eeaa8aecb8a686809c3516131738a8ce73861b78524f40b2eaac02d60c0`.
Runtime paths remain recorded verbatim in the receipt; the durable ignored
workspace directory above contains the identical archive/receipt, logs,
binaries, caller definitions, full manifests/reports/ledgers and baseline.
It holds 87 files / 147,338,727 logical bytes / 147,488,768 allocated bytes.
No generated evidence is tracked in Git.

The external crate imports only `synth_dicom_gen::sdk` from the product.
Cargo metadata resolves its dependency solely to the extracted immutable
source, not the original checkout or a sibling. Both builds use offline,
locked, no-default-features `aarch64-apple-darwin` compilation with
`rustc 1.85.0 (4d91de4e4 2025-02-17)` and
`cargo 1.85.0 (d73d2caf9 2024-12-31)`. The consumer lock is seeded from the
snapshot; all 136 registry dependency version/source/checksum tuples match.
Snapshot lock SHA is `4aa4b6c94043fb2f236ec888ac9b253f2ff451b666464609f81f82aaac6d8a4d`;
consumer lock SHA is `86bc1659509512efad20d82b84b348ffd1a21446f07fc2ebc12294b5f31353cf`.
The consumer and CLI binaries are respectively 57,741,456 and 69,314,672 bytes,
SHA-256 `37b6dee311510f2c1c815b3dd6edd491fc9c0fad81e1969a3eccac0b86997b43`
and `4ca0c6d6a8e4cbab81b7005b4354c7ff558c44747e00f41d22d3abbfd50b7768`.

Both exact extracted source roots were removed before runtime. Execution
used an unrelated private working directory and empty PATH. The caller's
three-case smoke subset retains unchanged recipe bytes, the fixed eight-profile
contract, and a schema-valid `metadata` compatibility-axis marker preserved
in every generated definition row and lossless report. Its verified identity
is `isolated-sdk.smoke` / 1.0.0, five files / 14,462 bytes, descriptor SHA
`8f45985cee3b70e77eaf68bfc393f6a0cc87ddda0baeb6c8aa62ef008a1a960e`, corpus SHA
`572f3b53a31ec87dad171aaff10eda01c8f2894bc37b65949374753e83792248`.
The installed split engine-core domain remains three members / 20,643 bytes /
`4268d9216842aaaca8e9ea1d3fd8e8538d7d02124deccf8cd17b63c180b86276`;
this is not the aggregate 80-member EngineResources identity. Schema identity
remains 57 / 889,930 / `a27fc5915c974c51a142b8bc4772a5fbc10a0077196e8a58e96a4a7cba378a83`;
legacy 240 / `dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410`
is unchanged. All complete installed domains are retained in the receipt.

SDK file/profile, bytes/case-ID, and reproduction runs plus CLI profile/ID
runs pass supported strict validation. Full matching-selector manifests,
capabilities, and definition-driven reports compare equal without identity
normalization. All five fresh outputs reproduce the three exact R0 smoke
hashes (926 + 926 + 938 = 2,790 DICOM bytes), UIDs and recorded per-case
semantics, including 25 internal / four standards / zero external checks
per file. This does not claim equality to the old whole-run plan/manifest
digest: selector and identity contracts deliberately changed. A separate
read-only comparison proves SDK preview, CLI preview and published CLI plan
SHA equal `6566c25b4280f3b4694cb2bdabe6bc1ba7322b6b83b5fe936e3107360f6b4bb0`.
SDK/CLI dry-run and a tiny planned-only selection remain nonpublished with
complete evidence and no output directory. SDK pre-cancellation and canonical
descriptor conflict preserve their registered error codes. No CLI signal
cancellation or independent conformance claim is made.

Consumer build took 25.656s, CLI build 19.500s, and all runtime commands
12.078s (SDK scenario 5.009s). Target measurement was 1,002 files /
880,565,231 logical bytes / 885,231,616 allocated bytes, then the exact target
was removed. Each CLI output holds three DICOM files plus manifest: profile
94,845 bytes and IDs 95,006 bytes. SDK outputs additionally retain reports:
profile/repeat 168,249 bytes each and IDs 168,529 bytes. These bounded scope
measurements are not a comparable replacement for R0 all-target build costs.

Failed attempts remain durably recorded under matching ignored `proof1`,
`proof2`, and `proof3` directories. Attempt1 failed before compile because
unfiltered metadata tried to unpack an irrelevant Android cache entry into
read-only cache. Attempt2 compiled but its fixture expected a JSON parse error
instead of the earlier canonical-descriptor conflict; attempt3 correctly
rejected unequal toolchain identities (consumer default 1.92 versus pinned
CLI 1.85). The final harness fixes those causes; none was converted into a
pass or normalized away. All failed target trees were measured if present
and removed; their archives, logs and receipts remain available.

Route dry-runs select only static ordinary evidence and defer actual consumer
execution to this explicit R5 boundary. Python fixture tests pass 4/4 (0.031s),
the exact Fast `ci_release_gates::isolated_corpus_proof_has_sdk_only_inputs_and_bounded_static_fixtures`
passes 1/1 (0.08s), routing tests pass 27/27 (2.837s), ownership passes
22 targets / 273 groups / 1,471 entries, spelling passes 957 occurrences,
and formatting/diff checks pass. Offline cached dependencies are required.
No cargo package, installed-release consumer, remote clone, release archive,
Heavy, provider, codec, Nightly or RC qualification ran. Independent review
accepted proof4 after verifying artifact hashes, toolchain/host/metadata,
source removal, complete parity and all 15 DICOM payload copies. R5 passes;
R6 location/remote authority and all later terminal rows remain open.

### R6.1 approved local foundation — 2026-09-04

Following explicit user approval, the local corpus repository was initialized
at `/Users/beatrice/AgentFiles/projects/dcmview-test-corpus`, on `main`, without
a remote. Foundation commits `a6381d9`, `3c90f1b`, and `7a0e0fc` separately
establish policy/ignore rules, README/licenses, and dated evidence. Root and
independent review accepted the six-file foundation: licenses match upstream
exactly, generated/private artifacts are ignored, documentation claims no
runnable workflow, and the handoff worktree was clean. No generator code or
case definition moved in this slice.

R6.1 remains partial. The next sequential slice pins the already measured R5
source-built native candidate for explicit offline acquisition through the
supported CLI. The corpus must not discover a sibling checkout, compile or
test generator internals, or label that candidate release-qualified. Initial
availability is limited to the exact no-default-feature macOS arm64 artifact;
unsupported hosts and absent artifacts must fail explicitly. Local acquisition
and fresh-clone evidence do not prove remote hosting or hosted-CI execution.
The recorded origin ref still predates the accepted R5 revision; no remote
availability is inferred from a local commit. Remote/publication authority
remains separate from the approved local work.

### R6.1 explicit local acquisition accepted — 2026-09-04

Corpus range `7a0e0fc..1c922af` is independently accepted for acquisition only.
Its lock pins the exact R5 native binary (69,314,672 bytes,
`4ca0c6d6a8e4cbab81b7005b4354c7ff558c44747e00f41d22d3abbfd50b7768`)
and complete version/capabilities document digests. The explicit caller input
is copied and reverified before private execution; an ignored content-addressed
cache is checked without replacement. Review caught and corrected a
same-bytes/wrong-permissions cache acceptance defect in `e27487d`; all 20
synthetic tests pass independently. Exact owner/mode checks now apply to both
existing entries and publication winners.

Actual post-fix first import and cache recheck passed in 1.737160167s and
1.657543125s, with 69,316,608 allocated cache bytes. The corpus retains commands,
implementation revision, complete discovery results, and measurement receipt
`37e5ebcc4c5b2a6d7622c7faf6efc2cf3cd87883c779a8a931af89ad492ab8ad`
under ignored `artifacts/acquisition-reviewed-20260904/`. Independent review
verified receipt/result/binary hashes and cache ownership/permissions without
reexecuting the candidate. The clean handoff is `1c922af`. This is explicit
local/offline acquisition, not a release, remote fetch, corpus-generation or
full R6 gate. Schema fixtures, definitions, runner, parity, viewer contract and
CI remain to be accepted sequentially.

### R6.2 smoke definitions and schema fixtures accepted — 2026-09-04

Corpus range `1c922af..d6ea8f8` is independently accepted for R6.2 and the R6.1
schema-fixture subset. Three registry rows and recipes preserve exact source
`232b9de` metadata, recipe bytes, case IDs, profiles and covered standards
evidence. All seven copied recipe/schema/baseline blobs match that source.
The canonical five-file bundle is `dcmview-test-corpus` version `0.1.0`,
14,403 bytes, corpus digest
`1daba69474fd1864dee80b09e2d0008ff97bb0c2f05ac20fd22177c26504ccc3`;
descriptor digest `f28ddec1b14281c78295d56ae10dde8e62ccdeedae7d6108cc81fed4fa3ae2ac`.
No R5-only metadata marker, generator template/catalog copy, or implicit sibling
dependency was introduced. Upstream embedded definitions remain until R9.

Five static checks verify source hashes and reference closure, without claiming
to implement JSON Schema. The complete pinned public CLI loader passed seven
recorded checks at clean harness `d29c96c`: three ready cases plus rejection of
unknown version, descriptor/registry/recipe schema errors, schema-valid profile
inconsistency, and member tampering. Calls total 2.552s. Receipt
`ffed8025de4fcd306469c8a2f164896970c672a94ec674a858efb0c23e3ea99c`
is retained in corpus `artifacts/definition-validation-reviewed-20260904/`.
Independent review verified the receipt, identities, rejection classes and
removed temporary fixtures; no generator replay was needed. All 25 ordinary
corpus tests pass. No generation occurred: publication and validation remain
`not_run`. The supported runner, migration parity, viewer result contract and
CI still remain before R6 can pass.

### R6.3 local CLI runner accepted — 2026-09-04

Corpus range `d6ea8f8..cd4783e` is independently accepted for the local runner
slice, not yet clean-clone acquisition or R6.4 parity. It consumes only the
supported CLI through the pinned native artifact, reverifies the cached binary
before each command, and preserves typed published/planned/no-executable
outcomes. Published results require actual validation success and matching
lossless report evidence. Returned paths, counts, request scope, ledger and
per-file plan hashes are checked; consumed-field parsers are not advertised as
a general JSON Schema evaluator.

The initial `5ba1dfe` run correctly failed strict validation because helper
evidence placed inside the generated root was undeclared. The failed output
and records remain preserved; generator validation was not weakened. Corrective
commit `7819bb5` uses an exclusive adjacent `<out>.dcmview-run` directory,
preflights both destinations, and creates neither for nonpublished outcomes.
Later failures preserve published output and report publication uncertainty
when a trusted response is unavailable.

Four actual checks at `7819bb5` passed: smoke 4.306s, one explicit case ID
3.476s, dry-run 2.802s, and empty-core no-execution 2.699s. Smoke output contains
four files / 94,780 logical bytes / 106,496 allocated bytes; its seven-file
adjacent evidence occupies 417,047 logical / 434,176 allocated bytes. Receipt
`95123a97f23c59bc4592709df4b4ec982e51a0d16e58276824f88068165465a4`
is retained in corpus `artifacts/runner-validation-reviewed-20260904/`.
Independent review verified result hashes, closed output inventories, payloads,
manifest/validation/report consistency, and absent nonpublished destinations.
All 19 runner tests and 45 ordinary corpus tests pass; post-evidence strict
validation also passes. No generator build, remote, Heavy, package or release
work occurred. Clean-clone proof, complete migration parity, viewer results and
CI remain before the R6 gate.

### R6.3 supplemental complete consumer-schema check — 2026-09-04

After the accepted runner checkpoint, a read-only root audit evaluated the
complete consumer result schema with locally available Ajv `8.18.0` (Draft
2020-12 implementation) under Node `24.19.0`. The generator-lock schema was
explicitly registered for its referenced URI; no network resolution occurred.
Meta-schema validation and compilation passed, and all four retained corrected
runner outcomes (smoke, case ID, dry-run and no-execution) validated with zero
errors. Evaluation used `allErrors: true, strict: false`: standard schema
evaluation, without Ajv's additional schema-authoring lint restrictions.
This is complete evaluation of the consumer-owned schema, not a claim that
the runner's consumed-field parsers evaluate arbitrary upstream schemas.

The validator was an existing local installation at
`/Users/beatrice/node_modules/ajv/dist/2020.js`; it is verification tooling,
not a new corpus runtime dependency or supported machine-specific acquisition
path. No package was installed and no generator command or build was repeated.
The four inputs remain hash-bound by runner receipt `95123a97f23c59bc4592709df4b4ec982e51a0d16e58276824f88068165465a4`.
Node executable SHA-256 is
`27db838bb204ef7c21df2931f5656e4c8fb32e6e947f363a402b49714d32b5b1`;
Ajv package-metadata SHA-256 is
`720f862de3e496df05e074c33df5174db92c47e60726e41ab1338cfefda9754c`.
Consumer schema SHA-256 is
`440ba785926505f6be441d5960b927a7a147c279a769fb41081467d11a6817c6`;
registered lock schema SHA-256 is
`8d418149a2b8ca890453c19bec9a0056ea9b3bbccaa669fe8e0967697a88e9bc`.
The successful verification process completed in approximately 0.2 seconds.

### R6.3 clean clone and R6.4 full smoke parity accepted — 2026-09-04

Corpus range `cd4783e..9cf30d7` is independently accepted for local clean-clone
acquisition and full R6.4 parity. Immutable candidate
`3a059de5aa8c10b086fcf58575ce16a2d1235227` was transferred as an 84,015-byte
Git bundle, SHA `9c9e07bdad307bcce64d6838e9c1c973b9b62b5e892d03e64ee1aa697f5ad7e1`.
The private clone has no alternates or remote; its input bundle was removed
after preserving a portable evidence copy. Absolute Python commands used a
restricted environment and unrelated working directory; resolved modules came
from the clone. The explicitly supplied binary was copied outside both real
repositories. No generator compilation, tests, or sibling lookup was needed.
Original workspaces were not moved or denied by an OS sandbox, and this is not
remote acquisition or package/release qualification.

A recovered byte-identical baseline manifest, not the deleted original R0
directory, matches raw SHA `6a6540ba8acc13afa5e76e35e46d246d77f46ffdc2e5dcce0497fb882ab684eb`.
It remains ignored, hash-bound evidence. All four fresh three-case runs match
the exact 14,109-byte R0 normalized projection
`18f154c38903677cadf4f955b0658ed2fd59162c44a970a9b15c5dc9905eabcd`,
including full recipes, case semantics, validation check names/statuses, and
standards evidence; all 12 payload copies match the frozen hashes. Only the
explicitly versioned run representation and verified empty skipped set are
normalized. Profile/repeat manifests and reports are identical; explicit IDs
differ only in selector. A separate clone-only metadata perturbation changes
only corpus identity while preserving file evidence and every installed
identity domain. It does not modify the committed corpus or baseline.

Recorded runtimes are profile 4.303s, IDs 3.636s, repeat 3.626s, metadata
perturbation 3.695s, and final post-sidecar strict validation 0.333s. The clone
source occupies 516,096 allocated bytes, each binary/input cache 69,316,608
bytes, and the measured entire proof root 141,910,016 allocated bytes. No build
tree was produced. Five focused and all 50 ordinary corpus tests pass.
Receipt `5d2904f5f15ea4dcbf644a249d87b7d37c4f90adeca5167e2d7fb45f8d836805`
and complete inputs/results/sidecars are retained in corpus
`artifacts/clean-clone-proof1-20260904/`. Independent review verified transport,
hashes, all projections/payloads, identities, output closure and costs without
native replay. Full R6 still requires its viewer-result contract and CI;
terminal remote/target/release evidence is not implied.

### 2026-09-04 — R6.5 read-only viewer-result contract accepted

Corpus implementation range `9cf30d7..f9e30e0` adds the versioned smoke
viewer-result schema, typed read-only validator and synthetic adversarial tests.
It binds exact manifest2 bytes, corpus identity and every generated artifact's
case ID, path, size and hash. Multi-artifact cases require one observation per
artifact. Runtime availability remains distinct from per-artifact outcomes;
mixed failures are retained, and successful contract validation never means
viewer compatibility or DICOM conformance. Evidence references are bounded,
hash-checked files outside the generator root, resolved against the result's
parent without traversal, symlink or special-file acceptance. File integrity
does not establish observation authenticity. No generator evidence is edited.

Review corrected schema/parser path disagreement and two malformed-input
traceback branches (deep JSON and invalid Unicode paths). All 20 focused tests
passed independently in 0.195s; root ran all 70 ordinary corpus tests in 0.282s
with no failures and a clean diff check. These are synthetic contract checks,
not generator qualification or actual viewer observations.

Exact code candidate `f9e30e0d40736a5d9d88637ed5c2ac37b51554f8` checked the
existing strictly validated smoke root from an unrelated working directory in
0.042943s. The retained sample is explicitly `synthetic_fixture`, with three
`not_run` rows, zero other outcomes and no observation evidence. All four
generator-owned files remained hash-identical; neither generator nor viewer
was executed. Corpus `artifacts/viewer-contract-20260904/` retains the sample,
raw command output and receipt. Sample SHA is
`4b4c02ac7d4718f1e524bca1ec642cedc88f03986587ce39c20e28d0357bafe8`;
receipt SHA is `3bed8d9743bf4aa3051d51ddee51331ecef970ee7aff218ec12719171f7309b7`.

Root additionally evaluated the complete viewer schema with existing Node
24.19.0 and Ajv 8.18.0 (Draft 2020-12, `allErrors:true`, `strict:false`, no
installation or corpus runtime dependency). Schema/meta compilation and all
76 expected evaluations passed: valid/invalid fixtures and actual not-run
sample, twelve path variants, sixty runtime/status/evidence/version
combinations, and trailing-newline digest rejection. Schema SHA is
`6a356557f0e6775d142365e779816cd9defb4cbc0eab5ab56fa633d04f3ef173`.
This validates the complete consumer viewer-result schema, not the opaque
upstream manifest's entire schema, observation truth, independent conformance,
or later media/protocol scopes. R6.1 CI remains before aggregate R6 acceptance.

### 2026-09-04 — R6.1 local CI and aggregate R6 gate accepted

Corpus commits `882d33f` and `2fb2fc2` preserve exact historical import
assertions against an immutable 4,350-byte registry fixture (SHA
`39797914356219d903e67852d6ed7396d2f7bd61fe53d01d58e988cfc8cd7943`).
Current rows retain every execution, availability, standards and identity field;
only additional compatibility-axis annotations may vary, with refreshed member
hashes. All original recipe/schema and R0 assertions remain. A corrective
regression ensures already-present metadata cannot break the test itself.
No live corpus content or frozen parity baseline changed.

CI configuration `84762ec`, entrypoint `80e57d4`, tests `a1b5a96`, operating
docs `169f55e`, and result-binding correction `5e8a6af` implement one locally
executed, configuration-driven job. Exact committed base/head, ancestry and
clean checkout are required. Unconditional ordinary tests cannot be disabled
through configuration. Docs-only paths never generate; old/new recipe owners,
semantic registry/profile changes, reverse dependents and required dependencies
determine case selection. Shared changes use bounded smoke. Unknown paths,
removed case IDs and unsupported non-smoke scopes fail closed until explicit
migration. This scope restriction does not waive any later R7 or terminal row.

Each real job requires the explicit pinned macOS arm64 native artifact. Corpus
changes use the complete supported definition loader and selected-case runner;
consumer version, publication, validation, corpus identity, complete selection
and generated outcomes must agree. Protected/symlinked evidence paths fail;
failure stages, timeout logs and published output are retained. Whole-job and
per-command time, cache/output/evidence sizes and deferred classes are reported.
No generator compilation, internal test, full-corpus or parity replay is invoked.

Independent review approved exact implementation
`5e8a6af683198c0003ac1684a7745c96678c294f` and the isolated local fixture proof.
Fixture-only commits are docs `65d9972c20d8746022b394692932878a1c5ce525` and
metadata `c6ca7bf508a3742d47d828595e7caf12278b6780`; they are not changes to
the original corpus. Both repeated dry routes are byte-identical. Both jobs
passed all 94 ordinary tests. Docs ran tests plus acquisition only in 2.210458s;
the metadata job ran complete inspection and generated only mono1 in 7.151087s.
Its sole payload is the unchanged 926-byte R0 mono1 artifact; strict validation
passed and the report retains the complete manifest. Loader/run corpus identity
is `2166ac4bb65372f1f3e7cfa3b822584d0632a42a169e162b7034b10995d9bf7c`.

The one-case output has two files, 68,854 logical / 73,728 allocated bytes;
adjacent runner evidence has seven files, 317,110 / 335,872 bytes. Full docs
evidence is five files, 36,021 / 49,152 bytes; case evidence is eighteen files,
613,304 / 651,264 bytes. The verified native cache is 69,314,672 /
69,316,608 bytes; no build tree exists. The whole runtime proof before its
receipt is 112 files, 140,123,961 / 140,398,592 bytes, including input and cache
copies. Durable retention adds storage, not execution time.

Corpus `artifacts/local-ci-proof1-20260904/` retains both Git bundles, exact
fixture checkout, native input, raw logs and all results. Proof receipt SHA:
`76491ecf5741c7e3ab846424eacd10dc992cbf347d6c07e3fe0f114ee68ae711`.
Docs receipt SHA: `9b1448ab31bcdc9fe49e4cc0eff38daf4d3d908e428ee15ec637649c34decb30`.
Case receipt SHA: `7ce3c2132fb3cf307e05b682a45c6b241aec46fced840e3b1b74635891ef7b88`.
Reviewers verified hashes, fixture commits, clean/no-alternates/no-remotes
state, routing, selection, validation/report bindings and costs without native
replay. The original repositories remain outside the runtime dependency path;
OS-denied access to them is not claimed.

Together with the accepted foundation/acquisition, full clean-clone/parity and
read-only viewer contract, this closes R6.1–R6.5 and the local R6 gate. It does
not close hosted Corpus PR evidence, durable distribution, actual viewer
integration, complete corpus migration, or terminal qualification. No remote,
hosted runner, release or viewer-repository mutation has been authorized here.

### 2026-09-04 — R7 first native slice decomposed

Read-only source/registry/recipe/ownership review at generator `45a844d` and
corpus `9d5e877` identifies ten initial single-instance native SC core cases.
Their rows and recipe bytes match the pinned R5 source `232b9de`. Each has
one artifact, no dependencies or optional runtime requirements, byte-stable
classification, recipe `0.1.0`, provider `native.sc_plan`, and installed
monochrome/RGB SC template `1.0.0`. KB2026b evidence remains unchanged; this
slice requires no local-source-note or asset members.

Case IDs below have prefix `classic/sc/`; recipe files have prefix
`cases/recipes/classic/sc/` and suffix `.json`:

| Case suffix | Recipe filename stem |
| --- | --- |
| mono2_i16_explicit_le | sc_mono2_i16 |
| mono2_u16_explicit_le | sc_mono2_u16 |
| mono2_u16_odd_3x3_explicit_le | sc_mono2_u16_odd_3x3 |
| mono2_u16_padding_explicit_le | sc_mono2_u16_padding |
| mono2_u16_rect_2x3_explicit_le | sc_mono2_u16_rect_2x3 |
| mono2_u16_tiny_1x1_explicit_le | sc_mono2_u16_tiny_1x1 |
| palette_color_u8_explicit_le | sc_palette_color_u8 |
| rgb_planar1_explicit_le | sc_rgb_planar1 |
| ybr_full_422_explicit_le | sc_ybr_full_422 |
| ybr_full_planar0_explicit_le | sc_ybr_full_planar0 |

`nonsquare_pixel_spacing` is intentionally deferred to a multi-artifact slice
with its local source note: it emits two spatial variants. This is ordered
work, not permanent exclusion. Optional codecs, other native families,
relationships, providers, legacy, negative, fuzz, stress, media and protocol
coverage still require their own migration/availability evidence.

The pinned embedded CLI supports repeated `--case-id` with `--profile core`,
so the old baseline can be generated for only these ten cases, not the full
core profile. Embedded execution uses parallelism four; external comparison
will use the same value. This will be a newly captured pinned pre-migration
baseline, never mislabeled as historical R0 execution.

Next bounded tasks remain sequential: freeze baseline/provenance; establish a
versioned consumer compatibility precursor with actual submitted-definition
binding and frozen predecessor support; prepare live-vs-historical checks and
smoke/core routing; import the ten unchanged definitions as corpus content
`0.2.0`; then execute complete loaded assessment, explicit/profile selection,
byte/full-semantic parity, fresh repeat and bounded smoke regression. Shared
CI changes must select fixed smoke plus affected core/dependency closure, not
discard affected core cases or expand to the whole registry. Viewer-result1
stays smoke-only at this stage. No new generator capability, pin change,
generation, source deletion or external mutation occurred during this audit.

### 2026-09-04 — R7 first native baseline: reporting failure retained

Corpus range `9d5e877..f384e70` establishes source provenance, a bounded
capture helper and 103 passing ordinary tests. Clean capture candidate
`4cc22bd4bf0d6879e1a8c7c561bcb52ea30dac65` executed the ten-case request
once with the unchanged R5 native pin. Generation succeeded in 0.918764250s
(ten files, 9,512 payload bytes), and strict validation passed all ten in
0.220158500s. Machine reporting failed in 0.318525458s with exit 6,
`internal.invariant.failed`. The baseline gate remains incomplete: there is
no successful coverage report or accepted final baseline-expectations record.

Complete unchanged output, acquisition and machine logs remain in corpus
`artifacts/r7-native-core-baseline1-20260904`. Failed receipt SHA is
`76f37d0175eba3c2a177a837eaa0df54e910720988641ff1407d7bb5208e97dc`;
raw 223,968-byte manifest SHA is
`aafa1e5faed4965afd0889fc570058abb625b2bb6db359a199df2c6c384388f7`.
The manifest retains all ten generated objects and 24 nonselected core
bookkeeping rows; none are silently removed or reinterpreted as selected-case
capability failures. Whole job time was 3.262348292s; generated output is
233,480 logical / 266,240 allocated bytes. Pre-receipt evidence is 19 files,
315,582 logical / 364,544 allocated bytes, with no build tree.

Next work is bounded read-only reporting diagnosis against this retained
output, followed by an explicitly reviewed repair if required. No baseline
regeneration, live case import, consumer/schema change, pin replacement,
broader qualification or external mutation is authorized by this checkpoint.
Required reporting evidence is not waived; R7 and terminal gates remain open.

### 2026-09-04 — R7 reporting diagnosis and ordered repair decision

Corpus `d005aa4` records one additional read-only raw report invocation against
the unchanged baseline output: exit 6 in 0.302576333s, with diagnostic
`report schema invalid: false was expected`. Diagnostic receipt SHA is
`49df95f8a0352db6d8f00e7540e15008be4b35e6361ceb1c50607fc2e27b0482`.
The original receipt and all eleven output hashes remain unchanged. Independent
review verifies the generated/validated evidence is reusable, not complete.

The frozen coverage schema's nonsquare case condition requires concrete
artifact fields even for an unselected unavailable row. The producer's null
fields correctly avoid inventing pixel/geometry observations. Other case-only
guards were inventoried, but are not assumed broken: some intentionally retain
definition expectations. No blanket relaxation or removal of rows is permitted.

The next sequential generator-only boundary is an additive coverage `1.1.0`
contract: preserve frozen `0.1.0` and `1.0.0` schema bytes and readers; retain
every previously accepted row; add only an explicit known non-generated,
all-seven-nonsquare-fields-null alternative. Generated variants retain their
full strict constraints. Current manifest1 reporting will use the new version;
legacy manifest reporting remains unchanged. Existing machine wrapper and
discovery versions may stay only if their frozen schemas demonstrably permit
the new advertised report version. The repair needs owning ordinary/Fast
verification, resource/identity/compatibility tests and a clean code checkpoint
before a separately identified reporter is built. It does not authorize broader
Heavy qualification or replacement of the corpus generator pin.

After code acceptance, a separately hash-bound reporter may read the original
baseline; its provenance must remain separate from the unchanged generating
binary. A later corpus helper repair must also compare exact pinned standards
enrichment (registry-first, conditional appended records, first-occurrence
deduplication), preserving all records rather than assuming registry equality.
Only then can complete baseline evidence be reviewed. Live case imports and
consumer-version migration remain queued behind this boundary.

### 2026-09-04 — R7 additive report reader checkpoint

Generator `6b454e1` adds coverage `1.1.0` schema/reader support and its
compatibility, routing and resource inventories. This is an accepted
schema/reader checkpoint, **not** acceptance of the reporting repair or the
baseline: producer integration and full routed ordinary verification remain
pending. No replacement reporter or native baseline replay has executed.

The 215,018-byte schema SHA is
`1550441969f1093f5686d6249ef645e1545557fbba0e6f27d9655b1247624fc2`.
Independent structural review confirms the original nonsquare branch remains
exact, all other row rules are unchanged apart from explicit frozen-schema
references, and the new alternative requires the case ID, a known non-generated
status and all seven null fields. Frozen coverage0.1/1 schemas are unchanged.
The reader applies runtime-identity uniqueness checks to the new version too.

Focused Rust report-contract tests pass 5/5 in 6.06s after 23.00s compilation,
including both generated variants, old-field preservation, null/malformed
rows, missing identity and duplicate runtime IDs. Ownership, spelling,
formatting and diff checks pass. These do not substitute for the pending
integration route's unconditional Fast and affected ordinary tests.

Root independently used Node24.19.0/Ajv8.18.0, Draft2020-12 with `strict:false`
(schema validation remains enabled), to validate 46 synthetic full-report
projections in 0.583001s. Checks cover all five non-generated statuses,
35 invented-field mutations, generated-null/unknown/missing-status rejection,
and preserved old concrete rows. Three additional frozen-outer-schema checks
in 0.459156s confirm capabilities3 and report-result1 permit a coverage1.1
version. These are schema tests, not native report or viewer evidence.

At this schema-only checkpoint, current resources are 81 files / 1,505,420
bytes, SHA `1c1884b655a528f5c667b3bffa38b036e367964acdd8621fbd2e5de7fcca0cb5`;
the schema domain is 58 files / 1,104,948 bytes, SHA
`a81087ba5bc1951a281b22486b7b9a7316df92448d16379fe65c53fc5f5aa166`.
Physical snapshot inventory is 261 files / 2,918,678 bytes. Legacy membership
remains 240 with the existing `dc61…` digest because the new schema is excluded
from that historical membership. Subsequent genuine Cargo/provider-lock changes
for the product patch must change their actual identities truthfully; no frozen
byte adapter may hide changed inputs. Historical source/pin evidence remains
immutable and separate from the forthcoming reporter's identities.

### 2026-09-04 — R7 product-version experiment and compatibility decision

A local, uncommitted `0.2.1` integration experiment truthfully changed the root
Cargo package entry and all three backend-lock references. Cargo lock SHA became
`0dec9e0c5a533ff266b7bbecd4cbf0b130be6719400b59131773f717a522024e`;
backend lock SHA became
`309c5dc33250f40fc358d30686c6fce0e5f731402b677f3c394581aed3b04952`.
The resulting legacy resource digest was
`007b47600fe65fcb27b0de5f9a2494088c2da7fca56809fb64f727e9d700b1c2`.
The focused one-case report regression failed before reporting: frozen
manifest1 legacy provenance requires `dc61…`. Version2 discovery's shared
migration definition also pins that literal. The failed focused run took 1.83s
after 2.14s compilation; no broader route, reporter build, retained-baseline
replay or schema mutation followed. Earlier focused output was not retained
across an agent context refresh, so one ordinary test rerun was explicitly
recorded; it is not additional baseline qualification.

Independent policy review classifies coverage1.1 as an **additive report-schema
capability within the unreleased 0.2.0 source candidate**, consistent with the
earlier R4/R5 independent schema changes. It is not a replacement released
artifact or a same-schema correction of successfully emitted invalid output:
the old reporter rejected its result before emission. The committed policy's
product-release rules and independent schema-version rules remain unchanged.
The planned product patch is therefore withdrawn from this bounded boundary;
only the agent-owned uncommitted product/lock deltas and associated temporary
expectation edits are to be restored to their actual schema-checkpoint bytes.
No frozen-byte adapter, fake digest, historical fixture rewrite or frozen-schema
relaxation is permitted.

The repaired reporter must still have its own exact source revision, binary
hash and changed schema/current-resource identities. Product version equality
does not establish artifact equivalence. The old corpus pin and failed native
baseline remain unchanged. Any later product-version bump requires an explicit
versioned identity-contract migration; the discovered literal coupling is an
open prerequisite for terminal R9 identity/release work, not assumed solved or
waived. Current documentation must not claim that a future patch bump already
works. The complete reporting gate remains open pending producer integration,
ordinary verification and separate report-only evidence.

For storage context only, the existing cumulative target tree measured before
the deferred full route contained 9,665 regular files / 12,630,138,007 logical
bytes and 11,312,676 KiB allocated. It contains earlier builds and is neither a
clean-build cost nor an incremental cost of this repair. Final route evidence
must distinguish before/after storage rather than repurpose this total as a
new performance baseline. The schema checkpoint's subsequently reported Fast
checks passed 73/73 in 2.25s after 22.16s compilation; full integration routing
is still pending.

### 2026-09-04 — R7 reporting implementation and ordinary gate accepted

The coverage1.1 implementation through clean
`c2ffe41a9af6b72857f51b507ae6165a14deacdb` is independently accepted for its
affected ordinary verification class. Product remains the unreleased `0.2.0`
source candidate. Cargo/backend locks, frozen schemas/fixtures and DICOM salts
are unchanged from the schema-only checkpoint. The retained ten-case baseline
is **still incomplete**: no repaired reporter has yet been built or run against
it, and its original failed report evidence remains immutable.

`1ad35db` integrates coverage1.1 production for manifest1, the additive discovery
reader window and a one-case CLI/raw-envelope/SDK agreement regression.
Legacy manifest reporting and external manifest2/report2 dispatch remain
unchanged. This commit also contains the directly related changelog/policy/guide
updates: a staging error left implementation files staged before the
docs-prefixed commit. Review confirmed one coherent reporting boundary and no
unrelated files; its actual scope is recorded here rather than rewriting the
logged history. `5e3f36d` clarifies that reporting checks contracts but does not
rerun strict corpus validation.

The ordinary route exposed four stale test-oracle locations, corrected in
separate commits without changing producer behavior or removing assertions:
`d1c6cf6` updates two physical snapshot count/size expectations; `3b77165`
updates the current core report version; `61b6e31` updates the shared current
report helper version while preserving full SDK validation and JSON equality;
`c2ffe41` updates the current resource count from 80 to 81. All failed logs
remain retained. Prior passing commands were reused only across these reviewed
test-literal/ownership changes; each failed group was rerun in full.

The accepted evidence is explicitly **composite**, not a single clean full
run or terminal qualification:

| Command indices | Exact candidate | Accepted scope |
| --- | --- | --- |
| 0–4 | `5e3f36db2afe1d4bc6c88d3ca140ae5af4e48484` | First five configured ordinary groups |
| 5–36 | `d1c6cf66fcc2c57837d97f40db54255ed30a7377` | Cache group rerun and following ordinary groups |
| 37–70 | `3b771650cde85d8c301218914cc19f8c0bb903e9` | Full report group rerun and following ordinary groups |
| 71–74 | `61b6e31d0630f37f2558817889777097c2131cc0` | Full corpus and affected engine groups plus remaining mapped groups |
| 75–78 | `c2ffe41a9af6b72857f51b507ae6165a14deacdb` | Resource group rerun, final ordinary group and two explicit Fast targets |

The 77 configured command argv lists are identical across attempts. The local
dispatcher reports coverage owned by unconditional Fast but does not execute
those two targets in this list, so `release_ci__fast` and
`schema_resources__fast` were run explicitly with `cargo test --locked
--no-default-features --test TARGET`. Their results are 15 and 73 passes
(2.07s and 1.96s test time). In total, all 79 mapped commands pass: 877 ordinary
plus 88 Fast passed test executions, not unique cases. Six ignored executions
are four separately owned provider timing tests and two subprocess fixtures;
they are not the six R2 heavy dispatcher entries. No deferred Heavy, Nightly,
codec-feature/provider qualification, package or release class was run.

Composite receipt:
`generated/r7-report-compat-resume4-20260904/composite-receipt.json`, 87,722 bytes,
SHA `8c48b6416edca7e23d98119db5df83bef8aa340194ddc37a86dce8e7f1485a15`.
It binds exact commands, source partitions, all mapped log hashes, prior failed
receipts, argv equality and toolchain. Independent review checked 162 referenced
artifacts/logs/toolchain hashes. Root additionally verified all 185 supplemental
evidence files, including failed logs and hygiene output, against
`evidence-inventory.json`, SHA
`2346b9bf9cee4cec009c29217e2aebb47bd8ff9ac95afcfe0065d6d387263bc0`.
Those files total 442,338 logical / 1,089,536 allocated bytes, excluding the
inventory itself. Ownership, spelling, formatting and full-range diff checks
pass; both repository worktrees remain clean at this review boundary.

Recorded route-attempt wall times total 375.792789166s, including failed
attempts but excluding separately reported focused reruns. Initial per-command
wall times were not recorded and remain null, not fabricated. The cumulative
target stayed at 9,665 files, growing from 12,630,586,526 to 12,635,813,674
logical bytes (+5,227,148), and from 11,313,116 to 11,318,232 KiB allocated
(+5,238,784 bytes). This is warm mixed-history growth, not clean-build cost.
Toolchain is rustc1.85.0 / cargo1.85.0, host aarch64-apple-darwin, features empty;
resolved executable fingerprints are bound in the receipt.

Next: build one separately identity-bound reporting candidate from a frozen
source snapshot, then establish explicit report-only acquisition/completion
evidence over the unchanged baseline. No live corpus import, generator-pin
replacement or baseline regeneration follows from this ordinary gate alone.

### 2026-09-04 — R7 immutable reporting candidate, build-only checkpoint

Independently reviewed one native build from source
`c2ffe41a9af6b72857f51b507ae6165a14deacdb`, tree
`299a533f86caaddd7d5655b5c5c3ea9c234dc940`. The retained 18,339,840-byte
source archive has SHA
`8374cf734e7177d508e40b5548acc8f112380e927baf7526530324803d54764d`;
all 898 extracted source files remain byte-equal to that archive. Source and
fresh target remain under `/private/tmp/r7-reporter-candidate-cwzauz31`.
This is an unreleased source-built reporting candidate, not a replacement for
the corpus generation pin or an independently qualified release.

The retained binary is
`generated/r7-reporter-candidate-20260904/bin/synth-dicom-gen`, 69,743,968
bytes, SHA `c56f49b0fc6626948f93f83568b592efd35109919c128da1191f818aab4bb383`.
The offline, locked, no-default-feature build explicitly targets
`aarch64-apple-darwin` with rustc/cargo1.85.0. Its dev debug0/incremental0
settings match the archived Cargo.toml defaults; the validation receipt
corrects an earlier commentary assertion that those defaults were absent.
The target was fresh, but the Cargo registry cache was already warm.
Build time was 23.717816958s and whole capture time 25.81105125s. The new
target contains 973 files, 578,521,607 logical / 582,344,704 allocated bytes.
These are reporter-build costs, not a terminal CI or release measurement.

Version and capabilities were the only runtime commands, from an unrelated
directory with empty PATH. Both identify product0.2.0 and agree on the separate
identity domains: engine3/`4268d921…` and legacy240/`dc61cc01…` remain unchanged;
schema58/`a81087ba…` and aggregate resources81/`1c1884b6…` include the additive
report contract. Producers advertise coverage1.1/2 and readers retain
coverage0.1/1/1.1/2. Root additionally validated the actual version2 and
capabilities3 results and success envelopes against full Draft2020 schemas
with Ajv8.18.0/Node24.19.0: four checks passed in 0.339068708s. No native
command was rerun for that schema check.

Evidence under `generated/r7-reporter-candidate-20260904`:

- Build receipt SHA `96e325b520fe2bcc9f572b66484c2c019afc9fa1b7d9545fecc04f4abf1d0e01`.
- Provenance/discovery assertion receipt SHA
  `e6247991749894185ced8e6c4dac5fe15e7b092552b65d2b0cbb7d1f27e64db9`.
- Complete inventory SHA
  `e4a09590e636cdce44ff3e1580160666cdf42231e39b14a544df0a505a998265`;
  root verified all 22 listed file hashes and sizes. They total 88,676,042
  logical / 88,707,072 allocated bytes, excluding the inventory itself.

No report against the retained baseline, generation, validation replay, SDK
consumer build, Heavy, package or release qualification ran at this boundary.
The original generator lock and failed baseline receipts are unchanged. Next
is a separately reviewed, explicitly acquired report-only completion helper
with exact pinned standards-enrichment expectations. R7 baseline acceptance,
live core import and the later product-version identity-contract migration
remain open.

### 2026-09-04 — R7 retained ten-case baseline reporting completed

The separate corpus helper checkpoint `51158d5ad3f81f33682a12aab752fe810e130fee`
passed review and 119 ordinary tests (final clean-candidate run 0.786s).
Granular corpus commits are `e3d393a` (separate reporter lock/schema and exact
source-derived enrichment), `7de9082` (helper and fourteen synthetic tests),
`ad338fb` (six explicit CI support-path mappings, unchanged routing policy),
and `51158d5` (documentation). The earlier ordinary attempt's one unmapped-path
failure remains recorded; no failing result was relabeled. Full reporter-lock
schema evaluation also passed, including ten required-field deletions and an
unexpected-field rejection. The original generator lock, source provenance,
capture helper, live corpus and consumer runner remained unchanged.

One explicitly acquired report-only completion then passed. It used the
separate c2ffe41 reporter above against the original source232b9de baseline
under corpus `artifacts/r7-native-core-baseline1-20260904/generated`, from an
unrelated directory with empty PATH. No generation or strict validation was
replayed. The report command took 0.888731042s, helper 2.646760333s, and outer
capture 2.681410084s. Corpus execution record commit: `7f1a458`.

New evidence under corpus `artifacts/r7-native-core-report-completion1-20260904`:

- Completion receipt, 9,478 bytes, SHA
  `50e35a73f027ccceaac04a3adc58d2fa64d6046b8f763503fe3370401b43c89b`.
- Full machine report, 940,203 bytes, SHA
  `ac18f3eed0dfe080ed98285a1e49fbb515738042aaf548c4c2bf37c82edc9841`.
- Coverage report1.1, 695,509 bytes, SHA
  `8ec116d6036a2302ec1837004d56ab7464575059d09e05413eb2957decfcfc8c`.
- Separate invocation receipt under
  `artifacts/r7-native-core-report-invocation1-20260904`, SHA
  `5792b1b290fbbd3105100c87536897093fb12ac649fb5ece026ebcefdf006056`.

The eight completion files occupy 1,791,974 logical / 1,806,336 allocated bytes.
The new reporter cache contains one 69,743,968-byte executable occupying
69,746,688 allocated bytes. All 23 original evidence files, including the later
diagnostic and original failed receipt, remain unchanged: 324,089 logical /
380,928 allocated bytes. The generated root remains the same eleven files.
These costs exclude the separately recorded reporter build and are not a
terminal CI or release measurement.

Independent review verified original and completion inventory hashes, all ten
generated rows, all twenty-four unselected statuses/reasons/gap messages, exact
source-derived standards enrichment and complete original file/skipped objects.
Root validated the actual coverage1.1 report, report-result1 and success
envelope with full Draft2020 schemas using Ajv8.18/Node24.19: three checks
passed in 0.46077775s, with no native rerun. The report preserves the original
manifest identity; new reporter acquisition records its distinct artifact and
schema identity separately. Nonsquare observations remain truthfully null.

The retained embedded baseline for these ten cases is now complete and accepted.
This is not migrated-corpus parity, wider R7 completion, independent conformance
or release evidence. The original failed capture remains failed; the separate
completion receipt supplies the previously missing report. Next gates remain
consumer content-version compatibility, live-versus-historical selection/CI,
the exact ten-case import, and supported-boundary parity before further slices.

### 2026-09-04 — R7 consumer content-version prerequisite accepted

The reviewed corpus code checkpoint `6c7657b` establishes a versioned consumer
result boundary before any core definition import. Commits `2a25fb0`,
`b0d6d44` and `6c7657b` separately cover descriptor-bound result production,
the reviewed exact-integer/failure-evidence correction, and CI result parsing.
Documentation and independent acceptance are recorded at `4044414`/`ca4a41b`.
The original result1 schema remains byte-identical, SHA
`440ba785926505f6be441d5960b927a7a147c279a769fb41081467d11a6817c6`.
Content0.1 still emits its unchanged result1 shape. Other numeric
major.minor.patch content versions under bundle schema1 use result2, with
required submitted-descriptor identity, raw SHA and byte size. The new schema
SHA is `44b89ef8ed9c335a37798d3cff5f37cbd3a0c95191d98c13d31e9da50cf49bdb`.

Both paths capture bounded, no-follow descriptor bytes and match returned
ID/content version/bundle schema/raw manifest hash to those exact bytes.
The descriptor's byte size is not confused with whole-bundle size. Trusted
published results retain the raw descriptor and generation response outside
the closed output before checking descriptor stability; later mutation fails
without deleting output. Untrusted responses retain unknown publication status
and do not create sidecars. Nonpublished paths recheck stability and remain
output/evidence-directory free. This is not hostile concurrent-writer atomicity;
the supported generator loader still owns member integrity.

CI explicitly dispatches result1/result2 and checks captured, inspected and
generated identity. Selection and shared-smoke routing semantics are unchanged.
No generator pin, live corpus, historical proof fixture or viewer scope changed.

Verification: 26 runner tests passed in 0.056s, 22 CI tests in 0.144s, and all
127 ordinary tests in 0.799s. Independent review reran runner/CI tests with
26/22 passes in 0.056s/0.145s. Root full Draft2020 evaluation using existing
Ajv8.18/Node24.19 passed 29 checks in 0.029376459s: retained result1, synthetic
result2's three outcomes at two content versions, cross-version rejection,
malformed versions and missing/invalid submitted fields. Exact uint64 limits
and cross-document equality are separately checked with integer-aware Python
tests; JavaScript numeric precision is not claimed to prove that boundary.
Review caught and corrected a rounded uint64 schema literal before acceptance.
Frozen-schema equality and intended-only structural differences are tested.

This prerequisite is accepted without native execution, new generation or
qualification. Current corpus content is still0.1/smoke-only. Live-versus-
historical definition and CI selection work, exact core import and migrated
parity remain the next sequential gates.

### 2026-09-04 — R7 historical/live selection and bounded CI prerequisite

Accepted corpus checkpoint `bbcb03d`: `5a6e68a` separates historical smoke
assertions from live inventory and updates the explicit loader verifier;
`eb269af` versions the local CI configuration/route to2; `bbcb03d` documents
the boundary. Historical fixture bytes remain unchanged. Live inventory must
retain all three smoke IDs and may add only cases from the accepted ten-case
source provenance, with exact registry/recipe identity and empty dependency/
evidence closure. Smoke recipe path/size/hash bindings are also pinned. Only
the pre-existing descriptive smoke compatibility annotations may evolve; that
allowance cannot apply to core rows.

Shared changes now union the fixed smoke set with affected cases, then expand
old/new reverse impact and current dependency closure. They neither replace
affected core cases with smoke nor select the whole live registry. Smoke-only
and core-only routes use those profiles; a mixed route uses the existing valid
`all` union with explicit bounded IDs. Extended and isolated memberships remain
empty and the `all` union/opt-in stress definition is fixed. This is not full-
profile generation. Unknown paths/cases, removals, altered source bindings and
unsupported scopes fail closed. Future relationship slices require their own
versioned ownership expansion; synthetic graph tests do not authorize new live
dependencies in this slice.

The smoke verifier still exercises seven full supported-loader checks without
generation, now selecting the named historical smoke subset and binding the
actual full descriptor identity. Adverse fixtures target named smoke cases,
not index0, and each command rechecks the explicitly acquired executable.
No historical native proof was rewritten or replayed at this prerequisite.

Verification: smoke tests10/10 in0.010s, CI24/24 in0.186s, full ordinary131/131
in0.858s. Independent review reran smoke10/10 in0.009s and CI24/24 in0.189s.
The clean current-repository route dry-run from `ca4a41b` to `eb269af` selected
only the fixed smoke3, with no acquisition or output creation. Help and diff
checks passed. Review caught and corrected missing-smoke acceptance in a
standalone routing snapshot before final acceptance.

No native command, live corpus import, pin change, generated artifact or wider
qualification occurred. The next sequential task is the exact ten-case core
definition import, followed by complete public-loader checks and an explicitly
reviewed migrated-parity boundary.

### 2026-09-04 — R7.1 exact first ten core definitions imported

Accepted static corpus checkpoint `538d09a`, range `2ba68a3..538d09a`.
Commit `8a7dbdb` fixes historical CI fixture construction; `8a34471` imports
exactly ten recipes and updates the registry/descriptor; `538d09a` updates
current documentation. Corpus content is now0.2.0 under unchanged bundle1.
The earlier accepted source232b9de ten-case selection is the sole core import.
All thirteen live registry rows and recipes were independently compared to
that immutable source, including unchanged smoke definitions. Generator-side
copies remain until the scheduled R9 removal boundary.

The descriptor is 6,992 bytes, SHA
`f97cd5cd09515b068414bd0ce0a665ec88006ccd31b6f5d4bc1bb54d45c478de`;
the registry is 29,370 bytes, SHA
`29866d4355e5137fbc36b54a518abeb9add00e2cc688f322372a6a81a9a4593e`.
The exact closed definition root contains fifteen regular files, 70,435 logical
bytes: descriptor, registry and thirteen recipes. Eight profile definitions
retain their boundaries: smoke3, core10 and the remaining concrete profiles
empty, with the fixed valid `all` union and stress opt-in. No dependencies,
assets or evidence files were introduced. There is no runtime sibling lookup
or internal generator import; source checkout access was migration provenance
verification only.

The initial ordinary suite exposed six stale CI synthetic-fixture assumptions
when live inventory grew: four failures/two errors in0.851s. The narrowly
reviewed test-only correction builds historical smoke fixtures from immutable
provenance rather than current live inventory, while keeping expanded-core
tests and adding a separate actual-live source-bound fallback check. Production
routing was not changed. Corrected CI25 tests passed in0.201s; full ordinary132
passed in0.862s. Root full Draft2020 schema evaluation (Ajv8.18/Node24.19)
passed fifteen checks and thirteen declared recipe hash/size bindings in
0.09819125s. Independent review verified all source bytes and closed membership.

The clean route dry-run from `2ba68a3` to `8a34471` selects exactly thirteen
explicit IDs, transported through valid `all`, with no dependencies. It did
not acquire a generator or create output. Diff/help checks pass. Current docs
identify old native examples as content0.1 history, not evidence for0.2.

This accepts data ownership and static verification only. No native capability,
generation, validation or report command ran for the imported corpus. Public-
loader availability, supported-interface byte/full-semantics parity, repeated
generation and the later complete R7/terminal qualifications remain open.

### 2026-09-04 — R7 imported-definition public-loader availability accepted

One bounded loader-only proof passed on clean corpus `538d09a`, using the
unchanged explicitly supplied source232b9de generator artifact. The existing
smoke verifier ran its seven positive/adverse checks once. Core capabilities
then ran once by profile and once by the exact ten explicit IDs, seed1 and
parallelism4, from an unrelated directory with empty PATH. No generation,
validation, report or rebuild ran.

The smoke assessment selects three ready cases from the full thirteen-case
definition. Both core assessments select ten unique ready/direct cases with
zero dependencies, ten artifact IDs, no artifact paths and publication/
validation `not_run`. Their complete assessments are equal except the declared
selector. The planned core hash is exactly the retained embedded baseline's
`6769b685d9fec095215acd9b3359948e28d78f7c6d426c4ccb8379ffcfac99bb`;
this is planning equality, not byte-output proof. The verified corpus digest is
`0b651cdc15a6618725bbef7e7d5a5a84276acdfebe1ec47a026fc1496249fda2`,
with fifteen files /70,435 bytes and the previously recorded raw descriptor hash.

Evidence lives in corpus `artifacts/r7-imported-definition-smoke1-20260904`,
`artifacts/r7-imported-definition-core1-20260904`, and
`artifacts/r7-imported-definition-invocation1-20260904`. Invocation receipt SHA:
`2e579070e7c305a6ee315fb3a9515823538135104da121f60997afc0bbe50759`.
Raw core profile stdout SHA:
`e3a8cf4f00c1aa526b24c281a21b0f954f178459300576f12d78fa57a44af114`;
raw core explicit-ID stdout SHA:
`b176293c7183009dabdecb2bbbc6c1e636e7ec0fa179c7f86212e5e99614a258`.
Receipt discovery digests label inner result documents; acquisition separately
validated complete-envelope digests against the unchanged generator lock.
These different canonicalization scopes must not be substituted for each other.

Smoke-helper wall time was4.698362167s, core acquisition1.566495875s,
core-profile command1.339004833s and core-ID command1.346308625s; whole
capture9.070288708s. Retained smoke evidence is16 files,145,605 logical /
176,128 allocated bytes; core evidence5 files,285,330/294,912 bytes; invocation
evidence including final receipt5 files,33,101/45,056 bytes. All fifteen corpus
inputs remain unchanged at70,435 logical/94,208 allocated bytes. The existing
generator cache remains one69,314,672-byte file/69,316,608 allocated bytes,
with zero growth.

Independent review verified every inventoried artifact/input/cache hash and
size, candidate Git blobs, binary mode0500 and lock discovery bindings, all
seven exact smoke outcomes, and complete core assessment equality. Root full
Draft2020 evaluation (Ajv8.18/Node24.19) passed twelve checks in0.325471458s:
three capabilities3 results, their success envelopes and six adverse error
envelopes. Neither review reran native commands.

Loader availability for this imported slice is accepted. Actual external
generation, strict validation, full report preservation, byte/full-semantics
parity and reproducibility remain the next separately reviewed boundary.

## R7 first native core parity proof preparation — 2026-09-04

Corpus commit `980a535` adds an explicitly invoked, isolated four-run proof:
core profile, the same ten explicit IDs, core repeat, and the frozen R0 smoke
regression, all seed1/parallelism4. This is preparation, not executed parity.
Original failed and completed baseline receipts and every retained input are
authenticated before use. Full core file arrays must compare exactly; only the
declared manifest1-to-manifest2 boundary may differ. The R0 smoke comparator
is unchanged. Public runner execution uses an exact Git archive, unrelated
working directory, empty PATH and a copied, verified native pin. No build,
internal generator import, sibling-path fallback or OS-denial claim is added.

Streaming SHA-256/size inventories cover retained source, inputs, native/cache
and outputs. Timeout, launch, inventory and retention failures preserve a failed
receipt rather than triggering a retry. Final source checks reject additions
other than the exact pinned cache; runtime bytecode writes are disabled.

The initial ordinary run exposed a historical tracked-path fixture problem:
142 tests,141 passed/one error in0.911s. The earlier import suite ran before
staging the new data, so its tracked-file inventory missed the expansion.
Test-only corpus commit `2fddf53` uses the actual live snapshot for that one
inventory test, retaining historical synthetic fixtures elsewhere. Its25
focused tests passed in0.214s. After the proof/test files were committed,
146 ordinary tests passed in0.937s;14 focused synthetic tests passed in0.041s.
Independent review passed those14 in0.039s and accepted the failure handling,
exact comparisons and isolation; root review concurs. Diff checks passed.
The native proof remains a separate, one-attempt execution boundary after
the corpus documentation checkpoint is clean. No heavyweight qualification,
generation or broader R7/terminal acceptance is implied by this preparation.

## R7 first native core isolated parity execution — 2026-09-04

The single authorized proof from clean corpus candidate
`c07b9c162a925459c7973fb26a72f75dc124d4f4` passed all four public-runner calls,
without a build or retry. Final post-documentation ordinary verification was
146 passes in0.914s. Evidence is retained at corpus
`artifacts/r7-native-core-parity1-20260904`; the receipt is99,483 bytes, SHA
`d51a36d70b20d37caf2f42711feb3a259fe2a781c525f577d05a7cb85b6ada85`.
The source archive SHA is
`fb8a8aeee10760f660313e0e086a029b73097c743d888fc4e520abfb743feb69`.
Both the private runtime `/private/tmp/r7-core-parity-srbttw36` and its durable
`retained-runtime` copy remain available. This is local archive isolation,
not remote acquisition, package qualification or operating-system access denial.

Core profile, ten explicit IDs and core repeat each emitted ten payloads totaling
9,512 bytes. Their complete canonical file arrays equal the original baseline:
`3e16678d3a82c89d0c8ef43d8210245bec46b769fdc5a34e33731c476ddf4b30`.
No file field, ordering, UID, recipe, standards or validation evidence was
discarded. Repeated whole manifests and reports are equal; explicit-ID manifests
differ only in the selector. The manifest1-to-manifest2 transition retains the
authenticated original24 unselected rows separately and publishes an exact
ten-case generated selection ledger with full registry definitions.

Smoke emitted three payloads/2,790 bytes and retained the exact frozen14,109-byte
R0 semantic projection SHA
`18f154c38903677cadf4f955b0658ed2fd59162c44a970a9b15c5dc9905eabcd`.
All four runs passed strict validation and report2, including the complete
source manifest and original generation/validation/report envelopes. The pin
remained the69,314,672-byte native artifact from232b9de, SHA4ca0c6d6…0b7768;
the separate repaired reporter was not substituted. Copied and original inputs,
tracked source and the exact pinned cache passed final integrity checks.

Runner wall times: core profile3.893840042s, IDs3.365399458s,
repeat3.425715209s and smoke3.348479583s; whole proof14.848957333s.
Runtime inventory:162 files/145,297,479 logical/145,694,720 allocated bytes.
Durable evidence before its receipt:181 files/146,745,710 logical/147,169,280
allocated bytes; receipt cost is additional. Core output roots each contain11
files/241,664 allocated bytes (206,211 logical for profile/repeat;206,733 for
IDs). Smoke output contains4 files/94,781 logical/106,496 allocated bytes.
Adjacent full evidence costs are retained, not omitted from the receipt.

Root full Draft2020 schema validation passed36 checks in0.333293s with
Node24.19/Ajv8.18: every actual result2, manifest2, report2, three original
success envelopes and their versioned payloads for each run. No native rerun
occurred. Generator evidence
`generated/r7-core-parity-schema-review-20260904/receipt.json` is7,630 bytes,
SHA `3835e8c0e34b589189916239592e1ee07ada6830408b879b5280cd0b8473621a`;
the2,817-byte validation script SHA is
`ced451b8846e91df3acdf95fdd421493393458761b6ceb440cf4b0d79d8aaab6`.
Independent review verified all181 inventoried durable files and162 runtime
files, exact candidate Git blobs, authenticated inputs, native/discovery pins,
nine recorded orchestration commands and every parity/validation/report result,
without native replay. Root and independent review accept this first ten-core
slice plus smoke regression. Including its receipt, durable evidence contains
182 files/146,845,193 logical/147,271,680 allocated bytes. The33 payload
executions cover13 distinct cases and31,326 payload bytes. This bounded result
does not close the remaining R7 slices, R8/R9 or terminal qualification rows.

## R7 next metadata cohort: source-only decomposition — 2026-09-04

Generator commit `5d21d93` records the reviewed bounded inventory in
`docs/r7-remaining-native-slice-inventory-2026-09-04.md`. Corpus status/current
documentation checkpoint `ee06fc9` separately records the accepted first-core
proof. Both repositories were clean before the next assignment.

The next cohort is exactly `metadata/sc/empty_type2_attributes`,
`metadata/sc/private_creator_blocks`, and `metadata/sc/utf8_person_name`:
single-artifact, native byte-stable SC recipes with no case dependencies.
Root and independent source review accepted the three registry rows, recipes,
local source-note hashes, provider/template bindings and preserved planning/
projection orders against232b9de. Raw UTF-8 PN bytes and `DTS_PRIVATE_ALPHA` /
`DTS_PRIVATE_BETA` are payload contracts, not product branding to rename.

This is source-only selection of the next work unit, not native availability
or an import. Remaining core cohorts, broader extended scope and metadata
loader case-ID/namespace coupling are explicit in the inventory. Generic
capability promotion remains required before full R7 acceptance. Live consumer
source guards still admit only the first ten core cases; the historical proof
also has a64-file source-snapshot ceiling. Both require separately reviewed
preparation before expansion, without weakening frozen historical evidence.

The next delegated unit owns only new metadata source provenance, static tests
and their explicit CI path mappings in the corpus repository. It may not alter
live definitions, pins, old baseline helpers or native outputs. Subsequent
baseline preparation will use the original native pin for generation/validation
and the separately pinned repaired reporter on its first report attempt; no
repeat of the known old subset-report failure is required. Expanded profile
selection will be inspected, not silently turned into whole-core generation.
The inventory documentation route selected no ordinary commands; unconditional
Fast targets were reported but not run. Diff checks passed.

## R7 metadata source provenance frozen — 2026-09-04

Corpus commits `a84f995` (two explicit shared CI paths) and `4e59ea8`
(source fixture and static tests) complete this source-only preparation unit.
`docs/r7-native-metadata-source-provenance.json` is57,609 bytes, SHA
`db735caa8c9757c471d4336296e50b447d5f440db715393200877d00332c8699`.
It is self-contained: three exact registry rows, raw recipe and local-note
texts, typed contracts, six source bindings and the exact standards projector
excerpt at source232b9de. It requires no sibling checkout, Git lookup or
generator internal import at test runtime. No live definition or pin changed.

Independent review checked every source binding, raw member, query closure and
ordered standards array against232b9de. The metadata arrays preserve registry
records, followed by four nonduplicate SC records, with first-wins deduplication
on `(source, query)` only. UTF-8/empty Type 2/private-creator array lengths are
9/7/6; their complete canonical hashes are respectively
`af5d8ba69f937c04394b92d7aea4a55f46a24850de00c3d2b0491806a5005290`,
`9b128c23a1720615011f7f6f31268e456a8beb7f2cfb62a9c4fb0d6f9e83956f`,
and `9fe4cf95703fb2c9b1863f43dfe2731b2646036944ccc76544e806bdd7946f3d`.
Canonicalization is sorted compact UTF-8 JSON, ensure_ascii=False, plus one LF;
it is not the differently scoped historical R0 comparator.

Root test review corrected a temporary live-absence assumption that would have
invalidated the historical fixture on a later legitimate import, and aligned
the optional-pixel checks with the actual source fields. Eleven static tests
passed in0.010s; after new paths were committed,157 ordinary tests passed
in0.946s. The dry route fromee06fc9 to4e59ea8 selects only existing smoke3
fallback, no dependencies; it executed neither definition inspection nor
generation. Configuration SHA:
`e38ee2fc7e88476cbf4f433e756bca62e3de5613caed9cb40432fb3748dcbd58`.
Diff checks passed. Root and independent review accept provenance only, not
metadata availability, baseline generation, migrated parity or conformance.

## R7 metadata baseline prerequisite: complete source core rows — 2026-09-04

While reviewing the unexecuted metadata baseline helper, root found that the
historical `source_core_registry_scope` contains only case ID, status and
profiles. It cannot supply complete standards evidence for the31 unselected
rows. A real-fixture test run reproduced this initialization defect:14 tests,
zero passes/14 setup errors in0.022s. No native command or baseline was run.

Corpus prerequisite commit `0c664c9` adds the separately reviewed
`docs/r7-native-core-registry-source-2026-09-04.json`,103,156 bytes, SHA
`f879075135b1616a9693502c827c21ab79d516ec7635a0dd92b033bc8580fafa`.
All34 full rows exactly match the core filter in source232b9de registry order.
Independent review also confirmed that their minimal projection equals the
unchanged historical scope, and that the three metadata rows match the newly
accepted metadata provenance. This is source evidence, not live corpus import.

The fixture binds original `src/lib.rs`,1,490,527 bytes, SHA
`e0eb3231dfbd90218b5bda6cfa9e6434f19da516a83b212ac7ec5f5c3b87b467`,
the exact skipped-case constructor at34866–34940, and parallelism source lines
650/694/1246. Parallelism4 is an embedded executor setting, not an invented
legacy CLI flag. Root and independent review accept this prerequisite.
The earlier minimal and metadata fixtures remain unchanged. Helper/tests/CI
preparation remains owned uncommitted work pending final review and ordinary
verification; native execution is still not authorized at this checkpoint.

## R7 metadata baseline helper readiness — 2026-09-04

Corpus `ce87a86` adds the bounded metadata helper and synthetic tests;
`dea7ed9` adds only their explicit CI path mappings. The helper SHA is
`bb5d2d20330a20b5f832922e8ccc79aa3bbc70b081988a30a459b8a60826ba9b`;
test SHA `0c2b98a78d7963dd3ceef7b80cbca2d8a45a8c0c6ec15088918fe2502586a1ce`.
This is reviewed preparation, not native execution. The source-only full-core
fixture above replaces the incorrect assumption about the historical minimal
scope without editing that historical fixture.

The fixed workflow is original-pin generation of exactly metadata3 in core at
seed1/embedded parallelism4, original-pin strict validation, then separately
pinned repaired coverage1.1 reporting. Both explicit artifacts and full
discovery identities are verified; report identity remains the original
manifest's identity. Complete source-derived31 skipped rows, full metadata
observations and9/7/6 standards arrays are retained. All35 report metadata
fields, prior pixel/identity bindings, and null observations on unselected rows
are checked. These comparisons do not substitute coverage summaries for full
manifest/raw payload evidence.

Readiness review required and verified unchanged generated bytes across both
read-only stages, exact copied-input closure, original/source fingerprints and
modes, acquisition timing/failure evidence, and guarded final receipt creation.
Root and independent review also found and closed a directory-only mutation
gap: helper-local output snapshots now bind declared ancestor directories after
generation, validation, reporting and at finalization. Shared historical
inventory code is unchanged. Failure tests preserve available evidence without
retry or deletion. No build, provider, codec, viewer or broad qualification ran.

Final focused verification:23 passes in0.215s locally and0.216s independently.
Precommit ordinary178 passed in1.106s before the last two directory tests;
after all helper/test paths were tracked,180 passed in1.130s. Help/interface and
diff checks passed. Root and independent review accept code readiness; the
current documentation checkpoint must be clean before separate one-attempt
native authorization. Live corpus definitions and case-routing scope remain
unchanged, and no metadata baseline payload or runtime cost is yet claimed.

## R7 metadata-three embedded baseline accepted — 2026-09-04

The single authorized capture from clean corpus candidate
`dd59877e4c84de158f21f269adb2a2829e0f7b1e` passed without a retry. Evidence is
retained at corpus `artifacts/r7-native-metadata-baseline1-20260904`. Its
27,721-byte receipt SHA is
`04143fa6113464c9904ae6b6375b38b0db3f008279dfdac59f8719fa09d32185`.
The original232b9de generator performed generation/strict validation; the
separately pinnedc2ffe41 reporter produced coverage1.1 on its first attempt.
No old-report failure was replayed, and no build or broader qualification ran.

| Selected case | Payload bytes | SHA-256 |
| --- | ---: | --- |
| `metadata/sc/utf8_person_name` | 978 | `b1334cff9865e0a8f4e6d9af50f15fd043beea971c98be596fbaa9d200936ac9` |
| `metadata/sc/empty_type2_attributes` | 932 | `7f457e4f9593a8d41dff970d32de86c8b5493841546dd6d60b219f311a7abc7c` |
| `metadata/sc/private_creator_blocks` | 1114 | `5a0726a68554bb55a6dc5f7a74f639138dc365e8a46f444013303261705141e9` |

Total payload is3,024 bytes. Raw manifest1:167,360 bytes, SHA
`262582845014f2fe3beb3491b263618baab9e6608e65ae1a91719fda953e2feb`.
Complete canonical files:25,331 bytes, SHA
`3404c2a8b8c47e974f8856fea3c7e69748cca97c72d23c98aeb787cd4c662c01`.
Complete31 unselected rows:48,621 bytes, SHA
`b2762741f88d8433ef4774c01ccab79957a1e07b9a78493509cf6eb291ef5891`.
Whole plan SHA:
`05620ed8acf08ed665e3c73291b8ed45706440ba248c86b55d1157ebf3b59c03`.
Full report:698,584 bytes, SHA
`f8919cd1a966dd7428bb429da3d418090c987d605a94b2d1b4d119948e3db542`;
its original wrapper is944,090 bytes, SHA
`269bc30b1bf3ee8e6ffcca407829acd3ae25c6fd16552820e889abc7e4ad706a`.
Full metadata observations,9/7/6 standards arrays, all35 report metadata fields
and null unselected observations passed their reviewed comparisons. No full-file
field was removed or normalized. The original generator identity remains
separate from the reporter acquisition identity.

Whole capture5.939972833s: generator acquisition1.641825875s, reporter
acquisition1.594357083s, generation1.302557417s, strict validation0.228056584s,
report0.918813541s. All three corpus commands exited0 with empty stderr.
Output:4 files/5 directories,170,384 logical/180,224 allocated file bytes,
unchanged through validation/reporting. Evidence excluding receipt:25 files,
141,228,473 logical/141,287,424 allocated bytes, including both native caches.
Including receipt:26 files,141,256,194 logical/141,316,096 allocated bytes.
Original artifacts, copied inputs and the64-file candidate source stayed intact.

Independent review verified every inventoried file, exact candidate Git blobs,
source/pin/discovery bindings, raw metadata and payloads, all31 skipped rows,
full reporting and output-directory closure without native replay. Root full
Draft2020 validation passed8 checks in0.459828583s with Node24.19/Ajv8.18:
manifest1, report1.1 and three original success envelopes/result payloads.
Generator supplement `generated/r7-metadata-baseline-schema-review-20260904/receipt.json`
is1,881 bytes, SHA
`6cc03c8c98157858887e71b43919c9a3882975c3962fabf0633832cc63e9d87e`;
the2,427-byte script SHA is
`580673cf2e9fe3330f074b17e22d8b657425f7bdd334fe0665261897d29f0ab3`.
Root and independent review accept this embedded baseline only. Metadata live
import, loaded-corpus availability and migrated parity remain unrun.

## R7 later classic/VL cohort: source-only audit accepted — 2026-09-04

Generator `bd2859e976dd0e9e27bc5ff3767248a4115ae59b` records the bounded
eleven-case inventory in
`docs/r7-classic-single-instance-source-inventory-2026-09-04.md`. Root and an
independent reviewer accepted all eleven canonical registry-row and raw recipe
identities, planning/projection orders, template/algorithm bindings, and the
three declared source-note identities against source `232b9de`. The named
loader, planners, execution/projector sources, catalog and standards lock have
no scoped source drift. This is not a whole-engine equivalence claim.

The proposed sequential subcohorts are CT1, DX/MG3, CR1, US1, PET1, XA/XRF2
and VL2. Shared algorithm providers prevent treating these case sets as
independent implementation boundaries. Fixed names, prefixes and projection
rules remain explicit R7.2/R7.3 debt; native sample-byte hints are not measured
Part 10 output sizes. No baseline, import, generation, parity or new capability
was established by this audit. Metadata-three consumer preparation remains
the sole active migration boundary.

Verification was read-only source extraction, JSON/byte/hash comparison and
independent source review. `git diff --check` passed. The documentation-only
changed-test dry route selected no commands; its reported unconditional Fast
coverage was not executed. No native command, build, Heavy, package, release
or external-state mutation ran for this source-only unit.

## R7 metadata source-note consumer prerequisite accepted — 2026-09-04

Corpus `7a07be81e9e1f5e69f2540b2da8982ce852a51fe` extends the source-bound
CI and static member checks for exactly the frozen metadata-three provenance.
Live content remains0.2/13 cases; content0.3 appears only in synthetic fixtures.
No registry, recipe, note, binary pin, schema or earlier evidence was changed.

Notes retain exact original paths, public `source-note.<stem>` IDs,
`text/markdown`, raw hashes/sizes and per-case references. Unknown, missing,
duplicate, orphan, renamed or altered evidence fails closed. Metadata rows
have no smoke annotation exception, assets or dependencies. Raw Git snapshots
authenticate regular member modes, closed inventory and registry/recipe/note
bytes before routing; the parsed registry and descriptor path must coincide.
These integrity checks do not replace the complete public loader.

Configuration2/route2 shapes and config bytes are unchanged. The additive note
ownership reason unions old/current owners, reverse impact and dependency
closure. Global changes retain fixed smoke3 union affected cases, never a
full-core fallback. Historical smoke/core fixtures and the smoke-only verifier
remain intact. Root reviewed the complete diff and final raw-Git regression;
independent review accepted the production boundary and42 focused tests.

Focused CI passed31 in0.265s and static smoke12 in0.033s. All ordinary tests
passed188 in1.282s before commit and188 in1.230s after commit; no failures
occurred. The clean-candidate dry route from `3caac10` to `7a07be8` selected
smoke3, seed1/parallelism2 and zero dependencies, with config SHA
`71ccf9726648d9573dbfafa57b1b82f8b0d9eaac2aefd3db689bcb6092db75c7`.
This was route inspection only: no artifact was opened or output created.
Diff checking passed. No native acquisition, generation, validation, report,
build or Heavy qualification ran. Exact metadata import and subsequent public
availability/parity evidence remain separate, sequential boundaries.

## R7.1 exact metadata-three definitions imported — 2026-09-04

Corpus `24485f027431945398fd21a4fd98125a4e9ffeb3` adds exactly the three
accepted metadata registry rows, recipes and original local source notes as
content0.3.0 under unchanged bundle1. Root and independent review verified
all six new member byte strings against frozen provenance, preserving prior
thirteen registry rows, case descriptors and recipe bytes. Only core membership
gains the three explicit IDs; other profile objects, dependencies, assets,
source planning/projection orders and both binary pins are unchanged.

The complete definition closure is21 regular files totaling98,603 logical bytes.
Descriptor9,541 bytes has SHA
`8e53a7f8eaa54737f93351a7ca93f5b069cbc54c9e5e236383e1549e31ecc038`;
registry34,886 bytes has SHA
`5d0242b1b0ecbb16c4baa4f4aa536810cd74bafd5edefc2a934d5001bc6c609c`.
The six new recipes/notes retain their previously recorded20,103 raw bytes.
These are definition costs, not generated-payload or runtime measurements.

Static definition checks passed12 in0.035s; all ordinary tests passed188
in1.229s before commit and188 in1.217s after tracking the new members.
Diff checking passed. The exact clean dry route from `fc7b894` to `24485f0`
selects only smoke3 union metadata3, explicit IDs transported through `all`,
with zero dependencies; it does not select all core cases or execute a native
job. Configuration SHA remains `71ccf9726648d9573dbfafa57b1b82f8b0d9eaac2aefd3db689bcb6092db75c7`.

Root additionally evaluated the full Draft2020 schemas for the clean candidate's
descriptor, registry and all16 recipes:18 checks passed in0.138266208s using
Node24.19.0/Ajv8.18.0. Retained supplement
`generated/r7-metadata-import-schema-review-20260904/receipt.json` has SHA
`2176ec4deeeb9a9d7ed66b2a908d1366ec16d029e28bfa0edd6593de2d1f0eca`;
its script SHA is `0aaebecd72bfcce068a479e08c91cc4c57091f61042693d6671d247f35e20c22`.
The private original is retained at `/private/tmp/r7-metadata-import-schema.JRV0uB`.
This is static schema evidence, not native availability or DICOM validation.

Metadata public-loader selected planning and migrated generation/parity remain
unexecuted. No original baseline or earlier parity proof was replayed, and no
new native acquisition, generation, validation, report, build, Heavy or release
qualification occurred. Embedded copies remain until the terminal removal gate.

## R7 metadata retained-availability verifier prepared — 2026-09-04

While native execution awaits separate approval, corpus `131c306` adds
`scripts/verify_metadata_availability.py` and self-contained synthetic tests;
`59da8c8446a8a3be0c0055e42caf608c132c1b88` explicitly maps those two paths
to bounded ordinary CI. The checker only reads explicit response, acquisition,
bundle and baseline paths. It neither acquires nor launches a generator, writes
evidence, or authenticates that a native invocation actually occurred.

Its exact response contract binds the accepted baseline receipt and raw manifest,
receipt-bound expectations, original pinned discovery documents, full current
bundle members and all16 cases/eight profiles. The selected assessment must be
exactly metadata3/core/seed1/parallelism4 with sorted explicit selectors, complete
direct-ready rows, source-ordered artifact IDs and the accepted plan hash.
Publication and validation remain `not_run`. Discovery changes only the loaded
corpus plus its explicit migration-status field; the distinct planning projection
retains every non-corpus domain and legacy identity. No permissive normalization
or general schema-evaluation claim was added.

Independent review identified an empty-directory/enumeration-error gap in the
first capture draft. The accepted checker rejects undeclared directories and
walk errors, as well as links, extra/missing files and changed raw bytes. Tests
use explicit synthetic trust roots, not ignored artifacts or sibling sources.
Root separately exercised only its pure baseline/acquisition/bundle-input
functions against the already retained real documents. Those bindings passed:
content0.3 digest `231690d2d27667afea83f3ab3553cdca5375858c511da6c22029130792789292`,
21 files/98,603 bytes, original source `232b9de`, and plan `05620ed8acf08ed665e3c73291b8ed45706440ba248c86b55d1157ebf3b59c03`.
No actual metadata availability response exists yet, and none was fabricated.

Focused tests passed13 in0.371s and independently13 in0.375s. An independent
initial filename typo selected zero tests and is not counted as verification.
All ordinary tests passed201 in1.602s before commit and201 in1.595s afterward.
Read-only `--help` and diff checks passed. The exact clean dry route from
`4d92998` to `59da8c8` selected fixed smoke3 and zero dependencies, with config
SHA `5cd42d3c2d401bdac35bbb91db9ed61cc1466fa89161f4a345c43bef0a01bd2c`;
it did not open an artifact or create output. Native approval remains pending;
no generation, validation, report, baseline replay, build or Heavy qualification
ran. R7 availability/parity and the broader terminal gates remain incomplete.

## R7 metadata native availability failure retained — 2026-09-04

After explicit user approval, one native loaded-capabilities call at corpus
candidate `f7946d8c101c7b3d5bff2d08b6888123cfb7b86f` failed with exit2,
`resource.document.invalid`, `retryable:false`. It selected exactly metadata3
under core/seed1/parallelism4, from a copied bundle and unrelated empty-PATH
working directory. Stdout was empty; stderr269 bytes has SHA
`acbe96fec7f984a81945f8669c56fb963db381d0d6325945706e92d206a06f93`.
No availability binding check or DICOM generation occurred, and no native retry ran.

Root checked the retained error envelope against full Draft2020 schema:
one error-schema check passed in0.048109416s with Node24.19.0/Ajv8.18.0.
Supplement `generated/r7-metadata-availability-failure-schema-20260904/receipt.json`
has SHA `0e627d98d212e418c1e74afbbf593d0392fbf50b82be33dd779ad85af92e19bf`;
script SHA `09bb676b925b5bb3f97f6637e1a6bf997ae94895c79245249dee54cb02f199a2`.
This validates the failure document's shape, not availability or successful loading.

The retained corpus root `artifacts/r7-metadata-availability1-20260904` contains
receipt46,530 bytes SHA
`e4a2e4eecbac89ff7198c566e6d2a002b8955dce7810be42a5c7be905ccfea40`.
Fresh pinned acquisition took1.625143500s, the single loaded capabilities call
0.597122292s, and the entire capture2.453987917s. Before its receipt, evidence
contained32 files/69,711,294 logical/69,771,264 allocated bytes; the receipt
adds49,152 allocated bytes. Copied definitions remained21 files/98,603 logical/
131,072 allocated bytes, and the verified cache remained69,314,672 logical/
69,316,608 allocated bytes. Root and independent review accepted failure
evidence only: all72 tracked source blobs/modes, inputs, copied bundle, original
artifact and final cached binary were verified unchanged; the working directory
remained empty. The corrected capture harness SHA is
`2ccdd03870dced2525fc31edee2c4740c27e19be5aa33599291264946056d83a`.

An earlier Python3.9.6 preflight failed before evidence creation, acquisition
or native execution because `Path.stat(follow_symlinks=False)` is unsupported.
The exact failed harness, stderr and invocation are separately retained under
`artifacts/r7-metadata-availability-preflight1-20260904`. Root reviewed the
single equivalent replacement with `Path.lstat()` and its read-only artifact
check before authorizing the native attempt. The preflight is not silently
discarded or counted as a native run.

Read-only pinned-source diagnosis identifies the first rejection:
`src/corpus_definition/mod.rs:513` requires evidence storage paths under
`evidence/`, whereas this import used `standards/source-notes/`. The later
`source_note_evidence_id` accepts both reference namespaces but does not bypass
that earlier storage gate. Our prior source/schema reviews missed this ordering;
their static passes do not establish runtime acceptance. This is an invalid
caller storage path, not demonstrated absence of metadata generation capability.

The bounded proposed correction relocates only note storage/declaration paths
to `evidence/`, retaining raw bytes, source provenance, evidence IDs, registry
query strings, recipes and generated standards expectations. Consumer guards
must distinguish immutable source paths from canonical caller storage paths;
that correction and a new content identity require separate review before a
fresh native check. No generator implementation or pin change is indicated by
the proven first rejection. Metadata availability/parity remain unproven.

## R7 canonical evidence-path consumer correction accepted — 2026-09-04

Corpus `91401e0` introduces an explicit versioned source-to-storage mapping:
metadata-bearing content0.3.0 retains original paths only for rejected historical
base inspection; content0.3.1 requires `evidence/<same-basename>`. Stable note
IDs, media types, raw hashes/sizes and all registry source queries remain exact.
Other metadata content versions and mixed layouts fail closed. Every route
rejects0.3.0 as its target, including documentation-only routes; old/base snapshots
remain available for exact old/current ownership comparison. The retained
availability checker now requires canonical0.3.1, with no producer or pin change.

Root and independent review accepted the two production changes and four
affected test files. The additional baseline-test file changes only its routing
fixture: it asserts actual historical rejection before separately testing a
synthetic corrected target. Baseline generation/byte/skip oracles and native
capture helpers are unchanged. Tests construct corrected bundles in private
temporary fixtures without mutating live data, preserve the enumeration-error
regression, and remove only exact now-empty fixture directories after relocation.

Focused tests passed32 CI in0.278s,13 static definition in0.045s and14 retained
checker tests in0.387s; independent checks passed32/0.275s,13/0.044s,14/0.384s
plus9 baseline tests in0.042s. The initial full204-test run had203 passes and
one stale routing-fixture error in1.630s. After that narrow correction, all204
passed in1.638s and again1.626s after commit. An initial ad-hoc route inspection
used the wrong config-function arity; its TypeError was corrected, then three
expected live0.3 route rejections were verified without execution.

The exact clean CLI dry route from `a488643` to `91401e0` returned expected
exit2/`ci.scope.unsupported` in0.228382458s: the still-live rejected0.3 input
cannot run CI. This is an accepted rejection check, not a generation plan or
availability pass. Its evidence destination stayed absent. Diff checking
passed; config bytes, corpus data, schemas, pins and historical evidence remain
unchanged in this prerequisite. The separate data relocation and subsequent
corrected-candidate native check are still required.

## R7 canonical evidence data correction accepted — 2026-09-04

Corpus `54edb7737325ef813edacf50d198c82571d778f7` changes only content
version0.3.0 to0.3.1 and three evidence declaration/storage paths to
`evidence/<same-basename>`. Independent and root review confirmed three
byte-identical Git renames; all registry queries, raw registry/recipes, evidence
IDs, media types, hashes, sizes and profile membership remain unchanged. Exact
old empty directories were removed. The closed bundle has21 files/98,561 bytes;
descriptor9,499 bytes SHA-256
`e12f9fff740aa43e20a5963bf5c926094288af71c7fd91f16d6b558244de6697`;
public corpus digest
`91d63240f00c5398d698abd900baadca7a3f4617573a365c45868f49367bfd65`.

Static13 tests passed in0.042s. Before staging, the ordinary204-test run had
203 passes/one tracked-index transition error in1.625s: the old index named
removed paths while both inspected snapshots were the corrected live tree.
After selectively staging the exact repair, all204 passed in1.634s; no test
or routing relaxation was made. Initial empty-hunk relocation was rejected
atomically and then corrected with context-only moves. Two read-only diagnostic
reports reversed a helper return tuple (KeyError/nonserializable set); the
corrected report confirmed old/current routing selects fixed smoke3 plus
metadata3 with no dependencies. No generation ran.

Root full Draft2020 descriptor validation passed one check in0.080277417s
against the clean committed candidate, Node24.19.0/Ajv8.18.0. Retained supplement
`generated/r7-evidence-storage-schema-review-20260904/receipt.json` SHA-256
`e76992a447a74e58c6a745d04712048c2a820cfb02f55a65f376593dca143038`;
script SHA-256
`b78f3b4ae46456e95bf4c6c042aafe01f5c396922b3c64bf585ee9fb5bc35189`.
Registry and recipes are unchanged from their prior full-schema checks. This
is static evidence only: a fresh corrected-candidate public-loader response is
still required. Historical rejected0.3 input and failed native evidence remain
immutable; metadata availability and migrated parity are not yet established.

## R7 corrected metadata loading accepted — 2026-09-04

After the user's bounded loading approval, one corrected-candidate check ran
against clean corpus `ca561cbea6b0f8553434dd111a1e29272dab2185`
(data54edb77, statusaf55cdc, current-docsca561cb). No generator change, rebuild,
pin update or accepted baseline replay occurred. The reviewed harness differs
from the failed-candidate harness only in exact source revision and fresh
evidence root. Its retained8,233 bytes SHA-256
`671f2d503f756dec0c1140577006b3ac40c1602299744cc77785548350aad7d5`
bind `artifacts/r7-metadata-availability2-20260904` in the corpus repository.
The original failed run and preflight remain immutable.

Fresh acquisition of the original `4ca0c6d6` macOS-arm64 candidate passed in
1.603038792s. Exactly one loaded `capabilities` call selected the three explicit
metadata IDs under core, seed1/parallelism4, stress disabled, from an unrelated
empty working directory with empty PATH. It exited0 in1.814245125s with empty
stderr. Response98,111 bytes SHA-256
`bcf63c2350d39fed38a28058fa14475012b5d9806f3d8bcebaf1b313d7c15442`
contains three direct ready rows, no dependencies or generated paths, all16
catalog cases/eight profiles, the exact verified0.3.1 corpus identity above and
the unchanged baseline plan
`05620ed8acf08ed665e3c73291b8ed45706440ba248c86b55d1157ebf3b59c03`.
The full response checker passed in0.011548208s, preserving all identity-domain
fields except the documented loaded-corpus transitions. Its retained1,016-byte
binding result SHA-256 is
`6806df41a194509fdf44fe10fc0afa8d9f5fb2df8f23812ca36a6c945c5cf181`.

Receipt46,496 bytes SHA-256
`337f02fbced484a0604229ec1c5c3255dff3f52e5a0cdde3a5dda94fc56556ed`
records whole-job3.663707583s, unchanged originals/copies/source/cache, empty
working directory and no output/DICOM. Before receipt,33 files occupy69,810,110
logical/69,869,568 allocated bytes; final34 files occupy69,856,606 logical/
69,918,720 allocated bytes. Root reapplied the documented read-only checker;
independent review verified every33-file inventory hash and72 source Git blobs,
pin/acquisition bindings, full response, closed bundle and all unchanged guards.

Full Draft2020 success-envelope/capabilities3 checks both passed in0.317841541s
(Node24.19.0/Ajv8.18.0), without native replay. Generator supplement
`generated/r7-metadata-availability2-schema-review-20260904/receipt.json`
SHA-256 `4617c8d8ae33ad802e7b1cedf66ec38867bc305d5de2ef2bbee8d5fcaeada970`
binds response and both complete schemas; script SHA-256
`09bb676b925b5bb3f97f6637e1a6bf997ae94895c79245249dee54cb02f199a2`.

This closes only corrected metadata3 loading/selected-planning availability.
No generation, strict validation, reporting, all-core or Heavy qualification
ran. Retained JSON binding validity is not invocation authenticity; the separate
capture/log/source evidence supplies that narrower invocation record. Neither
proves independent conformance, migrated payload parity, a release or completeR7.
The next separate gate is a reviewed bounded migrated-parity helper and explicit
native execution authority, not a replay of the accepted embedded baseline.

## R7 metadata parity helper prepared, not executed — 2026-09-04

Corpus `ae0a606` adds only `scripts/prove_native_metadata_parity.py` and its
self-contained synthetic tests. Separate `bc42d25` adds those two exact paths
to existing ordinary/shared-smoke routing, without changing configuration2 or
route2 semantics. Loading evidence remains bound to ca561cb; subsequent current
documentation commits f3b3822/487104d record that acceptance without relabeling
the source candidate.

The new helper requires an explicit original generator artifact, accepted
metadata baseline, accepted corrected availability evidence and fresh evidence
root. It authenticates complete inventories before acquisition can execute
discovery, archives exact clean committed source, and copies caller inputs and
the pinned binary into an unrelated private runtime. The intended native scope
is exactly two identical public-runner calls selecting metadata3/core/seed1/
parallelism4. Each would generate, strictly validate and report, then compare
the complete manifest and report with its repeat. No whole-core, smoke, reporter
baseline, source build, Heavy or terminal replay is included.

The oracle authenticates all31 original unselected rows, exact full file
objects/raw metadata/standards, payload hashes and baseline plan. Only manifest1
to2 schema, explicit request selector/kind, verified caller identity and the
three-row complete selection ledger may differ. The historical report1.1 stays
authenticated; report2 must equal its complete public manifest projection, not
an arbitrary normalization. Neither report upgrades independent evidence.
The helper observes final exact bytes and repeat/final closure, not byte
immutability between the public runner's internal stages.

Root and independent review accepted the final429-line helper/271-line tests.
Review corrected JSON-only equality applied to raw byte maps/sets, missing
expected-path checks, suppressed final checks, and full permission/retention
bindings before acceptance. File and directory modes use full permission bits;
retained caches use the established private-parent/owner/executable verifier.
Original/copied inputs, outputs, sidecars and retained runtime are independently
checked, including failures. No failed check was normalized into a pass.

Focused17 tests passed in0.183s, root0.212s and independent0.186s. Ordinary221
passed in1.847s before tracking (not routing proof) and1.830s after the explicit
mapping; focused routing32 passed in0.277s. A prior16-test run had15 passes/one
fixture failure because macOS stripped setuid4500 to0500; the test now uses a
preserved sticky1500 mode to exercise special-bit rejection. An attempted
py_compile check could not write the protected system bytecode cache and wrote
no file; read-only AST parsing replaced it. Exact helper help and diff checks
passed. No native invocation ran in these tests.

Separately from ordinary tests, root read-only authentication of actual retained
baseline26 files and availability34 files (including receipts) passed in
0.1985115s, then0.190530292s after hardening, with exact corpus91d63240 identity.
The latter binds helper SHA-256
`0631c50478568b9f7f46e8fa250131976ad38c9abfd96a3c06a26d9bb896fff3`.
This confirms input readability/bindings, not execution or generated parity.
The loading-only user approval does not authorize the new two-run boundary;
explicit native approval and later exact output/schema review remain required.

Final preparation documentation is corpus9a920eb/ce04867, clean full candidate
`ce048678890a762c04c8310e0b5d9d66189105ee`. Root corrected the initially
misreported full hash against direct Git output; the short ce04867 identity and
all artifact evidence were unchanged. The exact clean CI dry inspection
487104d tobc42d25 passed in0.256991875s with configuration SHA-256
`ff4bbbb31b922a4d4e2ee70e7574c41444d5ceeb3f90f1eeb17945f8e731eec4`:
ordinary unit discovery plus bounded smoke3 fallback were selected; parity
replay, full corpus, hosted/viewer, Heavy and release remained deferred. No
selected native command was executed. Generator docs-only routing inspection
selected no commands; unconditional Fast was reported, not run. Both worktrees
were checked clean at this preparation handoff; this is not terminal hygiene
qualification or goal completion.

## R7 metadata-three migrated parity accepted — 2026-09-04

Following explicit user approval, the prepared boundary ran once against clean
corpus `ce048678890a762c04c8310e0b5d9d66189105ee`. Evidence is retained at corpus
`artifacts/r7-native-metadata-parity1-20260904`; receipt183,141 bytes SHA-256
`199a18f9237038ee9021758e97b2c121f78a918c447983e0f9660b20f50ed386`.
The10.224665666s proof invoked only Git archive, a clone-local import audit and
the two approved runner calls. It did not build, retry, run whole core/smoke/
Heavy or invoke the historical reporter.

The first/repeat runners completed in3.973892625s/3.53421125s with empty stderr.
Each used the original pinned generator through the public corpus runner,
selected the same explicit metadata3/core/seed1/parallelism4 request, generated
three files, strictly validated exactly three with no failures, and projected
report2. Raw manifests are byte-identical101,955-byte objects SHA-256
`c86a40077784e4702dcdb1a7eb1454ee5515e76275e7f5ff5d2ee48750fe339c`;
raw reports are byte-identical113,957-byte objects SHA-256
`8c7320f195823e14ddc56e041fb788b30516b3bc506b4805a3e83e85ee7778c2`.
Canonical manifest/report SHA-256 are respectively
`7d96e2906a347e14dcba673735c06d519a95e11ee3f54e3739a2e6b469f293d4`/
`9c7f164128cb6962bf12af73b6f66168cb73e9d8ce1f7cb2cc523cd8e8bb1986`.

The complete file projection remains
`3404c2a8b8c47e974f8856fea3c7e69748cca97c72d23c98aeb787cd4c662c01`:
three payloads/3,024 bytes with original hashes7f457e4f,5a0726a6,b1334cff,
full raw metadata and standards arrays9/7/6. All31 historical skips authenticate
before their exact replacement by three direct/generated ledger rows with no
dependencies. Only the documented manifest1→2 schema/request/verified0.3.1
identity/ledger boundary differs. Report2 exactly equals its full pure source
projection and does not upgrade independent evidence.

Independent review reauthenticated baseline26 files/141,256,194 bytes,
availability34/69,856,606 bytes, source74 tracked files/1,113,262 bytes and exact
archive. It rechecked payloads, metadata, standards, ledger, manifests, reports,
strict validation, eight-file sidecars, pins, modes, directory closures and empty
unrelated CWD. Root's separate pure recheck passed in0.219526s. Pre-receipt
evidence has169 files/353,372,087 logical/353,718,272 allocated bytes; final has
170/353,555,228/353,902,592. Retained runtime has160 files/351,995,598 logical/
352,329,728 allocated bytes. Retention cost is explicit, not a Fast-PR cost.

Root full Draft2020 validation passed18 checks in0.368554083s across both actual
consumer results, manifests, reports and all generation/validation/report
envelopes/results. Generator supplement
`generated/r7-metadata-parity-schema-review-20260904/receipt.json` SHA-256
`a7766fa3f7f94beb46419be64daa499dcbfbbc4b39ba06142ecbee24c7e72ed8`;
schema script SHA-256
`be39e056485a8778c5a94754c56c6d2e4d5c55e4d6c4a520d9163e9e932ecbda`;
pure-recheck script SHA-256
`79d5964cfb03d2a8f384b303cff5e58beac75c0c8f6362df16d953fee423c0a7`.
An additional independent read-only audit passed34 Draft2020 checks in0.277043s
validator time (3.5s invocation). It covered both standalone and consumer-
embedded envelopes/results plus embedded/file report payload equality, and
reauthenticated receipt, source archive
`aa2ae265c32c98c5c935455af24367fbfbf2a1a9459a618980a628cf98c63699`,
input receipts, inventories and payloads. It created no permanent artifact and
did not replay native execution; the retained root18-check supplement is the
durable schema receipt.

This accepts R7.1/R7.5 only for the metadata-three slice. It does not complete
R7.2/R7.3 generic-capability review, wider ordinary-native migration,
relationships, codecs, external providers, isolated legacy/negative/fuzz/stress,
independent conformance, a release or terminal qualification.

## R7 CT1 source prerequisite accepted — 2026-09-04

The next ordinary-native slice is exactly
`classic/ct/mono2_i16_rescale_12bit_explicit_le`. Corpus604dc0f adds a
26,113-byte immutable source fixture SHA-256
`4b2ae940d6d5262e6573ee3e9b2667aa151ffde26ef8c971b2c89332ecfea4ba`
and its14,228-byte static test SHA-256
`48604cc8832f0f372e9ae0e3c5f0a09d8d181eb98ce963adaf0eb8d348f45542`;
b4dbd12 separately maps those exact paths to ordinary CI ownership.

The fixture binds source232b9de, the complete2,358-byte canonical registry row
SHA-256 `1b6c043c5df419ddbc7b27466f8fa19f8034688f78479d6a01b0432beaf6affa`,
raw3,938-byte recipe SHA-256
`f014e51c72b094dd188267fbe6a36e3caeebc6828fcc415045fcb02a758a84ca`,
all11 ordered standards records, template `classic/ct@1.0.0`, native classic/
pixels providers, algorithm, orders200/87, signed2x2/12-bit pixels, rescale/
window values, no dependencies/assets/member evidence/runtime requirements and
the original DTS/dicom-test-suite/0.1.0 payload strings. The eight-byte frame
hash `d3e8d5fb...` is explicitly not a Part10 file identity.

Root directly read every one of12 declared Git objects at source232 and verified
all bytes/sizes/SHA values, plus full row/raw/typed recipe equality and exact11
evidence rows. Independent review repeated the12-object comparison against both
source232 and5c01827, including modes/blobs/template entry/standards lock/schema.
Focused8 tests passed in0.006s root/agent and0.005s independent. Routing32 passed
in0.291s; all229 ordinary tests passed in1.850s; diff checking passed.

Source inspection proves the pinned4ca0 binary has a verified-bundle route to
this exact external recipe, but does not prove runtime readiness. CT planning
still depends on classic/CT namespaces, orders200..206, fixed template/content/
algorithm contracts and typed static constraints, so R7.2/R7.3 genericity debt
remains explicit. No accepted retained CT payload/full-file/report baseline
exists. One unretained temporary output suggested a1,198-byte b7a7e95d payload,
but the fixture records only truncated expected-to-confirm diagnostics and no
hash-refresh authority. A new authenticated embedded CT1 baseline is required
before data import; no native invocation, import or availability claim ran here.

## R7 CT1 baseline helper prepared, not executed — 2026-09-04

Corpus commit `394b6144e1b9fde73a84a5b97388c913abc8c7db` adds the
fail-closed CT1 baseline helper and synthetic tests; corpus commit
`0a5cc525a2df21d07f55443464f7eaece2560836` separately maps those two
paths into bounded ordinary CI ownership. The31,890-byte
`scripts/capture_native_ct_baseline.py` has SHA-256
`454b1f7a9209ebba370d00832229480e6674aa344477b586df135dfccf94da95`;
the25,635-byte `tests/test_capture_native_ct_baseline.py` has SHA-256
`0d2eabeec62b895837a92248c22f3a48634077f2823f5f965bb94616b7dd0843`.

The helper binds source232, the full34-row source fixture and exact33 skips,
then permits only one exact CT1 core/seed1 generation with the original pinned
generator, one strict validation with that same generator and one coverage1.1
report with the separately pinned repaired reporter. Complete payload, Part10,
pixel, six-UID,39+4 validation,11-standard, report, source, mode and directory
closure checks are retained with acquisition records and a failure receipt.
Pins are reverified before every command; there is no build, download, network
or sibling fallback, normalization, retry, deletion, Heavy, conformance, import,
public-loader request or parity claim.

An initial active synthetic draft exposed two contract errors (12/14 passed in
0.069s): test pin overrides did not reach the shared globals, and the command
incorrectly expected an unsupported parallelism flag. Both were corrected
without weakening the evidence contract. The final focused suite passed15/15
in0.215s for the authoring agent,0.201s for root and0.200s independently;
routing passed32/32 in0.335s, all244 ordinary tests passed in2.073s and diff
checking passed. The corpus repository was clean after the two commits.

No native artifact was acquired or invoked and no CT DICOM, manifest, report or
retained baseline exists yet. Live corpus content remains0.3.1 with16 cases and
21 members. Native capture still requires separate approval and a fresh evidence
root; CT import, runtime availability, migrated parity and R7.2/R7.3 genericity
remain open.

## R7 CT1 baseline1 failed; helper correction accepted — 2026-09-04

After explicit authorization, the prepared helper ran once at clean corpus
`4f0c3e6882d715efddf2ce239f11123e9cc48ae3` and failed closed without retry.
Evidence remains at corpus `artifacts/r7-native-ct-baseline1-20260904`. Its
26,894-byte receipt SHA-256 is
`d0a967ca377d318594e065ae40826b26bf73f24ecca3439d7dfff87db075afdf`;
status is failed at generation postcheck with `unaccepted full-file diagnostic
differs`. The5.241005083s job completed both pinned acquisitions, then invoked
exactly one generation. That command exited0 in1.404867959s with empty stderr;
strict validation and reporting never ran.

The retained 1,198-byte Part10 file SHA-256
`b7a7e95dced9092c23e56815b6083e4b630f557bcb1508d55ef82d4d8fb7e732`
and its resolved instance plan
`598cb71e85d1cfa9b8976a1025a5c97e59438a36c756ada4821777d838a8b8df`
match the source diagnostics. The143,348-byte manifest SHA-256 is
`07fae743d9f511472673d5a53c40cd43c67070429db4bafa0c1ecdd849ee0201`.
Its9,221-byte canonical file object SHA-256 is
`82fd8a1658aa588cff6ae2644ee1e2538bf60fc6df03a2d7d1ed14e40030dcbd`
and its explicit-selection plan is
`ad33f99c00e2d17f93e07b2aa663ca82bbd63444aeb3ceb94c47b620adbe4d6e`.
The earlier78ca59/35cfc5 prefixes came from a non-authoritative profile-wide
diagnostic: the global selected-plan identity is embedded in the file object,
so it cannot constrain a one-case selection. This is not generator
nondeterminism or a DICOM failure.

Review also found four masked helper assumptions: source emits full image-*
recipe keys, an object-shaped visual check and five CT UID fields; stored signed
12-bit bytes are `000c00000004ff07` and require low-12-bit sign extension. Corpus
commit `26aaf61` preserves the old diagnostics as superseded, binds the exact
retained failed-run observations, corrects those source-shaped checks and adds
adversarial tests. Final files are: provenance27,630 bytes SHA-256
`9188dcad32ddfcb648d379b197ea38e48ffb256839f6138d18bb98c5ff8f55a1`;
provenance test16,458 bytes SHA-256
`9c7cb8783b505085b6f456b3479442f997a223bb061c351a6bee7d2e126e1a79`;
helper32,695 bytes SHA-256
`a2dbf85e3ebe60bf217c7576b25c2575b638aea68bc8aee54a3aa858a0b4bca5`;
helper test30,307 bytes SHA-256
`46bdca744b2dc31ed11e72cdb663c8e5926ddd4c90f4cda957367e3e2e4ceca4`.

Author/root/independent focused results were respectively provenance8/8 in
0.010s/0.008s/0.009s and helper18/18 in0.236s/0.232s/0.234s. Diff checking and
read-only validation of the retained generated output passed. No correction
changed the generator or reporter, reran native code or promoted baseline1.
The retained set has15 files/139,435,737 logical/139,472,896 allocated bytes.
A fresh baseline2 must independently complete strict validation and reporting
before CT import, availability or parity can proceed.

## R7 CT1 embedded baseline2 accepted — 2026-09-04

Under the existing authorization, the corrected helper ran once from clean
corpus `62676290b3056ea7dc1f2022b6418b6737bdcec8`. The bounded baseline is
retained at corpus `artifacts/r7-native-ct-baseline2-20260904`; its37,034-byte
receipt SHA-256 is
`e34a25ca0bb2720dab0d5736bb08f51736fcfb169e0db54608ce10b3c5c3da51`.
The6.611080083s job executed exactly one generate, one strict validate and one
coverage1.1 report in1.408821999s,0.251495459s and1.011095916s. All exited0
with empty stderr; there was no retry, build, network, broader profile or Heavy
work.

The output exactly reproduces baseline1's generation bytes: manifest143,348
bytes SHA-256
`07fae743d9f511472673d5a53c40cd43c67070429db4bafa0c1ecdd849ee0201`
and Part10 file1,198 bytes SHA-256
`b7a7e95dced9092c23e56815b6083e4b630f557bcb1508d55ef82d4d8fb7e732`.
Canonical manifest/file/skips SHA-256 are respectively
`8b2aa76210738f2cac3c43408f633fcd910860563da8545a36d7e7a56c4696f3`,
`82fd8a1658aa588cff6ae2644ee1e2538bf60fc6df03a2d7d1ed14e40030dcbd`
and `2ffb7b01d4c900a77333029e75df85c3e34d046611c16cf3f3330704533a1b4d`.
The exact corpus/resolved plans are ad33f99c/598cb71e above. The raw frame is
`000c00000004ff07`, which sign-extends at12 bits to -1024,0,1024,2047.
The source-shaped recipe, visual object, five UIDs,39 internal+4 standards
checks and11 ordered standards records all match.

Strict validation accepted exactly one file with no failures. The697,228-byte
report SHA-256
`1f2016e94846d0693a46753e2342e431f78814c595d7d9a2a3a0567fb2841d9b`
has34 unique rows, counts generated1/skipped33/others0 and33 exact gaps. Its
complete object equals the report envelope projection. The retained pre-receipt
inventory is22 files/141,110,112 logical/141,164,544 allocated bytes; final is
23 files/141,147,146 logical/141,205,504 allocated. Output remained unchanged
across validation/report, and source, pins, copied inputs, both caches, modes,
owners, directories and empty private CWD passed closure checks.

Independent artifact/source review accepted the same exact command, payload,
semantic, report and closure boundary while confirming baseline1 remains failed
and unchanged. A separate Draft2020 audit passed8/8 schemas in0.511119417s;
24/24 exact envelope/projection/inventory bindings passed in0.093373916s. This
accepts only the original embedded CT1 baseline prerequisite. It is same-project
evidence, not independent conformance or viewer evidence, and does not import
CT1, establish public-loader availability or migrated parity, close R7.2/R7.3,
or qualify wider R7/release scope.

## R7 CT1 consumer version/routing prerequisite accepted — 2026-09-04

Corpus commit `9a42290802bd513518e2253a47f5bea4c145dadb` prepares the
consumer boundary without importing CT1. `scripts/run_ci.py` now pins exact
version inventories: content0.1.0 smoke3;0.2.0 smoke3+core10; rejected0.3.0 and
live0.3.1 plus metadata3; future0.4.0 plus CT1 only. The separate `approved_ct()`
does not widen prior approved cases, and0.4.0 retains the corrected metadata
`evidence/<basename>` ownership. Unsupported version/inventory combinations
fail closed.

The0.3.1→0.4.0 descriptor/registry transition selects exactly CT1 plus the
fixed smoke3, profile all. A CT recipe-only change selects exactly CT1, profile
core, without fallback. Neither route selects metadata3 or the other ten core
cases. The production retained-metadata checker remains frozen to0.3.1; only
its test fixture can reconstruct that exact historical input after a future
live CT import.

An initial combined62-test draft exposed three newly exact version assumptions
and was corrected without relaxing the inventory contract. Final routing,
static-definition and metadata-availability suites passed34/34 in0.415s,14/14
in0.075s and14/14 in0.504s; all250 ordinary tests passed in2.698s and diff
checking passed. Live content remains0.3.1 with16 cases/21 members. No corpus
data, schema, pin or native command changed; the exact three-file CT import is
the next sequential boundary.

## R7.1 CT1 static import accepted — 2026-09-04

Corpus test commit `6664ecc` first corrects the synthetic metadata transition
fixture to reconstruct exact content0.3.1 after the live tree advances. This was
required because the initial exact data draft exposed three static failures:
two fixtures relabeled a17-case clone as0.3.1 and one appended CT twice. The
13-line fixture correction preserves the strict version inventories.

Corpus commit `c2bf8d7` then imports exactly three data paths. The raw CT recipe
is3,938 bytes SHA-256
`f014e51c72b094dd188267fbe6a36e3caeebc6828fcc415045fcb02a758a84ca`;
the17-row registry is38,525 bytes SHA-256
`cdfaa0041b1b8fe67e257cf43bb26e6de1aa6b0e191defb0b339337e655a2840`;
the content0.4.0 descriptor is9,997 bytes SHA-256
`80488c8248124e8215131bd670c6c2fe330400c960bcef5f4a8b145083a4d691`.
The recipe is byte-identical and the registry row semantically exact to frozen
source232. The descriptor adds CT1 only, with no dependencies, assets or
evidence members; smoke remains3, core becomes14, and the closed bundle has22
files/106,636 bytes.

Independent Draft2020 validation accepted the complete recipe, registry and
descriptor against their vendored schemas. After the fixture correction,
smoke-definition14/14, routing34/34 and metadata-availability14/14 passed in
0.062s/0.324s/0.417s; all250 ordinary tests passed in2.187s and diff checking
passed. This accepts static R7.1 ownership for CT1 only. Public-loader
availability, generated parity and R7.2/R7.3 genericity remain separate; no
native command, viewer, conformance or wider core run occurred.

## R7 CT1 selected planning readiness accepted — 2026-09-04

Corpus commits `be2e239192a298c1a2bc75045d9209b85fc99dcc` and
`fedd06b44183227c294a5dff895fe195e5d97769` add the bounded capture helper and
its explicit ordinary routing; corpus status commit
`4073834f2faae62ca2c6a6381463a3f67de4ed77` records the independently accepted
execution. The retained evidence is
`artifacts/r7-ct-availability1-20260904` in the corpus repository. Its
86,647-byte canonical receipt SHA-256 is
`2c50290fe972004483bf274a70929cecb7d597a887ddb28e1e4c1dad44ff7f5d`.

From clean corpus source `fedd06b44183227c294a5dff895fe195e5d97769`,
the helper acquired the unchanged69,314,672-byte generator artifact SHA-256
`4ca0c6d6a8e4cbab81b7005b4354c7ff558c44747e00f41d22d3abbfd50b7768`
from source `232b9de41f97ee95abe1ecc40b6b8b70ebeeea5f` under the exact R5
macOS-arm64 pin. Acquisition took1.624879916s. Exactly one managed command ran,
without retry: `capabilities` for profile core, seed1, parallelism4 and only
`classic/ct/mono2_i16_rescale_12bit_explicit_le`, using the copied content0.4.0
bundle from an empty private working directory and empty `PATH`. It exited0 in
1.872721334s; binding took0.013188625s and the whole capture took3.786612542s.

The copied22-file/106,636-byte corpus has descriptor SHA-256
`80488c8248124e8215131bd670c6c2fe330400c960bcef5f4a8b145083a4d691`
and framed identity
`ccd14b73d81cf9d6f49f950174331cb418824e8888fb476768365a623b4b6d79`.
The retained result contains the complete17-case/eight-profile catalog and one
direct, ready CT ledger row with no dependencies or artifact paths, artifact
`curated_ct_mono2_i16_rescale_instance`, executable artifacts true and plan
`ad33f99c00e2d17f93e07b2aa663ca82bbd63444aeb3ceb94c47b620adbe4d6e`;
publication and validation are both `not_run`. The final evidence closure is34
files/71,294,167 logical/71,356,416 allocated bytes and14 directories. Source,
pin, original artifact, copied and original corpus/baseline inputs, private
cache, modes, owners, directories and empty working directory all passed the
recorded before/after guards.

Independent review reauthenticated all83 source-archive members, the complete
evidence inventory, both discovery documents, the raw response and the pure
binding result. With jsonschema4.26.0 Draft202012Validator, all57 generator
schemas and8 corpus schemas passed their meta-schemas and all25 applicable
retained instances passed in1.56s; a separate pure-checker replay passed in
0.06s. These audits launched no native process.

This accepts only CT1 selected public-loader planning readiness. It does not
generate, strictly validate, report or publish the migrated case, and it does
not establish migrated parity, viewer or independent-conformance behavior,
whole-core readiness, R7.2/R7.3 genericity, packaging or release qualification.
The next bounded CT boundary is the separately reviewed migrated parity proof.

## R7 CT1 migrated parity accepted — 2026-09-04

Corpus commits `5eef29c68e4cfc0cb44ab0a63ce932eba9193a24` and
`29adf18430f0f97bb4dd08cd934e2459e2964235` add and route the bounded CT1
parity proof; `b3b2ed06452ae8acbcdbec55d4ed1757755ea77f` records its accepted
evidence and `776f59c0267b3926156a45b026804561e5cd6fec` updates the current
corpus guidance. The proof ran once
from clean source `29adf18430f0f97bb4dd08cd934e2459e2964235`. Its complete retained
evidence is `artifacts/r7-native-ct-parity1-20260904`; the214,692-byte receipt
SHA-256 is
`589283faeb834c9074bfe9065f46a90668783adfe33840289c1386eb364d11ce`.

The9.841235333-second job executed exactly four outer commands: a source
archive in0.023593250s, an isolated snapshot-import check in0.116439500s, and
two identical explicit CT-only public-runner calls in3.897124833s and
3.409951709s. Each runner used profile `core`, seed1, parallelism4 and only
`classic/ct/mono2_i16_rescale_12bit_explicit_le` from an empty private cwd
with `PATH=""`. Each performed one external-corpus generation, one strict
same-generator validation and one report2; all commands exited0, all stderr
streams were empty, and no command was retried. No build, network, smoke,
whole-core or additional native call ran.

Both runs emitted the same1,198-byte DICOM SHA-256
`b7a7e95dced9092c23e56815b6083e4b630f557bcb1508d55ef82d4d8fb7e732`.
Their74,640-byte raw manifests are identical at SHA-256
`767363c3ce60d6c68293064d69c6f95b4d840eca6d45e8cd3d42cd6334f5a2d2`;
the parsed manifest canonicalizes to56,610 bytes/SHA-256
`79633200bd93fc7b31e932b6579e7077f90677e8f4dc7714bd203b981840ab12`.
The complete file object is9,221 canonical bytes/SHA-256
`82fd8a1658aa588cff6ae2644ee1e2538bf60fc6df03a2d7d1ed14e40030dcbd`
and the one-row direct ledger is2,639 canonical bytes/SHA-256
`f5c9bb9163498541d17edb7f0a0879c54dc5fc9217f306298d43a10a87798725`.
The manifest1-to-manifest2 difference is limited to the declared external run
kind/selector, verified corpus identity and replacement of the complete33-row
unselected bookkeeping by that one direct generated row.

Both83,967-byte raw reports are identical at SHA-256
`d9fd7719a1c17d4c313aacc0e352260677d3f588695ff933f23f2f4f8129202b`;
the exact report2 projection canonicalizes to60,630 bytes/SHA-256
`0914a95222e31ab8abedbcd32173c80e14025871bd2d092c0d622acebbb038c9`.
Each validation-result1 checked one file, returned valid with zero failures,
and preserved the accepted CT pixel, UID, semantic, standards and full-file
baseline. Report2 records one logical/direct/generated case and one emitted
file, with zero dependencies or qualifications. It remains a manifest
projection with validation and independent conformance `not_assessed` and
`payloads_reopened: false`; the distinct strict-validation result is not
rewritten into the report.

The proof reauthenticated baseline receipt
`e34a25ca0bb2720dab0d5736bb08f51736fcfb169e0db54608ce10b3c5c3da51`,
availability receipt
`2c50290fe972004483bf274a70929cecb7d597a887ddb28e1e4c1dad44ff7f5d`,
the22-member/106,636-byte content0.4.0 corpus and framed corpus identity
`ccd14b73d81cf9d6f49f950174331cb418824e8888fb476768365a623b4b6d79`.
It used the unchanged69,314,672-byte generator SHA-256
`4ca0c6d6a8e4cbab81b7005b4354c7ff558c44747e00f41d22d3abbfd50b7768`
from source `232b9de41f97ee95abe1ecc40b6b8b70ebeeea5f`, product0.2.0,
aarch64-apple-darwin, no enabled features. The1,464,320-byte clean-source
archive SHA-256 is
`b5c6cc2680403ae4c66688d83b94ccb7a6a3b2a42cdc9375a6813f35638e0960`.

Before the receipt, the evidence root held173 regular files/354,939,023
logical/355,303,424 allocated bytes. The original and retained runtime trees
each held164 files and55 directories, with353,324,302 logical and353,677,312
allocated bytes; bytes, closure, modes and owners matched. Source, original
artifact, copied inputs, cache, unrelated cwd, both output roots and both
eight-file sidecar roots passed all final guards.

Independent semantic replay passed14 check groups in0.368613083s. A separate
read-only Draft2020 audit validated all57 generator and8 corpus schemas against
their meta-schemas and85 applicable retained instances in1.59s. The generator
schema set has no coverage-report1.1 schema, so the historical baseline
report1.1 remains hash- and semantic-bound rather than schema-qualified;
purpose-specific receipts, baseline projections and parity binding records
likewise have no standalone schemas. Neither independent audit launched a
native process.

This accepts exact same-project migrated parity for CT1 through the public
runner. It does not establish independent conformance, viewer behavior,
interoperability, package/release qualification, whole-core or wider classic
coverage. R7.2/R7.3 also remain open: current CT recipe admission and planning
still depend on classic/CT case-name prefixes and the reserved planning-order
range, so independently named generic CT dispatch requires a separate reviewed
generator change before embedded ownership can be removed.

## R7.2/R7.3 CT1-local genericity accepted — 2026-09-04

Generator commits `932c93c508a13582d108c0126c67146b1d4bca8d`
(planner), `800efab2f322d60738d197c253be2965a8fd85cd` (loader),
`61c47fce535f8068848906fc8e335cb3bb06eacc` (standalone SDK/CLI
proof), `82be8c8d5f1ee546315b2d07e24feea05f03d29b` (exact byte oracle)
and `a03354d11b03a03a30c6589da43d880bda039063` (corrected routing
count) close the reviewed CT1-local R7.2/R7.3 genericity boundary.

Planning and loaded-corpus admission now recognize the complete stable tuple:
provider `native.classic_plan`, template `classic/ct@1.0.0`, content
`content.native_pixels` with empty parameters, algorithm
`algorithm.classic_ct`, projection family `ct`, and the strict typed
provider/artifact parameters. Any CT marker makes partial or mixed tuples fail
closed. The exact accepted tuple no longer relies on caller case ID, recipe ID,
output namespace or a reserved planning-order range: the proof uses
`caller/arbitrary/signed-ct`, recipe `caller_signed_ct`, output
`caller/arbitrary/signed-ct/caller-instance.dcm`, and planning/projection order
900. Case and recipe strings still intentionally participate in deterministic
identity and metadata; they no longer select CT capability.

The planner tests reject wrong or missing algorithm, template/version,
content, projection, provider, planning order, provider/artifact parameters and
mixed multi-artifact tuples. CT dispatch short-circuits before transitional
name matchers. Non-CT `native.classic_plan` fallthrough remains bounded to its
existing named families, DX and other classic behavior is unchanged, and
stress CT remains on separate provider `native.stress_ct_plan`. Legacy and
stress selections were regression-tested but were not generalized by this
slice.

The standalone fixture is exactly three files / 6,498 bytes / three
directories and contains no embedded-resource or sibling-repository
dependency:

| Member | Bytes | SHA-256 |
| --- | ---: | --- |
| `definition.json` | 1,607 | `1f33541dfba0df229be6e3d9d3aadc405d842f8842cb7ca7eff9ea7cf29efb5d` |
| `members/cases/registry.json` | 991 | `efb0c68ba6fa5c9b8b417e23b24934e031ef3cb8e503af356b2fff2bb3c69359` |
| `members/cases/recipes/caller-signed-ct.json` | 3,900 | `63b3b00b0577d7e0b227e11250471d6763e25ca596b766e51c0e0690c43051db` |

Its verified identity is `fixture.generic-ct` version `1.0.0`, schema1,
manifest SHA-256
`1f33541dfba0df229be6e3d9d3aadc405d842f8842cb7ca7eff9ea7cf29efb5d`,
and framed corpus-definition SHA-256
`8e99cc8d2983f3063583e7f2bf558380a7cdbb9d2001772ec00f4ec5f5079544`.
The fixture has no dependency, evidence or asset members and contains none of
the migrated CT case, recipe-path or output-path strings. Its support
compilation unit imports product code only through `synth_dicom_gen::sdk`; the
CLI proof invokes `CARGO_BIN_EXE_synth-dicom-gen` as a black box.

SDK inspection and CLI/SDK generation agree on exact plan SHA-256
`d3a5a83f33caf7abdce7a6df5c3675754e48e40e78d17968fe83236b1fdfadb4`.
Each generates exactly one direct selected file and zero dependencies. The
1,194-byte Part 10 file SHA-256 is
`c292a81584998e9afe56330545f455c8894684e475c817d60c1c93ef755e1ce1`;
the recipe frame SHA-256 is
`d3e8d5fb105307e91174c36e8413e25cb8494efc509628cf515819478b217121`.
Manual measurement of the caller output records a 68,966-byte raw manifest2,
SHA-256
`290879c1929ce97dc081c33ad7ffbf6702afa89206ceebe7c4278c9d27c1bd29`.
The SDK and CLI manifests and payloads compare exactly, and output closure is
only the DICOM plus `manifest.json` under the arbitrary caller namespace.

Generation-time validation records all file checks passed. Separate SDK and
CLI strict validation each check exactly one file, return valid and have zero
failures. Their report2 projections preserve the complete source manifest and
verified identity, with one logical/direct/generated case, zero dependencies,
one emitted file, `manifest_projection`, validation and independent
conformance `not_assessed`, and `payloads_reopened: false`. These are
same-project generation, validation and reporting boundaries, not independent
conformance.

Accepted routed and focused verification was:

| Scope | Result | Time |
| --- | ---: | ---: |
| captured planning library route | 4/4 | 5.30s |
| external CLI module route | 7/7 | 22.60s |
| SDK corpus module route | 8/8 | 45.62s |
| corpus-generation subsystem route | 92/92 | 43.30s |
| engine corpus-plan module route | 22/22 | 0.77s |
| corpus-definition library route | 25/25 | 9.46s |
| planner exact unit | 1/1 | 0.51s |
| loader adversarial units | 2/2 | 0.46s |
| SDK standalone CT proof | 1/1 | 2.56s |
| CLI standalone CT proof | 1/1 | 6.25s |
| historical CT byte-projection oracle | 1/1 | 4.48s |
| routing regression after count correction | 28/28 | 2.698s |

Ownership passes 22 targets / 274 entry groups / 1,479 entries, including 920
integration entries; its SHA-256 is
`2876a19c05af104903d95e406163b646ce35fcef0675bed633b7d215b9671d0f`.
The corrected routing configuration SHA-256 is
`ad90a2c6faff0ba69fc530ae93f4277293c48fe9e4d361b032398f937bce1367`.
The bounded predecessor-to-`a03354d` route selects exactly the six routed
commands above plus unconditional Fast coverage and preserves explicit Heavy,
future external-corpus and release-candidate deferrals. The corpus-definition
route now binds declared, actual-list and regression counts at 25/25/25.
Formatting, routing and diff checks passed and the worktree was clean.

The exact ordinary command `cargo test --locked --all-targets
--no-default-features` is recorded as a non-passing diagnostic, not acceptance
evidence. At `a03354d` Cargo stopped after the library target in approximately
30.2–30.65s: 538 tests were observed, with 530 passed, two failed and six
ignored; later test binaries did not run. The exact unrelated failures were
`composition::manifest::tests::projects_the_same_plan_used_by_the_writer_without_case_identity`
(`Schema("\\"1.0.0\\" was expected")` at `manifest.rs:1105`) and
`tests::validate_generated_root_rejects_parent_traversal` (expected substring
`safe relative path` at `lib.rs:39143`). Both reproduced identically from a
clean archive of prepatch HEAD
`c0914fbfccf065cfee8e62aaf6b7421e98d97f08`, establishing that they predate
and are unrelated to this CT change. This does not imply a full ordinary pass.
No Heavy body ran.

This accepts R7.2/R7.3 only for the exact CT1 capability tuple and its
standalone SDK/CLI proof. Reverse-dependency cleanup and removal of the
embedded CT recipe remain separate work. Other classic/VL, series, codec,
stress and legacy families, whole-core migration, independent conformance,
viewer interoperability, packaging and release qualification remain open.

## Post-CT ordinary baseline restored — 2026-09-04

The exact no-default ordinary baseline now passes at generator HEAD
`bb86ac1`. The repairs were kept granular and did not broaden the verification
boundary:

- `fb64c85` updates the parent-traversal regression to assert the current
  fail-closed manifest-contract rejection, and `375c035` uses the current
  `synth-dicom-gen-` temporary-path prefix in the composition fixture.
- The diagnostic resource-identity change `f000126` was reversed exactly by
  `0496d6e`; the pair is retained in history and has no net resource-identity
  effect. `2facc13` instead makes the composition fixture consume the frozen v1
  resource identity used by production and rejects substitution of the current
  v2 identity.
- `d4b7849` freezes the provider-backed quantitative output version and adds
  exact byte/plan regressions for the five affected outputs. `79b99a0` updates
  the generation-spine audit to positively classify that version boundary, and
  `79c0eb4` gives the audit an exact ordinary change route.
- Release-test compatibility fixes `6e006c7..12bcae6` preserve the renamed
  product diagnostic, consume current producer result schemas, format the
  focused Rust change, assert split resource provenance, and refresh the two
  fail-closed release ownership digests. `bb86ac1` then routes standards hashing
  through verified `EngineResources` bytes instead of a repository-relative
  standards read.

The diagnostic progression is part of the evidence. The first exact all-target
run cleared the two pre-existing library failures but stopped when the pinned
composition backend environment was absent. After preparing that locked local
test environment, focused qualification exposed and repaired the P8 byte
oracle. The second exact run then reached the stale spine audit; later exact
runs exposed the release-harness assumptions and captured-planning resource
lookup. Each failure was repaired at its owning boundary before the exact
command was repeated. The final command was:

```sh
/usr/bin/time -p cargo test --locked --all-targets --no-default-features
```

The first passing run began from a clean `bb86ac1`, but the status edit and
`bda2282` commit occurred while its later test binaries were still executing.
Its 305.84s real, 1441.39s user and 101.99s sys result is therefore retained
only as a passing mixed-revision diagnostic, not exact-revision acceptance.

An uninterrupted repeat with clean, unchanged HEAD
`bda2282458e46250a1e92c3b897ab157406a9468` passed in 293.95s real, 1414.70s
user and 97.54s sys. The library target reported 533 passed and six ignored.
The ordinary nightly-labelled harnesses executed, including release nonfast
14/14 and schema subsystem 86/86, but the six R2.3 Heavy test bodies remained
ignored. No Heavy qualification script was invoked. This uninterrupted run is
therefore exact ordinary regression acceptance only: it is not a Nightly Heavy
or release-candidate run and does not establish independent conformance,
viewer interoperability, package qualification, cross-target support, or any
terminal acceptance row by itself.
The status-only follow-up passed change routing, `git diff --check`,
`release_ci__fast` 15/15 and `schema_resources__fast` 73/73; release-candidate
evidence remained explicitly deferred.

## R7.1 DX/MG3 static import accepted — 2026-09-04

The source-bound DX/MG3 transition is accepted at corpus commit
`bdba58a984d107435ec1f8060250901a9107cdf2`. The imported cases are the DX
display-shutter object and the MG for-presentation and for-processing objects.
The live bundle is now content `0.5.0`: its exact inventory contains 20 logical
cases and 25 physical members. This remains an additive, static-definition
boundary; it does not claim that the public loader can generate the new cases.

The fail-closed routing prerequisite was established by corpus commits
`ded7d23708fa884782e67a85deb93e740c376af7`,
`13246fe4e33e0054eca4ee71f318494f6a9706ad`,
`dda191fa94888585a8ce024a154a5f61b9ae9993`, and
`70b4ca1e0701df2936f2f8b4cf8c3158721a1e6e`. Those changes admit only the
source-provenance-bound `0.4.0` to `0.5.0` transition, freeze the future
inventory by identity rather than count, assign the three exact recipe paths,
and declare the bounded `struct` import used by the retained baseline helper.
Before the live definition advanced, commits
`0b82cf61d9e4e6d7731b8407c56f3ce83f80618c` and
`553fbf2b2ffc329aa2024d1287776ccd5aed9648` repaired historical fixtures so
they reconstruct their original inventories instead of silently consuming
new live rows.

The exact imported static artifacts are:

| artifact | bytes | SHA-256 |
| --- | ---: | --- |
| `corpus/cases/registry.json` | 52,836 | `d39b5cc57dae7c525b90eb2cedead7a2725feaade636719f552eaec550220b37` |
| `corpus/corpus-definition.json` | 11,575 | `d8981e050b9b280bb398f725c09d2f167e1167c954c18e6da599f5095d77e71b` |
| `corpus/cases/recipes/classic/dx/dx_display_shutter_mono2_u16.json` | 4,037 | `82228d5ef2be7496cf084c41b7b885bb31b3c6f911e93211f92158f71768bb68` |
| `corpus/cases/recipes/classic/mg/mg_for_presentation_mono1_u16.json` | 4,160 | `4c963fca2cacab5f78dc28eae701278a4dc0445d06fef0f15d17a0afba9e76aa` |
| `corpus/cases/recipes/classic/mg/mg_for_processing_mono2_u16.json` | 4,694 | `cec2846945593d2d6c70e2faf839b459045c67128a082eee59f673262de07d78` |

Focused smoke-definition, metadata-availability and routing suites passed
15/15, 14/14 and 38/38, respectively. The combined static suite passed 78/78,
and the full corpus ordinary suite passed 318/318 in approximately 3.883s. The
change-route dry run selected the fixed smoke three plus exactly the DX/MG
three with configuration SHA-256
`757e335e8790572908f1f39f0d8ce255d26240e872c935507adab9c830ac891e`.
It continued to defer hosted delivery, full-corpus qualification, parity
replay, viewer execution, Heavy qualification, and package/release evidence.
The normal route was intentionally not executed because doing so would cross
this static-only acceptance boundary into generation.

This accepts R7.1 static ownership for the DX/MG3 slice only. Public-loader
availability, migrated parity, R7.2/R7.3 genericity, independent conformance,
viewer interoperability, wider R7 completion, R8, R9, and every remaining
terminal qualification are still open. No native generation or Heavy
qualification ran for this checkpoint, and the R7 summary state remains in
progress.

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
| Representative corpus PR | At R0, the corpus repository did not exist and embedded edits selected the full graph | Not independently measurable | R6 local configured metadata-change fixture: 7.151087s, one 926-byte payload; output 73,728 allocated bytes, cache 69,316,608 bytes, no build tree. Docs-only fixture: 2.210458s, no generation. | Local measurements accepted; hosted and exact terminal PR measurements remain open; no comparable R0 Corpus PR percentage is fabricated |
| Representative viewer PR | Viewer repository not in current scope | — | — | Not measured |
| Nightly and release-candidate cost | No separate Nightly/RC trigger; run `33491521696` is exact candidate evidence | Nightly not independently measurable; provider/default/release critical chain 123m53s | — | Explicit boundary recorded |

R2.2 now has a comparable local linking-cost reduction. CI class-specific
terminal costs still require the later routing and R9.6 measurements; the R0.2
baseline alone is not proof that those budgets have passed.

## Blockers and authority boundaries

- On 2026-09-04 the user explicitly approved local file edits and Git commits
  in `/Users/beatrice/AgentFiles/projects/dcmview-test-corpus` after permission
  review rejected the first report-only helper file creation. That rejected
  attempt wrote no files and produced no corpus commit; both repositories
  were clean at resume. This approval resolves the local-path authority
  blocker only. Remotes, pushes, releases and viewer-repository changes remain
  outside scope. The report-only helper and its single retained completion
  execution subsequently passed review, as did the bounded first-core import
  and isolated parity execution recorded above.
- On 2026-09-04 the user explicitly approved creation of the local corpus
  repository at `/Users/beatrice/AgentFiles/projects/dcmview-test-corpus`.
  The empty local Git repository was initialized on `main`; no remote was
  configured and no release was created. Remote creation, publication, and
  generator pushes remain outside this approval. R6 must establish an
  independently obtainable immutable generator pin rather than infer that an
  unpublished local commit is remotely available. The authorized disposable
  R1 probe was closed and its branch deleted after evidence capture.
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
| External corpus contract | In progress | R5 SDK/CLI, loaded-corpus discovery and the dated 2026-09-04 isolated immutable-source consumer gate are independently accepted, with SDK-only imports and both build source roots removed before runtime. This is not package/installed-release or the later separated-repository terminal qualification; R6+ evidence remains required before this row passes. |
| Identity separation | In progress | R4.3 independently projects engine, toolchain, template, provider, schema, standards, execution, verified loaded-corpus, and invocation runtime identities through v2 discovery and current generation, composition, assembly, report, and release contracts. R4.4 removes `cases/**` and `Cargo.lock` from the authoritative v2 engine digest while retaining exact named v1 reconstruction for required compatibility fields. Embedded paths do not fabricate corpus or runtime identities. The supported R5 external-corpus generation route and later physical/default-reader migration remain before the terminal cross-repository row can pass. |
| Smoke migration | Passed for the local R6 smoke boundary | The separate corpus repository clean-clone proof at `3a059de` reproduces all three frozen R0 hashes and the complete 14,109-byte normalized semantic/validation/standards projection. Four runs, explicit-ID selection, repeat equality and metadata-only identity sensitivity are independently accepted. Later terminal candidate qualification remains required; this does not imply remote delivery or a qualified release. |
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
