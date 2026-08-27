# Phase 3 Twelve-lead ECG Waveform Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `non-image/waveform/twelve_lead_ecg`
- Recipe ID: `non_image_waveform_twelve_lead_ecg`
- Recommended provider: `rust_native`
- Output: 12-lead ECG Waveform Storage
  (`1.2.840.10008.5.1.4.1.1.9.1.1`), Explicit VR Little Endian
- Recommended manifest field: `expected_waveform`

The case is a synthetic waveform-read and display-compatibility object, not a
diagnostic tracing. UIDs remain deterministic functions of the selected run
seed. All dates, times, patient identity, equipment identity, channel
metadata, and waveform samples are fixed recipe inputs independent of the
host, locale, network, or clock.

## Locked IOD And Module Contract

PS3.3 A.34.3 and Table A.34.3-1 make Patient, General Study, General Series,
General Equipment, Acquisition Context, Waveform Identification, Waveform,
and SOP Common mandatory. The recipe shall include those Modules and omit the
optional Patient Study, Clinical Trial, Synchronization, and Waveform
Annotation Modules. It contains no referenced Instances, Image Pixel Module,
or Pixel Data.

Modality is `ECG`. Content Date and Time are `20260101` and `000000`,
Acquisition DateTime is `20260101000000`, Series Number is `90`, and Instance
Number is `1`. Acquisition Context Sequence `(0040,0555)` is the required
Type 2 empty Sequence. Synthetic Data `(0008,001C)` is `YES`.

The Waveform Sequence `(5400,0100)` is SQ VM 1 with exactly one multiplex
group. The group has Waveform Originality `(003A,0004)` `ORIGINAL`, Number of
Waveform Channels `(003A,0005)` `12`, Number of Waveform Samples
`(003A,0010)` `500`, Sampling Frequency `(003A,001A)` `500`, and Multiplex
Group Label `(003A,0020)` `RESTING_12_LEAD`. This is exactly one second of
simultaneous channel data. The values remain within A.34.3's limits of one
through five groups, one through thirteen channels per group and thirteen in
total, at most 16,384 samples per group, and 200 through 1,000 Hz.

Waveform Bits Allocated `(5400,1004)` and every channel's Waveform Bits Stored
`(003A,021A)` are `16`. Waveform Sample Interpretation `(5400,1006)` is `SS`.
Waveform Data `(5400,1010)` is OW with exactly 12,000 little-endian bytes and
no value-field padding. Waveform Padding Value is absent.

## Locked Channel Definitions

Channel Definition Sequence `(003A,0200)` has exactly twelve Items in the
following order. Each Channel Source Sequence `(003A,0208)` has exactly one
Item with Coding Scheme Designator `MDC` and the locked CID 3001 code and
meaning.

| Ordinal | Label | Code Value | Code Meaning |
| ---: | --- | --- | --- |
| 1 | I | `2:1` | Lead I |
| 2 | II | `2:2` | Lead II |
| 3 | III | `2:61` | Lead III |
| 4 | aVR | `2:62` | aVR, augmented voltage, right |
| 5 | aVL | `2:63` | aVL, augmented voltage, left |
| 6 | aVF | `2:64` | aVF, augmented voltage, foot |
| 7 | V1 | `2:3` | Lead V1 |
| 8 | V2 | `2:4` | Lead V2 |
| 9 | V3 | `2:5` | Lead V3 |
| 10 | V4 | `2:6` | Lead V4 |
| 11 | V5 | `2:7` | Lead V5 |
| 12 | V6 | `2:8` | Lead V6 |

Waveform Channel Number `(003A,0202)` is the one-based ordinal and Channel
Label `(003A,0203)` is the table label. Channel Sensitivity `(003A,0210)` is
`1`; Channel Sensitivity Units Sequence `(003A,0211)` is the UCUM code `uV`,
meaning `microvolt`; Channel Sensitivity Correction Factor `(003A,0212)` is
`1`; and Channel Baseline `(003A,0213)` is `0`. Channel Time Skew
`(003A,0214)` is `0` and Channel Sample Skew `(003A,0215)` is absent. The
explicit zero satisfies C.10.9's mutually conditional skew requirement and
locks all channels to simultaneous sampling.

The official 2026b CID 3001 HTML was reviewed from
`https://dicom.nema.org/medical/dicom/2026b/output/chtml/part16/sect_CID_3001.html`.
The temporary 120,165-byte source artifact has SHA-256
`f8ee9bcd0797f85bc1a9fc3a47b828328931562fef6d8c645b4c85aae9b3f227`.
It remains outside git as required by the standards-lock policy.

## Deterministic Payload Contract

Let `s` be the zero-based sample index from 0 through 499 and `c` the
zero-based channel ordinal from 0 through 11. The signed stored value is:

```text
((s * (c + 1) * 37 + c * 101) mod 2001) - 1000
```

The range is exactly `-1000` through `1000`. PS3.3 C.10.9 requires values to
be interleaved by channel and then sample, so the encoded order is
`C1S1, C2S1, ... C12S1, C1S2, ... C12S500`. The complete 12,000-byte payload
SHA-256 is
`98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713`.
After independent deinterleaving, the per-channel little-endian signed
16-bit byte hashes in channel order are:

1. `7b4aee068e05c2bdff3896937c78a4c7a32f9ed2bde64d91b1d925913bf29476`
2. `bd775dc70f76ea153a25832ad622b0cc26fbe6a37cf3ec6548a30965c4d17fba`
3. `19d26b694df281209aa1296abbfa8f7d360e24a03a091422aba6f67663e2f3b1`
4. `bb4c99d7857dbfcee5ee620bcff09b7060b61c5f2432427affc6139cb8d3cf9b`
5. `230f52ed2ac57624a9a35214d7867711008dd56014f4176ce258623e5b596d3a`
6. `60e167db3c081ba5bca957aba820afb519b790d048b660634d49566df88105f2`
7. `cf8c73bebf746b799b1fe8aa2c908ca69bc7acc72311c64cbf4131fc8976609f`
8. `0f11e5fb5105dac699fa4bcfc01c79fbe696a81db04606f39a719de57b4c7c30`
9. `a41d5962abceb6dbe25f8421091ce3df6a69202c45b24ab6b0736159d15e253b`
10. `d655e2cbb23d70e229ed52fedba9c45573e22729fed0a794ab690df8d7f33804`
11. `005c539f9f4256a86d9e0a212b3bfe73741f99942b0677fb483c0c48db9583cd`
12. `f448df95acb226c5c992363e27707a42efc3ffb974ebeff38e2a81522b57d82c`

Strict validation shall recompute the formula, full payload hash, every
deinterleaved channel hash, byte length, signed interpretation, interleave
order, and min/max values. The independent waveform payload adapter shall use
the `uv`-locked pydicom runtime to extract raw OW bytes and Python `struct` to
decode them without NumPy or project generator code.

## KB Query And Locked Local Evidence

- Query: `dicom-kb lookup uid TwelveLeadECGWaveformStorage --edition 2026b`
- Edition: 2026b
- Result: `1.2.840.10008.5.1.4.1.1.9.1.1`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the KB registry evidence proves the SOP Class UID but does not
  expose the complete IOD module table, waveform content constraints,
  interleaving rules, or CID 3001 channel definitions needed by this recipe.

The exact contract is anchored in PS3.3 A.34.3 and Table A.34.3-1 (IOD,
mandatory Modules, group/channel/sample/rate constraints, and `SS`), C.10.8
(Waveform Identification), C.10.9 and Table C.10-9 (Waveform Sequence,
channel definition, signed storage, skew, and data), and C.10.9.1.7
(channel-then-sample interleaving). PS3.16 CID 3001 locks the channel source
codes. PS3.4 Table B.5-1 identifies the Storage SOP Class. PS3.6 Tables A-1
and 6-1 lock the UID and data element VR/VM definitions.

The repository lock records official source artifacts as
`unavailable_not_downloaded`; none is committed. The independently locked
validator cache pins official PS3.3 DocBook SHA-256
`4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`,
PS3.4 SHA-256
`8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`,
and PS3.6 SHA-256
`512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`.

## Prototype And Independent Validator Evidence

The temporary prototype implements the exact contract above. The final
15,222-byte file has SHA-256
`9ce36490d6da3628223b1d18fe3157d040412e183e28015fde5a62d815b1ab80`.
Locked dicom3tools `dciodvfy -new` recognized `TwelveLeadECG` and emitted no
errors or warnings. DCMTK 3.7.0 `dcmdump +fo` parsed the complete object and
OW payload. The separate, `uv`-locked `dicom-validator` 0.8.2 adapter reported
`Passed` with zero errors against the exact 2026b definitions.

Qualification mutations establish the boundary of external validation. Both
IOD validators rejected a channel missing both Channel Time Skew and Channel
Sample Skew. Both accepted Sampling Frequency `199`, sample interpretation
`US`, a duplicate lead-I source replacing V6, a changed waveform payload
byte, and Number of Waveform Samples `501` with the unchanged 12,000-byte
payload. These are validator gaps, not accepted semantics. No finding may be
silently allowlisted.

Strict Rust validation therefore owns all IOD content constraints, exact
channel count/order/uniqueness/codes, sensitivity and simultaneous-sampling
metadata, sample interpretation, byte-length arithmetic, deterministic
values, interleaving, and hashes. The `uv`-locked payload extractor provides
an implementation-independent comparison rather than trusting the native
generator or either IOD validator.

## Decision Checkpoint Audit

Proceeding with this native slice triggers no Section 11 decision checkpoint
in `docs/coverage-expansion-plan.md`:

- Python is not made mandatory for generation or an existing profile; the
  case-scoped secondary IOD and payload adapter is an independently locked
  conformance capability and default tests do not invoke it.
- This recommendation does not select a long-term external-backend runtime
  manager. The user separately adopted `uv` for the existing locked Python
  conformance environment.
- The user explicitly authorized selecting and locking another independent
  IOD validator. The qualified waveform adapter supplements, and never
  replaces, locked `dciodvfy`.
- The recipe is lossless and introduces no certificates, keys, stress job,
  protocol-conformance rule, or change to the meaning or inclusion rules of
  `all`.

## Manifest, Validation, Report, And Acceptance Contract

`expected_waveform` shall bind the exact IOD identity; one-group topology;
duration; originality; sample count and rate; channel ordinal, label, code,
sensitivity, units, correction, baseline, bits, and skew; signed storage;
little-endian channel-then-sample interleave; raw and per-channel hashes;
value range; and absence of padding, annotations, synchronization,
references, pixels, and Pixel Data.

JSON and Markdown reports should expose waveform IOD kind, group/channel and
sample counts, sampling rate and duration, channel labels and source codes,
bits allocated/stored, sample interpretation, storage VR, payload length and
hash, interleave order, channel hash count, simultaneous-sampling state,
pixel absence, and external-validator disposition.

## Project Action

- Current registry status: planned and `semantic_stable`.
- Current registry provider: external backend `dcmtk`.
- Current registry blockers: `backend_contract_unimplemented` and
  `independent_payload_validator_unavailable`.
- Recommended next provider: `rust_native`; DICOM-rs exposes all required
  waveform tags and can write the nested Sequences and exact OW signed sample
  bytes deterministically, while every external validator remains independent
  of generation.
- A separate provider-selection commit should replace the stale provider and
  blockers with `rust_native` and exactly `recipe_unimplemented`; this evidence
  commit intentionally does not change registry state.
- Promotion requires strict Rust validation, the `uv`-locked independent IOD
  and raw waveform payload routes, DCMTK parsing, deterministic generation,
  schemas, reports, tests, two-run evidence, and documentation.
- Should become KB patch: yes; expose Tables A.34.3-1 and C.10-9, A.34.3
  content constraints, C.10.9 interleaving semantics, and CID 3001 as
  structured 2026b queries.
