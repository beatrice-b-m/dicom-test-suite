# Phase 3 Minimal RT Radiation Set Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case IDs:
  `non-image/rt/carm_photon_electron_radiation_minimal` and
  `non-image/rt/radiation_set_minimal`
- Recipe IDs:
  `non_image_rt_carm_photon_electron_radiation_minimal` and
  `non_image_rt_radiation_set_minimal`
- Recommended provider: `rust_native`
- Outputs: C-Arm Photon-Electron Radiation Storage
  (`1.2.840.10008.5.1.4.1.1.481.13`) followed by RT Radiation Set Storage
  (`1.2.840.10008.5.1.4.1.1.481.12`), Explicit VR Little Endian
- Recommended manifest fields: `expected_rt_radiation` and
  `expected_rt_radiation_set`

These are synthetic treatment-definition and metadata compatibility objects,
not an approved or clinically deliverable treatment. UIDs are deterministic
functions of the run seed. Dates, times, patient, equipment, coded concepts,
control points, and references are fixed recipe inputs independent of the
host, locale, network, and clock.

## Locked Dependency And Reference Topology

An RT Radiation Set cannot be implemented as a standalone instance. PS3.3
C.36.10 makes RT Radiation Sequence `(300A,0616)` Type 1 with one or more
Items, and C.36.10.1.2 requires every Item to reference a second-generation
RT Radiation SOP Instance. The existing RT Plan, RT Image, RT Dose, and RT
Structure Set cannot fill that role. The minimum complete graph is therefore:

```text
existing RT Structure Set <- existing RT Plan -> existing RT Dose
                                  |
                           definition source
                                  v
new C-Arm Photon-Electron Radiation (.481.13)
                                  |
                        exact radiation reference
                                  v
new RT Radiation Set (.481.12)
```

The C-Arm Radiation is a distinct registry case rather than an unregistered
support file. It shares the Plan's Patient, Study Instance UID, Study ID
`DTS-RTSTRUCT`, and patient Frame of Reference UID. The Radiation is generated
only after reopening and hashing the Plan. The Set is generated only after
reopening and hashing both the Plan and Radiation. All three use distinct
Series and SOP Instance UIDs.

The Radiation's Definition Source Sequence references exactly the RT Plan and
Referenced Beam Number `1`. The Set's Definition Source Sequence references
exactly that Plan. Its RT Radiation Sequence references exactly the new
Radiation, and its single Treatment Position Group references that Radiation
once and only once. Common Instance Reference mirrors every direct reference
by Study, Series, SOP Class, and SOP Instance. Manifest edges additionally
bind role, source case and relative path, source SHA-256, and patient Frame of
Reference identity. No edge may be inferred from filename or generation
order.

## Locked C-Arm Photon-Electron Radiation IOD Contract

PS3.3 A.86.1.5 and Table A.86.1.5-1 define the IOD. Patient, General Study,
General Series, Enhanced RT Series, General Equipment, Enhanced General
Equipment, Frame of Reference, General Reference, RT Delivery Device Common,
RT Radiation Common, C-Arm Photon-Electron Delivery Device, C-Arm
Photon-Electron Beam, SOP Common, Common Instance Reference, and Radiotherapy
Common Instance are mandatory. Patient Study and all Clinical Trial Modules
are absent. Modality is `RTRAD`; Series Number is `74`; Series, Instance
Creation, and Content Date/Time values are `20260101` and `000000`.
Referenced Performed Procedure Step Sequences and Treatment Session UID are
absent. Author Identification Sequence is present with zero Items.

Patient Name is `DTS^Synthetic^Patient001`, Patient ID is
`DTS-PATIENT-001`, Birth Date is `19700101`, and Sex is `O`. Referring
Physician Name and Accession Number are present empty. Position Reference
Indicator is present empty. General and Enhanced General Equipment identify
Manufacturer `dicom-test-suite`, Manufacturer's Model Name
`Native C-Arm Photon-Electron Radiation`, Device Serial Number
`DTS-LINAC-001`, and the package Software Versions value.

Enhanced Content Identification fixes User Content Label `(3010,0033)` to
`DTS_RADIATION` and Content Description `(0070,0081)` present empty. RT
Radiation Physical and Geometric Content Detail Flag `(300A,0638)` is
`IDENT_ONLY`, which expressly permits identifiable devices without a complete
physical and geometric definition. RT Record Flag `(300A,0639)` is `NO`.
This planning branch does not claim recorded delivery. RT Treatment Technique
Code Sequence contains exactly `(130102, DCM, "Static Beam")` from CID 9511.
Treatment Machine Special Mode, tolerance, and time-limit content are absent.

### Treatment device and coordinate contract

Treatment Device Identification Sequence contains exactly one Item:

- Manufacturer `dicom-test-suite`, Model Name `DTS C-Arm LINAC`, Model
  Version `1`, Device Label `DTS_LINAC`, Device Serial Number
  `DTS-LINAC-001`, Software Versions equal to the package version, and
  Manufacturer's Device Identifier `DTS-LINAC-001`;
- Manufacturer's Device Class UID and Device Alternate Identifier present
  empty; the conditional alternate-identifier Type and Format absent;
- Device Type Code Sequence with exactly
  `(130361, DCM, "Radiotherapy Treatment Device")` from CID 9551;
- all optional UDI, date, institution, and long-description content absent.

Radiation Dosimeter Unit Sequence contains exactly
`({MU}, UCUM, "Monitor Units")` from CID 9552. RT Device Distance Reference
Location Code Sequence contains exactly
`(130358, DCM, "Nominal Radiation Source Location")`. RT Beam Modifier
Definition Distance is `500`. Equipment Frame of Reference UID is the
A.86.1.5-fixed IEC coordinate-system UID `1.2.840.10008.1.4.3.1`, distinct
from the patient Frame of Reference UID. Equipment Reference Point
Coordinates Sequence is present empty. Number of Patient Support Devices is
`0`, and Patient Support Devices Sequence is absent. Radiation Source-Axis
Distance is `1000`.

Treatment Position Sequence has exactly one Item with index `1`, an identity
Image to Equipment Mapping Matrix, empty Patient Location Coordinates and
Patient Support Position Sequences, and these 2026b coded concepts:

- Patient Orientation: `(102538003, SCT, "recumbent")`, CID 19;
- Patient Orientation Modifier: `(40199007, SCT, "supine")`, CID 20;
- Patient Equipment Relationship: `(102540008, SCT, "headfirst")`, CID 21.

This is the coded equivalent of the existing head-first-supine topology.

### Identification-level static-beam control points

Number of RT Control Points `(300A,0604)` is `2`. C-Arm Photon-Electron
Control Point Sequence `(300A,062F)` contains exactly two ordered Items.
Unlike the legacy RT Plan, second-generation RT Control Point indexes begin
at `1`.

Control Point `1` contains RT Control Point Index `1`, Cumulative Meterset
`0`, Referenced Treatment Position Index `1`, Source Roll Angle `0`, and RT
Beam Limiting Device Angle `0`. Delivery Rate, Source to Patient Surface
Distance, and Source to External Contour Distance are present empty as Type
2C values. Delivery Rate Unit Sequence is absent because Delivery Rate is
empty. Control Point `2` contains only RT Control Point Index `2` and changed
Cumulative Meterset `100`; unchanged values are absent and inherited from
Control Point `1` under C.36.2.2.5.1.1. Recorded control-point attributes are
absent because RT Record Flag is `NO`.

Under the selected `IDENT_ONLY` branch, Radiation Generation Mode, RT Beam
Limiting Device Definition and Opening, Wedge, Compensator, Block, RT
Accessory Holder, General Accessory, Bolus, and Beam Area Limit count and
Sequence attributes are absent. Zero counts must not be substituted for this
locked absence contract. Pixel Data, Image Pixel attributes, synchronization,
and all image content are absent.

## Locked RT Radiation Set IOD Contract

PS3.3 A.86.1.4 and Table A.86.1.4-1 define the Set IOD. Patient, General
Study, General Series, Enhanced RT Series, General Equipment, Enhanced General
Equipment, Frame of Reference, General Reference, RT Radiation Set, SOP
Common, Common Instance Reference, and Radiotherapy Common Instance are
mandatory. RT Dose Contribution is conditional and is absent because this
planning-only recipe does not track delivered dose. Patient Study and all
Clinical Trial Modules are absent.

The Set shares the locked patient, Study, Study ID, and patient Frame of
Reference with the Radiation and Plan. Modality is `RTRAD`; Series Number is
`75`; the same fixed Series, Instance Creation, and Content Date/Time values
apply. User Content Label is `DTS_RADSET`, and Content Description is present
empty. Author Identification, Referenced RT Physician Intent, and
Instance-level or Series-level Performed Procedure Step Sequences are present
empty where Type 2 and otherwise absent as locked above.

Referenced RT Physician Intent Sequence `(300A,063B)` is present with zero
Items, so Intended Number of Fractions `(300A,0636)` is `1`. No fraction
pattern is defined, so Fraction Pattern Sequence is absent. RT Radiation Set
Intent `(300A,0637)` is `TREATMENT`.

Treatment Position Group Sequence `(300A,060A)` contains exactly one Item:
deterministic Treatment Position Group UID, label `DTS_TPG_1`, and exactly one
Referenced RT Radiation Sequence Item identifying the companion `.481.13`
instance. RT Radiation Sequence `(300A,0616)` independently contains exactly
that same Radiation identity. Definition Source Sequence contains exactly the
existing RT Plan identity. Common Instance Reference contains the Plan and
Radiation in deterministic Series order. Duplicate, missing, dangling, or
reordered identities; a Radiation in zero or multiple position groups; or a
different Study, patient Frame of Reference, or treatment-device identity are
invalid.

## KB, Official Source, And Code Evidence

- Queries:
  `dicom-kb lookup uid CArmPhotonElectronRadiationStorage --edition 2026b`
  and `dicom-kb lookup uid RTRadiationSetStorage --edition 2026b`
- Results: `1.2.840.10008.5.1.4.1.1.481.13` and
  `1.2.840.10008.5.1.4.1.1.481.12`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

The KB proves SOP identities and exposes the top-level IOD/module rows, but it
does not fully expand the Enhanced Content Identification, Fraction Pattern,
SOP Instance Reference, Treatment Device Identification, Treatment Position,
or RT Control Point macros. It also does not machine-evaluate same-patient-FOR,
same-device, unique User Content Label, once-only position-group membership,
Definition Source class, or dose-tracking conditions. Those gaps require this
source note and strict project validation.

The exact contract is anchored in PS3.3 A.86.1.4, A.86.1.5 and their module
tables and constraints; C.36.2.1.1, C.36.2.1.6, C.36.2.2.1, C.36.2.2.2,
C.36.2.2.4 through C.36.2.2.16, C.36.3, C.36.4, C.36.10, and C.36.12 through
C.36.15; Tables 10-11, 10-15a, 10.35-1, and 10.36-1; PS3.4 Table B.5-1; PS3.6
Tables A-1 and 6-1; and PS3.16 CIDs 19, 20, 21, 9511, 9551, and 9552.

The independently locked validator cache pins official PS3.3 DocBook SHA-256
`4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`,
PS3.4 SHA-256
`8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`,
PS3.6 SHA-256
`512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`,
generated IOD definitions SHA-256
`ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683`,
and generated module definitions SHA-256
`9f4853924ef520dd9b97ada0f14abd206fb15e6d8622e4d24a90f8b404a3e8c3`.
The repository lock correctly retains official downloaded sources, including
PS3.16, as `unavailable_not_downloaded`; the selected code rows were checked
against official 2026b PS3.16 HTML and are recorded here without rewriting
that lock history. Official source artifacts and generated definitions remain
outside git.

## Manifest, Strict Validation, And Report Contract

`expected_rt_radiation` shall bind the Radiation identity, Plan source path
and digest, patient and equipment Frames of Reference, device identity, coded
concepts, detail and record flags, treatment position, exact ordered control
points and inheritance, and all locked absences. `expected_rt_radiation_set`
shall bind the Set identity, exact Plan and Radiation source paths and hashes,
intent and fraction semantics, position-group UID and membership, direct and
Common Instance references, shared identities and device, dose non-claim, and
all locked absences. Both image and pixel fields are null.

Strict validation reopens and hashes every source before writing and again
checks exact tag VR, VM, value, Sequence cardinality and order; reference and
manifest-graph closure; same Study and patient/equipment Frames of Reference;
device equality; code triplets; first-control-point population, final
meterset, and inheritance; position-group once-only membership; and absence
contracts. Every cardinality is checked before indexing.

Negative tests shall cover a wrong Plan source class, UID, beam, digest,
Study, or Frame of Reference; wrong equipment UID or device; missing or
duplicate device/position/code Items; `FULL` or `YES` without their required
branches; wrong control-point count, index, order, first/final meterset,
position index, angles, or Type 2 presence; and forbidden beam-modifier or
record content. Set controls shall cover missing, dangling, duplicated, or
reordered Plan/Radiation references; wrong Radiation SOP Class, Study, Frame
of Reference, or device; zero or duplicate position-group membership; Common
Instance Reference disagreement; non-empty Physician Intent without its
prescription; missing intended fractions; and any Dose Contribution claim.
No finding may be silently allowlisted.

JSON and Markdown reports shall expose Radiation label, physical-detail and
record flags, device and equipment identities, treatment position,
control-point order and meterset range, Plan definition-source closure, and
external-validator disposition. Set rows shall expose label, intent,
fractions, Treatment Position Group and Radiation reference identities,
Plan/Radiation closure, dose-contribution absence, and external-validator
disposition. Planned cases retain explicit null expectations until promotion.

## Independent Validator Selection And Promotion Gate

The native Rust generator is not its own conformance authority. Locked
`dicom3tools dciodvfy` knows the SOP UID names but returns `Information Object
Not found` for these current IODs. Locked PixelMed 20260608 likewise reports
the IOD unrecognized. DCMTK 3.7.0 `dcmdump` recognizes and parses the SOP UIDs
but is not an IOD validator. None of those results may be presented as an IOD
opinion.

The independently implemented `uv`-locked `dicom-validator` 0.8.2 with exact,
hash-locked official 2026b definitions selects both IODs by SOP Class UID and
produces module-specific findings. It is selected as the required primary IOD
validator for exactly these two cases, subject to the locked defect correction
below. This selection does not qualify a prototype or change registry status.
Promotion requires empirical zero-error valid prototypes plus a recorded
mutation detection boundary for both IODs.

Prototype testing found one deterministic validator-definition defect. For
Recorded RT Control Point DateTime `(300A,073A)` inside C-Arm Control Point
Sequence, the generated 2026b JSON serializes a `MandatoryOrConditional`
condition for RT Record Flag `YES` without its required `other_cond`. The
validator unconditionally indexes that missing member when the standards-
required C-Arm value is `NO`, causing `KeyError: 'other_cond'` instead of an
IOD result. Changing RT Record Flag to `YES` makes the validator pass but
violates A.86.1.5.4.3 and is forbidden.

The selected route shall therefore add a narrowly guarded adapter correction
before qualification: verify the locked original definition shape, then supply
the omitted alternative condition that permits Recorded RT Control Point
DateTime only while RT Record Flag is `YES`. The original official artifacts,
their hashes, and the correction must remain visible in the composite adapter
fingerprint and qualification record. No other condition may be rewritten,
and a definition that no longer matches the expected malformed input must
fail closed. This standards-aligned crash correction does not use generator
code or weaken the external engine's findings.
DCMTK `dcmdump +fo` remains required independent parsing, and isolated
`dcentvfy -f` evidence must prove that removing the Plan or Radiation adds a
missing-reference finding without hiding any immutable upstream diagnostic.

Promotion additionally requires two complete seed-7 extended roots with
byte-identical Radiation, Set, and manifest bytes; exact source hashes and
graph closure; strict Rust mutation coverage for every externally missed
semantic; schema and report tests; and integrated conformance with no silent
or accepted finding. Until all evidence exists, both registry cases remain
planned with an explicit recipe blocker.
