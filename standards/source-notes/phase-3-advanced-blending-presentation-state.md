# Phase 3 Advanced Blending Presentation State Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `derived/presentation-state/advanced_blending`
- Recipe ID: `derived_presentation_state_advanced_blending`
- Selected provider: `rust_native`
- Source case: `geometry/ct/multiseries_shared_frame_of_reference`
- Output: Advanced Blending Presentation State Storage
  (`1.2.840.10008.5.1.4.1.1.11.8`), Explicit VR Little Endian
- Recommended manifest field:
  `expected_advanced_blending_presentation_state`

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

PS3.3 Table A.33.7-1 makes Patient, General Study, General Series,
Presentation Series, Frame of Reference, General Equipment, Enhanced General
Equipment, Presentation State Identification, Advanced Blending Presentation
State, Advanced Blending Presentation State Display, ICC Profile, Common
Instance Reference, and SOP Common mandatory. The native recipe includes
those Modules. It omits the optional clinical-trial, patient-study,
displayed-area, graphic-annotation, graphic-group, and specimen Modules.
Spatial Transformation and Graphic Layer are absent because their conditions
are false. The output contains no Pixel Data.

The Presentation State copies the source Patient and Study identity, uses the
shared source Frame of Reference UID, and has a distinct Presentation Series
UID. Frame of Reference UID `(0020,0052)` is UI VM 1 and Position Reference
Indicator `(0020,1040)` is a present empty LO Type 2 value. Modality is `PR`,
Series Number is `80`, Instance Number is `1`, and Laterality is `R`.
Presentation Creation Date and Time are `20260101` and `000000`; Content Label
is `DTSADVBLEND`; Content Description is
`Synthetic DTSADVBLEND presentation state`; and Content Creator Name is
`DTS^Generator`. General and Enhanced General Equipment identities are
deterministic and do not depend on the host, locale, network, or clock.

## Two-Input Advanced Blending Topology

Advanced Blending Sequence `(0070,1B01)` is SQ VM 1 with exactly two Items,
ordered by Blending Input Number. The locked Items are:

1. input 1: Blending Input Number `(0070,1B02)` US VM 1 value `1`, the exact
   source Study UID, source Series ordinal 1 UID, and Referenced Image Sequence
   `(0008,1140)` with exactly the two ordinal-1 CT SOP references in slice
   order; Time Series Blending `(0070,1B07)` CS VM 1 is `FALSE`; Geometry for
   Display `(0070,1B08)` CS VM 1 is `TRUE`;
2. input 2: the same structure with input number `2`, source Series ordinal 2,
   its two CT SOP references in slice order, Time Series Blending `FALSE`, and
   Geometry for Display `FALSE`.

Study Instance UID and Series Instance UID in each input are UI VM 1.
Referenced Image Sequence is SQ VM 1 and each nested Referenced SOP Class UID
and Referenced SOP Instance UID is UI VM 1. No Referenced Frame Number is
present because every referenced single-frame CT is selected as a complete
Instance. Referenced Spatial Registration, optical-path selection, Softcopy
VOI LUT, Palette Color Lookup Table, threshold, and other optional input
transformations are absent. Input numbers are unique ordinal values starting
at 1 and increasing by 1. Exactly one input has Geometry for Display `TRUE`,
and that input supplies the output geometry.

## Final Display Operation

Pixel Presentation `(0008,9205)` is CS VM 1 value `TRUE_COLOR`. Blending
Display Sequence `(0070,1B04)` is SQ VM 1 with exactly one Item. Its Blending
Display Input Sequence `(0070,1B03)` is SQ VM 1 with exactly two Items in the
significant order input `1`, then input `2`; each nested Blending Input Number
is US VM 1. Blending Mode `(0070,1B06)` is CS VM 1 value `EQUAL`.

Relative Opacity `(0070,0403)` is absent because the mode is not `FOREGROUND`.
The display Item's optional output Blending Input Number is also absent,
thereby marking this sole operation as the final displayed output. Strict
validation shall reject duplicate, missing, non-ordinal, or dangling input
numbers, reversed display-input order, more than one geometry source, a mode
other than `EQUAL`, opacity in this recipe, and an output number that would
turn the sole operation into an intermediate result.

## ICC And Reference-Closure Contract

ICC Profile `(0028,2000)` is OB VM 1 and reuses the project's deterministic
736-byte synthetic sRGB profile. Its exact SHA-256 is
`8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef`.
The profile header has device class `scnr`, input color space `RGB `,
connection space `XYZ `, and signature `acsp`; DICOM Color Space
`(0028,2002)` is CS VM 1 value `SRGB`. Validation owns complete byte identity,
not only presence or header recognition.

The Common Instance Reference Module's Referenced Series Sequence
`(0008,1115)` contains exactly two Items in source-series order. Each Item has
the exact Series Instance UID and exactly two Referenced Instance Sequence
`(0008,114A)` Items in source slice order. Those four SOP Class and SOP
Instance identities exactly mirror the four Advanced Blending input
references. Studies Containing Other Referenced Instances Sequence is absent
because the Presentation State and all inputs share one Study. Redirected,
dangling, duplicated, omitted, reordered, or cross-Study references are
errors, even if an external validator accepts the file.

## KB Query And Locked Local Evidence

- Query: `dicom-kb lookup uid AdvancedBlendingPresentationStateStorage --edition 2026b`
- Edition: 2026b
- Result: `1.2.840.10008.5.1.4.1.1.11.8`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the KB registry evidence proves the SOP Class UID but does not
  expose the IOD module table, nested blending topology, display-operation
  graph, reference closure, or ICC payload rules required by this recipe.

The exact contract is anchored in the locally locked 2026b evidence named by
`standards.lock.json`: PS3.3 A.33.7 and Table A.33.7-1 (IOD and mandatory
Modules), C.11.33 and Table C.11.33-1 (inputs, references, input numbering,
time-series and display geometry flags), C.11.34 and Table C.11.34.1-1 (pixel
presentation and display operation), C.11.10 (content identity), C.11.15
(ICC), C.12.2 (Common Instance Reference), C.7.4.1 (Frame of Reference), and
Tables 10-3 and 10-12 (SOP references and content identity). PS3.4 Table B.5-1
identifies the Storage SOP Class. PS3.6 Tables A-1 and 6-1 lock the UID and
data element VR/VM definitions.

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
3,044-byte prototype has SHA-256
`3e3f753545385fb448f3c5eb8618977663c7230158736fc943b9708ed62320d1`.
DCMTK 3.7.0 `dcmdump` parsed it successfully, isolated `dcentvfy` over the
Presentation State and all four source CT files was silent, and the separate,
`uv`-locked `dicom-validator` 0.8.2 presentation-state adapter reported
`Passed` with zero errors against the exact 2026b definitions.

`dciodvfy -new` recognized the Advanced Blending Softcopy Presentation State
but emitted two findings: it says Frame of Reference UID `(0020,0052)` and
Position Reference Indicator `(0020,1040)` are not present in the standard
IOD. This contradicts the locked PS3.3 Table A.33.7-1, which makes the Frame
of Reference Module mandatory. The attributes remain in the recipe and the
two warnings remain unresolved independent-conformance findings. They are not
allowlisted, suppressed, reclassified, or used to weaken the official module
contract.

Qualification mutations show why strict Rust validation remains mandatory.
Both IOD validators accepted duplicate Advanced Blending Input Numbers, a
display Item that refers to a nonexistent input, and a Common Instance
Reference hierarchy missing one of the four source Instances. These are tool
limitations, not accepted semantics. The temporary prototypes, generated
DICOM, validator output, Python caches, and official source artifacts remain
outside git.

## Manifest, Validation, Report, And Acceptance Contract

`expected_advanced_blending_presentation_state` shall bind:

- all four source paths, hashes, Study, Series, Frame of Reference, SOP Class,
  SOP Instance, 2 by 2 geometry, orientation, positions, ordering, and
  complete-instance selection;
- exact Presentation State identity, same-Study/shared-Frame and
  different-Series relationships, content identity, and creation values;
- two ordered Advanced Blending Items, exact input numbers, source Series and
  SOP references, `FALSE/FALSE` time-series flags, and `TRUE/FALSE` geometry
  flags;
- one final display Item, ordered inputs `[1,2]`, `EQUAL` mode, absent opacity,
  and absent output input number;
- `TRUE_COLOR`, the exact ICC bytes and header, `SRGB`, the mirrored Common
  Instance Reference hierarchy, optional-transform absence, and Pixel Data
  absence.

Strict Rust validation owns all cardinality, ordering, uniqueness, graph,
source closure, geometry, ICC, and absence invariants. External IOD validation
is additive and cannot replace these checks. Negative tests shall cover every
semantic gap observed above plus redirected source Study/Series/SOP values,
reordered source Images, wrong time-series or geometry flags, wrong pixel
presentation or blend mode, added opacity or output number, corrupt ICC,
cross-Study common references, forbidden optional transforms, and Pixel Data.

JSON and Markdown reports should expose source-series and image counts, source
closure, input numbers and ordering, time-series and geometry flags, display
operation count and input order, final-output status, blend mode, ICC size and
hash, Color Space, Common Instance Reference closure, optional-module
presence, pixel absence, and unresolved external-validator findings.

## Project Action

- Registry status: planned; retain `semantic_stable` until two generated runs
  prove byte identity.
- Registry provider: `rust_native`; DICOM-rs can encode the required nested
  Sequences and exact ICC OB payload deterministically while the `uv`-locked
  pydicom/dicom-validator route remains an independent implementation.
- Registry blocker: exactly `recipe_unimplemented`; source, standards,
  provider, and independent IOD route are selected and locked.
- Promotion requires strict Rust validation, DCMTK parsing, isolated entity
  closure, both IOD validators, deterministic generation, schemas, reports,
  tests, and documentation. The two contradictory `dciodvfy` findings remain
  visible unless the independent tool or locked standard evidence resolves
  them. No new finding may be silently allowlisted.
- Should become KB patch: yes; expose Table A.33.7-1, C.11.33, C.11.34,
  blending-graph semantics, and reference closure as structured 2026b queries.
