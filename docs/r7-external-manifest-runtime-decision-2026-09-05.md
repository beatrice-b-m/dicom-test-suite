# R7 external manifest runtime dispatch decision — 2026-09-05

Status: accepted implementation direction; verification pending.

Schema correction a1d1e12 preserves generic external file structure. Runtime
validation still loses ManifestContractKind below validate_loaded_manifest_root
and dispatches several historical contracts by caller case identity. Independent
read-only audit confirms external report2 already uses corpus_report::project;
legacy reporting requires no correction. Geometry uses IDs as grouping keys.

Execute three sequential units within this one open compatibility boundary:

1. Forward explicit manifest contract context through file/pixel validation.
   External U32/U1/ICC/nonsquare and metadata checks activate from declared
   evidence, not historical names. Preserve required evidence and wrong-scope
   rejection for legacy manifests. Include nonnative nonsquare rejection.
2. Apply evidence-based activation to reference closures and VL/WSI/STL
   contracts. Preserve declared evidence checks and source closure. Group
   external collective evidence by caller case/group identity, never across
   independent cases. Do not incidentally generalize pinned source semantics.
3. Separate historical stress-name interpretation from authenticated external
   profile/qualification evidence. Preserve negative, fuzz and stress isolation;
   names alone cannot turn an ordinary valid caller case into stress evidence.

Each unit has explicit file ownership and granular commits, focused tests and
independent review before the next unit. Extend existing tests where practical.
Prove external historical-ID collisions through publication, reopened strict
validation and report2; also prove renamed declared evidence still executes
and tampered evidence/payloads fail. Retain legacy missing-evidence and
wrong-name regressions. WSI groups and nonnative pixel paths need separate
coverage from single-file native publication. No blanket external bypass,
identity rewriting, silent evidence dropping or legacy requirement weakening.

This corrects rejection promised by external manifest2 under compatibility
policy section2 in the unreleased product0.2.0 line. Retain schema, recipe and
template versions and pinned native artifacts. Broader affected verification
and standalone US public CLI/SDK proof precede full genericity acceptance.
R7 remains incomplete; no R8/R9 or release qualification is implied.
