# `synth-dicom-gen` / `dcmview-test-corpus` cost baseline

**Recorded:** 2026-09-01

**Baseline revision:**
`65a296bbb489fcaaff22e38fa35036f0805ccab6`

**Code-equivalent starting revision:**
`fbd0f76a36dc5726bb41602f44bff290588f560d`

**Plan item:** R0.2 of
`docs/synth-dicom-gen-dcmview-corpus-separation-plan.md`

## Scope and interpretation

Revision `65a296b` differs from the starting revision `fbd0f76` only by the
R0.1 ADR and migration-status document. `git diff --quiet` confirms that
`.github/workflows/ci.yml`, `Cargo.toml`, `build.rs`, `src/`, and `tests/` are
code-equivalent. This record therefore freezes the R0.2 development-cost
baseline at `65a296b` without attributing a code or workflow change to R0.1.

The closest authoritative remote timing source is GitHub Actions run
`33491521696` at exact immutable release-candidate revision
`69d3e5f8e045752b6e183781a7e13190a61430ff`. Public API metadata and the dated
standalone status provide job, step, artifact, and qualification identities.
The workflow at `65a296b` differs from that candidate workflow in one material
cost detail: commit `f1d1727` removed the feature-independent
`composition_curated_migration` target from each in-process codec job after the
run exposed the duplication. Consequently the observed codec durations below
are exact historical measurements and conservative evidence for the current
job projection, not a claim that `65a296b` was remotely timed. A new remote
run was deliberately not created for this documentation baseline.

No WSI, stress, all-profile, full-profile, package, archive, release, codec,
or external-provider qualification was rerun for R0.2. The only new compile
was an all-target, no-default-feature `--no-run` build in a task-specific
temporary target directory. No generated corpus or other durable artifact was
created.

## Local clean-compile and linking baseline

The repository was clean before measurement. The exact operation was:

```sh
baseline_target=$(mktemp -d /private/tmp/dts-r02-target.XXXXXX)
baseline_log=$(mktemp /private/tmp/dts-r02-compile.XXXXXX)
CARGO_TARGET_DIR="$baseline_target" /usr/bin/time -p \
  cargo test --locked --all-targets --no-default-features --no-run \
  2>&1 | tee "$baseline_log"
du -sk "$baseline_target"
rg -c '^  Executable ' "$baseline_log"
rm -rf "$baseline_target"
rm -f "$baseline_log"
```

| Measurement | Exact R0 value | R1/R2 acceptance informed |
| --- | ---: | --- |
| Clean compile wall time | 72.29 seconds | R1.5 build reporting; R2 gate clean-build comparison |
| Compile user/system CPU | 381.29 / 50.37 seconds | Diagnostic context for link amplification; not a wall-time substitute |
| Explicit target-directory size | 8,013,463,552 bytes (7,825,648 KiB; 7.46 GiB) | R1.5 disk controls; R2 gate clean disk comparison |
| Cargo-reported executable artifacts | 188 | R2.2 integration-binary consolidation and R2 gate |
| Integration-test targets | 186 | R2.1 ownership inventory and R2.2 target limit of at most 20 |
| Other Cargo harnesses | 2: `src/lib.rs` and `src/main.rs` | Explains 188 minus 186; these are not integration targets |

The integration count was independently obtained in two ways: 186 top-level
`tests/*.rs` files and 186 Cargo metadata targets whose kind contains `test`.
The temporary target directory was exactly
`/private/tmp/dts-r02-target.xAApSK`; the temporary log was exactly
`/private/tmp/dts-r02-compile.cI5wKJ`. Both paths were confirmed absent after
measurement. The existing repository `target/` was not used, measured,
modified, or removed.

## Current workflow ownership and routing

At `65a296b`, `.github/workflows/ci.yml` is the only workflow. It triggers the
same complete job graph on every `push`, every `pull_request`, and every manual
`workflow_dispatch`. It has no path filters, scheduled trigger, workflow-level
or job-level concurrency group, or `cancel-in-progress` policy. Thus a branch
push associated with a pull request can receive equivalent push and PR runs,
and superseded runs are not cancelled. There is no independently selectable
Fast PR, Corpus PR, Nightly, or release-candidate workflow.

| Current job | Current work and owner | Required verification-class destination | Acceptance informed |
| --- | --- | --- | --- |
| `Native provider contract` | Prepares the locked highdicom Python runtime and serially owns strict cancellation/provider timing plus two focused migration/quantitative contracts | Subsystem; Nightly/Release candidate only when its provider surface is invalidated | R1.2, R1.4, R1.5 |
| `Default corpus` | Owns formatting/JSON, the complete no-feature all-target suite under global `RUST_TEST_THREADS=1`, standards lock, smoke/core/extended generation and validation, report, and smoke reproducibility | Split Fast PR and named Subsystem work from Nightly broad defaults and Release candidate | R1.2, R1.4, R1.5; R2.2-R2.4 |
| `Standalone release contract` | Owns warning-denied checks, package, external SDK consumer, native archive build/verification, five installed consumers, adversarial archive test, and upload | Release candidate; narrow contract tests may also have Fast PR/Subsystem owners | R1.2, R1.5, R1.6; R2.1-R2.4 |
| Five `Codec (<feature>)` jobs | Each prepares highdicom, compiles all 188 targets under a distinct feature, runs codec targets/rejection, and generates and validates the complete `extended` profile | Codec Subsystem for feature-sensitive tests and selected cases; applicable Nightly matrix for broader capability evidence | R1.2, R1.3, R1.5; R2.2-R2.4 |
| Two `External codec compile (<feature>)` jobs | Each recompiles all targets for one external-codec feature, with default features enabled because `--no-default-features` is absent | External-codec Subsystem and applicable Nightly/Release candidate target | R1.2, R1.3, R1.5; R2.2-R2.4 |

Provider/runtime setup is repeated in the provider, default, release, and all
five in-process codec jobs. Package/archive ownership is confined to the
release job, but that job separately invokes Cargo package verification, an
external SDK test, a release build, installed consumers, and the archive test.
The two external-codec jobs compile only and upload nothing. The only upload
step is in the release job; generated default and codec corpora are ephemeral
runner content.

## Exact remote job measurements and verification-class projection

Run `33491521696` was a `push` run. Attempt 1 began at
2026-09-01 09:18:30 UTC; its release job ended at 11:22:23 UTC. The failed
JPEG 2000 job was retried alone from 11:23:28 through 11:30:45 UTC. The full
observed wall interval through successful retry was therefore 7,935 seconds
(132m15s). The jobs run concurrently except for the provider -> default ->
release dependency chain, so wall time must not be computed by summing jobs.

| Job execution | Class projection | Exact runner time | Per-job rounded billable minutes |
| --- | --- | ---: | ---: |
| Native provider contract | Subsystem | 179 s (2m59s) | 3 |
| Default corpus | Nightly broad default | 6,745 s (112m25s) | 113 |
| Standalone release contract | Release candidate | 504 s (8m24s) | 9 |
| Codec: JPEG | Codec Subsystem / Nightly matrix | 400 s (6m40s) | 7 |
| Codec: CharLS | Codec Subsystem / Nightly matrix | 400 s (6m40s) | 7 |
| Codec: JPEG XL | Codec Subsystem / Nightly matrix | 439 s (7m19s) | 8 |
| Codec: deflate | Codec Subsystem / Nightly matrix | 332 s (5m32s) | 6 |
| Codec: JPEG 2000, failed attempt | Codec Subsystem / Nightly matrix | 391 s (6m31s) | 7 |
| Codec: JPEG 2000, successful retry | Codec Subsystem / Nightly matrix | 437 s (7m17s) | 8 |
| External codec: OpenJPH | External-codec Subsystem | 360 s (6m00s) | 6 |
| External codec: legacy DCMTK JPEG | External-codec Subsystem | 358 s (5m58s) | 6 |

The actual run consumed 10,545 seconds (175m45s) of runner time across attempt
1 plus the isolated retry. Applying per-job whole-minute rounding yields 180
billable runner minutes. By exclusive primary class this is 3,296 seconds and
58 rounded minutes for Subsystem jobs (including the failed and retried codec
job), 6,745 seconds and 113 minutes for Nightly broad-default work, and 504
seconds and 9 minutes for Release candidate work.

For a one-successful-execution-per-current-job projection, replacing the
failed JPEG 2000 duration with its successful retry produces 10,154 seconds
(169m14s) and 173 rounded runner minutes. This remains a projection because
the exact run predates `f1d1727`'s removal of the unrelated curated migration
target from codec jobs. No cost reduction is claimed until comparable R1/R2
measurements exist.

## Existing heavyweight ownership and measured timings

The authoritative standalone status records these isolated measurements. They
were not rerun. Each remains required when its named dependency surface changes
and once for an applicable exact release candidate; none belongs in Fast PR.

| Expensive target and exact recorded timing | Owner and required class | Acceptance informed |
| --- | --- | --- |
| `case_recipe_catalog::data_first_sc_and_metadata_values_and_hashes_match_current_generator_bytes` — 683.69 s | Byte-parity/generation owner; Nightly and Release candidate | R1.2; R2.1, R2.3, R2.4 |
| `curated_stress_manifest::typed_stress_projection_matches_frozen_file_values_and_resources` — 681.02 s | Stress projection owner; explicit heavy Nightly/Release candidate | R1.2; R2.1, R2.3 |
| `curated_stress_sc_integration::all_stress_sc_cases_execute_through_private_streaming_services` — 688.21 s | Stress execution owner; explicit heavy Nightly/Release candidate | R1.2; R2.1, R2.3 |
| `generate_cli::generate_command_writes_all_profile_union_and_skips_planned_cases` — 686.18 s | Full-profile CLI owner; explicit heavy Nightly/Release candidate | R1.2; R2.1, R2.3 |
| `wsi_direct_plan::ordinary_wsi_direct_plans_match_fresh_seed_one_bytes_and_manifest_facts` — 691.07 s | WSI parity owner; applicable provider Nightly/Release candidate | R1.2, R1.3; R2.1, R2.3 |
| `wsi_pyramid::stress_profile_emits_complete_three_instance_wsi_pyramid` — 685.37 s | WSI stress owner; explicit heavy Nightly/Release candidate | R1.2, R1.3; R2.1, R2.3 |

These six measurements total 4,115.54 seconds (68m35.54s). The later exact
candidate execution also records 722.30, 732.99, 747.37, 754.35, 724.44, and
699.13 seconds for the same six ownership categories. Both series show why the
targets need explicit entry points and invalidation routing; neither series is
substituted for the current clean-compile measurement.

The run's exact release-job step timings further expose rebuild ownership:
mandatory contract gates took 164 seconds, package plus SDK consumer 128
seconds, installed archive build/qualification 149 seconds, and archive
harness 50 seconds. Those are release-candidate observations, not Fast PR
budgets. The standalone status separately records a locked offline package
containing 794 files, 13.7 MiB uncompressed and 2.2 MiB compressed with
SHA-256 `bc104dec6834c1a42e8f2aa31dcbc85e77908775bb1a82d1e7a7e98f0cbf5b25`;
it is historical package evidence, not a newly generated R0 artifact.

## CI artifact inventory

The public Actions API reports one artifact for run `33491521696`:

| Identity | Exact value |
| --- | --- |
| Artifact ID | `9798112659` |
| Actions name | `dicom-test-suite-Linux-X64` |
| API artifact ZIP size | 9,929,745 bytes (9.47 MiB) |
| Source revision | `69d3e5f8e045752b6e183781a7e13190a61430ff` |
| Artifact ZIP SHA-256 | `69013d604e481099ec1678b57ac8b40f3477951a577d14ef6315e7128187a258` |
| Contained archive | `dicom-test-suite-0.1.0-x86_64-unknown-linux-gnu.tar.gz` |
| Contained archive SHA-256 | `7d573e6f213884c660e1c025be4d42816303640f9576b7ea57334f7d0d0afe0e` |
| Resource identity | 240 resources; `3b2f84098c7a9ccdcea58758b21f1b11b5d989b4c9f391dbef771524b95ea46a` |

The API does not expose the contained archive's uncompressed or raw tarball
size, and the dated status does not record it. It is therefore explicitly not
available from the accepted evidence. The artifact was not downloaded for
R0.2. Current workflow projection is one uploaded release artifact when the
release job runs successfully and zero uploaded corpus artifacts.

## Representative development-cost boundaries

| Requested scenario | R0 baseline |
| --- | --- |
| Generator Fast PR | Does not independently exist. Every push/PR selects the full graph; the closest exact successful graph is 132m15s wall through retry, 175m45s actual runner time, and 180 rounded billable minutes. |
| Generator Subsystem change | Not independently measurable because jobs have no path/metadata routing. Named subsystem jobs exist, but the full graph still runs. |
| Corpus-definition edit | Not independently measurable. Corpus files are embedded product inputs and a pull request selects the same full graph; there is no Corpus PR class yet. |
| Viewer PR | Not measurable in this repository and no viewer workflow is in scope here. The downstream corpus repository does not yet exist. |
| Nightly | Does not exist as a scheduled or separately invocable class. Broad default and codec work currently run on every event. |
| Release candidate | Not independently invocable: the release job is part of every event and depends on the default job. Exact candidate critical-chain work completed from 09:18:30 through 11:22:23 UTC (123m53s), before the isolated codec retry. |

R1 must make these classes independently observable before it can claim a Fast
PR budget or nonduplicated ownership. R2 must repeat the clean compile with the
same flags and an explicit fresh target directory, report its size and linked
harness count, and compare it to this baseline without omitting behavioral
assertions.

## Data availability and non-claims

The local `gh` client was authenticated with an invalid token. Public unauthenticated
GitHub API reads nevertheless supplied the run, attempt, job, step, and artifact
metadata. GitHub's account-level billing export, cache inventory, and artifact
download contents were not accessed. Monetary charge is not inferred; the
billable figures above are runner minutes calculated from exact job durations
using per-job whole-minute rounding.

R0.2 records a baseline only. It does not pass the R0 gate, prove R1 or R2,
qualify `synth-dicom-gen`, establish `dcmview-test-corpus`, or transfer the
historical `dicom-test-suite 0.1.0` evidence to either future product.

## Proportional verification

```text
cargo test --locked --no-default-features --test standalone_docs
result: 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
elapsed: 0.20 seconds

git diff --check
result: passed with no output
```

The documentation-only verification does not qualify generation, packaging,
release artifacts, codecs, providers, stress, WSI, or either future repository.
