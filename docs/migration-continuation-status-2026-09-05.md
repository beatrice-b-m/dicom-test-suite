# Separation migration continuation — 2026-09-05

Contract: [separation plan](synth-dicom-gen-dcmview-corpus-separation-plan.md).
Prior accepted evidence remains in the
[migration record](synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md).
This continuation does not replace historical hashes or promote unrun gates.

## Starting checkpoint and ownership

Generator revision `1d8a354` and corpus revision `2dfc7cc` were initially clean.
R0–R6 are accepted at their recorded boundaries. R7 is incomplete; the latest
DX/MG3 evidence proves selected planning availability, not migrated parity.
CT1 parity and its later source-only genericity qualification are distinct
accepted boundaries. The corpus pin remains source `232b9de`.

The root owns this record and integration review. A read-only inventory agent
audited remaining R7 dependencies. A second agent reviewed the existing DX/MG
parity helper, then received exclusive edit ownership of corpus
`scripts/prove_native_dx_mg_parity.py` and `tests/test_native_dx_mg_parity.py`.
No genericity or corpus-definition edits run alongside this parity boundary.
Unrelated edits appeared in corpus `scripts/run_ci.py` and
`tests/test_ci_routing.py`; those files are excluded from our ownership and
must be committed or otherwise resolved by their owner before a clean native
evidence candidate can be frozen.

## Remaining bounded task queue

Completed R0–R6 items are not replayed. Each following unit requires selective
staging, its own descriptive commit, review, proportional verification and an
evidence entry before dependent work starts.

| Plan item | Bounded units and owned surfaces | Acceptance and verification |
| --- | --- | --- |
| R7.1/R7.5 DX/MG3 | Repair existing parity comparator and tests; review; freeze clean corpus commit; execute two exact selected-case public-runner calls; independently inspect retained receipt | Full baseline payload/manifest semantics and repeat equality; separate strict validation and report projection; Corpus PR synthetic preparation followed by explicit parity execution |
| R7.2/R7.3 DX/MG | Capability tuple in generator `src/recipes/classic_dx_mg.rs`; sequential loader/planner integration; standalone CLI/SDK tests and guides | Independently named cases and paths work; partial tuples fail closed; owning Subsystem and public-consumer verification |
| R7.2/R7.3 metadata | Typed variant admission in `src/recipes/metadata_sc.rs` and loader; separate standalone fixtures | Preserve one/two-artifact and pixel/metadata constraints while removing exact case-ID dispatch; Subsystem plus public-consumer evidence |
| R7.1–R7.5 remaining core | CR1, US1, PET1, XA/XRF2, VL2; then paired variants, multiframe and series/geometry, one closure at a time | For each: source freeze, embedded baseline, exact import/routing, loaded availability, migrated/repeat parity, then genericity; no simultaneous shared loader changes |
| R7.1–R7.5 extended and relationships | Query registry and recipe closures; freeze family-specific inventory before assigning files; migrate ordinary native and derived relationships separately | Explicit reference closure, source notes, full semantic comparison and availability; owning Corpus PR/Subsystem gates |
| R7.1–R7.5 codecs/providers | Partition by feature, provider and dependency closure; move definitions then qualify generic capabilities through supported boundaries | Preserve byte/semantic determinism classes, executable fingerprints and missing-runtime outcomes; selected feature/provider gates at explicit cadence |
| R7.4/R7.5 isolated scopes | Separate legacy, negative, fuzz, stress, media and protocol units with named source/consumer owners | Expected-invalid, payload-free, reduced-scale and independent/peer evidence remain isolated; applicable explicit qualification slices |
| R8.1 | Artifact-key schema, canonical implementation and invalidation fixtures in corpus repository | Bind generator revision/version, definition, seed, features and external runtimes; key boundary accepted before parallel plumbing |
| R8.2/R8.4 | Separate publication/retention workflow and changed-case/refresh routing units | Manifest/report/checksum/unavailable closure; changed cases plus dependencies only; synthetic workflow checks then explicit publication evidence |
| R8.3 | Locate viewer repository and read its guidance; implement artifact consumption in viewer CI | Ordinary viewer changes neither compile generator nor regenerate full corpus; measured representative viewer PR |
| R8.5 | Corpus/viewer triage schema and binding tests after artifact key acceptance | Generator, definition, artifact and viewer failures have distinct owners; no automatic release-evidence invalidation |
| R9.1 | Remove embedded ownership by already-qualified slices, sequentially updating resource/planner/test contracts | No generator dependency on corpus policy; preserve supported historical readers and evidence |
| R9.2 | Package include inventory and independent package-consumer checks | Required neutral resources/licenses/guides retained; corpus tests and payloads excluded |
| R9.3 | Current documentation in each repository, one coherent section/contract per commit | Search current claims and execute documented commands; preserve marked historical records |
| R9.4/R9.5 | Freeze exact independent candidates; execute each claimed native target and corpus qualification row once | Complete release matrix, explicit unavailability and viewer artifact entry; no evidence inherited from old candidates |
| R9.6 | Collect terminal class-specific timing/storage/artifact measurements and compare R0 | Generator, corpus and viewer ordinary budgets plus Nightly/release costs recorded without fabricated comparisons |

Only disjoint read-only audits proceed during shared compatibility work.
Later R7 implementation parallelism requires proven nonoverlapping case/provider
closures and named file ownership. R8 artifact plumbing and viewer-result work
may run independently only after the artifact key is accepted. R9 releases
remain sequential exact-candidate gates.

## Review finding and measurements

The existing DX/MG repeat comparator rebases the output root but misses the
legitimate adjacent `<out>.dcmview-run` evidence root and its report path.
Consequently authentic repeated consumer results would fail comparison after
generation. The fix must recognize exactly those two roots and descendants,
while preserving lookalike sibling strings. No native proof ran before review.

Root preparation check:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -p 'test_native_dx_mg_parity.py'
```

At corpus `2dfc7cc`, 12 synthetic tests passed in 0.339 seconds. This did not
detect the missing real consumer sidecar shape and is not native parity proof.
No build tree or generated artifact was created by this check.

R8, R9, remote delivery, viewer execution and terminal release evidence remain
unqualified. The goal remains active until all plan and terminal gates pass.

## R7 DX/MG3 migrated parity accepted — 2026-09-05

The isolated proof at corpus source `80335e3043b981aaec42800f5f3be411ff70cf43`
passed in `artifacts/r7-native-dx-mg-parity1-20260905`. Its 253,806-byte receipt
has SHA-256 `93c0b66eed29f669cd89d2272116200c5bcd81e08f6fc04105d70769b08fece6`.
The source was a clean temporary clone of that exact commit, excluding unrelated
uncommitted CI edits. The user subsequently confirmed no other active task and
authorized necessary changes; those reviewed CI edits were committed separately
as `272ca9d` and do not change the proof's frozen source.

Preparation commit `80335e3` fixes repeat comparison of the exact adjacent
`.dcmview-run` evidence root while continuing to reject lookalike siblings.
Its 13 synthetic tests passed in 0.344s. CI commit `272ca9d` routes the two exact
parity helper/test paths to ordinary synthetic verification, with 42 routing
tests passing in 0.494s. Root review repeated these suites successfully in
0.367s and 0.487s; diff checks passed. Neither ordinary suite invokes generation.

Exactly two public-runner calls selected the three DX/MG IDs under core,
seed1, parallelism4. Each performed generation, strict validation and report2
through the pinned supported CLI from an empty private unrelated directory
with empty PATH. The calls took 3.981423083s and 3.591930833s; the complete proof
took 10.388857208s. There was no compiler, retry, whole-core generation,
external provider, viewer or independent conformance invocation.

The explicit generator remains source `232b9de`, product0.2.0, artifact SHA-256
`4ca0c6d6a8e4cbab81b7005b4354c7ff558c44747e00f41d22d3abbfd50b7768`.
The submitted content0.5.0 identity remains
`c826f39f14e525f0b9c8e465d64ca6c7a9f4389c25df82128fd2a68c3825fec3`.
Both outputs reproduce all three accepted baseline5 payloads (4,628 bytes):
MG presentation 1,586 bytes, MG processing 1,546 bytes and DX shutter 1,496 bytes.
The entire file array, including UIDs, plans, pixels, validation and standards,
is unchanged, canonical SHA-256
`eed5b71d90dc1918042602fba685cdb5c6dbc052c50deaeca4ff4a4a1cde1c20`.
Only the declared manifest1-to-external2 identity/selection migration applies;
the historical 31 unselected rows remain authenticated in the baseline.

Both raw manifests are 125,881 bytes with SHA-256
`91778ac092229e8abdaf662bd2106c45e28b60d1e9effed6f6dbcb9539c2c1ee`.
Both raw reports are 140,896 bytes with SHA-256
`e1457873526658180ba2e56a6618f68b228a34cf40346e5c8e82e78b661159ab`.
Each output occupies 139,264 allocated bytes. Evidence excluding the receipt
contains 195 files, 357,165,273 logical bytes and 357,556,224 allocated bytes,
including frozen source, copied prior evidence and explicit binaries. No build
tree was created and no payload, cache, ordinary manifest or report was committed.

Root independently checked all 195 recorded file hashes and complete baseline,
first and repeat payload/file-array equality. A separate read-only agent replay
with subprocess APIs forbidden authenticated baseline5, availability, copied
bundle, acquisition identities, full manifest/report parity and the 98 source
archive members against recorded SHA-256/Git blob identities. No native rerun
was used for review.

This accepts bounded R7 DX/MG3 same-project migrated parity. Report2 remains a
manifest projection with validation and independent conformance not assessed;
strict validation is separate captured evidence. DX/MG genericity (R7.2/R7.3),
remaining corpus slices, embedded removal, viewer behavior, R8/R9 and terminal
release qualification remain open. Accepted evidence must not be relabeled as
qualification of a later generator pin or corpus definition.

## R7.2/R7.3 DX/MG planner and admission checkpoint

Module commit `d7d3a78` and ownership follow-up `8ff5d6e` replace case-prefix
selection with the complete DX/MG template/provider/content/algorithm/projection
contract. Explicit caller paths are accepted; historical recipes and parameter
semantics remain unchanged. Three pure planner tests passed in 0.63s after a
27.37s initial clean compilation and 1.65s warm build. Assertions were added
within the existing four owned test entries. The root reviewed both commits.

The subsequent loader/shared-dispatch unit admits these exact tuples under
independent case/recipe names and dispatches before unrelated historical family
matchers. Partial or crossed tuples fail closed. New bundle and dispatch tests
cover all three variants, explicit caller paths and a misleading MR case name;
the non-generic negative control now uses CR. A separate read-only integration
review found no actionable defect. Root owns loader, shared planner, bundle
tests and their exact ownership/routing metadata for this unit.

Verification used `CARGO_TARGET_DIR=/private/tmp/dts-dx-mg-genericity-target`,
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_TEST_DEBUG=0` and
`CARGO_PROFILE_DEV_DEBUG=0`:

| Command scope | Result | Test time |
| --- | --- | --- |
| `--lib corpus_definition::tests::` | 26 passed | 10.92s routed; initial focused build 21.37s and test 10.94s |
| `--lib curated_plan::classic_ct_capability_tests::` | 2 passed | 0.52s |
| `--lib curated_plan::captured_input_tests::` | 4 passed | 6.38s |
| `--test corpus_generation__subsystem` | 92 passed | 31.29s, build23.41s |
| `--test engine__subsystem corpus_plan::` | 22 passed | 0.81s, build1.74s |
| `test_change_test_routing.py` | 29 passed | 2.946s |
| `--test release_ci__fast` | 15 passed | 2.18s |
| `--test schema_resources__fast` | 73 passed | 2.26s |

The dry-run route preceded execution. Ownership validation, rustfmt and
`git diff --check` passed. The shared target measured 782,924 KiB after Fast
checks (801,714,176 bytes), below the ordinary 4-GiB ceiling. No Heavy,
all-profile, provider, package or release command ran. Public caller-owned
CLI/SDK generation proof and documentation remain the next sequential unit;
this checkpoint alone does not close DX/MG genericity or wider R7.

### DX/MG malformed-dimension guard

Before public consumer execution, independent source review found unchecked
`u32` multiplication of caller rows/columns in DX/MG inspection. The root
replaced it with checked multiplication/conversion returning a contract error,
with a 65536-by-65536 malformed-input regression across the three variants.
No pixel allocation is needed to test rejection. Existing valid recipe bytes
and semantics are unchanged. Wrong SOP parameters already fail qualified
template resolution before the ready ledger or publication.

The existing strict-parameter test passed1/1 in0.78s after20.32s compilation
using the shared explicit low-debug, nonincremental target. The route was
inspected; this bounded guard does not require replaying the accepted migrated
parity run. Diff check passed. Root owns only the provider, its test digest,
existing test and this status entry; in-progress public fixture work is owned
separately and excluded from this commit.

## R7.2/R7.3 DX/MG genericity accepted — 2026-09-05

Generator commits `d7d3a78`, `8ff5d6e`, `368a18b`, `66ec6ca` and
`b345ce5` establish DX/MG-local genericity through the supported CLI and SDK.
The exact tuple binds native plan/content/algorithm providers, DX/MG projection,
version1 template and strict typed family/parameters. Partial/crossed tuples
and overflowing dimensions reject safely. One `instance` artifact at order0
remains required; arbitrary case/recipe names, unique planning order and an
explicit caller path no longer determine dispatch. Historical parameter/VR
contracts and recipe/template versions remain unchanged.

The standalone fixture has five documents totaling29,453 bytes. Descriptor
SHA-256 is `60528101a95fa25b27ef19e99a5e8811688ddf9737798030d516b4a8c356f90b`;
framed definition identity is
`40d6fa4aba53a857f1f0f15808cc069f2604f128405f0740f312e89946e08d7b`.
Runtime copies those documents into an unrelated temporary directory; the support
module imports product code only through `synth_dicom_gen::sdk`. Case IDs are
`caller/acquisition/digital`, `caller/acquisition/presentation` and
`caller/acquisition/processing`, with distinct planning orders900–902.
CLI and SDK capabilities agree completely. Both generate the same manifest,
plan and payloads, pass separate strict validation, and return identical report2
projections. Frozen payload identities in generation order are:

| Output | Bytes | SHA-256 |
| --- | ---: | --- |
| `independent/image-1.dcm` | 1578 | `c9c18f9bc81b83cadd9b94ebf2624c58392de050a1aefc12ab1498de105a9471` |
| `independent/image-2.dcm` | 1536 | `6af836bc12a1fe6656588c3e8af351708fa9b9e131adcec0bed37125cf8e2a36` |
| `independent/image-0.dcm` | 1482 | `3b62a81fb80067bcf87194ae5d964751adc71ec26b92345578489f13725138e4` |

A separate exact original-three selection matches accepted baseline5 payload
hashes and sizes1586/1546/1496. The caller payloads intentionally differ because
caller names participate in UIDs and metadata. No output change was hidden by
normalizing payloads or narrowing the historical byte comparison.

The new test is independently named
`external_corpus_cli::caller_named_dx_mg_cli_is_sdk_identical_strictly_valid_and_reported`.
Review rejected attaching it to the CT entry because that would hide ownership
and add unrelated work to focused CT checks. Final exact DX/MG execution
passed1/1 in12.70s after2.37s compilation. Before this entry-only separation,
the full CLI module passed7/7 in26.10s (build2.61s). Final listing proves eight
individually selectable CLI entries, including separate CT/DX/MG proofs.
Ownership passes1483 total/921 integration entries; routing tests passed29/29
in24.581s and diff checks passed. Root reviewed the fixture, imports, complete
manifest/report comparison and direct historical-byte oracle; an independent
reviewer found no remaining defect after test separation.

Storage limitation: routing's internal list-only Cargo invocation accidentally
used the pre-existing default target (20.94s plus0.12s listing compilation).
Its observed total was11,519,240 KiB; the explicit proof target was851,544 KiB.
Those are totals, not measured incremental cost attributable to this unit;
no budget claim is made for the default tree and it was not deleted. Public
proof execution used `/private/tmp/dts-dx-mg-genericity-target` with debug0 and
incremental0. No generated DICOM, ordinary report/manifest or build tree was
committed.

Root documented the exact tuple and remaining limitations in README, system
specification, generation/SDK guides, compatibility policy and changelog.
Four focused documentation tests passed in0.01s (build0.30s). A fresh copied
fixture at `/private/tmp/r7-dx-mg-docs-e1ltzduz` exercised profile-core generation,
strict validation and report through the documented CLI forms in2.0228s,
0.3048s and0.1576s: three generated files, three strictly checked, no failures.
This run used parallelism2, supplementing the proof's parallelism4; no new
cross-parallelism equality claim is inferred from that command check. The
complete temporary command example used326,221 logical/352,256 allocated bytes.

This accepts only DX/MG R7.2/R7.3, not all R7. The corpus lock and accepted
migration proof still bind source232b9de; they are not relabeled as later-source
runs. Metadata and other family genericity, remaining migration slices,
reverse-dependency removal, independent conformance, viewer behavior, R8/R9,
packaging and terminal release qualification remain open.
