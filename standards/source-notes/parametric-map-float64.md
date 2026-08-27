# Float64 Parametric Map

Checked: 2026-08-27  
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `derived/parametric-map/float64_ct_derived_explicit_le`
- Recipe ID: `derived_parametric_map_float64_ct_derived_explicit_le`
- Registry status: implemented
- Provider: the optional uv-locked `highdicom_pydicom` backend
- Validation: Double Float Pixel Data, multi-frame dimensions, Real World Value
  Mapping, source-image references, and deterministic binary64 semantics

## Required Decision

The Phase 3 variant is a three-frame Parametric Map derived from the same
generated CT sorting series as the float32 proof. It uses native Double Float
Pixel Data `(7FE0,0009)` with VR `OD` and Bits Allocated `(0028,0100)` equal to
`64`. Float Pixel Data `(7FE0,0008)` and integer Pixel Data `(7FE0,0010)` are
absent. Bits Stored `(0028,0101)`, High Bit `(0028,0102)`, Pixel Representation
`(0028,0103)`, and Planar Configuration `(0028,0006)` are also absent.

The image has three 2 by 2 MONOCHROME2 frames with one sample per pixel and is
encoded using Explicit VR Little Endian
`1.2.840.10008.1.2.1`. Each frame retains its derivation and source-image
reference to the corresponding synthetic CT instance. Multi-frame Functional
Groups, Multi-frame Dimension, Common Instance Reference, and Synthetic Data
`YES` follow the established float32 Parametric Map contract.

The shared Real World Value Mapping uses Double Float Real World Value First
Value Mapped `(0040,9214)` and Double Float Real World Value Last Value Mapped
`(0040,9213)`, both FD. Its linear mapping has slope `(0040,9225)` equal to
`1.0`, intercept `(0040,9224)` equal to `0.0`, units `1` / `UCUM` / `no units`,
and quantity `110850` / `DCM` / `X-Ray Attenuation`. The integer first/last
mapped attributes are absent.

## Deterministic Binary64 Recipe

The stored CT values are multiplied by `0.25`. Spatial rank is multiplied by
`2^-30`, exactly `9.313225746154785e-10`, so the output contains distinctions
below binary32 precision while remaining exactly reproducible in binary64.
Highdicom serializes the axial source stack in ranks 2, 1, 0. The exact
little-endian IEEE 754 bit patterns and per-frame SHA-256 values are:

| Serialized rank | Unsigned binary64 bit patterns | Frame SHA-256 |
| ---: | --- | --- |
| 2 | `13866583252673691648, 4476578029606273024, 4643211215819014144, 4647710417399873536` | `921a8e74cc86e767d5436be2a4eb0c6d383bf3f210ec4c32e8f8c43c239f8abe` |
| 1 | `13866583252673724416, 4472074429978902528, 4643211215818997760, 4647710417399857152` | `be480ba76c1931f10052029005c539dd45b565f7020cc94a41a89825c3b6ea44` |
| 0 | `13866583252673757184, 0, 4643211215818981376, 4647710417399840768` | `ce1600d46bb7468f4a0f60c2d58cf96430234a89e50f0cacdd56bfd86bc3ec90` |

The complete 96-byte OD value has SHA-256
`21a27d41285f045a72c0de209c4b48ea98a09257d44520290bc6044b132fc002`.
The minimum is `-256.0`; the maximum is `511.75000000186265`. Exact bit
patterns and frame hashes, rather than formatted decimal strings, are the
normative project expectations. The case remains `semantic_stable`: two runs
must reproduce payload bits, hashes, references, identities, and manifest
semantics, but need not claim byte-identical Part 10 containers across backend
versions.

## KB Query

- Tool: `dicom-standard-kb`
- Input: `dicom-kb lookup uid ParametricMapStorage --edition 2026b`
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: Parametric Map Storage SOP Class UID
  `1.2.840.10008.5.1.4.1.1.30`
- Why insufficient: the UID lookup does not capture the conditional floating
  pixel module, native OD representation, multi-frame functional groups,
  Real World Value Mapping attributes, or reference requirements.

## Official Source Evidence

- PS3.3 A.75.1 and Table A.75-1: Parametric Map IOD modules
- PS3.3 C.7.6.24: Floating Point Image Pixel Module
- PS3.3 C.7.6.16 and C.7.6.17: Multi-frame Functional Groups and Multi-frame
  Dimension Modules
- PS3.3 C.8.32.1 and C.8.32.2: Parametric Map Image and Functional Group
  requirements, including Real World Value Mapping
- PS3.5 Section 6.2 and native little-endian encoding rules: OD value
  representation and binary64 byte encoding
- PS3.6 Table 6-1: data element tags and VRs

The PS3.3, PS3.5, and PS3.6 source artifacts are recorded as
`unavailable_not_downloaded` in `standards.lock.json`. The official 2026b CHTML
was reviewed without committing a copy.

## Backend Capability

The committed uv environment pins highdicom `0.28.1`, NumPy 2.x, and pydicom
`3.0.2`. Highdicom `0.28.1` explicitly maps a NumPy `float64` array to Double
Float Pixel Data, selects 64 Bits Allocated, and restricts floating-point
payloads to native transfer syntaxes. This is implementation feasibility
evidence, not the source of the DICOM contract. The float32 calculation must
retain binary32 arithmetic when the shared backend is generalized so existing
payload hashes do not change.

## Independent Validator Qualification

Promotion used all of the following against an isolated generated corpus:

1. the locked dicom3tools `dciodvfy -new` IOD validator and `dcentvfy` entity
   validator complete without unreviewed findings;
2. DCMTK `dcmdump` independently extracts `(7FE0,0009)`, its decimal values are
   parsed as `f64` and reconstructed as little-endian binary64, and every exact
   frame hash matches the manifest;
3. Rust independently recomputes the binary64 values from the staged CT source
   pixels and rejects any backend-authored bit pattern, RWVM, dimension, or
   reference claim that differs;
4. strict conformance verification binds all evidence to locked tool and
   adapter fingerprints and reports zero failures.

The separately uv-locked `dicom-validator` 0.8.2 adapter was evaluated as an
additional IOD candidate under the project's authorization to select another
independent validator. Its locked 2026b definitions currently report nine
known Parametric Map functional-group macro definition gaps for float32 and
float64 alike. It was therefore not substituted for the finding-free locked
`dciodvfy` acceptance oracle, and none of those findings was silently
allowlisted. The uv adapter remains case-scoped to the U32 and non-square SC
cases for which it was separately qualified.

On 2026-08-27, two seed-7 extended runs each wrote 88 files and produced
byte-identical float64 Part 10 objects with SHA-256
`1f50196e425771c51284f03893826e7dcb7910b4529190445151e26677358d21`.
Strict internal validation reported zero failures. Locked `dciodvfy -new`
identified `ParametricMap` with no findings. Locked DCMTK `dcmdump` reconstructed
all 12 values and reproduced the three frame hashes above. The integrated
conformance sidecar recorded independent status `passed`; isolated entity
validation is expected to report missing references when the three CT sources
are deliberately omitted, while the complete corpus includes and validates
those references.

## Project Action

- Registry status: implemented after generation, manifests, strict validation,
  reports, tests, and independent qualification were integrated
- Generator policy: highdicom/pydicom may construct the object, but Rust must
  reopen it and independently verify OD identity, bytes, mappings, dimensions,
  and references before promotion
- Determinism policy: preserve the float32 payload exactly while adding the
  separate binary64 calculation and semantic-stability evidence
- Should become KB patch: yes; module and conditional attribute queries should
  eventually replace this narrow local note
- Redistribution: do not commit generated DICOM instances, official standard
  artifacts, generated validator definitions, or generated conformance reports
