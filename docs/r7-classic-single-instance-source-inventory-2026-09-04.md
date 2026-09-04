# R7 classic/VL single-instance source inventory — 2026-09-04

Status: source-only audit and proposed subdivisions, not import authorization,
runtime readiness, migration parity, independent conformance, or R7 completion.
Scope is exactly cohort 2 of the
[remaining-native inventory](r7-remaining-native-slice-inventory-2026-09-04.md).
Metadata3 consumer preparation remains a separate slice; its accepted baseline
does not imply live import. No broader extended, series, or codec audit is made.

## Authority and common contracts

Authoritative source: `232b9de41f97ee95abe1ecc40b6b8b70ebeeea5f`.
Compared current source at `5c018272a41d23aa62835250696d3c69bb4fbfdc`.
The registry, all eleven recipes, three referenced notes, standards lock,
`templates/catalog.json`, five classic family planners, `src/recipes/classic.rs`,
`src/recipes/loader.rs`, `src/curated_plan.rs`, `src/curated_execution.rs`,
`src/curated_manifest.rs`, and `src/curated_manifest/classic.rs` have no drift
from the authoritative source. This scoped comparison is not whole-engine parity.

Raw `cases/registry.json`: 463479 bytes, SHA-256
`354a5447c3b8f40b5f777f184a8d8850beb74f87fa08dc0c259e4b211142731b`.
Raw `standards.lock.json`: 4170 bytes, SHA-256
`823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559`.
Preserve its 2026b policy; source-note historical claims are not fresh evidence.

All eleven rows are `implemented`, profiles exactly `["core"]`, registry provider
`rust_native/rust_native`, recipe/schema version `0.1.0`, `kind=dicom`, plan
provider `native.classic_plan`, content provider `content.native_pixels`, and
`byte_stable`. Required features, external codecs, external validators, blockers,
and recipe dependencies are empty. Native dataset encoding is Explicit VR Little
Endian (`1.2.840.10008.1.2.1`), except MG processing: Implicit VR Little Endian
(`1.2.840.10008.1.2`). No external runtime or caller binary asset is declared.
Installed native/template capability must still be assessed by the public loader.

Each recipe declares one logical artifact `instance`, artifact order 0, with
output `<exact case ID>/instance.dcm`; each resolves to one frame. Thus this
dated source selection declares 11 artifacts/11 frames, not measured output.
Templates below are all version `1.0.0`; algorithm IDs abbreviate
`algorithm.classic_`. Every recipe/artifact uses `validation.shared` and
`projection.curated`; DX adds `validation.classic.dx`, both MG variants add
`validation.classic.mammography`. No attribute operations are declared.

## Exact selection, bindings, and proposed disjoint subcohorts

The key identifies a row in subsequent tables, never a substitute selector.
Sizes below are native sample-byte hints, not Part 10 sizes or cost measurements.

| Key | Exact case ID | Recipe ID | Template ID | Algorithm | Plan / projection | Pixels; bytes |
| --- | --- | --- | --- | --- | --- | --- |
| CT | `classic/ct/mono2_i16_rescale_12bit_explicit_le` | `ct_mono2_i16_rescale` | `classic/ct` | `ct` | 200 / 87 | 2×2 i16, 12 stored; 8 |
| CR | `classic/cr/overlay_modality_voi_explicit_le` | `cr_overlay_modality_voi` | `classic/cr` | `mr_cr` | 500 / 152 | 2×2 u8; 4 |
| DX | `classic/dx/display_shutter_mono2_u16_explicit_le` | `dx_display_shutter_mono2_u16` | `classic/dx/for-presentation` | `dx_mg` | 304 / 142 | 2×2 u16, 12 stored; 8 |
| MG-P | `classic/mg/for_presentation_mono1_u16_12bit_explicit_le` | `mg_for_presentation_mono1_u16` | `classic/mammography/for-presentation` | `dx_mg` | 300 / 138 | 2×2 u16, 12 stored; 8 |
| MG-R | `classic/mg/for_processing_mono2_u16_12bit_implicit_le` | `mg_for_processing_mono2_u16` | `classic/mammography/for-processing` | `dx_mg` | 302 / 140 | 2×2 u16, 12 stored; 8 |
| US | `classic/us/mono2_u8_explicit_le` | `us_mono2_u8` | `classic/ultrasound/single-frame` | `nuclear` | 403 / 150 | 2×2 u8; 4 |
| PET | `classic/pet/rescaled_activity_explicit_le` | `classic_pet_rescaled_activity_explicit_le` | `classic/pet` | `nuclear` | 401 / 145 | 2×2 u16, 16 stored; 8 |
| XA | `classic/xa/monoplane_explicit_le` | `classic_xa_monoplane_explicit_le` | `classic/xa` | `vl_projection` | 608 / 148 | 4×4 u8; 16 |
| XRF | `classic/xrf/monoplane_explicit_le` | `classic_xrf_monoplane_explicit_le` | `classic/xrf` | `vl_projection` | 609 / 149 | 4×4 u8; 16 |
| VL-RGB | `vl/photo/rgb_planar0_explicit_le` | `vl_photo_rgb_planar0` | `vl/photographic` | `vl_projection` | 600 / 7 | 2×2×3 u8, planar 0; 12 |
| VL-PAL | `vl/photo/palette_color_explicit_le` | `vl_photo_palette_color` | `vl/photographic` | `vl_projection` | 604 / 11 | 2×2 u8 indices; 4 |

Proposed bounded sequence: CT1, DX/MG3, CR1, US1, PET1, XA/XRF2, VL2.
These are disjoint case sets, not disjoint engine providers: DX/MG share one
algorithm; US/PET share one; XA/XRF/VL share one. Baseline/import/parity should
remain sequential per accepted subcohort. No all-eleven execution is implied.

## Source identities and member closure

Recipe path is `cases/recipes/<first two case-ID segments>/<recipe ID>.json`.
Row hashes/sizes use UTF-8 JSON, sorted keys, compact separators,
`ensure_ascii=False`, one final LF. Recipe and note hashes/sizes use raw bytes.

| Key | Canonical row bytes | Canonical row SHA-256 | Recipe bytes | Raw recipe SHA-256 |
| --- | ---: | --- | ---: | --- |
| CT | 2358 | `1b6c043c5df419ddbc7b27466f8fa19f8034688f78479d6a01b0432beaf6affa` | 3938 | `f014e51c72b094dd188267fbe6a36e3caeebc6828fcc415045fcb02a758a84ca` |
| CR | 2861 | `3028df6e549e8b7b2ce08ce9119336871370affd309598e402c67341311973c9` | 4013 | `7ee5bd86e7a83db9b484ce4cdb2f12243616749fdf392ec155ff3d7604b8c8d1` |
| DX | 3246 | `4b2d5d267d697f6dc793a8ed02ad30c016e90da339805b807ec68a4dbe6a77b1` | 4037 | `82228d5ef2be7496cf084c41b7b885bb31b3c6f911e93211f92158f71768bb68` |
| MG-P | 2945 | `d5cba0957108303480f065bf993c8f59bee050003e42265d67db4cb143523caf` | 4160 | `4c963fca2cacab5f78dc28eae701278a4dc0445d06fef0f15d17a0afba9e76aa` |
| MG-R | 3224 | `fffa29d87df2ad75d4bc6733577b4dd807b9f185e05a41d213c275c17b61cf14` | 4694 | `cec2846945593d2d6c70e2faf839b459045c67128a082eee59f673262de07d78` |
| US | 2076 | `b07edd543e7d4d4ba730cf85e6ce2b22bdb9b475f2037178b7792816a88da6ff` | 3312 | `cdd7bdd0179dcd6db5759ee443463318bb7dd798f3d45fa342edd6f2376b9635` |
| PET | 2029 | `972fd9a25ceeb5615d476ac1bfa5bacbe6d019a56973850405f32557d523edc4` | 4486 | `aa5bb7c928daa315242c9b144b01fafcb68fbfd87d4e8edace475822edc6db9e` |
| XA | 1820 | `828a8aca92daaafac4c1339ea458b8d78dbfbe1f4fdb4443bd0419e7c857282e` | 4643 | `b3662a1d5f2b87678b115f0816c8dcb2493b666ceb14aadc09cfb2f7794e5a07` |
| XRF | 1849 | `c48afd1b51fd52458f37a168acf3b1f4a246faf43bc8a0eab277a67aab449ac6` | 4287 | `4ef49d9365a9ca7546413cbc4851a22870b449477fd5bca481a115eb837f7890` |
| VL-RGB | 1493 | `4e342fc9f08744668ebbff394c67d92a25fd0a585dc73f943744c9437bddc49e` | 4371 | `0a3f074f3e32fa75aae2af39d073a6d4a5e2d8c8d2ea5859319a1228c976f648` |
| VL-PAL | 2803 | `1c33c6b9c82418ff144def169f73e2b7d79a4d6ef18d3faab93c7e494b2f1f3b` | 4476 | `09ccd7e56a90ebf5fb643fbb89b477fc1f87c0c1e29a8e21ae9ddcb1a65c0abf` |

| Referencing key | Exact note path | Bytes | Raw SHA-256 |
| --- | --- | ---: | --- |
| PET | `standards/source-notes/phase-2-pet-rescaled-activity.md` | 4958 | `288c02f8eee37bd4590ab3c9d1d2866d9fca01299179e74c11bb88024e088959` |
| XA | `standards/source-notes/phase-2-xa-monoplane.md` | 7968 | `90a8bf897127999a1d4aee7e2176a05b3cf9fff1dbe8a3064d9f3f717ec4fc2e` |
| XRF | `standards/source-notes/phase-2-xrf-monoplane.md` | 8528 | `963cb04762d8784f39b6f8f73722eb590ad383c7014c068343fb57bfe91e43b5` |

Other rows cite KB evidence only. Inline samples, CR overlay/LUT data and VL
palette data are recipe-owned; no binary/ICC member or case dependency is
needed by these eleven. Copy only declared notes, not Standard/KB artifacts.
The classic projector declares empty references for this slice. There is no
metadata3 case/recipe/note/template/algorithm overlap; shared native registry
provider, loader, executor, writer and validation infrastructure remain common.

## Genericity debt and parity review requirements

`src/recipes/loader.rs:1496` still accepts classic recipes through namespace
exemptions. `src/curated_plan.rs:4515` tries five family planners and requires
exactly one match; the algorithm ID alone is not generic dispatch. CT and DX/MG
planners gate on prefixes (`classic_ct.rs:546`, `classic_dx_mg.rs:566`). CR uses
exact recipe identity plus fixed case/path/order (`classic_mr_cr.rs:93,250`).
US/PET use exact case-family lookup (`classic_nuclear.rs:657`); VL/XA/XRF use
exact case/order membership plus prefix branches (`classic_vl_projection.rs:171`).
The parameterized `classic.rs` module is a reusable seam, not proof that these
entrypoints accept independently named caller cases. R7.2/R7.3 remain open;
future genericity work needs public-path, renamed-case positive/adversarial
tests without weakening topology, template, pixel or namespace safety checks.

Preserve source payload identity strings (`DTS`, `DICOMTEST`, `SMOKE`,
`dicom-test-suite`, software `0.1.0`) and UID indices: VL-RGB 3, VL-PAL 7.
Do not renumber either planning or projection order to subset positions.
MG-P is MONOCHROME1; all other monochrome rows are MONOCHROME2. PET preserves
BQML/rescale, not SUV; XA/XRF preserve distinct equipment geometry and explicit
absences, not patient-space calibration, multiframe, cine or biplane coverage.

For each reviewed subcohort, obtain a separately authorized source232b9de
seed-1/core/explicit-ID baseline, strict validation by the same pinned binary,
and report-only evidence from the separately pinned coverage1.1 reporter.
Authenticate binaries, raw evidence, full file arrays, bytes/sizes, frame hashes,
UIDs, plan/resolved-plan hashes, parameters, validation, semantics and standards.
The classic projector appends recipe standards evidence in order: MG-R 2,
XA 1, VL-RGB 4, VL-PAL 3; all others 0. It also handles implementation-version
and validation-check projection specially for both VL rows. Derive exact
full-array comparison and only explicitly versioned normalization from these
contracts; do not reuse SC/metadata subsets.
Review versioned manifest/report identity and skipped-ledger differences, then
public-loader profile/explicit-ID assessment, exact import, selected generation,
strict validation, repeat parity and bounded regression. Unknown differences
stop review. Preserve frozen earlier evidence and consumer selection guards.

Verification: read-only Git source extraction, JSON queries, byte equality and
SHA-256 calculations; no native commands, builds, tests, generated artifacts,
live corpus edits or external-state changes. Source counts are dated queries.
