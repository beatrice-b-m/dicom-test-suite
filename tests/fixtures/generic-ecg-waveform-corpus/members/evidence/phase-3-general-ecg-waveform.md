# Phase 3 General ECG Waveform Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `non-image/waveform/general_ecg`
- Recipe ID: `non_image_waveform_general_ecg`
- Recommended provider: `rust_native`
- Output: General ECG Waveform Storage
  (`1.2.840.10008.5.1.4.1.1.9.1.2`), Explicit VR Little Endian
- Recommended manifest field: generalized `expected_waveform`

The case is a synthetic waveform-read and display-compatibility object, not a
diagnostic tracing. It deliberately exercises two multiplex groups with
different channel layouts, sample counts, and sampling frequencies. UIDs are
deterministic functions of the run seed. Dates, times, patient and equipment
identity, channel metadata, and samples are fixed recipe inputs independent of
the host, locale, network, and clock.

## Locked IOD And Module Contract

PS3.3 A.34.4 and Table A.34.4-1 make Patient, General Study, General Series,
General Equipment, Waveform Identification, Waveform, Acquisition Context, and
SOP Common mandatory. The recipe shall omit the optional Patient Study,
Clinical Trial, Synchronization, and Waveform Annotation Modules. It contains
no referenced Instances, Image Pixel Module, or Pixel Data.

Modality is `ECG`. Content Date and Time are `20260101` and `000000`,
Acquisition DateTime is `20260101000000`, Series Number is `91`, and Instance
Number is `1`. Acquisition Context Sequence `(0040,0555)` is the required Type
2 empty Sequence. Synthetic Data `(0008,001C)` is `YES`.

PS3.3 A.34.4.4 constrains Waveform Sequence `(5400,0100)` to one through four
Items, each Item to one through twenty-four channels, each Sampling Frequency
`(003A,001A)` to 200 through 1,000 Hz inclusive, every Channel Source Sequence
`(003A,0208)` to CID 3001, and Waveform Sample Interpretation `(5400,1006)` to
`SS`. This recipe locks exactly two Waveform Sequence Items:

| Group | Multiplex Group Label | Channels | Samples per channel | Sampling frequency | Duration |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1 | `STD12_250HZ` | 12 | 1,000 | 250 Hz | 4 s |
| 2 | `AUX4_1000HZ` | 4 | 4,000 | 1,000 Hz | 4 s |

Both Items have Waveform Originality `(003A,0004)` `ORIGINAL`. Each Item owns
its own Channel Definition Sequence, Waveform Bits Allocated, Waveform Sample
Interpretation, and Waveform Data. Multiplex Group Time Offset, Trigger Time
Offset, Trigger Sample Position, and Waveform Padding Value are absent. Equal
duration does not assert synchronization between the two multiplex groups.

Waveform Bits Allocated `(5400,1004)` and every channel's Waveform Bits Stored
`(003A,021A)` are `16`. Waveform Sample Interpretation is `SS`. Each Waveform
Data `(5400,1010)` value uses OW and little-endian signed 16-bit samples with no
value-field padding. Group 1 therefore contains exactly 24,000 bytes and Group
2 exactly 32,000 bytes; their ordered aggregate is 56,000 bytes.

## Locked Channel Definitions

Channel ordinals restart at one in each multiplex group. Every Channel Source
Sequence has exactly one Item with Coding Scheme Designator `MDC` and the
following CID 3001 code and meaning.

| Group | Ordinal | Label | Code Value | Code Meaning |
| ---: | ---: | --- | --- | --- |
| 1 | 1 | I | `2:1` | Lead I |
| 1 | 2 | II | `2:2` | Lead II |
| 1 | 3 | III | `2:61` | Lead III |
| 1 | 4 | aVR | `2:62` | aVR, augmented voltage, right |
| 1 | 5 | aVL | `2:63` | aVL, augmented voltage, left |
| 1 | 6 | aVF | `2:64` | aVF, augmented voltage, foot |
| 1 | 7 | V1 | `2:3` | Lead V1 |
| 1 | 8 | V2 | `2:4` | Lead V2 |
| 1 | 9 | V3 | `2:5` | Lead V3 |
| 1 | 10 | V4 | `2:6` | Lead V4 |
| 1 | 11 | V5 | `2:7` | Lead V5 |
| 1 | 12 | V6 | `2:8` | Lead V6 |
| 2 | 1 | A1 | `2:75` | Auxiliary unipolar lead 1 |
| 2 | 2 | A2 | `2:76` | Auxiliary unipolar lead 2 |
| 2 | 3 | A3 | `2:77` | Auxiliary unipolar lead 3 |
| 2 | 4 | A4 | `2:78` | Auxiliary unipolar lead 4 |

The sixteen distinct sources deliberately exceed the 12-lead ECG IOD's
thirteen-channel total while remaining within the General ECG per-group
limit. This makes the case a separate channel-layout and sampling-model slice,
not merely the Twelve-lead payload under another SOP Class UID.

Waveform Channel Number `(003A,0202)` is the one-based local ordinal and
Channel Label `(003A,0203)` is the table label. Channel Sensitivity
`(003A,0210)` is `1`; Channel Sensitivity Units Sequence `(003A,0211)` is the
UCUM code `uV`, meaning `microvolt`; Channel Sensitivity Correction Factor
`(003A,0212)` is `1`; and Channel Baseline `(003A,0213)` is `0`. Channel Time
Skew `(003A,0214)` is `0` and Channel Sample Skew `(003A,0215)` is absent. The
explicit zero satisfies C.10.9's mutually conditional skew requirement and
locks simultaneous sampling within each multiplex group.

The official 2026b CID 3001 HTML previously reviewed for the Twelve-lead
slice is also authoritative here. The temporary 120,165-byte source artifact
has SHA-256
`f8ee9bcd0797f85bc1a9fc3a47b828328931562fef6d8c645b4c85aae9b3f227`
and remains outside git.

## Deterministic Payload Contract

Let `g` be the zero-based multiplex-group ordinal, `c` the zero-based channel
ordinal local to that group, and `s` the zero-based sample index. Every signed
stored value is:

```text
((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000
```

The formula's possible range is `-1000` through `1000`. Within each Waveform
Sequence Item, PS3.3 C.10.9.1.7 channel-then-sample interleaving produces
`C1S1, C2S1, ... CnS1, C1S2, ... CnSm`. Each Item owns a separate OW payload;
the two payloads must never be interleaved with each other. The builder
prototype shall derive and lock each group payload SHA-256, every independently
deinterleaved channel SHA-256, the ordered aggregate SHA-256, and the actual
minimum and maximum values without changing this formula or shape.

Strict validation shall recompute formula values, byte-length arithmetic,
group and channel hashes, group order, local channel order, signed
interpretation, and interleaving. The independent payload adapter shall use the
`uv`-locked pydicom runtime to extract both raw OW values and Python `struct` to
decode them without NumPy or project generator code.

## KB Query And Locked Local Evidence

- Query: `dicom-kb lookup uid GeneralECGWaveformStorage --edition 2026b`
- Edition: 2026b
- Result: `1.2.840.10008.5.1.4.1.1.9.1.2`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the KB result proves the UID but does not expose the complete
  IOD module table, group/channel/rate constraints, interleaving rules, or CID
  3001 definitions needed by this recipe.

The exact contract is anchored in PS3.3 A.34.4 and Table A.34.4-1, A.34.4.4,
C.10.8, C.10.9 and Table C.10-9, and C.10.9.1.7; PS3.16 CID 3001; PS3.4 Table
B.5-1; and PS3.6 Tables A-1 and 6-1. The repository lock records official
artifacts as `unavailable_not_downloaded`. The independently locked validator
cache pins official PS3.3 DocBook SHA-256
`4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`,
PS3.4 SHA-256
`8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`,
PS3.6 SHA-256
`512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`,
and generated IOD definitions SHA-256
`ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683`.

## Independent Validator Qualification Plan

Locked dicom3tools `dciodvfy -new` remains the primary IOD validator. The
existing independently implemented, `uv`-locked `dicom-validator` 0.8.2
environment shall be qualified for General ECG and run additively as the
secondary IOD opinion. DCMTK `dcmdump +fo` supplies independent parsing but is
not an IOD or waveform-payload authority.

Qualification starts from the exact valid prototype and independently mutates:
five Waveform Sequence Items; twenty-five channels in one Item; Sampling
Frequency `199` and `1001`; sample interpretation `US`; a channel missing both
Channel Time Skew and Channel Sample Skew; a wrong or duplicate CID 3001 source;
sample-count/payload-length disagreement; reversed group order; and one changed
payload byte. Every missed mutation remains visible as a validator gap. No
finding may be silently allowlisted. Strict Rust validation and the separate
group-aware raw payload route own semantics that the IOD validators do not
enforce.

## Decision Checkpoint Audit

Proceeding with this native slice triggers no Section 11 decision checkpoint
in `docs/coverage-expansion-plan.md`:

- Python is not mandatory for generation or an existing profile; the
  case-scoped secondary IOD and payload adapter remains optional tooling whose
  availability is explicit in conformance evidence.
- The user has adopted `uv` for the existing locked Python conformance
  environment, so this does not select a new long-term runtime manager.
- The user explicitly authorized selecting and locking another independent IOD
  validator. The qualified adapter supplements and never replaces locked
  `dciodvfy`.
- The recipe is lossless and adds no certificates, keys, stress job, protocol
  rule, or change to the meaning or inclusion rules of `all`.

## Manifest, Validation, Report, And Acceptance Contract

The waveform manifest model shall be generalized before generation so
`expected_waveform` represents an ordered `multiplex_groups` array. Each group
owns its label, originality, sample shape, rate, duration, channel definitions,
storage contract, payload hash, and channel hashes. Aggregate fields bind the
group count, total channel count, ordered group hashes, 56,000-byte total, and
ordered aggregate hash. The Twelve-lead case shall migrate through the same
model without changing its generated DICOM bytes.

Strict validation must enforce the IOD identity and mandatory/absent Modules;
two-group order and cardinality; every local channel ordinal, label, CID code,
sensitivity, units, correction, baseline, bits, and skew; heterogeneous sample
counts and rates with equal duration; signed OW storage; separate payload
arithmetic; formula, interleave, raw and deinterleaved hashes; and absence of
padding, annotation, synchronization, references, images, and Pixel Data.

JSON and Markdown reports shall expose General ECG IOD kind, group count,
ordered group shapes (`12x1000@250Hz; 4x4000@1000Hz`), labels and source codes
with group boundaries, common duration, bits and interpretation, per-group and
total payload sizes and hashes, total channel/hash counts, simultaneous
sampling within groups, pixel absence, and external-validator disposition.
They must not collapse heterogeneous group rates or sample counts into a false
scalar.

Promotion requires schema and CLI tests, builder and tamper tests, strict
validation, the locked additive IOD and raw payload routes, independent parse,
two-run byte reproducibility, report coverage, isolated entity validation,
integrated conformance verification, and documentation. Unavailable tooling
must remain explicit and may not silently reduce the case contract.

## Project Action

- Current registry status: planned and `semantic_stable`.
- Current registry provider: external backend `dcmtk`.
- Current blockers: `backend_contract_unimplemented` and
  `independent_payload_validator_unavailable`.
- Recommended next provider: `rust_native`; DICOM-rs already writes the needed
  Sequences, attributes, and exact OW values, while all external validators
  remain independent of generation.
- A separate provider-selection commit should change determinism to
  `byte_stable`, replace the stale provider with `rust_native`, and reduce the
  blockers to exactly `recipe_unimplemented`; this evidence commit
  intentionally does not change registry state.
- Should become KB patch: yes; expose Table A.34.4-1, A.34.4.4 constraints,
  C.10.9 interleaving, and CID 3001 as structured 2026b queries.
