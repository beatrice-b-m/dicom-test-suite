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
| R7.1–R7.5 remaining core | Bounded smoke/core SC genericity accepted at f073d7e; next CR1, US1, PET1, XA/XRF2, VL2; then paired variants, multiframe and series/geometry, one closure at a time | For each: source freeze, embedded baseline, exact import/routing, loaded availability, migrated/repeat parity, then genericity; no simultaneous shared loader changes |
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

## R7 metadata admission checkpoint — 2026-09-05

Generator commit `ea4d151` admits the complete typed UTF-8 Person Name,
qualified EmptyType2 and PrivateCreators contracts independently of caller
case/recipe names. The SC monochrome template@1, native Explicit VR Little
Endian encoding, one-artifact topology, content and validation contracts remain
required. UTF-8 bytes must exactly match decoded PN and declared round trip;
Type2 attributes are a nonempty unique subset of the five qualified
Patient/General Study tag-keyword-VR tuples. Maximum high-bit input rejects
through checked arithmetic. Historical ISO2022, timezone, string-boundary and
sequence-length variants retain exact-name admission.

The implementation agent owned loader, bundle tests and exact routing/ownership
metadata. Root and a separate read-only reviewer inspected the change; review
added maximum-high-bit, artifact-count and legacy catalog-admission checks.
The final focused metadata test passed1/1 in2.88s after20.91s compilation.
Routing tests passed29/29 in3.009s and ownership1484 entries passed. All Cargo
and routing subprocesses used the explicit low-debug, nonincremental task target.

Root inspected then executed the loader/bundle route at this commit:
`python3 scripts/route-changed-tests.py --path src/recipes/loader.rs --path tests/corpus_definition_bundle.rs`.
The27 bundle tests passed in11.32s, four captured-input tests in5.47s,
92 corpus subsystem tests in29.77s (build21.37s), and22 corpus-plan tests
in0.81s (build1.72s). Unconditional Fast targets passed15/15 in2.17s and
73/73 in2.12s (combined build1.28s). The shared explicit target measured
857100 KiB (877670400 bytes) before Fast. Diff checks passed. No Heavy,
whole-profile generation, provider, package, release or public caller proof ran.
Public CLI/SDK metadata generation and historical-byte preservation are the
next sequential unit; this checkpoint alone does not accept metadata genericity.

Corpus documentation commit `96b4c0c` also synchronizes stale AGENTS.md live
content claims with accepted content0.5.0. A descriptor query confirms20 cases;
counts remain derived. This does not change any definition, pin or evidence.

Read-only follow-up audit confirms earlier smoke/core SC genericity still has
namespace admission debt. The [bounded SC audit](r7-sc-genericity-audit-2026-09-05.md)
adds its sequential closure before CR1; accepted historical parity is unchanged.

## R7 metadata3 genericity accepted — 2026-09-05

Generator `ea4d151` admission and public proof `ca0737f` establish the bounded
UTF-8 PN, qualified EmptyType2 and PrivateCreators capability. Root and an
independent reviewer found no remaining actionable defect after exact source
closure and fixed identity/payload assertions were added. The proof imports
product code only through the supported SDK and runs CLI commands from an
unrelated private directory with empty PATH. It separately proves complete
capabilities equality, raw manifest and actual payload equality, strict
validation and report2 projection. Static expected metadata includes full PN
raw bytes/component groups, all five empty attributes and all private blocks.

The fixture has8 documents,28914 bytes: descriptor SHA-256
`8e06f07cdc80da927d185607fffa9ab5b0b2cb972f6dd5ab559a1a12d5002be7`, framed
identity `7ab6abe3e65dc9c950fa56d4b0fbbdb931d9c33e75a9fe8cbfa25f0ab58d65f9`.
Root confirmed registry changes are only case/recipe IDs; recipe changes are
only identity, planning/projection order and output path. The three source
notes retain exact bytes and hashes. Historical IDs within these declared
notes are inert provenance, not planner dispatch. Two pre-existing Markdown
hard-break lines in copied notes trigger default staged blank-at-eol checking;
only that whitespace check was disabled for the exact-copy proof commit.
Implementation and current-document diff checks pass normally.

Caller planning order900–902 is name/empty/private; the canonical selector and
ledger order is empty/name/private. An initial test expectation conflated
these orders and failed before acceptance. The corrected proof adds no
semantic normalization. Frozen caller payloads, independently SHA-256 checked
from both CLI and SDK output, are:

| Output | Bytes | SHA-256 |
| --- | ---: | --- |
| independent/name.dcm | 962 | `16f828849a97a809fed56fad7d04c5c1b7fffea071809ea5ec8cbd8e848065aa` |
| independent/empty.dcm | 910 | `51df3b4077125281dde05747812a83574160b343a9da22974973339b9777731f` |
| independent/private.dcm | 1094 | `62acb19d45bc19ef63cf0574634165bbf1c21be65db660a386e1d8d16017a46a` |

The same test separately selects the three original IDs and reproduces their
978/932/1114-byte historical payloads. Root independently authenticated prior
receipt183141 bytes SHA-256
`199a18f9237038ee9021758e97b2c121f78a918c447983e0f9660b20f50ed386` and all
three retained payload hashes before reviewing the frozen oracle. This is a
bounded later-source regression, not a relabeling of the prior migration run.

The final exact public test passed1/1 in11.86s (build2.27s). Routing passed29/29
in3.054s; all internal list-only Cargo calls used the explicit target. Ownership
passed1485 total/922 integration entries. Target storage was858020 KiB
(878612480 bytes). No Heavy, whole-profile, provider or release check ran.

Current README, system specification, generation/SDK guides, compatibility
policy and changelog document the complete tuple and remaining boundaries.
Four documentation checks passed in0.00s (build0.04s). Exact documented CLI
forms generated3 files with profilecore/seed1/parallelism2, strictly checked3,
and emitted report2 at `/private/tmp/r7-metadata-docs-gj128soy`. All three
commands succeeded; the surrounding measurement script then expected an
envelope for the direct JSON report and failed. Read-only inspection confirmed
the actual report shape, full source manifest and all payload hashes without
rerunning generation. The command harness took2.4326s including the post-command
error; per-command timings were not retained. The temporary example contains
253694 logical/282624 allocated bytes. No cross-parallelism equality claim is
inferred from this supplemental example.

This accepts metadata3 R7.2/R7.3 only. Corpus pin232b9de remains unchanged;
SC genericity, remaining families/scopes, R8/R9 and terminal release evidence
remain open. A read-only lookup located clean viewer checkout
`/Users/beatrice/AgentFiles/projects/dcm-view` at43c2cadc2cef813a15a9621e1fc8ad7b56ca0784.
Its current compatibility campaigns are opt-in and reference the historical
checkout model. No viewer edit, build, execution or R8 acceptance occurred.

### Metadata fixture spelling-inventory correction — 2026-09-05

The next unconditional Fast run caught four unlisted path/token records in the
metadata proof: preserved private-creator identifiers in the new recipe and
static oracle. The payload proof remained valid, but its post-fixture spelling
inventory was incomplete. Root registered exactly those four occurrences
(six matches) as `dicom_payload_identifier`, with an explicit byte-preservation
reason, without changing payloads, renaming tokens or weakening the checker.
Existing records retain their order and meanings. The resulting snapshot is
1015 matches, SHA-256
`5fe2af89ae2679bf0f068610a120c71825614eb0a9a8e5030fb121b4025fd649`.

The initial Fast run was14 passed/1 failed in2.16s and stopped before the second
target. After correction, spelling audit passed and its four synthetic tests
passed in0.000s. The inventory route was inspected; it selects unconditional
Fast only. Both Fast targets then passed15/15 in2.19s and73/73 in2.09s,
build0.04s, under the same explicit task target. Diff checks passed. No native
proof was replayed. This closes the inventory omission in metadata acceptance.

## R7 SC admission checkpoint — 2026-09-05

Generator `29ac276` admits a complete bounded native single-frame SC tuple
independently of caller names. It covers the13 migrated smoke/core shapes:
monochrome8/16-bit signed or unsigned as qualified, palette8, RGB planar0/1,
YBR_FULL planar0 and even-width YBR_FULL_422. Matching monochrome/RGB template@1,
content, encoding, pixel/color topology, both-level pixel/specialized rules and
projection remain required. Bit packing, integer-word, encapsulation metadata,
unrelated overrides and other topologies are excluded. High-bit arithmetic is
checked. Partial nonhistorical caller tuples reject during shared validation;
legacy namespace and EOT fallback remain broader and are explicitly outside
this bounded genericity claim.

The implementation agent owned loader/bundle tests and exact ownership/routing.
Root and an independent read-only reviewer found no actionable defect after
review additions. The focused28-test bundle suite passed in11.88s after19.96s
compilation; routing29/29 passed in3.034s, ownership1486 entries passed, and
staged diff/log checks passed. No generation occurred in that unit.

Root inspected and executed the loader/bundle route:28 bundle tests passed
in11.77s, four captured-input tests in5.41s,92 corpus subsystem tests in29.35s
(build20.81s), and22 corpus-plan tests in0.83s (build1.71s). The first Fast run
exposed the preceding metadata inventory omission, corrected separately in
`cd5443c`; final unconditional Fast passed15/15 and73/73 as recorded above.
The shared low-debug, nonincremental target measured855280 KiB
(875806720 bytes) before Fast. No Heavy, provider, whole-profile or release
execution ran. A separate standalone13-case CLI/SDK proof is next; admission
alone does not accept SC genericity or close R7.

Root authenticated existing core parity receipt99483 bytes SHA-256
`d51a36d70b20d37caf2f42711feb3a259fe2a781c525f577d05a7cb85b6ada85` and
all retained smoke3 payloads (2790 bytes) and core10 payloads (9512 bytes).
Their separate historical selections and identities remain unchanged; the
future caller proof must not infer combined-manifest parity from those arrays.

## R7 bounded native SC genericity accepted — 2026-09-05

Admission `29ac276` and public proof `f073d7e` establish caller-defined native
single-frame SC for the13 migrated pixel/layout shapes. Root and independent
review found no remaining actionable defect after fixing the complete semantic
oracle digest. Historical namespace/EOT fallback remains broader and is not
newly generalized. Recipe/template versions and original payloads are unchanged.

The standalone bundle has15 documents,70145 bytes, descriptor SHA-256
`5f2c0ecb9ea828598d84a079d5f11cbd154148d8fd7bfae3bf14f7e086b78f63` and
framed identity `32726bd36c65de1aee646aa694c1451261e5137e7b88e14889ea026283009f28`.
It intentionally places all13 caller cases in core; original historical smoke3
and core10 selections remain separate. Exact closure, independent identities,
paths and planning/projection orders are checked. Product imports are SDK-only;
CLI runs from an unrelated private directory with empty PATH.

The targeted [semantic oracle](../tests/fixtures/generic-sc-semantics.json)
is SHA-256 `f3cb9b1ba11c0d363a43254463883d74093e727caf94b03ed761af74912f597d`.
It fixes all caller sizes/hashes (12348 payload bytes), historical sizes/hashes,
image/pixel-data, recipe parameters and semantic projections by mapped case.
Root independently authenticated all historical fields against the retained
accepted arrays; the test verifies the complete oracle hash without a runtime
sibling/evidence dependency. No ordinary run manifest was copied into the fixture.

CLI/SDK complete capabilities, raw manifests, actual payload bytes and report2
projections agree. Independent SHA-256 checks open every CLI and SDK payload;
separate strict validation checks13 files. The same test reproduces original
smoke3 (2790 bytes) and core10 (9512 bytes) through separate exact selections.
This is neither combined historical-manifest parity nor independent conformance.

The final digest-bound test passed1/1 in15.54s (build2.19s). After selective
staging, unconditional Fast passed15/15 in2.11s and73/73 in2.00s (build0.04s),
spelling audit passed1015 matches in1.29s, ownership1487 passed in0.23s, and
routing29/29 passed in3.04s. Staged diff checks passed. All Cargo subprocesses
used the explicit low-debug, nonincremental task target, measured855044 KiB
(875565056 bytes). The pre-existing default target11519240 KiB was untouched;
no incremental-cost claim is made for it. No generated payload was tracked.

Root updated current README, system specification, generation/SDK guides,
compatibility policy and changelog. Four documentation checks passed in0.00s
(build0.04s). A fresh copied bundle at `/private/tmp/r7-sc-docs-ssbn8b51`
generated13 files with profilecore/seed1/parallelism2. All13 actual payload
hashes equal the frozen parallelism4 oracle, a bounded payload comparison only.
The command harness accidentally added unsupported `--cli-api` to `validate`;
that request rejected syntax after successful generation. The failed request
is retained. Correct documented validation then passed13 files in0.3018s, and
report with its supported envelope flag passed in0.1593s without regeneration.
The first harness took2.1276s including the syntax failure; isolated generation
timing was not retained. The example occupies624647 logical/704512 allocated
bytes. No full-manifest cross-parallelism or resource-scaling claim is inferred.

This accepts only bounded SC R7.2/R7.3. Remaining classic families, specialized
scopes, embedded removal, R8/R9 and terminal release evidence remain open.
In parallel, disjoint source-only CR provenance was committed in corpus
`459f15e` and independently accepted at `3e5256b`: exact34-row source core
snapshot and11 source objects authenticated against232b9de. It caused no live
import or runtime execution. CR baseline preparation is the next sequential unit.

## CR1 embedded baseline accepted — 2026-09-05

After the bounded SC gate, corpus helper `8c4c0fa`, wording correction `0241bfb`
and reviewed readiness `2f40eab` preceded one exact CR1/core/seed1/parallelism4
capture. Corpus acceptance `378b935` binds the full record. Receipt is49293
bytes, SHA-256 `1e8f489341d60babbdebd859a1bf3bd36c4937e9c0daeba70295e67fbd940350`,
at corpus `artifacts/r7-native-cr-baseline1-20260905/receipt.json`.
The single1306-byte Part10 hash is
`c76c07478ba42de9093918bc50ed53c6a99eaabee571187ba4d8e4a4c7bdd075`.
Generation/strict-validation/report passed in1.303777/0.231281/0.931742s,
whole job6.083096s. All33 unselected source rows remain explicit; native pixels,
overlay, both LUTs and44 internal/4 standards findings matched frozen source.
Root authenticated all22 retained files; a separate offline replay passed0.18s.
Evidence excluding receipt uses141219013 logical/141271040 allocated bytes.

This uses original source232b9de generator plus the separately pinned report-only
artifact, with no build, retry, viewer, independent conformance or release claim.
CR template limitation text conflicts with the actual U8/OB recipe; source bytes
remain preserved as explained in `r7-cr-genericity-audit-2026-09-05.md`.
Static CR content0.6.0 import is assigned to the corpus agent (recipe, registry,
descriptor and static tests only). Verifier/CI compatibility integration follows
sequentially, then public-loader availability, migrated parity and genericity.
R7 remains incomplete and R8/R9 have not begun. Documentation diff check passed.

## CR1 import, selected availability and migrated parity accepted — 2026-09-05

Corpus data `dc08491`, static tests `fd0ac5d` and CI `e7f798f` establish the exact
source-preserving content0.6 CR1 addition. Root authenticated prior entries and
raw CR recipe. Descriptor12073 bytes SHA
`2248f04046f388bf3914cfc473adad0969e337d0b83670b9c918ec23536d8c80`, registry57239
bytes SHA `f597311d2fa942de6b633a9b4d78c25fb0e6100bbf397e257084c4de30b413c2`;
derived snapshot21 cases/26 files/144330 bytes. Historical availability test
adaptation `f1e02e7` preserves exact old identities. See corpus590f41f for focused
static/routing/runner tests and rejected mutation coverage.

Reviewed loader helpers43b7f04/cd84762/07ebf52 preceded one exact loading-only
run from11fcd95. Accepted corpusda1a8b0 binds receipt104700 bytes SHA
`669f6796f5e9a2fd25ff0a89eb1b4a02ee994c8183defa4f5c041872a8760ecf`, framed
bundle `56838de5e7dea97bb1816193baad72f973758e58631065835df57cacadab689b`.
CR1 was ready at core/seed1/parallelism4, publication/validation not_run.
Capabilities1.892188s, whole3.903346s;37 files72255090 logical/72314880 allocated
bytes. Independent retained replay0.082645s passed. Full ordinary425 tests
passed7.188s after exact struct-import and disjoint US path-routing fixes;
the earlier failed synthetic runs remain recorded in the corpus status.

Reviewed parity07aafe7 and readiness9f7b4e0 then performed exactly two CR1
public-runner calls. Corpus acceptancee0dd96e binds receipt257702 bytes SHA
`98b514fe0a15a4eeee7ba0a1240d4d89dea75d019b5dd35a4fbb3f0caaa75d24` at
`artifacts/r7-native-cr-parity1-20260905`. Calls3.913546/3.477539s, whole10.244171s.
Both1306-byte payloads reproduce embedded SHA
`c76c07478ba42de9093918bc50ed53c6a99eaabee571187ba4d8e4a4c7bdd075`;77421-byte
manifests match SHA `c28ea96884d4b14b94d5c37a36da630811a5617d92e79020fc14c54f6e24852e`,
86868-byte reports match SHA
`77d04cc3c756d304bee06a81c154fa03357abc78a2442cd7d9f3557f2b82a76f`.
Full file projections preserve semantics, UIDs, standards and validation;
33 old skips and the narrow manifest1-to2 boundary remain explicit. Root and
independent reviewer authenticated203 retained files; offline comparisons and
repeat equality passed0.722538s. Evidence excluding receipt occupies358089039
logical/358481920 allocated bytes, with a duplicate private runtime retained at
`/private/tmp/r7-cr-parity-859d0i7p`. No capture rerun followed an initial reviewer
script mistake about null-returning finalizers.

No generator build, pin update, viewer, independent conformance, Heavy or
release qualification occurred. Source-only US provenance518f9c7 was prepared
in disjoint files, independently authenticated13 objects/full34-core context
in0.178s and accepted corpus13fc1ae; no US import/runtime is implied.
CR genericity now proceeds sequentially: planner/pure tests, loader/dispatch,
then caller CLI/SDK proof. The CR template wording discrepancy remains a
separate policy review. R7 is incomplete; R8/R9 have not begun. Diff check passed.

CR template policy follow-up (2026-09-05): the apparent pixel-depth discrepancy
is resolved by scope. Qualified composition defaults are unsigned12-in16;
curated CR uses its separately qualified U8/OB recipe. The CR audit now clarifies
both routes. Catalog/template version and bytes remain unchanged, avoiding an
unnecessary template-domain identity change. This is documentation clarification
under compatibility policy, not a new template or generation qualification.

## CR bounded planner accepted — 2026-09-05

Generator `edab9b8` adds `inspect_cr_capability` before historical MR/CR
recipe dispatch. Assigned ownership was limited to the planner, existing pure
planner tests and their ownership digest. Root and independent reviewer accepted
the final complete native tuple, checked U8 dimensions/count/range/min/max/hash,
overlay geometry/packing/unused bits and bounded four-entry16-bit LUT structure.
The inspector rejects crossed/mixed contracts, unrelated projections and native
fragment counts. Caller identity, explicit path and planning/projection order
are independent of historical names; the exact named CR-RLE route and pure MR
remain on their historical validation path. No broader codec capability follows.

An initial compile identified optional policy fields; corrected checks use
optional string access. Review identified the missing fragment-count exclusion,
which was added with mutation coverage before commit. Final focused pure suite
passed6/6 in0.02s after18.40s compile; ownership1487 passed0.27s and diff checks
passed. All Cargo/routing invocations used the explicit low-debug, nonincremental
`/private/tmp/dts-dx-mg-genericity-target`. No generation or broad qualification
ran; no claim is made from intermediate compile runs. Earlier owned checks and
review remained within the same bounded planner task.

Loader/shared validation and public caller proof remain pending. Their next
owner is assigned only loader, shared dispatch and relevant bundle/shared tests
plus exact ownership/routing records, after the planner's accepted commit.
MR-prefix validation must defer to the inspected CR tuple. Catalog/template
bytes stay unchanged under the documented composition/curated scope distinction.

## CR loader and shared validation accepted — 2026-09-05

Integration4c6e973 adds typed CR inspection to shared recipe validation and
native registry admission. The MR-prefix branch now yields to an inspected CR
capability. Actual SDK generation/reopened validation covers both arbitrary
caller names and misleading MR-style names, with explicit independent output.
Missing/crossed declarations reject. The earlier CT negative control used CR;
review changed it to still-unqualified US so new CR support does not invert the
control. Historical namespace fallback and MR/RLE behavior remain separately
bounded. Assigned ownership covered loader, shared execution, bundle tests and
exact ownership/routing records; an independent reviewer accepted the commit.

Focused caller test1/1 passed2.11s, corrected CT control1/1 passed0.29s and
exact routing-list check1/1 passed1.559s. Ownership is1488 total/923 integration;
bundle routing explicitly lists29 tests. Root inspected the overlapping route
before executing it:29 bundle tests passed12.04s,4 captured-planning tests
passed5.30s,92 corpus-generation tests passed28.10s after19.61s compile, and22
corpus-plan tests passed0.79s after1.66s compile. Unconditional Fast passed15/15
in2.15s and73/73 in2.01s after1.20s compile. Diff checks passed. All commands used
the explicit low-debug, nonincremental task target; Heavy, codec/provider and
release evidence remained deferred per route.

This accepts internal admission/dispatch only. The next separate unit is a
closed caller CR fixture plus public CLI/SDK proof with actual payload hashes,
full manifest/report equality and historical CR payload preservation. No corpus
pin changes, template identity migration or terminal R7 acceptance is implied.

## CR standalone proof accepted for tested names; dispatch gap open — 2026-09-05

Public proof6e4a4a1 adds a closed three-file caller fixture, targeted semantic
oracle, SDK-only support and one separately selectable CLI/SDK test. The fixture
has10055 bytes; descriptor SHA
`7dcc1359ac74edbb40357e31453239c7517b5e1c33b4d9e85ff4b0bbd8219264`, framed identity
`c101481618e81ad2bdd273d5821313f5c630e5b852081f8bb5a7ed162872aa48`.
Case `classic/mr/caller-radiography`, recipe `caller_radiography`, planning900,
projection901 and `independent/radiography.dcm` exercise caller identity and the
MR-prefix validation correction. Typed CR parameters remain source-exact.

Oracle SHA `5ebae3fbf82ccacf52a1b782cdef53b5f046e8eff53d8a513ad64ac3349f1be6`
is independently hashed and pinned in support. Root authenticated its image,
pixel, recipe and semantic fields plus actual original1306-byte payload against
the accepted baseline. Independent review authenticated source registry/recipe
transformations. Observed caller1300-byte SHA
`cd4c473583688813b83d63581218e278fef4671ffd6287114711c97c7994e9b6` is frozen;
original SHA `c76c07478ba42de9093918bc50ed53c6a99eaabee571187ba4d8e4a4c7bdd075`
is reproduced separately. Actual CLI/SDK payload hashes, full capabilities/raw
manifests/report projections and strict validation agree; no runtime sibling
or ignored-artifact dependency and no ordinary manifest is committed.

First proof1/1 passed10.48s after2.19s compile; final frozen proof1/1 passed10.64s
after2.20s compile. Fast15/73 passed2.11/1.97s, routing29 passed3.045s,
ownership1489 total/924 integration and staged spelling1015 passed. Diff checks
passed. Explicit low-debug target measured859128 KiB; no generated payload
was tracked. This is bounded same-project public evidence for its tested names.

The disjoint US read-only audit exposed a further CR dispatcher gap:
`classic_requests` continues into historical nuclear/VL matchers after CR.
A qualified CR caller using an existing PET/VL case name can be rejected or
multiply claimed. Full CR R7.2/R7.3 acceptance remains open. A separate owner
will add typed-CR short-circuiting before historical matchers and regressions
for misleading PET/VL names, preserving MR/RLE behavior. Existing proof remains
valid for its tested scope; it is not relabeled as universal name independence.

## CR genericity and current guidance accepted — 2026-09-05

Dispatcher correction `ba97c38` gives an inspected CR tuple precedence over
historical nuclear/VL names; its regression covers PET/VL/MR collisions and
crossed algorithms. Projection guard `689dbc1` rejects MR/ICC fields, appended
standards evidence, implementation-version overrides and profile overrides,
while requiring the source CR semantic labels. Root and independent review
accepted both corrections. The exact historical RLE path remains separate.
Current documentation `5754046` explains these bounds and the distinct
composition defaults without changing catalog bytes or template identity.

Final affected routing after `689dbc1` passed29 bundle tests in12.05s,92
generation tests in27.88s and22 planning tests in0.81s; compiles19.07/2.49/1.63s,
whole65.8542s. Log: `/var/folders/b9/w6_2zsyn4t1c8q1b3dyp8jbr0000gn/T/r7-cr-final-route-txunosd7.log`.
The exact public caller CR CLI/SDK proof passed1/1 in10.43s after2.18s compile.
Documentation routing was inspected; unconditional Fast passed15/15 in2.15s
and73/73 in1.95s after1.15s compile. All Cargo/routing commands inherited the
explicit low-debug, nonincremental task target. Diff checks passed.

The documented generate/validate/report workflow was exercised from fresh
`/private/tmp/r7-cr-docs-sirie3ha`, with the standalone CR bundle and its core
profile, seed1 and parallelism2. Commands succeeded in1.088045/0.289319/0.152172s.
Generation published; reopened validation checked one valid file with no
failures; report2 retained the selected CR case. Actual1300-byte payload SHA
`cd4c473583688813b83d63581218e278fef4671ffd6287114711c97c7994e9b6` matched the
public oracle. The11 files before the exercise receipt occupied190196 logical
and208896 allocated bytes. This checks documented syntax and bounded payload
stability, not full-manifest equality across parallelism settings.

Together with the separately accepted original baseline, content0.6 import,
public availability and migrated parity, this closes the bounded native CR
R7.1/R7.2/R7.3/R7.5 slice. Same-project evidence remains distinct from viewer,
codec/provider, independent and release qualifications. Neither pinned native
artifact is relabeled with this source capability. R7 remains incomplete;
R8/R9 have not begun. US baseline is the next sequential slice. Corpus ordinary
verification at `bd41ca8` passed457 tests in8.472s without native generation.

## US embedded baseline checkpoint — 2026-09-05

Corpus readiness `31c69f7` and acceptance `8804666` bind one native US1
embedded capture at `artifacts/r7-native-us-baseline1-20260905` in the corpus
repository. Receipt SHA
`8089876f1bd1d050396ac54466d3fc5aa06cecdf804633ea32fdca99b85150a9`;
payload1006 bytes SHA
`e616b8c983c59640a62fa081f636acea554ce681edde3fc8dd53a3c15098c30b`.
One generate/strict/report sequence passed in1.284419/0.227185/0.919014s,
whole6.000055s.29 internal and4 standards findings,33 skipped rows and full34
coverage rows remain distinct. Independent read-only replay passed0.212552s;
22 nonreceipt files occupy141208106 logical/141258752 allocated bytes.
No retry, build, viewer or independent conformance execution occurred.

US static import, loader availability, migrated parity and generic source
capability remain sequential. The task target now measures859204 KiB; the
pre-existing default build tree remains untouched. Current source CR acceptance
does not change either native artifact pin. R7 remains incomplete.

## US static corpus integration checkpoint — 2026-09-05

Corpus data9d28967/static631b793/CI6e1d9d9/historical-fixture4556b71 are
accepted at712da80. Content0.7 closes27 files151260 bytes,22 cases (smoke3/core19).
Descriptor SHA `f361acab478f173478e2c395fb5f5600fa7f79faa59841a3402c09feed0dc680`;
registry SHA `ec263585005a23ba69b18652c32dc0a480c7017f65f0e484f99571600efd6a04`.
Exact US provenance and all previous members/rows are preserved; historical
fixtures reconstruct byte-exact0.6/0.5 without relaxing production verifiers.

Static17 tests passed0.131s (independent0.137s); routing50 passed0.831s
(independent0.820s); historical53 passed2.836s, independent Git reconstruction
0.348891s. Root full ordinary461 passed8.670s; actual base/head CI dry-run
0.332155s selected only smoke3+US1/all. No native execution during integration.
Selected US availability helper preparation is next; no migrated parity or
generic source capability follows from static ownership. R7 remains incomplete.

## US selected availability checkpoint — 2026-09-05

Corpus checker8ccf2f6/captured8c6a7e/fix2fee1a0 and routingf0f4279 passed
498 ordinary tests in9.604s; native readinessa539a38 preceded one exact US
capabilities call. Accepted at9fa7397, receipt SHA
`f1ab63246bc9b63dffb01e8fe1f1fd0c894492a972701d6e5a7e820205084b9a`
binds content0.7 identity
`130eabbb6a8968fb4a4d5f54a031d6b2b6939bd3a2902d57bcd3e2dc5e73b01f`.
US1/core/seed1/parallelism4 is ready with baseline plan preserved and
publication/validation not_run. Capability1.880125s, whole3.848469s;38
nonreceipt files72698934 logical/72765440 allocated bytes. Independent retained
review passed0.107405s including complete source archive and checker replay.
No generated output, reporter, retry or build. Bounded parity preparation is
next; generic US source capability remains unopened until parity acceptance.

Disjoint XA/XRF source provenanceab40c0c is independently accepted ata02dcfd
in the corpus repository:17 immutable objects, full34 rows, two recipes/notes
and XA inline standards evidence; six tests0.048s. No data import or native
XA/XRF evidence follows. Source-only helper preparation may use disjoint files
while US parity integration proceeds; runtime boundaries remain sequential.

## US migrated parity checkpoint — 2026-09-05

Corpus acceptanceb41443d closes the bounded US1 parity gate after helper5702a46,
routing80e3f33 and readinessd6d56b3. Receipt
`fa96555c2d6c090034c806e7a89f5eea426bf24f8d9344e261fbe72fdc9c7571`
under corpus `artifacts/r7-native-us-parity1-20260905` binds two exact supported
runner calls US1/core/seed1/parallelism4. Both actual1006-byte payloads reproduce
`e616b8c983c59640a62fa081f636acea554ce681edde3fc8dd53a3c15098c30b`;
manifest71276 bytes SHA
`f873771842f28fde702ca5d1fb918f83c7a2394d9b9939d3145174f2c1e377e9`,
report80205 bytes SHA
`2c3ab1b4c38567a764869c461316dea5e31ad98af915fe8ab536b9faa074bb74`,
identical across both runs. Full baseline file/standards/semantics and33 skips
were preserved across only the explicit manifest identity/selection migration.

Calls3.879497/3.422479s, whole10.038937s;215 nonreceipt files359326555 logical/
359755776 allocated bytes. Independent216-file/source/runtime/finalizer and
comparison replay passed0.802069s. Full ordinary512 passed9.929s beforehand.
Original private runtime `/private/tmp/r7-us-parity-0gvj4oig` remains retained.
No retry, build or broader qualification occurred. US genericity now proceeds
planner, loader/shared dispatch, then public caller proof in separate units;
the original corpus artifact pin is unchanged and R7 remains incomplete.

## Disjoint XA/XRF helper preparation checkpoint — 2026-09-05

Corpus helper6eaef7c and exact ordinary routing8114ff9 are accepted at6960be9.
Source-static two-case/32-skip guards preserve66XA/120XRF internal findings,
four standards findings each, nested equipment geometry and explicit report
nulls/presence. Raw absence checks remain bounded byte markers for frozen
payloads, not a general parser. Owner18 tests0.884s/provenance6 tests0.049s;
independent18 tests0.888s and source validation/report review passed. Routing
and import60 tests1.037s; root full ordinary531 tests11.024s. No native capture,
import or build occurred. This preparation used disjoint corpus files while
the sequential US generator planner was implemented.

## US bounded planner accepted — 2026-09-05

Planner71c11d1 owns classic_nuclear, its existing pure tests and exact ownership/
spelling records. Typed native US inspection requires the complete template/
algorithm/provider/projection/encoding contract, safe caller path and orders,
checked single-frame U8 dimensions/range/extrema/hash, source semantics and
synthetic provider metadata. Independent review identified arbitrary patient/
acquisition metadata as outside the bounded qualification; exact17 source
provider values and absent optional fields now reject overrides with mutations.
Caller case/recipe identity, path and orders remain independent. Exact historical
RLE exemption and nuclear/PET/multiframe routes remain separate.

Final focused pure2 tests passed0.33s after18.51s compile; ownership1489 passed
0.53s, spelling1016 passed1.24s and diff checks passed. Spelling changes only
the retained manufacturer occurrence1-to2 and derived count/class/hash,
`34d65c5a569866138c36e47eb5b6b44235adc8e6e1b9cbcc578d9531960dcb24`;
this preserves source payload metadata, not an old public product name. All
Cargo/routing commands used the explicit low-debug nonincremental task target.
No native generation or loader/shared change occurred in this planner unit.

Commit discipline correction: root status commitaf0a52b unintentionally included
three planner files already staged by the owner in the shared index. Corrective
d602c7b reversed only those files without rewriting history, then restored their
exact saved contents. Subsequent provider review/fix and the complete planner
were committed independently at71c11d1 with explicit path-limited --only. No
work was discarded; future commits use explicit owned path limits as well as
selective staging. The original mixed commit remains visible, not amended.

Next is sequential loader admission, typed shared planning precedence and actual
SDK bundle validation. US public caller proof and final guidance remain pending;
R7 is incomplete and the corpus pin remains unchanged.

## US loader accepted; external schema coupling remains open — 2026-09-05

Loader108a2cc adds full typed US admission and returns its shared plan before
historical family matchers. Actual SDK generation/reopened validation covers
four independent or misleading namespace inputs; exact historical PET/VL IDs
and MR recipe names have pure planning precedence coverage. CT's name-only
negative control now uses still-unqualified NM. Independent review accepted
this bounded scope, not arbitrary historical-ID publication.

Focused generation1 test passed4.03s, exact-name planning1 in1.17s, CT control1
in0.26s and exact30-entry routing1 in0.073s; ownership1490 and diff checks passed.
Root inspected and executed the overlapping route:30 bundle tests12.75s,
4 captured-plan tests5.23s,92 generation tests27.43s after19.02s compile and
22 plan tests0.81s after1.63s compile, whole68.291674s. Log:
`/var/folders/b9/w6_2zsyn4t1c8q1b3dyp8jbr0000gn/T/r7-us-loader-route-px1grkr5.log`.
Explicit low-debug/nonincremental task target was used throughout; broad Heavy
and release evidence remained deferred.

An initial exact PET-ID US publication test failed after1.89s with
validation.manifest.invalid: legacy schema expected PET Image. Planning was
correct. External manifest2 still references legacy file conditionals, including
36 historical case-ID policies and one generic validity branch. The schema
coupling is a separate sequential public compatibility unit, not a waived
genericity gate. No schema changed in108a2cc; no full name-independent
publication or final US genericity claim is made yet.

## External manifest structure accepted; runtime scope open — 2026-09-05

Schema a1d1e12 implements decision abab694. Independent structural review
confirmed the external file definition preserves every generic property,
required field, evidence reference and validity branch, removing only the 36
historical case-ID conditionals. Legacy and v1 schemas remain byte-identical.
Seven manifest tests passed in 2.43s after 18.22s compile; SDK exact PET-ID and
VL-namespace publication, reopened validation and reporting passed in 4.64s
after 18.39s compile. Ownership 1490, spelling 1065, routing preview and diff
checks passed. Broader overlapping verification follows the immediate runtime
correction to avoid rebuilding the same compatibility surface twice.

Manifest v2 is now 18266 bytes, SHA-256
`7818bdc5324bd7fa49dbeadfcee9293ec5a74c7e820b6ea7cb672c13a6238dfc`.
The unchanged 81-member resource inventory totals 1518044 bytes, identity
`21711587ab6cc02e8f5dec11dfbd221e6f52a4bffaf798d3efecea612a380811`,
independently recomputed from source member paths, sizes and hashes. Initial
tests stopped at the old frozen resource oracle before validation; only the
current source oracle was refreshed. Pinned native artifacts are unchanged.

An exact ICC VL case-ID US publication passed schema validation but reopened
validation failed after 3.15s: the historical ICC validator requires ICC evidence
based on case name. This is an explicit remaining runtime boundary, not an
accepted full genericity claim. A read-only inventory of reachable validation
and reporting dispatch precedes its sequential fix; declared evidence and
legacy requirements must remain checked. US public caller proof, R7 completion
and R8/R9 remain pending.

## External schema subsystem verification checkpoint — 2026-09-05

The inspected schema/resource route passed bundle30 tests12.93s, captured
runner6 tests4.68s and captured report3 tests3.80s, then stopped at three stale
snapshot byte assertions (4 passed,3 failed,2.05s). Actual261-member snapshot
2931302 bytes exceeds the old2918678 by exactly12624 schema bytes. Root and
owner independently enumerated build.rs members to confirm the total. Follow-up
3ced103 updates only three assertions, the printed measurement and exact test
ownership digest. Snapshot7 tests passed2.06s after18.35s compile; ownership
1490 passed0.235s and diff checks passed. Root reviewed the complete diff.

Only the unexecuted remainder of the inspected route was then run: manifest7
tests2.40s, report5 tests5.28s, assembly25 tests3.07s (17.39s compile), report
CLI50 tests18.54s (2.21s compile), SDK corpus8 tests34.48s, SDK facade12 tests
3.34s and schema/resources86 tests6.85s (0.78s compile). All passed, whole
remainder96.146172s. Logs are /private/tmp/r7-external-schema-route-20260905.log
and /private/tmp/r7-external-schema-route-remainder-20260905.log. Task target
measured858308KiB after the first route and914952KiB after the remainder.
Explicit nonincremental low-debug target was used; default target untouched.
Unconditional Fast coverage remains due with the ensuing runtime integration.

Decision19df3c3 orders three runtime correction units after the independent
audit: scalar/metadata context, reference/VL/WSI/STL context, then qualification
isolation. Unit1 owner may now edit; runtime reporting requires no change.
This completes affected schema subsystem verification, not US genericity or
release evidence. Retained corpus executable and parity identities are unchanged.

## External runtime unit1 accepted — 2026-09-05

Commit844eb7c forwards manifest contract context through file/pixel and metadata
validation. External U32/U1/ICC/nonsquare evidence activates by declaration;
legacy names retain required-evidence and wrong-scope checks. A precheck rejects
declared scalar evidence outside native integer image layouts, including absent,
float and encapsulated forms. Independent review found this potential bypass
during implementation; it was corrected before commit. Explicit null evidence
does not disappear. Metadata temporal/sequence groups remain complete within
each caller case, rejecting cross-case completion and extra absent members.

Eight additional historical scalar/metadata identities pass actual US SDK
publication, reopened strict validation and report source-evidence absence
assertions. Renamed declared scalar evidence and tampered/null/empty contracts
retain validation; legacy failures and nonnative nonsquare are covered. Final
focused17 tests passed13.38s after0.11s warm compile; compile/list18.62s. Earlier
9-test filter passed0.01s after18.18s compile; an initial missing test import
compile failure was corrected before these results. Ownership1490 and diff
checks passed; only existing entry digests and accurate metadata test naming
changed. Independent reviewer and root accepted the complete owned diff.

Root inspected the broad src/lib.rs routing preview at
/private/tmp/r7-runtime-unit1-route-preview-20260905.json. Its all-ordinary
overlap will run once after the three sequential runtime units; immediate
proportional checks passed corpus-generation92 tests27.80s and Fast15 tests
1.52s plus73 tests2.05s, compile18.90s. Log:
/private/tmp/r7-runtime-unit1-subsystem-fast-20260905.log. Explicit task target
measured916124KiB; default target untouched. No Heavy or release gate ran.

Unit2 may now proceed with evidence-based reference/VL/WSI/STL and deeper WSI/
tile-SEG/SR dispatch, as refined ind00e95a. Unit3 qualification isolation and
full US public caller proof remain pending. No full arbitrary-name genericity
or completed R7 claim follows from this bounded acceptance.

## External runtime unit2 accepted with projection follow-up — 2026-09-05

Commit8afdcc3 carries contract context through reference, VL/WSI/STL and SR
validation, including deeper WSI and tile-SEG/SR dispatch. External source IDs
and paths resolve against authenticated source objects; declared family evidence
on incompatible IODs and missing required evidence fail. Typed selectors avoid
using historical names as external dispatch intermediates. Pyramid groups bind
all files of one caller case; independent groups remain separate. Legacy behavior
and generation-only RT validators are retained. Review found and corrected
extra-member omission and required RT evidence gaps before acceptance.

Fourteen focused entries passed: six contract tests0.04s after18.46s compile,
five reference closure tests0.00s, sparse payload1 test0.01s, optical-path1 test
0.00s, and expanded US SDK1 test26.51s after18.09s compile. Actual sparse WSI
reopening rejects geometry tampering; shared RT dispatch rejects changed source
case/hash/UID. Ownership1490, spelling1065, routing preview and diff passed.
Independent final review and root accepted the complete five-file commit.

The first expanded US SDK run failed6.20s before publication for exact caller
vl/wsi/pyramid_multiresolution: curated manifest group projection requires three
members based on historical name. Only that one SDK collision was removed
from unit2 pending immediate unit2b; reopened pyramid/group checks remain.
Decision525a1e6 requires restoring it with typed postprojection activation before
US genericity acceptance. This is an open gate, not waived evidence.

Root inspected77-command broad overlap at
/private/tmp/r7-runtime-unit2-route-preview-20260905.json; whole overlap remains
due after this sequential boundary. Proportional corpus-generation92 tests
27.63s, Fast15 tests1.54s plus73 tests2.05s and standards-validation1 test0.64s
passed after19.30s compile. Log:
/private/tmp/r7-runtime-unit2-subsystem-fast-20260905.log. Task target954512KiB.
The normal lib build exposed two now-test-only wrapper warnings. Follow-up
f675d8d adds only cfg(test) to those wrappers; root verified all callsites are
tests and reviewed both lines. Cargo check passed warning-free13.50s, ownership
1490 passed0.259s and diff passed. No redundant test rebuild was needed.

Unit2b may proceed now; unit3 also owns the audited stress postprojection name
trigger. No schema/native pin change, broad Heavy run, release or complete R7
claim occurred. Exact US public caller proof remains after these corrections.

## Typed pyramid postprojection unit2b accepted — 2026-09-05

Commitc445839 replaces historical-name activation with captured recipe and
resolved-plan pyramid intent, requiring the complete typed contract and all
three members of each caller case. Malformed or crossed intent fails; separate
groups cannot combine. Qualification artifacts are filtered consistently with
the main file projector. The historical role projection body remains unchanged.
The exact pyramid-ID US SDK regression is restored and passes publication,
reopened validation and reporting. Pure tests cover full historical projection
equality, independent groups, crossed/partial contracts, aligned extra members,
qualification interleaving and ordinary US with no pyramid intent. No WSI
payload was generated by the new synthetic projection test.

Final focused2 tests passed27.87s after18.63s compile; initial pure1 test0.34s
after18.40s compile. An initial template accessor compile error was corrected.
A broad corpus-bundle routing insertion caused10 failures among29 synthetic
tests in9.522s; the final dedicated exact-source route preserves existing corpus
coverage without changing unrelated selections. Final routing30 passed3.034s;
ownership1491 entries/275 groups passed0.292s, diff passed. Independent review
accepted source/test invariants; root reviewed final exact routing and ownership
changes. The temporary inventory reorder was corrected before commit.

Root inspected /private/tmp/r7-runtime-unit2b-route-preview-20260905.json and
ran its complete affected route plus Fast. Bundle30 tests32.23s, pure1 test
0.30s, corpus-generation92 tests27.72s after19.45s compile, plan22 tests0.79s
after1.62s compile, Fast15 tests2.15s plus73 tests1.97s after1.19s compile all
passed. Whole89.362902s; log:
/private/tmp/r7-runtime-unit2b-route-fast-20260905.log. Explicit task target
measured1042284KiB; default target untouched.

Unit3 qualification activation may now proceed, including stress postprojection
and captured-profile binding. Full overlapping ordinary verification and US
standalone public caller proof remain due. No pinned artifact, schema, WSI
qualification or broader generic WSI generation claim changed.

## Reduced WSI shared-template correction — 2026-09-05

Before unit3 writes, read-only preparation identified that reduced-stress WSI
shares the ordinary pyramid volume template. A source plan-only regression
confirmed c445839 incorrectly claimed that three-volume chain: one test failed
0.63s after18.18s compile with captured recipe/plan mismatch. No payload was
materialized. Unit3 paused while this newly identified overlap was corrected.

Corrective3d0aedc recognizes only the complete typed reduced WSI level chain:
three unique bounded levels/edges, native provider/algorithm, ordered-level
dependencies, source and resolved templates/SOP/transfer syntax/roles, bounded
matrix dimensions and required SameProject reduced qualification obligation.
Names and profiles do not bypass ordinary projection. Mixed, partial and crossed
contracts still reject. The existing reduced projection remains responsible
for its own evidence; this check does not grant new stress capability.

Final pure1 test passed0.64s after18.53s compile; intermediate typed-only test
passed0.64s after18.77s compile before the final obligation guard. Ownership1491
and diff passed. Independent reviewer and root accepted the exact obligation
binding and complete two-file correction; working tree was clean afterward.
No native WSI generation or full-scale resource qualification occurred. Broad
overlapping verification remains due after unit3, avoiding another duplicate
full route for this pure correction. Unit3 may now resume.

## Captured stress activation and ordinary verification — 2026-09-05

Unit3 c36fa03 binds external stress qualification to captured definition
profiles, binds file profiles as sets, and requires complete approved typed
stress projection intent. Qualification artifacts remain distinct from file
artifacts. Required reduced SameProject obligations, recipe/plan/output
identities, transfer syntaxes, cardinality and resource ceilings remain checked.
Unavailable cases do not become passes; ordinary US cases with historical
stress names publish and reopen without acquiring stress semantics.

Final focused10 Rust entries passed: pure stress projection1/0.84s, runtime
stress1/0.00s, external manifest7/2.54s after18.38s compile, and US SDK1/29.94s.
Routing31 passed3.122s; ownership1492 entries/276 groups passed0.257s; spelling
1065 and diff passed. Initial private parser access, test enum and lifetime
compile errors were corrected. A dependency test fixture needed its file profile
updated with the captured owner profile; an obsolete test-target invocation ran
no tests and was replaced by the exact library filter. Logs:
/private/tmp/r7-unit3-final-focused.log and
/private/tmp/r7-unit3-final-corrected.log. Independent and root reviews accepted
the implementation and evidence bindings.

The inspected all-ordinary selection contained79 commands. Initial execution
at c36fa03 passed the first21 groups, then identity passed3/4 and exposed the
remaining frozen schema-domain size oracle. d3bdef2 changes only that test oracle
and its ownership digest:58 schema members,1117572 bytes, SHA256
f6bf12057061673a9914bd2c3928cc9526568cb7a7ba732c21cb4eb2bc74502f.
Root independently recomputed the identity framing; the prior value was
reproduced with the pre-separation manifest2 schema. Identity4 passed1.37s
after18.26s compile (whole20.273s); ownership1492 and diff passed. Production
identity algorithms and pinned artifacts did not change.

The initial log is /private/tmp/r7-external-runtime-allordinary-fast-20260905.log
(filesystem write span57.230666s, not a process stopwatch). At clean d3bdef2,
regenerated selection matched the saved preview exactly. Commands22–78 and
Fast then all passed in274.667284s, avoiding repetition of the accepted prefix
and separately corrected identity group. Fast passed15 plus73 tests. Log:
/private/tmp/r7-external-runtime-allordinary-remainder-fast-20260905.log; preview:
/private/tmp/r7-external-runtime-allordinary-preview-20260905.json. Source edits
and parallel builds stayed paused during this run. This is ordinary same-project
evidence; heavy, codec feature matrix, native provider, isolated SDK consumer,
Nightly, release and future external-corpus evidence remain deferred.

Read-only review nevertheless found the reduced-stress WSI reopened-reader
regression described in b9b51a6. Ordinary suite success does not waive it.
That bounded correction precedes US standalone public proof and full external
boundary acceptance. R7 remains open; R8 and R9 have not begun.

## Reduced WSI reader correction accepted — 2026-09-05

6383f6c repairs reopening of the existing qualified reduced WSI case. Private
context is minted only after approved run qualification validation and complete
captured generated three-file membership, source levels/dimensions/spacing and
shared valid UIDs. Both applicability and deeper WSI dispatch require the bound
file; mixed ordinary evidence rejects. Existing legacy dispatch and reduced
payload checks remain intact. Root and independent review accepted the seven
owned files; no schema, recipe, pinned binary or generation bytes changed.

Two regressions use actual source planning/projection with explicitly synthetic
observations, plus a bounded196608-byte fixture adapted from existing WSI test
helpers. They reject incomplete/crossed groups, qualification/profile/path/UID
mutations and actual DICOM matrix/Pyramid UID changes. They are not an executed
native stress run or full-scale qualification. A separate ordinary-owned test
source and exact two-test route retain the existing sparse module's Nightly
classification; routing policy itself remains unchanged.

Initial2 tests passed0.65s after18.61s compile. Full sparse6 passed0.68s after
19.02s compile (whole20.402s); runtime stress1 passed0.00s. After source extraction,
exact2 passed0.65s, with18.05s compilation during list probing. Final routing32
passed3.080s, ownership277 groups/1494 entries and spelling1065 passed, and diff
passed. Logs /private/tmp/r7-reduced-wsi-reader-focused.log,
/private/tmp/r7-reduced-wsi-reader-final.log,
/private/tmp/r7-reduced-wsi-reader-routed-corrected.log and
/private/tmp/r7-reduced-wsi-reader-routing-final.log.

A formatter invocation recursively changed114 unowned source files. Automatic
approval review rejected broad restoration without proof. Independent
reproduction of rustfmt(HEAD) matched all114 working files byte for byte, zero
unmatched; verified-path restoration was then approved. Proof remains at
/private/tmp/r7-wsi-format-exact-proof.json. Incidental formatting inside owned
files was also removed. No other source writer was active and no such changes
were committed. The initial ordinary route inside a Nightly-owned source failed
32 routing tests with2 failures/63 errors, preserving the router's fail-closed
boundary. A nested module path compile error and then one expected bundle/count
assertion were corrected before the final32 pass.

Root inspected the80-command affected preview, including the exact new two-test
route and explicit deferred evidence. The previous completed79-command ordinary
selection remains applicable to unchanged scopes; after this local correction
root ran corpus-generation92/27.53s after18.93s compile and Fast15/2.12s plus
73/1.98s after1.16s compile at clean6383f6c. Whole53.146008s; log
/private/tmp/r7-reduced-wsi-reader-root-fast-20260905.log and preview
/private/tmp/r7-reduced-wsi-reader-root-preview-20260905.json. Task target measured
1178772KiB; default target untouched. Worktree was clean after verification.

This closes the bounded external reader correction and permits standalone US
public proof. US full genericity acceptance still requires that proof and current
guides. No native WSI, full stress, package, release or viewer evidence is added.

## US standalone public proof and bounded genericity accepted — 2026-09-05

bb687f6 adds the independently named US fixture, semantic oracle, SDK-only
support, separate CLI proof and exact ownership/routing/spelling integration.
Caller caller/acquisition/ultrasound, recipe caller_ultrasound, orders900/901
and independent/ultrasound.dcm preserve the qualified source tuple. CLI runs
from an unrelated directory with empty PATH; complete capabilities, raw manifest,
payload bytes and report objects agree with SDK results. Both outputs reopen
with strict validation. Separate current-source historical US1 generation
reproduces the original1006-byte payload and its accepted source semantics.

The closed fixture has3 files/8174 bytes: descriptor1637 SHA256
2fc7d9ace8a5e167def0d0701410350d39c7e81e4c9d5809ead622a1d22f7327;
registry3235 SHA256
5c377f4e7898e054fce69d9c135d785f34d2d2f63caaa671bc3135782b0beca8;
recipe3302 SHA256
a93424def7230a0a1dbbbb494c95402c9109b71aa50ac935ce2270c394f2399a.
Framed definition SHA256
bd3f6f093032f97370fe164fe8a114c58b99299cd4edaf8e2544a7a444575004.
Targeted oracle2012 SHA256
008a0426c4a395b4e47f34d45f49e5a75e2a160b00b4979fe56edc54c109e1b4
authenticates baseline8089876f and parityfa96555c receipts; no ignored artifact
path is a test runtime dependency. Caller1012 bytes SHA256
8604e1ff67e10a51ed5fe9fc115b2bc6d1c00116191b828475fa6135a89272ba;
original1006 bytes SHA256
e616b8c983c59640a62fa081f636acea554ce681edde3fc8dd53a3c15098c30b.

First observation1 passed10.52s after2.23s compile; final frozen1 passed10.49s
after2.19s compile. Conditional observation placeholders were removed before
acceptance. Fast15/2.13s plus73/1.93s, routing33/3.113s, ownership1495 entries/277
groups and diff passed. These owner results are tool-output-only; route preview
retained at /private/tmp/us-proof-route.json. The preserved manufacturer token
required one exact spelling inventory addition and corrected snapshot ordering;
final1066 matches SHA256
8e9495876ebc4cf8e75f1831c3aa45602b06a934dfaa17595fb35f8f5271683c.
No semantic test failure or generated payload commit occurred. Independent
review authenticated source/fixture/oracle hashes and all final assertions.

Root inspected the dedicated public route and ran all12 external_corpus_cli
tests at cleanbb687f6:28.64s (whole28.740314s). Log
/private/tmp/r7-us-public-root-subsystem-20260905.log; preview
/private/tmp/r7-us-public-root-preview-20260905.json. Owner's passed Fast was
not repeated. Task target measured1178564KiB; default target untouched.

Current guides d6811a3 document the bounded tuple, fixed synthetic metadata,
required unique orders, absent calibration region and evidence boundaries.
Independent six-document review and root stale-claim/spelling/diff checks passed.
Exact capabilities/generate/validate/report commands ran from fresh unrelated
/private/tmp/r7-us-docs-pr6u143u with empty PATH:1.288651s,1.089024s,0.330054s
and0.189148s respectively. Publication and strict1 passed; payload matched the
frozen caller hash. Retained9 files231769 logical/253952 allocated bytes;
measurement /private/tmp/r7-us-docs-measurement-20260905.json.

US1-local R7.1/R7.2/R7.3/R7.5 boundaries are now accepted with earlier data,
availability and parity records. This source capability does not update the
corpus232b9de artifact pin, relabel retained runs, qualify multiframe/RLE/
calibration coverage or add independent conformance, viewer, package or release
evidence. R7 remains incomplete. PET1 source preparation is the next sequential
ordinary-native slice; XA/XRF source/helper preparation remains independently
accepted without native execution.

## PET source preparation accepted — 2026-09-05

Corpus51ed60b records upstream US acceptance without changing its pin. PET1
source-only corpus0ff5bd4 now freezes15 immutable232b9de objects, the complete
34-row core snapshot, exact recipe/note and source-derived semantics. Provenance
146787 bytes SHA256
f19da711d5a438f4ae2fb2508b93acf4c9cf3fe57dd688912d43920050592fab;
recipe4486/aa5bb7c928daa315242c9b144b01fafcb68fbfd87d4e8edace475822edc6db9e;
note4958/288c02f8eee37bd4590ab3c9d1d2866d9fca01299179e74c11bb88024e088959.
Six pure tests passed0.035s. Root authenticated15Git objects/modes/hashes plus
full34 rows/selected row in0.212950s; independent review confirmed35 PET check
names, complete activity/geometry/timing/empty-sequence contracts and14 report
values. Diff and clean two-file commit inspection passed.

The helper audit fixes classic PET standards label sop_class_uid and report
lossy_image_compression=null despite raw00. Its next bounded two-file helper
unit must preserve those exact contracts, full33 skips and existing one-shot
two-pin capture/failure guards. Native generation, import, availability, parity
and PET caller genericity have not run. Preparation decision c95a46c remains
the sequential task map; source status is also recorded in the corpus ledger.

## PET baseline1 failed; source correction reviewed — 2026-09-05

Corpus helper29521f0 and CI2dab68c passed19 synthetic helper tests,56 routing
tests and6 clean-clone tests; full ordinary discovery passed558 in11.604s.
Corpus7a86fb2 recorded the reviewed one-shot readiness. Generator099d1f5
bounds later PET caller genericity; implementation remains sequentially held.

At clean corpus7a86fb2846cbe289cb168d0a29b9bb02fbc95415, the authorized
baseline1 generated successfully once in1.288898s, then failed its complete
recipe oracle before strict validation or reporting. Whole capture4.856045083s.
Retained corpus artifacts/r7-native-pet-baseline1-20260905 receipt37509 bytes
SHA256 945d9c700cf96df685dcfbd9d75bb225ae5218c5fe655df1f6137f5064d94515
is failed, not accepted evidence. Output2 files149783 logical/155648 allocated
bytes; payload1414 SHA256
78ced6c57926cafc6538ebf65459bb9efd7ecbb9a3c4ec90b28b4457cc795ce6;
manifest148369 SHA256
df0418e840c72948d4ca9fb859057a9b1ba92b375a1851b132d09ad98d2e9cf9.
Both pinned artifacts, principal/copied inputs, source and unrelated cwd were
unchanged. Logs /private/tmp/r7-native-pet-baseline1-20260905.stdout.json and
/private/tmp/r7-native-pet-baseline1-20260905.stderr.txt.

Pinned classic.rs nuclear_base merges ten generic pixel fields with geometry
and PET activity. The helper omitted those fields; offline replay additionally
exposed two retained native-frame findings, making62 internal findings, not60.
Pinned classic.rs and validation.rs independently establish both corrections.
The bounded two-file helper correction uses an independent literal12-key
fixture, missing/crossed/extra field mutations and missing-finding mutations.
Owner20 synthetic tests passed0.801s; independent source review and root diff
review accepted. Retained generation-only replay passed0.004615s with1 file
and33 skips; all retained hashes and failed receipt remain unchanged. No
strict/report/native retry occurred during correction. Baseline1 stays failed;
a fresh reviewed capture is required before import. R7 remains incomplete.

## PET baseline accepted; exact import opened — 2026-09-05

Corpus531a5b3 commits the two-file source correction;594462e records fresh
baseline2 readiness and773f20b accepts the independently audited capture.
Receipt56682 SHA256
0aec8cb252130433ba4626bf9c46d219f088e95fe5e3c7c3de958d2acd57c164
binds one generation1.294070s, strict1 validation0.223316s and report0.923888s,
whole5.993761083s. Retained22 nonreceipt files141238643 logical/141291520
allocated bytes under corpus artifacts/r7-native-pet-baseline2-20260905.
Independent offline audit1.164453s authenticated128 source files, complete
evidence closure/pins and generation/strict/report replay:1 payload,33 skips,
62 internal and4 standards checks. Payload and manifest match baseline1; its
failed receipt remains untouched. This is same-project embedded baseline only.

R7.1 next owner has exactly four corpus files: PET recipe, PET source note
under evidence/, registry and descriptor. Content0.8 adds the exact source
row/core membership and two members; existing0.7 bytes and artifact pins must
remain recoverable by authenticated byte-exact reversal. Static tests, CI,
historical fixtures, selected availability and two-run parity remain sequential
units. Caller genericity still waits for those gates; R7 remains incomplete.

## PET data/static/CI units accepted — 2026-09-05

Corpus1acb178 imports exact PET source data as0.8 in four files. Closed29
members164618 bytes; descriptor13397 SHA256
495ccb407846052cabdafb81059e113446e965e787e1c5edecd6b58a501414d9;
registry63447 SHA256
9ddf8a28ca23c2b7e4c6ce3194f2da42ba04f8cf910f8d00f456c124455b4760;
framed e9738a9596c1345c0ce96abd1af7f4a9bf875c07a820e701337178d384b55bcd.
Pure source/closure/exact0.7 reversal passed0.175821s, retained verifier/result
/private/tmp/verify-pet-content-import.py and
/private/tmp/pet-content-import-verification.json. Independent review and root
direct immutable-Git byte comparison accepted. Corpus e94228a records full data
measurements, initial unsupported-version static failures and source boundaries.

Static5f29d9e then passed18/0.168s with exact PET recipe/note/source inventory
and adversarial rehashed drift checks, resolving that module's earlier failures.
CI6cbb395 passed59/1.156s with fixed smoke3+PET1/all transition and isolated
recipe/note PET1/core ownership; historical versions and source-note mappings
remain exact. Root reviewed each diff; granular commits/diffchecks passed.
Status8ecf2a1/ff1c3a6 and current docs5639e70 distinguish accepted data/static/CI
from pending external evidence. Historical fixture adaptation is the next
two-file unit, followed by full ordinary discovery and the selected availability
checker/capture. No native run occurred in these units; pins remain unchanged.
PET parity and caller genericity remain unopened; R7 remains incomplete.

## PET ordinary integration accepted — 2026-09-05

Corpus9dc1dfa adapts only US/CR synthetic fixture modules, authenticating
live0.8 and source PET members before byte-exact0.7 reversal and existing
older-content chains. Production verifiers are unchanged.193 focused synthetic
tests passed6.501s (tool-output-only); root full diff review/diffcheck accepted.
Full ordinary discovery at clean9dc1dfa passed564/12.703s, retained log
/private/tmp/r7-pet-postimport-ordinary-20260905.log. Corpusb814078 records
acceptance; current docs7505846 reflect the completed integration.

The next two-file R7.5 unit owns only verify_pet_availability.py and its test.
It must bind accepted baseline2 plus complete live0.8 identity and report only
selected planning readiness. Source34 rows and live23 rows are distinct dated
inventories; the PET note participates in closed bundle identity. Capture and
parity remain subsequent sequential units. No native/build run occurred here.

## PET selected availability accepted — 2026-09-05

Corpusdc637aa offline checker passed14/0.781s and actual baseline/acquisition/
bundle binding0.016063s. Capture55bee79 passed15/0.176s, full retained input
authentication0.074697s; root and independent review accepted. CI7a9efbb maps
exact helper paths ordinary-only,60/1.191s. Full ordinary594/14.750s passed at
clean7a9efbb; log /private/tmp/r7-pet-preavailability-ordinary-20260905.log.
CI dry-run0.402958s preserved shared-smoke fallback; preview
/private/tmp/r7-pet-availability-ci-preview-20260905.json. Readiness5628bfa
recorded one selected assessment; no generation/parity was authorized by that
boundary.

Fresh corpus artifacts/r7-pet-availability1-20260905 passed. Receipt119808
SHA256 13f5a8b69271a4c6932ed0105f29ab6a25c43ce7c0404dfe152ecae7347c5a09;
response126907/661bb0723bee0b44e2d86f97fd6526d89ef64d58555a43fe61b23c0f12c13297.
Capability1.916512s/acquisition1.676506s/binding0.022460s, whole3.919236708s;
40 nonreceipt files73287582 logical/73355264 allocated bytes. Independent
offline audit0.156777s authenticated134 source blobs, retained inventory/modes,
copied inputs/cache/finalizers and checker replay without changing evidence.
Acceptance1a453de and docs28bdd62 state planning readiness only; publication
and validation not_run in this assessment. Pins and full0.8 identity unchanged.
Next two-file parity helper unit retains entire historical PET file and exactly
two public selections; no PET-specific normalization is needed. Parity and
caller genericity remain pending; R7 incomplete.

## PET parity accepted; first planner unit opened — 2026-09-05

Corpus308105b parity helper passed13/0.325s and actual retained authentication
0.202827s after corrected draft substitutions. CIa9fb70b passed61/1.192s; full
ordinary608/14.186s passed, log /private/tmp/r7-pet-preparity-ordinary-20260905.log.
Readinessa9e5865 bound exactly two public PET runner calls. Fresh parity1
receipt296514 SHA256
6bb4c99b48bb97baa7d6c9b4eccc20a627017800f1397761a9188a0c68190a7d
passed3.902212s/3.463542s, whole10.154198417s. Both1414-byte payloads match
baseline78ced6c5; complete file projection5f980091 remains unchanged.
Independent audit authenticated231 retained files/136 source archive objects,
both baseline/report2/repeat comparisons and request/sidecar/finalizer checks;
comparison0.304033s plus final sidecar0.160968s. Corpus5ff38c3 records full
hashes and sizes. Original /private/tmp/r7-pet-parity-t4pk6rkl is retained.

PET-local migrated parity is accepted without repinning artifacts or claiming
viewer/independent/release evidence. First source planner ownership is now
explicit in the PET preparation document; loader and public proof stay
sequential. No new provider family or widened quantitative contract is admitted.
R7 remains incomplete.

## Disjoint VL2 source-only preparation opened — 2026-09-05

While PET's nuclear planner is the sole partially migrated public boundary,
a separate agent may prepare immutable source provenance for exactly
vl/photo/rgb_planar0_explicit_le and vl/photo/palette_color_explicit_le.
These native photographic cases use the classic VL provider, disjoint from
PET's nuclear closure; XA/XRF shared-provider implementation is not active.
Plan section7 permits disjoint R7 case/provider work. This unit owns only corpus
docs/r7-native-vl-photo-source-provenance.json and
tests/test_native_vl_photo_provenance.py. Freeze232b9de source objects, fullcore
snapshot, exact recipes/notes and typed projection/validation/report semantics.
No native execution, helper, data import, provider implementation, template or
public compatibility change is included. Review and granular source-only commit
precede any later helper work. PET loader/public proof ordering is unchanged.
