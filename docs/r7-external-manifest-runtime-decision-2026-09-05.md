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

## Reachable dispatch inventory refinement

Read-only follow-up for unit2 found deeper dispatch in
validation::validate_manifest_wsi_file, plus tile-SEG and Comprehensive 3D SR
in validate_family_standard_elements. Forward explicit context to these seams:
actual WSI requires one supported declared WSI contract; Comprehensive 3D SR
selects declared SCOORD3D or TID1500 evidence; crossed or absent required
evidence must fail. A US file whose name matches those historical cases must
not acquire their semantics. Unit2 therefore owns src/validation.rs as well as
src/lib.rs and focused tests/ownership metadata.

Reference closures require authenticated source-object lookup. External color
presentation-state and RT Image may use caller consumer/source IDs and paths,
while preserving SOP, geometry, hashes, UIDs and relation checks. Spatial and
deformable registration already use declared source paths. Each declared
contract must be checked; an else-if chain must not drop crossed declarations.
Pyramid groups partition by caller identity and retain member ordering, complete
evidence, repeated contracts and byte ceilings.

Separate RT Plan/source identity locks in validation.rs are called only by
curated generation and direct tests, not reopened manifest validation. They
remain for later RT genericity migration; this correction does not claim those
generation capabilities are already independent. Legacy report paths remain
untouched. No native WSI or RT qualification ran during this inventory.

## Qualification activation refinement

Unit3 forwards explicit kind into stress qualification validation. For external
manifests, derive stress membership from selection_ledger case_definition
profiles, not a stress/ prefix or unbound file profile_membership. Require each
stress qualification to belong to a captured stress case before checking the
existing approved reduced recipe, actual/requested scale, byte totals and
resource ceilings. Preserve curated prefix activation and unavailable outcomes;
a captured unavailable case without files does not become a passed run.

Bind each external file's profile_membership to its captured definition as a
set (schema already rejects duplicates). This prevents contradictory report
profile evidence and keeps runtime selection grounded in the captured contract.
The existing negative/fuzz evidence/profile checks remain unchanged. This unit
does not introduce generic stress generation or relax approved reduced recipes.

Extend ordinary US collision checks with stress/caller and the exact historical
enhanced-CT stress ID. Pure tests cover neutral-name stress requiring evidence,
core cases rejecting attached stress qualifications, valid reduced evidence,
missing/duplicate/forged evidence and ceilings, plus unavailable rows. Extend
external manifest tests with profile-membership tampering. Run unit3 only after
unit2 acceptance; shared validation and manifest files cannot be edited in
parallel across those units.
