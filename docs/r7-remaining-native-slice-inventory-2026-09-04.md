# R7 remaining ordinary-native migration inventory — 2026-09-04

Status: proposal, not execution authorization. Accepted core10 and smoke3 are
recorded at corpus `ee06fc9` and generator `f6ee632`. Wider R7 remains open.

## Authority and bounded scope

The migration source is `232b9de41f97ee95abe1ecc40b6b8b70ebeeea5f`.
Current `cases/registry.json` is byte-identical to that source, SHA-256
`354a5447c3b8f40b5f777f184a8d8850beb74f87fa08dc0c259e4b211142731b`.
The query is explicit registry membership, not a case-ID prefix inference:
all 34 core rows are implemented, provider `rust_native/rust_native`, have
empty feature/codec/validator requirements, and their recipes have no case
dependencies. Ten are already migrated; the remaining 24 are partitioned below.
Counts are dated queries, not invariants or newly assessed runtime readiness.
This is a core audit, not exhaustive extended eligibility or conformance evidence.

## Recommended next cohort: three single-instance metadata cases

All three use recipe version `0.1.0`, `native.metadata_sc_plan`, template
`classic/secondary-capture/monochrome@1.0.0`, native Explicit VR Little Endian,
and one `instance.dcm`. Each has a typed 2×2×1 MONOCHROME2 unsigned 8-bit
pixel contract (four sample bytes), no dependency, and `byte_stable` determinism.
Four pixel bytes are a payload hint, not a measured Part 10 file size.

| Exact case ID | Recipe path beneath `cases/recipes/` | Content provider | Planning / projection order |
| --- | --- | --- | --- |
| `metadata/sc/utf8_person_name` | `metadata/sc/metadata_sc_utf8_person_name.json` | `content.metadata.person_name` | 64 / 76 |
| `metadata/sc/empty_type2_attributes` | `metadata/sc/metadata_sc_empty_type2_attributes.json` | `content.metadata.empty_type2` | 67 / 80 |
| `metadata/sc/private_creator_blocks` | `metadata/sc/metadata_sc_private_creator_blocks.json` | `content.metadata.private_creators` | 69 / 82 |

Recipe IDs are respectively `metadata_sc_utf8_person_name`,
`metadata_sc_empty_type2_attributes`, and `metadata_sc_private_creator_blocks`.
Raw recipes and the following notes were checked byte-equal to source232b9de:

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| UTF-8 recipe | 3614 | `4616511eea415bb4b65236ef631fb78f5d0a8703a976ac924ae8c13ef3a6aef0` |
| Empty Type 2 recipe | 3401 | `d8a8b313f0bdc4a916f7f79a053cd62379177c6e95d687aebe6ee85cc6311c92` |
| Private creator recipe | 4370 | `c1eb011682246ab8801e7aca4bf5cb34538c9064e0c0bb895521a041f5270ad2` |
| `standards/source-notes/phase-2-utf8-person-name.md` | 2274 | `e60139eda8c7d5bbb0c94e3fe94680542cee80f1828febb573ac4be2bdfeba8d` |
| `standards/source-notes/phase-2-empty-type2-attributes.md` | 2879 | `850e4b84c16f9aeb40074819dcf5864313e2726ded8884ef4cdcaff5c571b65c` |
| `standards/source-notes/phase-2-private-creator-blocks.md` | 3565 | `3afe345d1151e2a5e8673e5a194e40d2461355f6f48c07a1113489f6f5dfe388` |

Canonical registry-row hashes (UTF-8, sorted keys, compact separators,
`ensure_ascii=False`, one final LF) are:

- UTF-8: `020a52499260e950afe6e6f2f09f985520113c919e8b8df1cabe7dc6b72e25df`.
- Empty Type 2: `462cce81834825c478b370b6496a0b0db8d8e16e298dcf75d3539a49a91bcb94`.
- Private creators: `c985f4a82cf637d2358f9243cb5e5e0cbf099aec6e792b4dbd1af2bb10d823bc`.

The notes bind PN repertoire/groups, five zero-length Type 2 values, and private
block allocation. Copy the notes as declared evidence members, not the Standard
or KB database. Their historical conformance statements are not new qualification.
Preserve `DTS_PRIVATE_ALPHA`/`DTS_PRIVATE_BETA` payload identifiers and exact raw
UTF-8 PN bytes; migration is not authorization to rename brands inside payloads.

## Ordered, disjoint remaining core cohorts

Prefer single-instance objects first. Below, provider is `native.classic_plan`
unless stated; each later cohort needs source provenance and readiness/parity review.

1. **Metadata single-instance (3):** exact IDs above.
2. **Classic/VL single-instance (11), subdivide by template family:**
   `classic/ct/mono2_i16_rescale_12bit_explicit_le`,
   `classic/cr/overlay_modality_voi_explicit_le`,
   `classic/dx/display_shutter_mono2_u16_explicit_le`,
   `classic/mg/for_presentation_mono1_u16_12bit_explicit_le`,
   `classic/mg/for_processing_mono2_u16_12bit_implicit_le`,
   `classic/us/mono2_u8_explicit_le`,
   `classic/pet/rescaled_activity_explicit_le`,
   `classic/xa/monoplane_explicit_le`,
   `classic/xrf/monoplane_explicit_le`,
   `vl/photo/rgb_planar0_explicit_le`,
   `vl/photo/palette_color_explicit_le`.
   Templates: CT/CR, DX, mammography, US single-frame, PET, XA/XRF, VL photographic.
   PET/XA/XRF cite phase-2 notes; others use KB only. XA/XRF each have 4×4 samples.
3. **Paired variants (2 cases / 4 artifacts):**
   `metadata/sc/timezone_boundaries` (`native.metadata_sc_plan`, two 2×2×1
   images; phase-2-timezone-boundaries note), and
   `classic/sc/nonsquare_pixel_spacing` (`native.sc_plan`, two 4×6×1 images;
   phase-2-nonsquare-spacing-aspect-ratio note). These are not single-artifact
   cases even though each artifact is single-frame.
4. **Classic multiframe (2 cases / 2 artifacts):**
   `classic/us/multiframe_explicit_le` (4×4×4 u8) and
   `classic/nm/multiframe_explicit_le` (2×2×4 u16), with their respective
   phase-2 source notes and ultrasound-multiframe/nuclear-medicine templates.
5. **Series/geometry (6 cases / 19 artifacts):**
   `classic/mr/multislice_oblique_explicit_le` (3),
   `geometry/ct/duplicate_missing_instance_number` (3),
   `geometry/ct/gantry_tilt_series` (3),
   `geometry/ct/multiseries_shared_frame_of_reference` (4),
   `geometry/ct/nonuniform_slice_spacing` (3),
   `geometry/ct/spatial_sort_conflicts_instance_number` (3).
   Middle four CT cases cite phase-2-ct-geometry; MR/spatial-sort use KB evidence.
   Empty case dependencies do not erase intra-case series/reference semantics.

## Reusable-capability debt: R7.2 and R7.3 remain open

`src/recipes/metadata_sc.rs:30` translates typed metadata variants directly,
reuses the SC base plan, and preserves raw PN bytes. This is a useful generic
algorithm seam. However, `src/recipes/loader.rs:795` still maps seven specific
case IDs to metadata kinds, and derives a fixed one/two-artifact count from
that mapping. `loader.rs:1489` also requires the `metadata/sc/` case namespace.
Thus a differently named caller case with equivalent typed data is not yet a
generic metadata capability. Do not claim R7.2/.3 completed merely because the
downstream runner uses only the supported CLI.

Classic has namespace exemptions at `loader.rs:1496`; SC has nearby special gates.
Audit entrypoints, validation/projection, and synthetic case-name independence;
do not simply remove match arms or weaken constraints. Exact historical data
ownership can precede this work, but full R7 cannot. Do not implement generic
capability changes concurrently with data migration.

## Sequential preparation and evidence policy

1. Freeze the three rows, recipes, notes, orders, and provider/template bindings
   in a new source-provenance document. Preserve the earlier ten-case provenance
   and baseline helpers byte-for-byte.
2. Prepare a new baseline helper: original source232b9de/native pin generates
   only the three IDs within core, seed1, embedded parallelism4; the same old
   pin strictly validates. Use separately pinned report-only sourcec2ffe41
   (coverage1.1) from the first report attempt. Do not repeat the known failing
   old coverage1.0 report. Bind both binary identities and all raw evidence.
3. Start comparison from full canonical file arrays, raw payload hashes,
   plan/resolved-plan hashes, UIDs, and all metadata/validation/standards fields.
   Preserve planning/projection orders 64/76, 67/80, 69/82; do not renumber to
   subset registry positions. `curated_manifest.rs:3368` adds typed
   `expected_metadata` and recipe parameters, and its standards projection
   enriches registry evidence. Derive the exact metadata-cohort enrichment;
   neither reuse the first-ten arrays nor accept a subset-only comparison.
   UTF-8 canonicalization must be named explicitly and kept separate from the
   frozen R0 comparator. Any unexpected equality difference stops for review.
4. Before import, extend consumer static source guards and CI's explicit
   allowed-core provenance from ten to this reviewed cohort; currently all
   additional IDs and evidence members are intentionally rejected. Preserve
   changed-case closure, fixed-smoke fallback union, and explicit mixed-scope
   IDs. Existing result2 accepts numeric content versions; select the next
   corpus content version explicitly, without changing frozen result1 or pins.
5. The historical proof calls `completion.snapshot` with a 64-regular-file cap;
   its accepted source snapshot already has 59 files. Three recipes plus three
   notes exceed that capacity before additional provenance/tests. A new proof
   needs a reviewed bounded inventory contract (the newer streaming inventory
   supports 10,000 files), not silently widened historical snapshot checks.
6. Then exact import, public-loader profile and explicit-three-ID assessments,
   followed by separately reviewed explicit-three-ID/repeat parity and an
   appropriate bounded regression. Keep historical smoke/R0 evidence frozen;
   no whole-core generation is implied. Full report2 must retain the entire manifest,
   and old profile-wide skipped rows versus selected-only manifest2 ledger must
   remain an explicit, authenticated versioned difference.

## Wider extended inventory: follow-up, not executable scope

Query `implemented && provider.kind == rust_native && profiles contains extended`
returns 109 rows (100 feature/codec-requirement-free, not newly assessed ready).
Selected registry-order array canonical SHA-256:
`5106670561552f9cbf499b31dde1fc0193fb7438276c50efdf5ed3ecff5e3d0b`
using the row canonicalization above. Provider counts: SC51, classic14,
exceptional-SC8, enhanced6, RT6, quantitative5, presentation-state4, metadata3,
SR3, WSI3, encapsulated-payload2, registration2, waveform2.
Detailed extended closure/size/availability audit is deferred. Optional codecs,
relationships, pathology and isolated profiles must not become one native run.
This core-first order does not narrow the terminal scope. Verification here was
source-only; no new baseline, runtime cost, or availability promotion is claimed.
Root and independent source/hash review accepted this proposal. The documentation
route dry-run selected no ordinary commands; unconditional Fast targets were
reported, not executed. `git diff --check` passed.
