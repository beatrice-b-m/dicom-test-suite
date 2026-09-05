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
