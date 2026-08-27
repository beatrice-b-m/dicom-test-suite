# Phase 3 Linked RT Plan And RT Image Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case IDs: `non-image/rt/plan_linked`, `non-image/rt/image_linked`
- Recipe IDs: `non_image_rt_plan_linked`, `non_image_rt_image_linked`
- Recommended provider: `rust_native`
- Outputs: RT Plan Storage (`1.2.840.10008.5.1.4.1.1.481.5`) and RT
  Image Storage (`1.2.840.10008.5.1.4.1.1.481.1`), Explicit VR Little
  Endian
- Recommended manifest fields: `expected_rt_plan`, `expected_rt_image`

These are synthetic reference, plan-display, and image-display compatibility
objects, not a clinically deliverable treatment. UIDs are deterministic
functions of the run seed. Dates, times, patient, equipment, beam geometry,
and pixels are fixed recipe inputs independent of the host, locale, network,
and clock.

## Locked Dependency And Reference Topology

Generation order is strictly:

```text
Enhanced CT -> existing RT Structure Set -> existing RT Dose
                                            |             |
                                            +-> RT Plan <-+
                                                  |
                                                  v
                                               RT Image
```

The source CT is
`enhanced/ct/multiframe_shared_perframe_explicit_le`; the existing objects are
`non-image/rt/structure_set_single_roi_explicit_le` and
`non-image/rt/dose_grid_u16_explicit_le`. The new Plan shares their Patient,
Study Instance UID, and Frame of Reference UID. With RT Plan Geometry
`PATIENT`, it directly references exactly one existing RT Structure Set. It
also includes the standard-optional Referenced Dose Sequence with exactly one
Item identifying the existing RT Dose. The Dose remains
`DoseSummationType=RECORD`; its bytes, manifest contract, and references shall
not change and it shall not acquire a reverse Plan reference.

The Plan Study ID is `DTS-RTSTRUCT`, matching the existing Structure Set
rather than introducing a new Study-level value. The immutable enhanced CT and
Dose retain their historical Study IDs `DTS-ECT` and `DTS-RTDOSE`; those
differences are visible in the entity-verification evidence below and are not
normalized during qualification.

The new Image is generated only after the Plan identity is registered. It
shares the same Patient, Study, and Frame of Reference, directly references
that exact Plan, and identifies Referenced Beam Number `1`, Referenced Fraction
Group Number `1`, and Fraction Number `1`. Every direct edge in the manifest
binds role, source case and relative path, source SHA-256, Study, Series, SOP
Class, SOP Instance, and Frame of Reference identities. Series and SOP Instance
UIDs are distinct for all five objects. No reference may be inferred from
filename or generation order alone.

## Locked RT Plan IOD And Module Contract

PS3.3 A.20 and Table A.20.3-1 define the RT Plan IOD. Patient, General Study,
RT Series, General Equipment, RT General Plan C.8.8.9, and SOP Common are
mandatory. This recipe also includes Frame of Reference, RT Fraction Scheme
C.8.8.13, and RT Beams C.8.8.14. RT Prescription, RT Tolerance Tables, RT
Patient Setup, RT Brachy Application Setups, Approval, Clinical Trial, and
Common Instance Reference Modules are absent. Pixel Data and every Image Pixel
Module attribute are absent.

RT Plan Label `(300A,0002)` is `DTS_PLAN`, RT Plan Date `(300A,0006)` is
`20260101`, RT Plan Time `(300A,0007)` is `000000`, and RT Plan Geometry
`(300A,000C)` is `PATIENT`. Referenced Structure Set Sequence `(300C,0060)`
and Referenced Dose Sequence `(300C,0080)` each have exactly one Item with the
locked upstream SOP Class and SOP Instance UIDs. Referenced RT Plan Sequence
is absent.

The meaningful minimum is one fraction group and one beam, not a zero-beam
Plan. Fraction Group Sequence `(300A,0070)` has exactly one Item: Fraction
Group Number `1`, Number of Fractions Planned `1`, Number of Beams `1`, Number
of Brachy Application Setups `0`, and exactly one Referenced Beam Sequence Item
for beam `1`.

Beam Sequence `(300A,00B0)` has exactly one Item. Treatment Machine Name is
`DTS_LINAC`, Primary Dosimeter Unit is `MU`, Source-Axis Distance is `1000`,
Beam Number is `1`, Beam Name is `DTS_STATIC_AP`, Beam Type is `STATIC`,
Radiation Type is `PHOTON`, and Treatment Delivery Type is `TREATMENT`.
Number of Wedges, Number of Compensators, Number of Boli, and Number of Blocks
are all `0`; their conditional Sequences are absent. The ordered Beam Limiting
Device Sequence has exactly two Items: `X` then `Y`, each with one jaw pair and
Source to Beam Limiting Device Distance `500`.

Number of Control Points `(300A,0110)` is `2`, Final Cumulative Meterset Weight
`(300A,010E)` is `1`, and Control Point Sequence `(300A,0111)` contains exactly
two ordered Items. Control Point `0` fixes Nominal Beam Energy `6`, the X and Y
jaw positions to `-50\\50`, Gantry Angle `0` with rotation `NONE`, Beam Limiting
Device Angle `0` with rotation `NONE`, Patient Support Angle `0` with rotation
`NONE`, Table Top Vertical, Longitudinal, and Lateral Positions `0`, Table Top
Pitch and Roll Angles `0` with rotation `NONE`, Isocenter Position `0\\0\\0`,
and Cumulative Meterset Weight `0`. Control Point `1` contains its index and
Cumulative Meterset Weight `1`; unchanged geometry is absent and inherited
from Control Point `0` under C.8.8.14.1. A generator must not replace this with
one control point, a dynamic beam, or a zero-beam plan.

## Locked RT Image IOD, Geometry, And Pixel Contract

PS3.3 A.17 and Table A.17.3-1 define the RT Image IOD. Patient, General Study,
RT Series, General Equipment, General Acquisition, General Image, Image Pixel,
RT Image C.8.8.2, and SOP Common are mandatory. This recipe also includes
Frame of Reference. Patient Study, Contrast/Bolus, Cine, Multi-frame, Modality
LUT, VOI LUT, Approval, Clinical Trial, Frame Extraction, and Common Instance
Reference Modules are absent.

Image Type `(0008,0008)` is `DERIVED\\SECONDARY\\DRR`, so the conditional
Reported Values Origin required for `SIMULATOR` or `PORTAL` is absent.
Conversion Type `(0008,0064)` is `WSD`. RT Image Label `(3002,0002)` is
`DTS_DRR`, RT Image Plane `(3002,000C)` is `NORMAL`, and the conditional RT
Image Orientation for `NON_NORMAL` is absent. X-Ray Image Receptor Angle is
`0`, Image Plane Pixel Spacing is `1\\1`, RT Image Position is `-1.5\\1.5`,
Radiation Machine Name is `DTS_LINAC`, Radiation Machine SAD is `1000`, RT
Image SID is `1500`, and Primary Dosimeter Unit is `MU`. Isocenter Position,
Patient Position, Fluence Map Sequence, Exposure Sequence, overlays, and
encapsulated or lossy pixel attributes are absent.

The single-frame image is exactly 4 rows by 4 columns with Samples per Pixel
`1`, Photometric Interpretation `MONOCHROME2`, Bits Allocated `8`, Bits Stored
`8`, High Bit `7`, and Pixel Representation `0`. Native Pixel Data uses OB and
contains these 16 row-major bytes with no value-field padding:

```text
00 11 22 33 44 55 66 77 88 99 aa bb cc dd ee ff
```

Equivalently, zero-based pixel `(r,c)` is `17 * (4*r + c)`. The builder
prototype shall derive and lock the payload and independently decoded pixel
SHA-256 without changing this formula or shape.

## KB Query And Official Source Evidence

- Queries: `dicom-kb lookup uid RTPlanStorage --edition 2026b` and
  `dicom-kb lookup uid RTImageStorage --edition 2026b`
- Edition: 2026b
- Results: `1.2.840.10008.5.1.4.1.1.481.5` and
  `1.2.840.10008.5.1.4.1.1.481.1`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the KB rows prove the SOP Class UIDs but do not expose the full
  IOD module tables, Type 1/1C/2 conditions, control-point inheritance, Image
  Type conditions, or reference topology needed by these recipes.

The exact contract is anchored in PS3.3 A.20 and Table A.20.3-1, A.17 and
Table A.17.3-1, C.8.8.9, C.8.8.13, C.8.8.14 and C.8.8.14.1, C.8.8.2,
C.7.6.3, C.7.4.1, and Tables 10-11 and 10-12; PS3.4 Table B.5-1; and PS3.6
Tables A-1 and 6-1. The repository lock records official artifacts as
`unavailable_not_downloaded`. The independently locked validator cache pins
official PS3.3 DocBook SHA-256
`4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`,
PS3.4 SHA-256
`8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`,
PS3.6 SHA-256
`512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`,
and generated IOD definitions SHA-256
`ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683`.
Official source artifacts and generated definitions remain outside git.

## Manifest, Strict Validation, And Report Contract

`expected_rt_plan` shall bind the Plan identity, shared Study and Frame of
Reference, exact ordered upstream references and source hashes, label/date/time
and geometry, fraction-group cardinality, beam and device definitions, zero
accessory counts and absences, both ordered control points, inheritance, and
meterset endpoints. Its image and pixel fields are null.

`expected_rt_image` shall bind the Image identity, shared Study and Frame of
Reference, exact Plan/beam/fraction linkage, RT Image Type and geometry, native
storage, dimensions, formula, bytes, payload hash, decoded values and hash,
and all locked absences. Strict validation reopens and hashes every source
before writing and again validates exact tag VR, VM, value, Sequence
cardinality and order; reference identity and closure; control-point math and
inheritance; pixel length, bytes, extrema and hashes; and the absence contract.
Prechecked cardinalities are required before pairwise iteration.

Negative tests shall cover a missing Plan Label; `PATIENT` geometry without
the Structure Set; wrong, dangling, duplicated, or reordered Structure/Dose
references; fraction/beam cardinality mismatch; a dangling or duplicate Beam
Number; missing zero accessory counts; changed device order or jaw positions;
wrong control-point count, index, order, isocenter, or first/final meterset;
and wrong Study or Frame of Reference. Image controls shall cover missing Image
Type, label, or plane; `NON_NORMAL` without orientation; `PORTAL` without
Reported Values Origin; wrong Plan, beam, or fraction; pixel shape/length,
Bits Stored, High Bit, Pixel Representation, spacing, position, SAD, or SID;
one changed Pixel Data byte; and wrong Study or Frame of Reference. Graph
controls remove each dependency from the isolated entity-validation corpus
and alter a manifest source hash. No finding may be silently allowlisted.

JSON and Markdown reports shall expose the Plan label and geometry, fraction
and beam counts and identifiers, beam type, control-point order and meterset
range, Structure/Dose identities and closure; and Image Type, label, plane,
spacing, position, dimensions, bit contract, payload hash, Plan/beam/fraction
linkage, SAD/SID, pixel disposition, and external-validator disposition.
Planned cases retain explicit null expectations until promotion.

## Independent Validator Qualification And Acceptance

Locked dicom3tools `dciodvfy -new` remains the primary IOD validator. The
existing independently implemented `uv`-locked `dicom-validator` 0.8.2 with
hash-locked official 2026b definitions shall be empirically qualified and run
additively as a case-scoped secondary IOD opinion. Locked DCMTK `dcmdump +fo`
provides independent parsing. Locked dicom3tools `dcentvfy -f` runs on an
isolated file list containing exactly the CT, Structure Set, Dose, Plan, and
Image to prove reference closure. A new exact-case independent image decoder,
preferably locked DCMTK `dcm2img` producing 8-bit PGM, shall prove the 4 by 4
shape and exact ordered pixels; parsing alone is not pixel-decode evidence.

### Corrected RT Plan prototype qualification

The corrected RT Plan prototype has instance SHA-256
`e9337a6c46fe85b56f1f563120dd3caf56ea1335355792db42386db959be6db2`
and Study ID `DTS-RTSTRUCT`. Locked `dciodvfy -new` identified `RTPlan` with
exit code zero. The uv-locked `dicom-validator` 0.8.2 adapter selected the
2026b RT Plan IOD and returned `Passed` with zero errors. DCMTK
`dcmdump +fo` parsed the exact Part 10 file. These Plan results do not imply the
separate RT Image qualification recorded below.

All 20 Plan controls remained parseable by `dcmdump`; parsing is not semantic
detection. The empirical detection boundary is:

| RT Plan mutation | `dciodvfy` | `dicom-validator` | `dcentvfy` additive reference finding | Required owner when missed |
|---|---|---|---|---|
| Missing Plan Label | detected | detected | no | IOD validators |
| `PATIENT` without Structure Set | detected | detected | no | IOD validators |
| Wrong Structure reference SOP Class | missed | missed | no | strict Rust |
| Dangling Structure reference SOP Instance | missed | missed | detected | entity closure |
| Duplicated Structure/Dose references | missed | missed | no | strict Rust |
| Reordered Structure/Dose identities | missed | missed | no | strict Rust |
| Fraction/beam cardinality mismatch | missed | missed | no | strict Rust |
| Dangling Beam Number | missed | missed | no | strict Rust |
| Duplicate Beam Number | missed | missed | no | strict Rust |
| Missing all four zero accessory counts | detected | detected | no | IOD validators |
| Reversed X/Y device order | missed | missed | no | strict Rust |
| Changed jaw positions | missed | missed | no | strict Rust |
| Wrong control-point count | detected | missed | no | `dciodvfy` and strict Rust |
| Wrong control-point index | missed | missed | no | strict Rust |
| Reversed control-point order | missed | missed | no | strict Rust |
| Wrong isocenter | missed | missed | no | strict Rust |
| Wrong first meterset | missed | missed | no | strict Rust |
| Wrong final meterset | missed | missed | no | strict Rust |
| Wrong Study Instance UID | missed | missed | no | strict Rust |
| Wrong Frame of Reference UID | missed | missed | no | strict Rust |

The exact four-file CT/Structure Set/Dose/Plan `dcentvfy -f` baseline is not
silent or clean: it reports Dose Study ID `DTS-RTDOSE` versus Plan/Structure
Set `DTS-RTSTRUCT`, and enhanced CT Study ID `DTS-ECT` versus Plan/Structure
Set `DTS-RTSTRUCT`. These two immutable upstream diagnostics remain visible
and unallowlisted. For the RT Plan reference-closure control, acceptance means
that an exact isolated run produces no *additive* missing or dangling reference
finding beyond those two Study ID diagnostics. The dangling Structure Set SOP
mutation added `Missing SOPInstanceUID that was referenced`; the valid Plan
did not. A zero `dcentvfy` exit code is not claimed, and a run that omits files
or supplies a directory where `-f` expects a file list is invalid evidence.

### Corrected RT Image prototype qualification

The corrected RT Image prototype has instance SHA-256
`460d525ab06aaf74df963029f3ab39c2536e4e1c5bf4b75fcf16b500382db20c`
and the containing generated manifest has SHA-256
`b061e5f654eb426bbab0da9cce0ac945aadcf3cf506182eb6bf33acd3d7a3659`.
Locked `dciodvfy -new` identified `RTImage` with exit code zero. The uv-locked
`dicom-validator` 0.8.2 adapter selected the 2026b RT Image IOD and returned
`Passed` with zero errors. `dcmdump +fo` parsed the exact 1,416-byte Part 10
file.

The locked `dcmtk-dcm2img-rt-image` route produced one P2 image with 4 rows,
4 columns, maximum value 255, and the exact ordered samples `0, 17, ... 255`.
Its separate `dcmdump +W` extraction produced exactly one 16-byte native OB
value. The decoded samples and raw value both have SHA-256
`a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811`.
Integrated conformance run ID
`d0d78ffccf44218a27944cf1b80dec63c8afa7162b0e085532feb51706a04714`
has run JSON SHA-256
`87846c587a4f721b90624008a3f7abfc9ae70a31d83e28449e82528b408b3ce7`,
stable instance key
`146c7c29a15a573ab0348addd424b8e88547985f54d687bb6e793dcd88ac71d4`,
and pixel sidecar SHA-256
`071b32384d1648222424f77a0392e90ca11d6e51df0d5bd1fc0a241754bec1fc`.
Strict verification reports 211 older or unrelated failures and zero accepted
findings; the RT Image IOD, parser, and pixel routes themselves are clean.

All 20 Image controls remained parseable by `dcmdump`; parsing is not semantic
detection. The empirical detection boundary is:

| RT Image mutation | `dciodvfy` | `dicom-validator` | DCMTK pixel route | `dcentvfy` additive reference finding | Required owner when missed |
|---|---|---|---|---|---|
| Missing Image Type | detected | detected | missed | no | IOD validators and strict Rust |
| Missing RT Image Label | detected | detected | missed | no | IOD validators and strict Rust |
| Missing RT Image Plane | detected | detected | missed | no | IOD validators and strict Rust |
| `NON_NORMAL` without orientation | detected | detected | missed | no | IOD validators and strict Rust |
| `PORTAL` without Reported Values Origin | detected | missed | missed | no | `dciodvfy` and strict Rust |
| Wrong Plan SOP Instance UID | missed | missed | missed | detected | entity closure and strict Rust |
| Wrong referenced Beam Number | missed | missed | missed | no | strict Rust |
| Wrong referenced Fraction Group Number | missed | missed | missed | no | strict Rust |
| Wrong pixel shape | detected | missed | detected | no | `dciodvfy`, pixel route, and strict Rust |
| Wrong pixel length | detected | missed | detected | no | `dciodvfy`, pixel route, and strict Rust |
| Wrong Bits Stored | detected | detected | detected | no | IOD validators, pixel route, and strict Rust |
| Wrong High Bit | detected | missed | detected | no | `dciodvfy`, pixel route, and strict Rust |
| Wrong Pixel Representation | detected | detected | detected | no | IOD validators, pixel route, and strict Rust |
| Wrong spacing | missed | missed | missed | no | strict Rust |
| Wrong position | missed | missed | missed | no | strict Rust |
| Wrong SAD | missed | missed | missed | no | strict Rust |
| Wrong SID | missed | missed | missed | no | strict Rust |
| Changed Pixel Data byte | missed | missed | detected | no | pixel route and strict Rust |
| Wrong Study Instance UID | missed | missed | missed | no new missing/dangling finding | strict Rust |
| Wrong Frame of Reference UID | missed | missed | missed | no | strict Rust |

Thus `dciodvfy` detected 10 of 20 mutations, the secondary IOD validator
detected 6 of 20, and the exact pixel route detected 6 of 20. Isolated
`dcentvfy` uniquely added missing-SOP evidence for the wrong Plan UID. Strict
Rust retains the full exact contract, including every semantic value missed by
the independent tools.

The exact five-file CT/Structure Set/Dose/Plan/Image entity baseline retains
the same two immutable Study ID diagnostics recorded above and no missing or
dangling reference finding. Removing the CT, Structure Set, Dose, or Plan
individually added the expected missing-SOP finding or findings. Changing only
`expected_rt_image.plan_reference.source_sha256` to 64 lowercase zeroes was
also rejected after checking all 105 files:
`rt_image_plan_source_sha256: expected e9337a6c46fe85b56f1f563120dd3caf56ea1335355792db42386db959be6db2`.
This qualification step did not itself change registry status; promotion is a
separate dependency-ordered change.

Qualification begins with exact valid prototypes, executes every mutation
listed above in temporary storage, and records which tool detects each one.
IOD-validator misses remain explicit gaps owned by strict Rust validation,
`dcentvfy`, or the independent decoder; they are not accepted findings.
Promotion requires both objects to pass strict validation, the required
independent routes under their recorded acceptance rules, isolated entity
closure with no additive missing/dangling reference finding, integrated
conformance verification, two-run byte and manifest reproducibility, report
tests, and documentation. The immutable Study ID diagnostics do not authorize
an allowlist and cannot be hidden. Unavailable tooling remains explicit and
cannot silently reduce either case contract.

## Decision Checkpoint Audit

This evidence and the recommended native implementation trigger no Section 11
decision checkpoint in `docs/coverage-expansion-plan.md`:

- Python is not mandatory for generation or an existing profile. The
  case-scoped `uv` environment remains optional conformance tooling.
- The user adopted `uv` for the locked Python conformance environment and
  authorized selecting and locking another independent IOD validator; the
  secondary route supplements and never replaces `dciodvfy`.
- Both objects use native lossless storage and add no certificates, keys,
  stress job, protocol rule, or change to the meaning or inclusion of `all`.

Pause if implementation would make Python or DCMTK mandatory for existing
generation, select another long-term runtime manager, proceed without an
independent IOD opinion, introduce lossy RT pixels, or change profile
semantics. Evaluation of a current RT Radiation Set is a separate subsequent
decision after these legacy references are proven; it is not authorized by
this note.

## Project Action

- Current registry status: both cases are planned and `semantic_stable`.
- Current registry provider: external backend `dcmtk`.
- Current blockers: `backend_contract_unimplemented` and
  `independent_iod_validator_unavailable`.
- Recommended next provider: `rust_native` with `byte_stable` determinism;
  DICOM-rs already writes the required Sequences and native OB Pixel Data,
  while every conformance authority remains independent of generation.
- A separate provider-selection commit shall change only the two rows and
  reduce blockers to exactly `recipe_unimplemented`. This source-note commit
  intentionally leaves the registry unchanged.
- Existing RT Structure Set and RT Dose bytes and contracts are immutable
  inputs to this milestone.
- Should become KB patch: yes; expose structured 2026b RT Plan/RT Image module,
  conditional-attribute, control-point, and reference queries.
