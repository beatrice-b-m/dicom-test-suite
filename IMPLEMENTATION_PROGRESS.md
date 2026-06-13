# Implementation Progress

**Last updated:** 2026-06-13  
**Source specification:** `SYSTEM_SPEC.md` version 0.2.0  
**Current phase:** remediation planning before Phase 5 feature expansion

**Current implementation status:** Phase 0, Phase 0.5, Phase 1, Phase 2, Phase 3, and Phase 4 are functionally implemented, but the 2026-06-13 baseline review identified remediation work required before clean Phase 5 expansion: registry status now controls generation and skipped-case reporting, while the planned SEG case must still be restored to the registry, expected CLI commands remain incomplete, validation needs stricter Part 10 checks, standards lock pinning is partial, and reproducibility/schema/CI guard coverage needs expansion

This document is the durable hand-off log for coding agents implementing
`dicom-test-suite`. Keep `SYSTEM_SPEC.md` as the source of product and
architecture requirements. Use this file to record what has actually been done,
what remains, which decisions are still open, and the next safe implementation
step.

## How to Update This File

- Update this file in the same commit as any implementation work that changes
  project status.
- Keep entries factual and date-stamped when a decision, blocker, or standards
  note changes.
- Do not mark a phase complete until every exit criterion from `SYSTEM_SPEC.md`
  is satisfied.
- Record generated artifacts only by path pattern and manifest evidence. Do not
  commit generated DICOM files, generated manifests, reports, standards caches,
  or generated knowledge bases.
- Preserve viewer independence. Viewer-specific observations belong in optional
  reports or issue triage notes, not in generator logic.

## Repository Snapshot

Observed at creation of this progress file:

| Artifact | Status | Notes |
|---|---|---|
| `README.md` | present | Contains project purpose and planned commands, including `list-cases`. |
| `SYSTEM_SPEC.md` | present | Version 0.2.0 planning baseline. |
| `AGENTS.md` | present | Requires descriptive granular commits for completed work. |
| `IMPLEMENTATION_PROGRESS.md` | present after this task | Hand-off ledger for implementation state. |
| `.gitignore` | present | Covers generated DICOM outputs, reports, sidecars, caches, generated standards artifacts, and SQLite KB files. |
| `Cargo.toml` / Rust workspace | present | Single package named `dicom-test-suite`, using Rust 2024 edition; pins minimal DICOM-rs crates for Phase 1 object and transfer syntax work. |
| `build.rs` | present | Captures Rust compiler version and target triple for generated manifest metadata. |
| `rust-toolchain.toml` | present | Pins Rust 1.85.0 with `rustfmt` and `clippy`, matching an installed local toolchain. |
| `standards.lock.json` | present | Locks to DICOM 2026b base edition only using the pinned `dicom-standard-kb` MCP source manifest; local DB and source artifact hashes remain pending. |
| `schemas/` | present | Manifest, case registry, coverage report, and viewer report schemas have initial structured coverage. |
| `cases/taxonomy.md` | present | Documents normalized case ID format, path segments, descriptor conventions, profile definitions, and inclusion rules. |
| `cases/registry.json` | present | Tracks implemented smoke/core SC cases, classic radiology CT/MG/CR/MR/DX/US cases, plus Enhanced CT, Enhanced CT concatenation, and Enhanced MR extended cases with standards evidence from `dicom-standard-kb` MCP lookups. |
| `transfer-syntax/capability-matrix.json` | present | Records initial read/decode/write/encode, feature, external library, and determinism capabilities for baseline native transfer syntaxes. |
| `docs/deterministic-build-policy.md` | present | Documents determinism levels, reproducibility inputs, UID derivation, metadata controls, hashes, and two-run verification. |
| `standards/kb-integration.md` | present | Documents the pinned 2026b `dicom-standard-kb` MCP query workflow, evidence fields, and fallback path. |
| `standards/gap-workflow.md` | present | Documents standards gap handling, local source notes, blocked/skipped registry actions, and KB patch criteria. |
| `standards/source-notes/` | present | Contains a README/template plus `uid-2-25.md` for the PS3.5 UID root gap not covered by `dicom-standard-kb`. |
| `REMEDIATION_PLAN.md` | present | Defines the phased cleanup path for registry authority, missing planned cases, CLI completion, validation hardening, reproducibility/CI guard gaps, and standards lock pinning before Phase 5 feature work resumes. |
| `src/` or `crates/` | present | Single-package implementation now includes `list-cases`, `generate`, deterministic UID, run manifest, SC pixel writers, CT signed rescale writer, MG For Presentation/For Processing writers, CR overlay/LUT writer, MR multi-slice writer, DX display shutter writer, US single-frame writer, Enhanced CT and Enhanced MR multi-frame writers, Enhanced CT concatenation output, and Part 10 validation paths. |
| `tests/` | present | Includes schema artifact, `list-cases` CLI, `generate` CLI, UID, manifest, Part 10 readback, CT rescale readback, MG presentation/processing readback, CR overlay/LUT readback, MR multi-slice readback, DX display shutter readback, US readback, Enhanced CT and Enhanced MR multi-frame readback, Enhanced CT concatenation readback, and smoke reproducibility tests. |

## Non-Negotiable Implementation Constraints

- Standards-first: query `dicom-standard-kb` before adding IOD builders, module
  assumptions, SOP Class mappings, UID assumptions, enumerated values, or
  defined terms.
- Official DICOM source artifacts are authoritative when exact wording or
  conflict resolution matters.
- Generated DICOM files are build artifacts and must stay out of git.
- Generated output must be deterministic according to each case's declared
  determinism level.
- Every generated SOP Instance is synthetic and should set
  `Synthetic Data (0008,001C)` to `YES` unless a recipe documents a
  standards-based exception.
- Default output is DICOM Part 10 with valid File Meta Information and exactly
  one SOP Instance per `.dcm`.
- `cases/registry.json` is the planned authority for case status, profiles,
  requirements, standards evidence, and skip/block reasons.
- Optional codecs and external validators must be feature-gated and reported as
  generated, skipped, or unavailable.
- `dcmview` is an initial consumer, not a generator constraint.

## Phase Ledger

| Phase | Status | Summary |
|---|---|---|
| Phase 0: Repository initialization | complete | Scope docs, generated-artifact protections, Rust skeleton, toolchain, dependency pins, and initial schema placeholders are committed. |
| Phase 0.5: Standards and case registry foundation | complete | Standards base edition, schemas, taxonomy/profile rules, initial smoke/core registry, transfer syntax matrix, deterministic policy, standards workflows, and `list-cases` are in place. |
| Phase 1: Generator core | complete | `generate --profile smoke` writes all three initial Secondary Capture smoke Part 10 files with manifest hashes, file meta UIDs, pixel metadata, validation results, and byte-stable output across two identical runs. |
| Phase 2: Native pixel matrix | complete | Core native monochrome 16-bit unsigned/signed MONOCHROME2 OW Pixel Data, RGB planar configuration 1, PALETTE COLOR, YBR_FULL, YBR_FULL_422, odd-dimension, rectangular, tiny-image, pixel-padding, and broadened native pixel validators are implemented. |
| Phase 3: Classic radiology IODs | complete | CT Image Storage signed 12-bit rescale/window, MG For Presentation/For Processing 12-bit, CR overlay/Modality LUT/VOI LUT, MR multi-slice oblique geometry, DX display shutter, US Image Storage, and stable multi-file series generation are implemented. |
| Phase 4: Enhanced multi-frame | complete | Enhanced CT and Enhanced MR Image Storage cases with Shared and Per-Frame Functional Groups and Multi-frame Dimension metadata are implemented; MR Echo, Temporal Position, phase/velocity-encoding variation, and a two-member Enhanced CT concatenation case are covered. |
| Remediation before Phase 5 | in progress | R1 registry status gating is implemented; remaining remediation tasks in `REMEDIATION_PLAN.md` must be completed before adding new Phase 5 recipes. |
| Phase 5: Derived, presentation, and non-image objects | not started | SEG, presentation states, SR, KOS, RWVM, RT, and encapsulated documents pending. |
| Phase 6: Transfer syntax expansion | not started | Transfer syntax abstraction and compressed cases pending. |
| Phase 7: Pathology, video, and large object profiles | not started | VL, WSI, video, and stress cases pending. |
| Phase 8: Reporting and viewer integration | not started | Coverage reports, optional viewer runner, and compatibility schema pending. |
| Phase 9: Negative and fuzz profiles | not started | Invalid/malformed cases intentionally deferred. |

## Phase 0 Checklist

- [x] Add `README.md`.
- [x] Add `SYSTEM_SPEC.md`.
- [x] Add `.gitignore` rules for generated DICOM outputs, generated reports,
  caches, standards artifacts, and generated KB databases.
- [x] Choose initial Rust edition and workspace layout.
- [x] Add `Cargo.toml` and `Cargo.lock`.
- [x] Add `rust-toolchain.toml`.
- [x] Verify and pin the DICOM-rs crate family.
- [x] Add initial `schemas/` directory.

Phase 0 is complete only when the repository has clear scope, output policy,
implementation plan, a Rust project skeleton, generated-artifact protections,
and initial schema placeholders.

## Phase 0.5 Checklist

- [x] Add `standards.lock.json`.
- [x] Decide explicitly whether the lock targets only the base DICOM edition or
  the base edition plus final-text supplements/correction items as of a fixed
  date.
- [x] Add `schemas/manifest.schema.json`.
- [x] Add `schemas/case-registry.schema.json`.
- [x] Add `schemas/coverage-report.schema.json`.
- [x] Add `schemas/viewer-report.schema.json`.
- [x] Add normalized case ID taxonomy to committed project artifacts.
- [x] Add explicit profile definitions and inclusion rules.
- [x] Add initial `cases/registry.json` with planned smoke/core cases.
- [x] Add transfer syntax capability matrix.
- [x] Add deterministic build policy.
- [x] Add `dicom-standard-kb` integration instructions.
- [x] Add standards gap/patch workflow documentation.
- [x] Add initial `list-cases` command for the Phase 0.5 exit criterion.

Phase 0.5 is complete: `list-cases` can show planned smoke/core cases with
structured status and planned Phase 1/2 cases have standards evidence through
`dicom-standard-kb` or local source notes.

## Phase 1 Checklist

- [x] Add first `generate` command skeleton and argument model.
- [x] Define deterministic output-root setup and manifest path handling without
  writing DICOM instances.
- [x] Implement deterministic UID generation.
- [x] Implement manifest writing for empty generation runs.
- [x] Implement Part 10 file writing for the initial smoke cases.
- [x] Add file-level validation for generated Part 10 output.
- [x] Add two-run reproducibility checks for `smoke`.

Phase 1 is complete: `generate --profile smoke` writes the three required
Secondary Capture Part 10 files plus a valid manifest with hashes, file meta
UIDs, pixel metadata, validation results, and byte-stable output across two
identical runs.

## Phase 2 Checklist

- [x] Add first core native unsigned 16-bit monochrome pixel case.
- [x] Add signed 16-bit monochrome pixel case.
- [x] Add RGB planar configuration 1 case.
- [x] Add PALETTE COLOR case with palette LUT descriptors and data.
- [x] Add native YBR_FULL case.
- [x] Add native YBR_FULL_422 case with special byte-length validation.
- [x] Add odd-dimension, rectangular, very small image, and pixel padding cases.
- [x] Broaden pixel byte-length and photometric validators for Phase 2 cases.

Phase 2 is complete: smoke and core profiles cover the key Image Pixel
combinations targeted for this phase with computed byte-length validation, and
YBR_FULL_422 uses the required special native byte-length validator.

## Phase 3 Checklist

- [x] Add first CT Image Storage signed 12-bit MONOCHROME2 rescale/window case.
- [x] Add mammography For Presentation MONOCHROME1 12-bit case.
- [x] Add mammography For Processing MONOCHROME2 12-bit case.
- [x] Add CR overlay, Modality LUT, and VOI LUT coverage.
- [x] Add MR multi-slice oblique geometry sorting coverage.
- [x] Add first DX projection X-Ray display shutter coverage.
- [x] Add US and complete planned classic single-frame IOD builders.
- [x] Add multi-file series generation with stable Study/Series/Frame of
  Reference UIDs.

Phase 3 is complete. The core profile now includes standards-backed classic
single-frame SC, CT, MG For Presentation, MG For Processing, CR, DX, and US
cases plus a three-instance MR oblique series, covering the Phase 3 grayscale
transform, projection X-Ray, mammography, overlay/LUT, display shutter, and
multi-file series requirements.

## Phase 4 Checklist

- [x] Add first Enhanced CT Image Storage multi-frame case with Shared and
  Per-Frame Functional Groups.
- [x] Add Multi-frame Dimension metadata for the first Enhanced CT case.
- [x] Add Enhanced MR builder.
- [x] Add frame-varying echo case.
- [x] Add frame-varying temporal position case.
- [x] Add frame-varying phase case.
- [x] Add concatenation cases for extended profile.

Phase 4 is complete. The extended profile contains valid multi-frame CT/MR
cases, reports expected frame counts and geometry, and includes a two-member
Enhanced CT concatenation case for logical multi-frame object splitting.

## Initial Priority Case Queue

These case IDs come from `SYSTEM_SPEC.md` section 21 and should seed
`cases/registry.json` before implementation:

| Case ID | Profile | Implementation status |
|---|---|---|
| `classic/sc/mono2_u8_explicit_le` | `smoke` | implemented |
| `classic/sc/mono1_u8_explicit_le` | `smoke` | implemented |
| `classic/sc/rgb_planar0_explicit_le` | `smoke` | implemented |
| `classic/sc/mono2_u16_explicit_le` | `core` | implemented |
| `classic/sc/mono2_i16_explicit_le` | `core` | implemented |
| `classic/sc/rgb_planar1_explicit_le` | `core` | implemented |
| `classic/sc/palette_color_u8_explicit_le` | `core` | implemented |
| `classic/sc/ybr_full_planar0_explicit_le` | `core` | implemented |
| `classic/sc/ybr_full_422_explicit_le` | `core` | implemented |
| `classic/sc/mono2_u16_odd_3x3_explicit_le` | `core` | implemented |
| `classic/sc/mono2_u16_rect_2x3_explicit_le` | `core` | implemented |
| `classic/sc/mono2_u16_tiny_1x1_explicit_le` | `core` | implemented |
| `classic/sc/mono2_u16_padding_explicit_le` | `core` | implemented |
| `classic/ct/mono2_i16_rescale_12bit_explicit_le` | `core` | implemented |
| `classic/mg/for_presentation_mono1_u16_12bit_explicit_le` | `core` | implemented |
| `classic/mg/for_processing_mono2_u16_12bit_implicit_le` | `core` | implemented |
| `classic/cr/overlay_modality_voi_explicit_le` | `core` | implemented |
| `classic/mr/multislice_oblique_explicit_le` | `core` | implemented |
| `classic/dx/display_shutter_mono2_u16_explicit_le` | `core` | implemented |
| `classic/us/mono2_u8_explicit_le` | `core` | implemented |
| `enhanced/ct/multiframe_shared_perframe_explicit_le` | `extended` | implemented |
| `enhanced/ct/concatenation_two_part_explicit_le` | `extended` | implemented |
| `enhanced/mr/multiframe_echo_perframe_explicit_le` | `extended` | implemented |
| `enhanced/mr/multiframe_temporal_position_explicit_le` | `extended` | implemented |
| `enhanced/mr/multiframe_phase_velocity_encoding_explicit_le` | `extended` | implemented |
| `derived/seg/binary_multiframe_explicit_le` | `extended` | planned |
| `vl/photo/rgb_planar0_explicit_le` | `core` | planned |
| `vl/photo/palette_color_explicit_le` | `core` | planned |

## Open Decisions

| Decision | Status | Notes |
|---|---|---|
| Rust workspace layout | decided 2026-06-13 | Start as a single package named `dicom-test-suite`; keep module boundaries compatible with later spec crates. |
| Rust edition and toolchain | decided 2026-06-13 | Use Rust 2024 edition and pin toolchain `1.85.0`, the installed local stable toolchain sufficient for edition 2024. |
| DICOM-rs versions | decided 2026-06-13 | Crates.io verification found `dicom` 0.9.1, `dicom-object` 0.9.1, `dicom-core` 0.9.1, `dicom-transfer-syntax-registry` 0.9.1, and `dicom-dictionary-std` 0.9.0; pin minimal direct dependencies exactly and leave optional pixel/UL codecs disabled. |
| Standards baseline | decided 2026-06-13 | Use DICOM 2026b base edition only and exclude post-base final text until `standards.lock.json` is deliberately updated. |
| `dicom-standard-kb` pin | partially decided 2026-06-13 | The available MCP is pinned to generated 2026b reference data with source manifest SHA-256 `9959bee76fd293c7eda3fc81ce2ced7528612faa1b2df28cccd01504a83f54b0`; repository commit and local DB SHA-256 remain pending until exposed or independently verified. |
| Case registry storage shape | decided 2026-06-13 | Use `cases/registry.json` with `case_registry_schema_version` and a `cases` array conforming to `schemas/case-registry.schema.json`. |
| Transfer syntax capability matrix format | decided 2026-06-13 | Use `transfer-syntax/capability-matrix.json` entries with `read_dataset`, `decode_pixel`, `write_dataset`, `encode_pixel`, `feature_flags`, `external_libraries`, and `determinism` fields. |
| UID namespace and algorithm | decided 2026-06-13 | Use project namespace UUID `4f5b3b66-8b91-4f3d-a6a1-6d9a7fc6d4d8`, SHA-256 seed material, RFC 4122 version/variant bits, and DICOM `2.25.<decimal uuid>` output. The 2026b KB did not expose PS3.5 UID text, so `standards/source-notes/uid-2-25.md` records the local source note. |

## Implementation Notes

- 2026-06-13: `generate --profile smoke` now writes
  `classic/sc/mono2_u8_explicit_le/instance.dcm`, a tiny deterministic
  Secondary Capture Image Storage Part 10 file using Explicit VR Little Endian.
  The manifest records file hash, byte size, deterministic Study/Series/SOP
  Instance UIDs, Implementation Class UID, native OB Pixel Data metadata,
  `Synthetic Data (0008,001C) = YES`, and 2026b standards evidence from the
  pinned `dicom-standard-kb` MCP.
- 2026-06-13: Generated Part 10 output is read back through DICOM-rs before the
  manifest is written. Internal validation checks the Part 10 `DICM` marker,
  File Meta transfer syntax, File Meta/dataset SOP UID consistency,
  Implementation Class UID, `Synthetic Data`, required Image Pixel metadata, and
  native Pixel Data length. The manifest records these named validation results.
- 2026-06-13: The smoke generator has an integration reproducibility test that
  runs with the same seed into two separate output roots and compares DICOM
  bytes, manifest file metadata, skipped-case metadata, file hashes, and UID
  metadata.
- 2026-06-13: The remaining Phase 1 smoke recipes are implemented:
  `classic/sc/mono1_u8_explicit_le` and
  `classic/sc/rgb_planar0_explicit_le`. The RGB case uses Samples per Pixel 3
  and Planar Configuration 0, backed by 2026b `dicom-standard-kb` evidence for
  Photometric Interpretation and Planar Configuration. The smoke registry cases
  now report `implemented`.
- 2026-06-13: Phase 2 has started with `classic/sc/mono2_u16_explicit_le`, a
  core Secondary Capture 2x2 MONOCHROME2 unsigned 16-bit native Pixel Data
  case. The manifest records OW Pixel Data, 16-bit image metadata, value length
  8, pixel max 65535, and validation of Pixel Data VR and byte length. The case
  registry includes 2026b evidence for Secondary Capture, Explicit VR Little
  Endian, the Image Pixel Description Macro, Bits Stored, High Bit, and Pixel
  Representation.
- 2026-06-13: `classic/sc/mono2_i16_explicit_le` adds signed 16-bit native OW
  Pixel Data coverage using Pixel Representation 1. The manifest records the
  signed value range -32768 to 32767, and the registry cites the 2026b Image
  Pixel Description Macro text for Pixel Representation `0001H` as 2's
  complement.
- 2026-06-13: `classic/sc/rgb_planar1_explicit_le` adds RGB native OB Pixel
  Data coverage with Planar Configuration 1. The generated pixel bytes use
  color-by-plane ordering, and validation confirms Planar Configuration and
  native Pixel Data length.
- 2026-06-13: `classic/sc/palette_color_u8_explicit_le` adds PALETTE COLOR
  native OB Pixel Data coverage. The case uses single-sample 8-bit pixel
  indices, absent Planar Configuration, 16-bit Red/Green/Blue Palette Color
  Lookup Table Descriptors `[4, 0, 16]`, and OW LUT Data validated for VR and
  value length.
- 2026-06-13: `classic/sc/ybr_full_planar0_explicit_le` adds native 8-bit
  YBR_FULL OB Pixel Data coverage with Samples per Pixel 3 and Planar
  Configuration 0. The pixel bytes use the 2026b RGB-to-YCbCr equations for the
  existing red/green/blue/white pattern, and existing validation confirms
  photometric interpretation, Planar Configuration, Pixel Data VR, and native
  byte length.
- 2026-06-13: `classic/sc/ybr_full_422_explicit_le` adds native 8-bit
  YBR_FULL_422 OB Pixel Data coverage with Samples per Pixel 3 and required
  Planar Configuration 0. Validation now has a dedicated
  `native_ybr_full_422_pixel_data_length` result using the Phase 2 formula
  `rows * columns * frames * 2 * bytes_per_sample`, derived from the 2026b
  horizontal chroma downsampling semantics.
- 2026-06-13: Geometry coverage now includes
  `classic/sc/mono2_u16_odd_3x3_explicit_le`,
  `classic/sc/mono2_u16_rect_2x3_explicit_le`, and
  `classic/sc/mono2_u16_tiny_1x1_explicit_le`. Pixel recipes carry explicit
  Rows and Columns instead of relying on fixed 2x2 defaults, and validation
  confirms generated Rows, Columns, Pixel Data VR, and native byte length.
- 2026-06-13: `classic/sc/mono2_u16_padding_explicit_le` adds unsigned
  MONOCHROME2 Pixel Padding Value and Pixel Padding Range Limit coverage. The
  generated samples include the padding value `0`, and validation confirms both
  padding attributes against the 2026b US/SS data elements and MONOCHROME2
  value/range ordering rule.
- 2026-06-13: Phase 2 validator hardening computes native Pixel Data byte
  length from Rows, Columns, Samples per Pixel, Bits Allocated, and the selected
  formula instead of trusting the recipe byte slice length. Validation also
  records Bits Stored/Bits Allocated, High Bit, photometric Samples per Pixel,
  Planar Configuration presence, and YBR_FULL_422 Planar Configuration
  invariants.
- 2026-06-13: Phase 3 has started with
  `classic/ct/mono2_i16_rescale_12bit_explicit_le`, a 2x2 CT Image Storage Part
  10 case using Explicit VR Little Endian, signed 12-bit MONOCHROME2 native OW
  Pixel Data, deterministic Study/Series/SOP/Frame of Reference UIDs, Image
  Plane geometry, CT Image identifying attributes, HU rescale slope/intercept,
  and window center/width. Internal validation now supports optional CT Image
  checks for Modality, Frame of Reference UID, Image Type, Image Plane geometry,
  KVP, Acquisition Number, Rescale Intercept/Slope/Type, and Window
  Center/Width. The registry records 2026b `dicom-standard-kb` evidence for CT
  Image Storage, the CT Image IOD/modules, CT Image/Image Plane/Frame of
  Reference attributes, rescale attributes, and VOI window attributes.
- 2026-06-13: `classic/mg/for_presentation_mono1_u16_12bit_explicit_le` adds
  Digital Mammography X-Ray Image Storage - For Presentation coverage using a
  tiny unsigned 12-bit MONOCHROME1 native OW Pixel Data pattern. The generated
  file sets Presentation Intent Type `FOR PRESENTATION`, Modality `MG`,
  Image Laterality `L`, View Position `MLO`, DX Image rescale/window attributes,
  Presentation LUT Shape `INVERSE`, no lossy compression, no burned-in
  annotation, Imager Pixel Spacing, detector metadata, Anatomic Region Sequence,
  View Code Sequence, and an empty Acquisition Context Sequence. Validation now
  supports optional mammography checks for required DX/MG scalar attributes,
  code-sequence contents, and the Acquisition Context item count. The registry
  records 2026b `dicom-standard-kb` evidence for the MG SOP Class, Digital
  Mammography X-Ray Image IOD/modules, DX Series/Image/Detector/Anatomy,
  Mammography Series/Image, Acquisition Context, Presentation Intent Type,
  Imager Pixel Spacing, and Presentation LUT Shape.
- 2026-06-13: `classic/mg/for_processing_mono2_u16_12bit_implicit_le` adds
  Digital Mammography X-Ray Image Storage - For Processing coverage using
  Implicit VR Little Endian and the same tiny unsigned 12-bit native OW Pixel
  Data pattern. The generated file sets Presentation Intent Type
  `FOR PROCESSING`, Photometric Interpretation `MONOCHROME2`, Presentation LUT
  Shape `IDENTITY`, and omits Window Center/Width because the VOI LUT Module is
  not present for For Processing mammography. Validation now records dynamic SOP
  Class and transfer syntax standards checks plus paired mammography window
  presence/absence checks. The registry records 2026b `dicom-standard-kb`
  evidence for the For Processing SOP Class, Implicit VR Little Endian, Digital
  Mammography X-Ray Image IOD/modules, DX/MG modules, Presentation Intent Type,
  Photometric Interpretation, and the Window/VOI LUT conditional requirements.
- 2026-06-13: `classic/cr/overlay_modality_voi_explicit_le` adds Computed
  Radiography Image Storage coverage using Explicit VR Little Endian and a tiny
  2x2 8-bit MONOCHROME2 native OB Pixel Data pattern. The generated file sets
  CR Series type 2 Body Part Examined and View Position values, includes a
  group `6000` Overlay Plane with one 2x2 diagonal overlay, encodes a single
  item Modality LUT Sequence instead of rescale attributes, and encodes a
  single item VOI LUT Sequence instead of Window Center/Width. Validation now
  checks CR scalar attributes, overlay rows/columns/type/origin/bits/data, LUT
  descriptors, Modality LUT Type, and LUT Data VR/length. The registry records
  2026b `dicom-standard-kb` evidence for the CR SOP Class, Computed Radiography
  Image IOD/modules, CR Series/Image, Overlay Plane, Modality LUT, VOI LUT, and
  key overlay/LUT data elements.
- 2026-06-13: `classic/mr/multislice_oblique_explicit_le` adds MR Image Storage
  coverage using a three-instance Explicit VR Little Endian series. The files
  are emitted as `slice-001.dcm` through `slice-003.dcm`, share deterministic
  Study, Series, and Frame of Reference UIDs, and use deterministic per-slice
  SOP Instance UIDs. Each instance has 2x2 16-bit MONOCHROME2 native OW Pixel
  Data, Image Plane geometry with an oblique Image Orientation Patient
  `0.70710678\0.70710678\0\0\0\1`, advancing Image Position Patient values,
  Slice Location, Spacing Between Slices, and minimal MR Image acquisition
  attributes. Validation now checks MR scalar attributes and recomputes the
  slice-normal position used for deterministic geometry sorting. The registry
  records 2026b `dicom-standard-kb` evidence for MR Image Storage, the MR Image
  IOD/modules, MR Image, Image Plane, Frame of Reference, and key MR acquisition
  data elements.
- 2026-06-13: `classic/dx/display_shutter_mono2_u16_explicit_le` adds Digital
  X-Ray Image Storage - For Presentation coverage using Explicit VR Little
  Endian and a tiny unsigned 12-bit MONOCHROME2 native OW Pixel Data pattern.
  The generated file sets Presentation Intent Type `FOR PRESENTATION`, Modality
  `DX`, Image Laterality `U`, DX Image rescale/window attributes, Presentation
  LUT Shape `IDENTITY`, no lossy compression, no burned-in annotation, Imager
  Pixel Spacing, detector metadata, Anatomic Region Sequence, an empty
  Acquisition Context Sequence, and a rectangular Display Shutter with a
  monochrome Shutter Presentation Value. Validation now supports optional
  Digital X-Ray checks for DX scalar attributes, code-sequence contents,
  Acquisition Context item count, and Display Shutter shape/edge/value
  metadata. The registry records 2026b `dicom-standard-kb` evidence for Digital
  X-Ray Image Storage, the Digital X-Ray Image IOD/modules, DX Series/Anatomy
  Imaged/Image/Detector modules, Display Shutter, Presentation Intent Type, and
  shutter data elements.
- 2026-06-13: `classic/us/mono2_u8_explicit_le` adds Ultrasound Image Storage
  coverage using Explicit VR Little Endian and a tiny 2x2 8-bit MONOCHROME2
  native OB Pixel Data pattern. The generated file sets Modality `US`, Image
  Type `ORIGINAL\PRIMARY`, no-lossy-compression metadata, and
  Ultrasound Color Data Present `0`, while avoiding optional region calibration
  and color/palette modules in this first conservative US slice. Validation now
  checks US scalar attributes and the Ultrasound Color Data Present flag. The
  registry records 2026b `dicom-standard-kb` evidence for Ultrasound Image
  Storage, the Ultrasound Image IOD/modules, US Image attributes, Image Pixel
  requirements in US context, Lossy Image Compression, and Ultrasound Color
  Data Present.
- 2026-06-13: `enhanced/ct/multiframe_shared_perframe_explicit_le` starts
  Phase 4 with a two-frame Enhanced CT Image Storage Part 10 case using
  Explicit VR Little Endian and native 16-bit unsigned MONOCHROME2 Pixel Data.
  The generated file sets deterministic Study, Series, SOP Instance, Frame of
  Reference, Dimension Organization, and Irradiation Event UIDs; encodes
  Number of Frames `2`; places Pixel Measures, Plane Orientation, Frame Anatomy,
  Irradiation Event Identification, CT Image Frame Type, and CT Pixel Value
  Transformation macros in Shared Functional Groups; and places Frame Content
  plus Plane Position macros in Per-Frame Functional Groups. Validation now
  checks Enhanced CT scalar attributes, functional group sequence item counts,
  dimension metadata, shared rescale metadata, irradiation event UID, and
  per-frame Image Position Patient values. The registry records 2026b
  `dicom-standard-kb` evidence for Enhanced CT Image Storage, the Enhanced CT
  IOD/modules, Multi-frame Functional Groups, Multi-frame Dimension, and the
  functional group macros used by this first conservative Enhanced CT slice.
- 2026-06-13: `enhanced/mr/multiframe_echo_perframe_explicit_le` adds the first
  Enhanced MR Image Storage Part 10 case using Explicit VR Little Endian and
  native 16-bit unsigned MONOCHROME2 Pixel Data. The generated file sets
  deterministic Study, Series, SOP Instance, Frame of Reference, and Dimension
  Organization UIDs; encodes Number of Frames `2`; places Pixel Measures, Plane
  Orientation, Frame Anatomy, MR Image Frame Type, Pixel Value Transformation,
  and MR Timing and Related Parameters macros in Shared Functional Groups; and
  places Frame Content, Plane Position, and MR Echo macros in Per-Frame
  Functional Groups with frame-varying Effective Echo Time values. Validation
  now checks Enhanced MR scalar attributes, sequence item counts, dimension
  metadata, shared timing/rescale metadata, per-frame Image Position Patient,
  and per-frame Effective Echo Time. The registry records 2026b
  `dicom-standard-kb` evidence for Enhanced MR Image Storage, the Enhanced MR
  IOD/modules, Multi-frame Functional Groups, Multi-frame Dimension, and the MR
  Image Frame Type, common Image Flavor, MR Timing, MR Echo, and Volume Based
  Calculation Technique terms.
- 2026-06-13: `enhanced/mr/multiframe_temporal_position_explicit_le` adds
  frame-varying Temporal Position coverage to Enhanced MR Image Storage using
  Explicit VR Little Endian and native 16-bit unsigned MONOCHROME2 Pixel Data.
  The generated file uses the common `DYNAMIC` Image Flavor, repeats the same
  Plane Position across two frames, indexes the Multi-frame Dimension by
  Temporal Position Time Offset, and places Temporal Position Sequence plus
  Temporal Position Index in Per-Frame Functional Groups. Validation now checks
  per-frame Temporal Position Index and Temporal Position Time Offset in
  addition to the existing Enhanced MR shared functional group checks. The
  registry records 2026b `dicom-standard-kb` evidence for Temporal Position
  Macro attributes and the PS3.6 Temporal Position data elements.
- 2026-06-13: `enhanced/mr/multiframe_phase_velocity_encoding_explicit_le`
  adds frame-varying phase-contrast-oriented velocity encoding coverage to
  Enhanced MR Image Storage using Explicit VR Little Endian and native 16-bit
  unsigned MONOCHROME2 Pixel Data. The generated file keeps the object
  `DERIVED` to avoid expanding the first phase slice into a full ORIGINAL/MIXED
  MR Pulse Sequence build, indexes the Multi-frame Dimension by Velocity
  Encoding Direction, and places MR Velocity Encoding Sequence in Per-Frame
  Functional Groups with different direction vectors for each frame. Validation
  now checks per-frame Velocity Encoding Direction plus minimum and maximum
  velocity values. The registry records 2026b `dicom-standard-kb` evidence for
  the MR Velocity Encoding macro, velocity direction semantics, PS3.6 velocity
  encoding data elements, and Phase Contrast references from the MR Pulse
  Sequence module.
- 2026-06-13: `enhanced/ct/concatenation_two_part_explicit_le` completes
  Phase 4 with a two-member Enhanced CT Image Storage concatenation using
  Explicit VR Little Endian. The generated case writes `part-001.dcm` and
  `part-002.dcm`, each with one physical frame, distinct SOP Instance UIDs, the
  same Concatenation UID, the same SOP Instance UID of Concatenation Source,
  In-concatenation Numbers `1` and `2`, and Concatenation Frame Offset Numbers
  `0` and `1`. Shared Functional Groups and Dimension Organization metadata
  remain common across the members, while each member's Per-Frame Functional
  Groups carry the relevant Plane Position and logical Dimension Index Values.
  Validation now checks the concatenation attributes, top-level frame counts,
  logical dimension values, and readback of both generated Part 10 files. The
  registry records 2026b `dicom-standard-kb` evidence for Enhanced CT Image
  Storage, Multi-frame Functional Groups, Multi-frame Dimension, Concatenation
  UID, In-concatenation Number, In-concatenation Total Number, Concatenation
  Frame Offset Number, SOP Instance UID of Concatenation Source, and the
  ordering/source UID semantics from PS3.3 C.7.6.16.
- 2026-06-13: Remediation R1 started by making registry `status` authoritative
  for generation. Recipe selection now writes only matching `status:
  implemented` cases; `planned` cases are reported as unavailable with
  phase-aware `recheck_phase` metadata; `skipped` and `blocked` cases preserve
  their structured registry `skip` object in manifest `skipped_cases`; and
  `deprecated` cases remain listable but are excluded from generation and
  skipped-case accounting. Regression tests cover implemented, planned,
  skipped, blocked, and deprecated status behavior, plus `generate --profile
  core` reporting the two planned VL cases as unavailable for Phase 7.

## Current Blockers

The 2026-06-13 baseline review found remediation items that should be resolved
before Phase 5 feature expansion: the planned SEG priority case is missing from
`cases/registry.json`, registry/recipe standards evidence deduplication is still
pending, `list-cases --status`, `validate`, `report`, and `standards` CLI
commands are not implemented, validation does not yet enforce every required
Part 10 invariant, and reproducibility/schema/CI guard coverage is incomplete.
The local `dicom-standard-kb` repository commit/DB SHA-256 and official source
artifact hashes also remain unverified.

## Recommended Next Commit

Execute `REMEDIATION_PLAN.md` before starting new Phase 5 feature work:

1. Add the missing planned `derived/seg/binary_multiframe_explicit_le` registry
   entry.
2. Deduplicate generated file `standards_evidence` where registry and recipe
   evidence overlap, then mark R1 complete if its exit criteria are satisfied.
3. Complete the expected CLI, validation, reproducibility, CI guard, and
   standards-lock cleanup phases described in the remediation plan.
4. Resume Phase 5 with `feat(seg): add binary segmentation case` only after the
   remediation exit criteria are satisfied.

## Handoff Notes

- Use `rg`/`rg --files` first when inspecting the repository.
- Stage files selectively for each logical unit of work.
- After each completed task, run `git log --oneline -3` and confirm the new
  commit is present.
- If standards information is missing from `dicom-standard-kb`, add a local
  source note or mark the case blocked; do not make uncited assumptions.
- Keep smoke cases tiny, byte-stable, and free of optional codec requirements.
