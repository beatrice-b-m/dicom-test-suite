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

## Prepublication projection correction — unit2b

The unit2 expanded US SDK test exposed an earlier boundary: exact caller ID
vl/wsi/pyramid_multiresolution triggers curated_manifest::project_wsi_pyramid_group
and fails before publication because it requires three members. Record that
failed6.20s run; unit2 runtime acceptance excludes this one publication claim.
Restore the regression in immediate unit2b before qualification unit3.

Select bounded pyramid projection from captured native.wsi_plan recipe and
resolved template identity/version, WSI SOP and typed artifact parameters.
Validate volume/thumbnail/label roles, algorithms and membership; reject partial
or crossed intent rather than filtering malformed members. Group all members
of a participating caller case, requiring one complete ordered triple; separate
caller groups cannot combine. Preserve shared identities, hashes/sizes and
historical full projection exactly. US with the historical pyramid name has
no typed pyramid intent. Tests use captured-plan projection fixtures plus the
restored public US collision; this is not generic WSI generation qualification.

Unit2b owns curated_manifest.rs, the existing US test, focused curated manifest
projector tests/support as needed and exact ownership metadata. Implement only
after bounded unit2 verification; no parallel edits to the shared boundary.

The audit also found stress postprojection selecting by historical case ID in
curated_manifest/stress.rs. Add typed/captured-profile activation there to unit3
before runtime checks, preserving approved reduced recipes and exact prior
evidence. Otherwise the exact historical stress caller regression would fail
before reaching runtime validation. Payload-free qualification projection is
already gated by PlannedArtifact::Qualification and does not affect US files.

## Reduced WSI reopened-reader correction

After unit3, a read-only audit confirmed an additional unit2 regression: the
existing reduced-stress WSI projection emits expected_semantics rather than an
ordinary expected_wsi_* contract. Both external reader dispatch layers reject
it. The shared-template projection correction alone does not repair reopening.

Derive private reduced-WSI reader context only after run-level qualification
validation, from captured stress membership and the validated approved
wsi_pyramid qualification. Bind the complete three-file level chain and its
shared identities and bounded source semantics. Forward that context through
file applicability and the deeper WSI selector; reject mixed ordinary evidence.
Names, file profiles and a shared pyramid UID alone cannot grant this context.
Retain legacy dispatch and the existing reduced Part10/pixel/matrix checks.

This correction owns src/lib.rs, src/validation.rs, focused existing WSI tests
and ownership metadata, sequentially before the standalone US public proof.
Bounded synthetic and existing payload fixtures must exercise the dispatch and
negative cases. They do not constitute native stress qualification or full-scale
resource evidence. Root owns this decision and status record; implementation
and independent read-only review have separate owners.

The two new reduced-reader regressions use source planning and a bounded
196608-byte fixture. Add an ordinary routing bundle for exactly those tests,
including all-ordinary selection, while retaining the full WSI module's Nightly
ownership. A changed runtime boundary must not defer its new cheap regression
solely because the existing containing module is classified heavy. This adds
product/change-test-routing.json and its Python routing test to the owner's
files; it does not select native WSI or full stress qualification.

The router enforces source-level ownership and rejects an ordinary carveout
inside a Nightly-owned source. Keep that fail-closed rule. Put the two bounded
regressions in src/validation_wsi_reduced_reader_tests.rs, loaded as a nested
test module from the existing sparse fixture module, with its own ordinary
ownership record and exact route. Existing sparse tests remain Nightly. This
adds only a test source and module declaration, not a routing-policy exception.
