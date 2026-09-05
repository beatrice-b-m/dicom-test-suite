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
| R7.1–R7.5 remaining core | Close accepted smoke/core SC genericity first (see the 2026-09-05 SC audit), then CR1, US1, PET1, XA/XRF2, VL2; then paired variants, multiframe and series/geometry, one closure at a time | For each: source freeze, embedded baseline, exact import/routing, loaded availability, migrated/repeat parity, then genericity; no simultaneous shared loader changes |
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
