# Phase 3 Blending Softcopy Presentation State Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `derived/presentation-state/blending`
- Recipe ID: `derived_presentation_state_blending`
- Recommended provider: `rust_native`
- Source case: `geometry/ct/multiseries_shared_frame_of_reference`
- Output: Blending Softcopy Presentation State Storage
  (`1.2.840.10008.5.1.4.1.1.11.4`), Explicit VR Little Endian
- Recommended manifest field: `expected_blending_presentation_state`

The implemented source case has exactly two CT Series in one Study and one
Frame of Reference, with exactly two single-frame CT Images per Series. Every
Image is 2 by 2, has orientation `[1,0,0,0,1,0]`, and the ordered Image
Position (Patient) values are `[0,0,0]` and `[0,0,5]` in each Series. The
series order is ordinal 1 then 2 and the image order within each Series is
slice 1 then slice 2. The exact relative paths, in locked order, are:

1. `geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-001.dcm`
2. `geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-002.dcm`
3. `geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-001.dcm`
4. `geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-002.dcm`

UIDs and whole-file hashes remain deterministic functions of the selected run
seed rather than constants in this note. Generation shall reopen and hash all
four source files and bind their exact per-run Study, Series, Frame of
Reference, SOP Class, SOP Instance, geometry, and complete-instance selection
before writing the Presentation State.

## Locked IOD And Module Contract

PS3.3 Table A.33.4-1 makes Patient, General Study, General Series,
Presentation Series, General Equipment, Presentation State Identification,
Presentation State Blending, Displayed Area, Palette Color Lookup Table, ICC
Profile, and SOP Common mandatory. The recipe shall include those Modules. It
shall omit the optional clinical-trial, Patient Study, Graphic Group, and
Specimen Modules. Graphic Annotation, Graphic Layer, and Spatial
Transformation are absent because their conditions are false.

Unlike Advanced Blending Presentation State, this IOD does not include the
Frame of Reference or Common Instance Reference Modules. The shared source
Frame of Reference and identical geometry establish the spatial relationship,
but Frame of Reference UID `(0020,0052)` and Position Reference Indicator
`(0020,1040)` shall not be copied into this Presentation State.

The Softcopy Presentation LUT, standalone VOI LUT, and standalone Softcopy VOI
LUT Modules shall not be present. Overlay Plane, Overlay Activation, Display
Shutter, and Bitmap Display Shutter Modules shall not be present. The object
contains no Pixel Data.

The Presentation State copies source Patient and Study identity and uses a
distinct Presentation Series UID. Modality is `PR`, Series Number is `81`,
Instance Number is `1`, and Laterality is `R`. Presentation Creation Date and
Time are `20260101` and `000000`; Content Label is `DTSBLEND`; Content
Description is `Synthetic DTSBLEND presentation state`; and Content Creator
Name is `DTS^Generator`. Equipment identity is deterministic and independent
of the host, locale, network, or clock.

## Two-Set Blending Topology

Presentation State Blending Sequence `(0070,0402)` is SQ VM 1 with exactly two
Items. PS3.3 does not make Item order carry the underlying/superimposed
semantics, so the recipe adds a deterministic order and requires the following
exact Items:

1. source Series ordinal 1 is first and has Blending Position `(0070,0405)` CS
   VM 1 value `UNDERLYING`, the exact source Study UID, exactly one Referenced
   Series Sequence `(0008,1115)` Item for source Series 1, and exactly its two
   Referenced Image Sequence `(0008,1140)` CT SOP references in slice order;
2. source Series ordinal 2 is second and has Blending Position CS VM 1 value
   `SUPERIMPOSED`, the same source Study UID, exactly one Referenced Series
   Sequence Item for source Series 2, and exactly its two CT SOP references in
   slice order.

No Referenced Frame Number is present because each referenced single-frame CT
is selected as a complete Instance. The Modality LUT Macro in each Item locks
the source CT transformation as Rescale Intercept `(0028,1052)` `-1024`,
Rescale Slope `(0028,1053)` `1`, and Rescale Type `(0028,1054)` `HU`. Softcopy
VOI LUT Sequence `(0028,3110)` is absent because this recipe applies no VOI
transformation. Referenced Spatial Registration Sequence `(0070,0404)` is
absent because the two source Series already share a Frame of Reference and
the same sampled geometry.

Relative Opacity `(0070,0403)` is FL VM 1 with exact value `0.5`. Strict
validation shall reject any non-finite value or value outside the inclusive
range zero through one, a missing or duplicate Blending Position, a sequence
cardinality other than two, redirected source identities, reordered source
Images, or an unexpected frame, VOI, or registration reference.

## Displayed Area, Palette, And ICC Contract

Displayed Area Selection Sequence `(0070,005A)` is SQ VM 1 with exactly one
Item applying to all four referenced Images. Referenced Image Sequence is
absent in this Item. Displayed Area Top Left Hand Corner `(0070,0052)` is SL
VM 2 value `[1,1]`; Displayed Area Bottom Right Hand Corner `(0070,0053)` is
SL VM 2 value `[2,2]`; Presentation Size Mode `(0070,0100)` is `SCALE TO FIT`;
and Presentation Pixel Aspect Ratio `(0070,0102)` is IS VM 2 value `[1,1]`.
Presentation Pixel Spacing and Presentation Pixel Magnification Ratio are
absent.

The mandatory Palette Color Lookup Table uses Red, Green, and Blue Palette
Color Lookup Table Descriptors `(0028,1101)` through `(0028,1103)`, each with
US VM 3 value `[256,0,16]`. Each corresponding OW Data value `(0028,1201)`
through `(0028,1203)` is the same exact 512-byte identity ramp: 256 unsigned
16-bit entries from `0x0000` through `0xFFFF` in increments of `0x0101`,
encoded little endian. Each channel has SHA-256
`f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5`.
Segmented palette data and Palette Color Lookup Table UID are absent. This
neutral palette deliberately locks the already qualified minimal valid
topology; a visibly pseudo-color palette is a later distinct semantic case,
not an unreviewed substitution in this recipe.

ICC Profile `(0028,2000)` is OB VM 1 and reuses the project's deterministic
736-byte synthetic sRGB profile. Its exact SHA-256 is
`8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef`.
The profile header has device class `scnr`, input color space `RGB `,
connection space `XYZ `, and signature `acsp`; DICOM Color Space `(0028,2002)`
is CS VM 1 value `SRGB`. Validation owns complete palette and profile byte
identity, not only descriptor or header recognition.

## KB Query And Locked Local Evidence

- Query: `dicom-kb lookup uid BlendingSoftcopyPresentationStateStorage --edition 2026b`
- Edition: 2026b
- Result: `1.2.840.10008.5.1.4.1.1.11.4`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the KB registry evidence proves the SOP Class UID but does not
  expose the IOD module table, nested blending topology, palette payload,
  displayed-area selection, or reference closure required by this recipe.

The exact contract is anchored in the locally locked 2026b evidence named by
`standards.lock.json`: PS3.3 A.33.4 and Table A.33.4-1 (IOD and mandatory
Modules), C.11.14 and Table C.11.14-1 (two image sets, transformations,
positions, opacity, and optional registration), C.11.11 (Presentation State
Relationship Macro), C.10.4 (Displayed Area), C.7.9 (Palette Color Lookup
Table), C.11.10 (content identity), C.11.15 (ICC), and Tables 10-3 and 10-12
(SOP references and content identity). PS3.4 Table B.5-1 identifies the
Storage SOP Class. PS3.6 Tables A-1 and 6-1 lock the UID and data element
VR/VM definitions.

The repository lock records official PS3.3, PS3.4, and PS3.6 source artifacts
as `unavailable_not_downloaded`; none is committed. The independently locked
validator cache used for this check is identified by
`conformance-backends/dicom-validator/standard-lock.json`: PS3.3 DocBook
SHA-256
`4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`,
PS3.4 SHA-256
`8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`,
and PS3.6 SHA-256
`512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`.
These locked local identities, rather than an unpinned web page, are the
standards evidence for this decision.

## Prototype And Independent Validator Evidence

The temporary candidate implements the exact topology above. The final
4,128-byte prototype has SHA-256
`b6382bbc750feb18f25d3450ea14cf65aa5344950ee69c7e900926e6948056d4`.
DCMTK 3.7.0 `dcmdump` parsed it successfully. Locked dicom3tools `dciodvfy
-new` recognized `BlendingSoftcopyPresentationState` and emitted no errors or
warnings. The separate, `uv`-locked `dicom-validator` 0.8.2 presentation-state
adapter reported `Passed` with zero errors against the exact 2026b
definitions.

Qualification mutations show why strict Rust validation remains mandatory.
`dciodvfy` rejected a Blending Sequence containing only one Item, but the
`uv`-locked secondary validator accepted it. Both IOD validators accepted two
Items with duplicate `UNDERLYING` Blending Positions and accepted Relative
Opacity outside the standard zero-through-one range. Earlier qualification
also showed that the secondary validator missed absent conditional palette
data. These are tool limitations, not accepted semantics. No finding may be
silently allowlisted.

Strict Rust validation owns every cardinality, ordering, uniqueness, source
graph, complete-instance selection, rescale, opacity, displayed-area, palette,
ICC, and absence invariant. Independent IOD validation remains additive and
cannot replace project-owned semantics, DCMTK parsing, or isolated `dcentvfy`
reference closure over the Presentation State and all four source CT files.
Negative tests shall cover the observed validator gaps plus redirected Study,
Series, and SOP identities, reordered sources, wrong rescale values, unexpected
frame/VOI/registration references, wrong displayed area, corrupt palette or
ICC bytes, forbidden modules, and Pixel Data.

## Decision Checkpoint Audit

Proceeding with this native slice triggers no Section 11 decision checkpoint
in `docs/coverage-expansion-plan.md`:

- Python is not made mandatory for generation or any existing profile; the
  case-scoped secondary validator is an independently locked conformance
  capability and default tests do not invoke it.
- This recommendation does not select a long-term external-backend runtime
  manager. The user has separately adopted `uv` for the already locked Python
  conformance environment.
- The project explicitly authorized selecting and locking another independent
  IOD validator, and the exact-case presentation-state adapter already covers
  this SOP Class in addition to `dciodvfy`.
- The recipe is lossless and introduces no certificates, keys, stress job,
  protocol-conformance rule, or change to the meaning or inclusion rules of
  `all`.

## Manifest, Validation, Report, And Acceptance Contract

`expected_blending_presentation_state` shall bind:

- all four source paths, hashes, Study, Series, Frame of Reference, SOP Class,
  SOP Instance, 2 by 2 geometry, orientation, positions, ordering, and
  complete-instance selection;
- exact Presentation State identity, same-Study and different-Series
  relationships, content identity, and creation values;
- two ordered Blending Items, exact `UNDERLYING` and `SUPERIMPOSED` positions,
  source Series and SOP references, rescale values, and absent VOI and
  registration references;
- exact opacity `0.5`, displayed-area geometry and mode, exact palette
  descriptors, bytes and hashes, exact ICC bytes and header, `SRGB`, forbidden
  module absence, and Pixel Data absence.

JSON and Markdown reports should expose source-series and image counts, source
closure, ordered blending positions, opacity, per-item rescale and optional
transform absence, displayed-area geometry and mode, palette descriptors,
channel hashes and storage form, ICC size/hash/color space, forbidden-module
absence, pixel absence, and unresolved external-validator findings.

## Project Action

- Current registry status: planned and `semantic_stable`.
- Current registry provider: external backend `dcmtk`.
- Current registry blockers: `backend_contract_unimplemented` and
  `independent_iod_validator_unavailable`.
- Recommended next provider: `rust_native`; DICOM-rs can encode the required
  nested Sequences, OW palette payloads, FL opacity, and exact ICC OB payload
  deterministically, while both IOD validators remain independent of the
  generator.
- A separate provider-selection commit should replace the stale provider and
  blockers with `rust_native` and exactly `recipe_unimplemented`; this evidence
  commit intentionally does not change registry state.
- Promotion requires strict Rust validation, DCMTK parsing, isolated entity
  closure, both IOD validators, deterministic generation, schemas, reports,
  tests, and documentation. No new finding may be silently allowlisted.
- Should become KB patch: yes; expose Table A.33.4-1, C.11.14, palette and
  displayed-area requirements, and blending reference semantics as structured
  2026b queries.
