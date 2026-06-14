# Implementation Progress

**Last updated:** 2026-06-14  
**Source specification:** `SYSTEM_SPEC.md` version 0.2.0  
**Current phase:** Phase 5.5 RT Structure Set complete; RT Dose next

**Current implementation status:** Phase 0, Phase 0.5, Phase 1, Phase 2, Phase 3, Phase 4, the pre-Phase-5 hardening pass, Phase 5.0 foundation, Phase 5.1 BINARY Segmentation Storage, Phase 5.2 BINARY/FRACTIONAL/LABELMAP segmentation coverage, Phase 5.3 Presentation State/RWVM, Phase 5.4 SR/KOS, and the Phase 5.5 RT Structure Set slice are functionally complete. `IMPLEMENTATION_PLAN.md` defines the concrete Phase 5 implementation sequence. Phase 5.5 now includes `non-image/rt/structure_set_single_roi_explicit_le`, a non-image RT Structure Set Storage object with modality `RTSTRUCT`, one manual closed planar ROI, ROI Contour and RT ROI Observations sequences, Common Instance Reference and RT referenced frame-of-reference paths to the generated Enhanced CT source, and no Pixel Data. The extended profile now writes 15 files and reports 2 remaining planned Phase 5 cases.

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
| `IMPLEMENTATION_PLAN.md` | present | Phase 5 implementation plan with foundation, SEG, GSPS/RWVM, SR/KOS, RT, Encapsulated PDF, verification, and commit-boundary guidance. |
| `.gitignore` | present | Covers generated DICOM outputs, reports, sidecars, caches, generated standards artifacts, and SQLite KB files. |
| `Cargo.toml` / Rust workspace | present | Single package named `dicom-test-suite`, using Rust 2024 edition; pins minimal DICOM-rs crates for Phase 1 object and transfer syntax work. |
| `build.rs` | present | Captures Rust compiler version and target triple for generated manifest metadata. |
| `rust-toolchain.toml` | present | Pins Rust 1.85.0 with `rustfmt` and `clippy`, matching an installed local toolchain. |
| `standards.lock.json` | present | Locks to DICOM 2026b base edition only using the pinned `dicom-standard-kb` MCP source manifest; unavailable KB commit, local DB hash, and official source artifact hashes are documented with explicit non-fatal statuses. |
| `schemas/` | present | Manifest, case registry, coverage report, and viewer report schemas have initial structured coverage. |
| `cases/taxonomy.md` | present | Documents normalized case ID format, path segments, descriptor conventions, profile definitions, and inclusion rules. |
| `cases/registry.json` | present | Tracks implemented smoke/core SC cases, classic radiology CT/MG/CR/MR/DX/US cases, Enhanced CT, Enhanced CT concatenation, Enhanced MR extended cases, planned SEG, and planned VL cases with standards evidence from `dicom-standard-kb` MCP lookups. |
| `transfer-syntax/capability-matrix.json` | present | Records initial read/decode/write/encode, feature, external library, and determinism capabilities for baseline native transfer syntaxes. |
| `docs/deterministic-build-policy.md` | present | Documents determinism levels, reproducibility inputs, UID derivation, metadata controls, hashes, and two-run verification. |
| `standards/kb-integration.md` | present | Documents the pinned 2026b `dicom-standard-kb` MCP query workflow, evidence fields, and fallback path. |
| `standards/gap-workflow.md` | present | Documents standards gap handling, local source notes, blocked/skipped registry actions, and KB patch criteria. |
| `standards/source-notes/` | present | Contains a README/template plus `uid-2-25.md` for the PS3.5 UID root gap not covered by `dicom-standard-kb`. |
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
| Pre-Phase-5 hardening | complete | Registry authority, required CLI contracts, validation hardening, reproducibility/CI guards, and standards lock pinning policy are complete. Validation now covers raw Part 10 byte checks, parsed cross-field image invariants, manifest schema-conformance checks, baseline standards-derived Type 1/Type 2 checks, classic family-specific checks, and Enhanced CT/MR multi-frame standards-derived checks. |
| Phase 5: Derived, presentation, and non-image objects | Phase 5.5 RT Structure Set complete; RT Dose next | Full planned target queue is now in `cases/registry.json`. Manifest entries support nullable/absent image metadata plus a generated-file `references` array, coverage reports project manifest reference source case IDs into `derived_refs`, generated-root validation resolves same-run references while skipping image/pixel checks for non-image rows, and generation maintains an ordered source object registry for derived recipes. BINARY, FRACTIONAL, LABELMAP Segmentation, Grayscale Softcopy Presentation State, Real World Value Mapping, Basic Text SR, Comprehensive SR, Key Object Selection, and RT Structure Set objects are implemented and validated. The next implementation slice is RT Dose. |
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

## Phase 5 Checklist

- [x] Prepare concrete Phase 5 implementation plan in `IMPLEMENTATION_PLAN.md`.
- [x] Add planned registry rows and standards evidence for all Phase 5 target
      cases.
- [x] Refactor manifest schema, generated-root validation, and coverage reports
      to represent non-image objects and derived references.
- [x] Add generation-time same-run source object map for derived recipes.
      Generated-root reference-resolution validation is complete; generation
      now records each written manifest entry into an ordered source object
      registry and exposes already-generated source instances to later recipe
      code.
- [x] Implement BINARY Segmentation Storage case.
- [x] Implement FRACTIONAL Segmentation Storage case.
- [x] Implement LABELMAP Segmentation using Label Map Segmentation Storage.
- [x] Implement Grayscale Softcopy Presentation State case.
- [x] Implement Real World Value Mapping case.
- [x] Implement Basic Text SR case.
- [x] Implement Comprehensive SR case.
- [x] Implement Key Object Selection case.
- [x] Implement RT Structure Set detection case.
- [ ] Implement RT Dose detection case.
- [ ] Implement Encapsulated PDF detection case.
- [ ] Run Phase 5 completion verification across generation, validation,
      reporting, reproducibility, standards gaps, and artifact guards.

Phase 5 is complete only when the extended/all profiles include all planned
Phase 5 target cases, generated derived objects resolve references to source
objects generated in the same run, and viewers can use the corpus to test
graceful handling of common derived and non-image SOP Classes.

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
| `derived/seg/binary_multiframe_explicit_le` | `extended` | implemented |
| `derived/seg/fractional_probability_multiframe_explicit_le` | `extended` | implemented |
| `derived/seg/labelmap_multiframe_explicit_le` | `extended` | implemented |
| `vl/photo/rgb_planar0_explicit_le` | `core` | planned |
| `vl/photo/palette_color_explicit_le` | `core` | planned |

## Open Decisions

| Decision | Status | Notes |
|---|---|---|
| Rust workspace layout | decided 2026-06-13 | Start as a single package named `dicom-test-suite`; keep module boundaries compatible with later spec crates. |
| Rust edition and toolchain | decided 2026-06-13 | Use Rust 2024 edition and pin toolchain `1.85.0`, the installed local stable toolchain sufficient for edition 2024. |
| DICOM-rs versions | decided 2026-06-13 | Crates.io verification found `dicom` 0.9.1, `dicom-object` 0.9.1, `dicom-core` 0.9.1, `dicom-transfer-syntax-registry` 0.9.1, and `dicom-dictionary-std` 0.9.0; pin minimal direct dependencies exactly and leave optional pixel/UL codecs disabled. |
| Standards baseline | decided 2026-06-13 | Use DICOM 2026b base edition only and exclude post-base final text until `standards.lock.json` is deliberately updated. |
| `dicom-standard-kb` pin | decided with documented unavailable fields 2026-06-13 | The available MCP is pinned to generated 2026b reference data with source manifest SHA-256 `9959bee76fd293c7eda3fc81ce2ced7528612faa1b2df28cccd01504a83f54b0`; repository commit and local DB SHA-256 are field-specific non-fatal unavailable pins until exposed or independently verified. |
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
- 2026-06-13: Remediation R1 restored the planned
  `derived/seg/binary_multiframe_explicit_le` registry entry in the `extended`
  profile. The entry is standards-backed by 2026b `dicom-standard-kb` lookups
  for Segmentation Storage, Explicit VR Little Endian, the Segmentation IOD and
  module table, Segmentation Type, Segment Sequence, and BINARY segmentation
  standard text. `list-cases --profile extended` now shows the planned SEG row,
  and `generate --profile extended` reports it as an unavailable planned case
  with `recheck_phase` `phase-5`.
- 2026-06-13: Remediation R1 completed by deduplicating generated file
  `standards_evidence` entries. Manifest builders now preserve first-seen
  registry evidence and drop later recipe evidence with the same
  source/edition/query key, keeping generated manifests deterministic and
  avoiding duplicated citations. Focused tests cover the deduplication helper,
  smoke/core/extended generation, planned VL skipped rows, planned SEG skipped
  rows, blocked status prevention, and all registry statuses.
- 2026-06-13: Remediation R2 started with `list-cases --status <status>`.
  The command now supports `planned`, `implemented`, `skipped`, `blocked`, and
  `deprecated` filters, can combine `--status` with `--profile`, and returns a
  clear non-zero error for unsupported statuses. Library and CLI tests cover
  combined extended/planned filtering and unknown status rejection.
- 2026-06-13: Remediation R2 added `validate <generated-root>`. The command
  reads `manifest.json`, checks generated file size and SHA-256, reopens each
  DICOM file, compares File Meta and dataset SOP/transfer/implementation UIDs
  against manifest metadata, verifies synthetic/image/pixel attributes, and
  recomputes native Pixel Data length from parsed image attributes. It exits
  non-zero for missing manifests and for validation failures while reporting
  per-file failure rows. CLI tests cover a valid smoke root, a missing manifest,
  and a corrupted generated file.
- 2026-06-13: Remediation R2 added
  `report <generated-root> --format json`. The command reads the generated
  manifest plus the case registry and emits `schemas/coverage-report.schema.json`
  compatible coverage counts, matrix rows, grouped coverage, and gap entries for
  generated and unavailable cases. Generated-root rows use the manifest run
  profile, generated rows carry DICOM/image/validation metadata from
  `manifest.json`, and planned/skipped/blocked rows use registry metadata. CLI
  tests cover a core JSON report and the missing-manifest error path.
- 2026-06-13: Remediation R2 added
  `report <generated-root> --format markdown` on top of the same coverage model
  used by JSON output. The Markdown view reports run metadata, status counts,
  grouped coverage, gap rows, and the full coverage matrix in tables suitable
  for generated human-readable reports. CLI tests cover core Markdown report
  counts and generated/planned case rows.
- 2026-06-13: Remediation R2 added `standards check-lock`. The command
  validates the committed `standards.lock.json` shape and current 2026b base
  policy fields, verifies the pinned `dicom-standard-kb` edition and source
  manifest SHA-256, checks required source artifact and verification query
  records, and emits warnings for documented unavailable pins such as the KB
  commit, DB SHA-256, and not-downloaded source artifact hashes. CLI tests cover
  the committed lock and a malformed lock error path.
- 2026-06-13: Remediation R2 added `standards gaps --profile <profile>`. The
  command scans registry entries for the selected profile and reports blocked,
  skipped, missing-evidence, uncovered-evidence, and source-note-backed
  standards gaps as TSV rows. It supports `--registry` for alternate registries
  and tests cover gap classification, profile filtering, and missing profile
  arguments.
- 2026-06-13: Remediation R2 completed with
  `standards verify-kb --edition 2026b`. Because the standalone binary cannot
  access the Codex `dicom-standard-kb` MCP server, repository checkout metadata,
  or local KB database hash at runtime, the command returns a clear
  `status\tunavailable` result with a recommended MCP verification path. The
  limitation is documented in `standards/kb-integration.md`, and CLI tests cover
  the unavailable status plus unsupported edition rejection.
- 2026-06-13: Remediation R3 started with raw Part 10 validation in
  `validate <generated-root>`. The validator now checks the 128-byte normal
  preamble is all zero, the `DICM` marker is at byte offset 128, File Meta
  Information is parseable as Explicit VR Little Endian, required File Meta
  elements are present with expected VRs and values, File Meta group length
  matches the dataset boundary, deterministic Implementation Version Name is
  preserved when present, and no group `0002` elements appear after the File
  Meta Information. CLI tests mutate generated files to cover non-zero preamble
  and missing File Meta Information Version failures.
- 2026-06-13: Remediation R3 cross-field image validation now uses parsed DICOM
  values instead of returning manifest values after comparison. Bits Stored,
  High Bit, frame count, and native Pixel Data length invariants are computed
  from the values read out of the file, and CLI mutation tests cover an
  unexpected group `0002` element after File Meta Information plus an
  inconsistent High Bit value.
- 2026-06-13: Remediation R3 added generated manifest schema-conformance
  checks to the smoke, core, and extended generation CLI tests. The tests read
  `schemas/manifest.schema.json` and verify generated manifests satisfy the
  committed schema's top-level and nested required-field contracts plus
  disallowed additional properties for the primary manifest sections.
- 2026-06-13: Remediation R3 added baseline standards-derived validation for
  generated-root checks. Based on 2026b `dicom-standard-kb` module lookups for
  Patient, General Study, General Series, and General Image, validation now
  requires the generated Type 2 Patient/Study/Series/Image identity attributes
  to be present, compares Type 1 Study Instance UID, Series Instance UID, and
  Modality to the manifest, and includes a CLI mutation test for a missing
  Patient's Name Type 2 attribute.
- 2026-06-13: Remediation R3 expanded generated-root validation with the first
  family-specific standards-derived checks. Based on 2026b
  `dicom-standard-kb` module lookups for SC Equipment, CT Image, Image Plane,
  and Frame of Reference, validation now checks Secondary Capture Conversion
  Type, CT Image Type/rescale/KVP/acquisition attributes, CT Frame of Reference
  identity, and CT image-plane geometry against manifest-backed expectations.
  CLI mutation tests cover missing SC Conversion Type and missing CT Image
  Type.
- 2026-06-13: Remediation R3 broadened family-specific standards-derived
  generated-root validation across the remaining classic image IOD families.
  Based on 2026b `dicom-standard-kb` lookups for Mammography Image, DX Image,
  DX Detector, US Image, CR Series, CR Image, MR Image, Image Plane, and Frame
  of Reference, validation now checks MG Image Type/Positioner/Image
  Laterality/Organ Exposed plus shared DX attributes, DX Image Type and
  Type 1 grayscale transformation/detector spacing attributes, US Image Type,
  CR Body Part Examined/View Position Type 2 attributes, and classic MR Image,
  MR acquisition, Frame of Reference, and image-plane attributes. CLI mutation
  tests cover missing MG Positioner Type, DX Presentation LUT Shape, US Image
  Type, CR Body Part Examined, and MR Scanning Sequence.
- 2026-06-13: Remediation R3 completed by adding Enhanced CT and Enhanced MR
  standards-derived generated-root validation. Based on 2026b
  `dicom-standard-kb` lookups for Enhanced CT Image, Enhanced MR Image,
  Multi-frame Functional Groups, Multi-frame Dimension, CT Series, MR Series,
  and MR Pulse Sequence, validation now checks enhanced Image Type/common image
  description Type 1 attributes, Frame of Reference identity, Shared and
  Per-Frame Functional Groups sequence counts, Dimension Organization, and
  Dimension Index. Validation CLI tests now accept a generated extended root
  and cover missing Enhanced CT Shared Functional Groups Sequence and missing
  Enhanced MR Dimension Organization Sequence.
- 2026-06-13: Remediation R4 started by expanding reproducibility and union
  profile coverage. Two-run byte-stability tests now cover `core` and
  `extended` in addition to `smoke`, and `generate --profile all` has a CLI
  regression test that validates the manifest schema, union file count,
  multi-file case accounting, and unavailable planned SEG/VL skipped-case rows.
- 2026-06-13: Remediation R4 added a generated-payload artifact guard. The
  project artifact tests now inspect tracked and staged git paths and fail if a
  generated DICOM payload, generated manifest, or generated validation/report
  sidecar is tracked or staged.
- 2026-06-13: Remediation R4 completed with registry/generator consistency
  guard tests. Project artifact tests now fail when an implemented registry
  case has no generator recipe, when a generator recipe lacks an implemented
  registry row, or when an initial priority case from `SYSTEM_SPEC.md` section
  21 is missing from the registry.
- 2026-06-13: Remediation R5 completed with documented-unavailable standards
  lock pins. `standards.lock.json` now records field-specific non-fatal
  unavailable statuses and reasons for the `dicom-standard-kb` repository
  commit, generated local KB database SHA-256, and official source artifact
  hashes; `standards check-lock` rejects nullable KB pins that lack those
  field-specific statuses. `standards/kb-integration.md` documents the null-pin
  policy.
- 2026-06-14: The resolved `REMEDIATION_PLAN.md` task list was removed from
  active project documentation. `IMPLEMENTATION_PROGRESS.md` remains the
  durable implementation ledger and now points future work directly at Phase 5.
- 2026-06-14: `IMPLEMENTATION_PLAN.md` was added as the concrete Phase 5 plan.
  Codebase review found that the current manifest schema and generated-root
  validator are image-first, so Phase 5.0 must add object-aware manifest,
  validation, reporting, and same-run reference infrastructure before broad
  non-image recipes. 2026b `dicom-standard-kb` MCP checks confirmed Phase 5 SOP
  Class UIDs for Segmentation Storage, Label Map Segmentation Storage,
  Grayscale Softcopy Presentation State Storage, Basic Text SR Storage,
  Comprehensive SR Storage, Key Object Selection Document Storage, Real World
  Value Mapping Storage, RT Dose Storage, RT Structure Set Storage, and
  Encapsulated PDF Storage.
- 2026-06-14: Phase 5.0 started by adding planned registry rows for the full
  Phase 5 target queue: FRACTIONAL SEG, LABELMAP SEG, GSPS, RWVM, Basic Text
  SR, Comprehensive SR, KOS, RT Structure Set, RT Dose, and Encapsulated PDF.
  Existing BINARY SEG remains planned, so the extended profile now reports 11
  Phase 5 planned cases. The new rows use Explicit VR Little Endian,
  byte-stable determinism, no external codec requirements, and 2026b
  `dicom-standard-kb` evidence for each SOP Class, IOD/module table, and
  selected implementation-driving data elements. Tests were updated so
  `list-cases`, generated manifest skip accounting, and registry artifact
  guards treat the full Phase 5 queue as durable state.
- 2026-06-14: Phase 5.0 manifest/report foundation continued by updating
  `schemas/manifest.schema.json` so file entries require a `references` array
  while `image` and `pixel_data` may be absent or explicitly `null` for
  non-image objects. Manifest assembly now supplies `references: []` for all
  existing generated image cases, preserving current generation behavior while
  establishing the schema contract future derived recipes will populate.
  Coverage reports now project manifest reference `source_case_id` values into
  generated rows' `derived_refs`; a synthetic non-image report test confirms
  reports keep photometric, bits, frames, and geometry empty instead of
  inventing image metadata. Generated-root validation is still image-first and
  must be made object-aware before flipping any non-image recipe to
  `implemented`.
- 2026-06-14: Phase 5.0 generated-root validation is now object-aware and
  reference-aware. `validate <generated-root>` builds a same-run source object
  map from manifest file entries, checks each manifest `references` row against
  generated source path, source case ID, SOP Class UID, SOP Instance UID,
  optional Series Instance UID, and optional frame numbers, and reports
  mismatches as validation failures. The existing Part 10, baseline identity,
  family-specific, and Synthetic Data checks still run for every file. Image
  and Pixel Data validation now runs only when both `image` and `pixel_data`
  manifest objects are present; rows with both absent or `null` are accepted as
  non-image object rows, and partial image/pixel metadata is rejected. CLI
  regression tests cover a resolved same-run reference and a mismatched
  referenced SOP Instance UID.
- 2026-06-14: Phase 5.0 foundation is complete. Generation now uses an
  internal `GenerationContext` with an ordered same-run source object registry.
  Each generated manifest entry is registered by source path and case ID after
  it is written, capturing SOP Class UID, SOP Instance UID, Series Instance UID
  when present, and frame count when present. The registry exposes helpers for
  future derived writers to build manifest `references` entries from only
  already-generated source instances. No Phase 5 recipe status was changed.
- 2026-06-14: Phase 5.1 BINARY Segmentation Storage is implemented.
  `derived/seg/binary_multiframe_explicit_le` now writes a tiny two-frame
  Explicit VR Little Endian Segmentation Storage object after the
  `enhanced/ct/multiframe_shared_perframe_explicit_le` source is generated.
  The SEG object shares the source Study Instance UID and Frame of Reference
  UID, uses its own deterministic SEG Series/SOP/Dimension Organization UIDs,
  sets `Synthetic Data (0008,001C)` to `YES`, encodes one BINARY segment with
  one-bit native OB Pixel Data, records Segment Sequence and per-frame Segment
  Identification/Derivation Image references, and includes Common Instance
  Reference back to the source image. The manifest records a `source_image`
  reference with frame numbers `[1, 2]`, reports bit depth 1 in coverage, and
  `cases/registry.json` now marks this case `implemented`.
- 2026-06-14: Phase 5.2 FRACTIONAL Segmentation Storage is implemented for
  `derived/seg/fractional_probability_multiframe_explicit_le`. The SEG writer
  now parameterizes bit depth, pixel length formula, pixel value ranges, and
  fractional Type 1C attributes across SEG variants. The new fractional case
  writes two 2x2 8-bit probability frames with `Segmentation Type`
  `FRACTIONAL`, `Segmentation Fractional Type` `PROBABILITY`, `Maximum
  Fractional Value` 255, per-frame Derivation Image references to the generated
  Enhanced CT source frames, and a Common Instance Reference to the source
  image. Internal generation validation and generated-root validation both
  check the fractional subtype and maximum fractional value. The manifest and
  coverage report show the fractional SEG as a derived/reference object with
  bit depth 8, and `cases/registry.json` now marks this case `implemented`.
- 2026-06-14: Phase 5.2 LABELMAP Segmentation is implemented for
  `derived/seg/labelmap_multiframe_explicit_le`. The SEG recipe now carries
  SOP Class UID/name metadata so BINARY and FRACTIONAL continue to use
  Segmentation Storage while LABELMAP uses Label Map Segmentation Storage
  `1.2.840.10008.5.1.4.1.1.66.7`. The new case writes two 2x2 8-bit labelmap
  frames with `Segmentation Type` `LABELMAP`, one Segment Sequence item,
  per-frame Segment Identification/Derivation Image references to the generated
  Enhanced CT source frames, and Common Instance Reference back to the source
  image. Generated-root segmentation validation now compares the SOP Class UID
  against each manifest entry rather than hardcoding only Segmentation Storage.
  The manifest and coverage report show the LABELMAP SEG as a derived/reference
  object with bit depth 8, and `cases/registry.json` now marks this case
  `implemented`.
- 2026-06-14: Phase 5.3 Grayscale Softcopy Presentation State is implemented
  for `derived/presentation-state/grayscale_softcopy_ct_window_explicit_le`.
  The GSPS recipe writes a non-image Explicit VR Little Endian Presentation
  State object after the generated Enhanced CT source, shares the source Study
  Instance UID, uses its own deterministic PR Series and SOP Instance UIDs,
  sets `Synthetic Data (0008,001C)` to `YES`, and records a same-run
  `source_image` manifest reference to the Enhanced CT object. The dataset
  includes Presentation Series Modality `PR`, Presentation State
  Identification, Presentation State Relationship with Referenced Series/Image
  Sequence, Displayed Area Selection covering the 2x2 source image, Softcopy
  VOI LUT window center `350` and width `1400`, and Presentation LUT Shape
  `IDENTITY`. The manifest schema now allows UID blocks without
  `frame_of_reference_uid` so non-image objects are not forced to claim an
  image-frame spatial identity. Coverage reports show the GSPS row as a
  derived non-image object with null photometric/bits/frames metadata and
  `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`.
- 2026-06-14: Phase 5.3 Real World Value Mapping is implemented for
  `derived/rwvm/linear_ct_mapping_explicit_le`. The RWVM recipe writes a
  non-image Explicit VR Little Endian Real World Value Mapping Storage object
  after the generated Enhanced CT source, shares the source Study Instance UID,
  uses its own deterministic RWV Series and SOP Instance UIDs, sets Synthetic
  Data to `YES`, and records a same-run `source_image` manifest reference to
  frames `[1, 2]` of the Enhanced CT object. The dataset includes Real World
  Value Mapping Series Modality `RWV`, Content Identification, a Real World
  Value Mapping Sequence item with LUT Label `DTS_HU`, mapped stored-value
  range `0..700`, Real World Value Intercept `-1024`, Real World Value Slope
  `1`, UCUM measurement units code `HU`, Referenced Image Sequence back to the
  source frames, and Common Instance Reference back to the source image. RWVM
  has a dedicated non-image generation validator. Coverage reports show the
  RWVM row as a derived non-image object with null photometric/bits/frames
  metadata and
  `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`.
- 2026-06-14: Phase 5.4 Basic Text SR is implemented for
  `derived/sr/basic_text_observation_explicit_le`. The Basic Text SR recipe
  writes a non-image Explicit VR Little Endian Basic Text SR Storage object
  after the generated Enhanced CT source, shares the source Study Instance UID,
  uses its own deterministic SR Series and SOP Instance UIDs, sets Synthetic
  Data to `YES`, and records a same-run `source_image` manifest reference to
  frames `[1, 2]` of the Enhanced CT object. The dataset includes SR Document
  Series Modality `SR`, SR Document General `Completion Flag` `COMPLETE` and
  `Verification Flag` `UNVERIFIED`, Current Requested Procedure Evidence
  referencing the source Study/Series/SOP Instance, and a minimal SR Document
  Content tree with a root `CONTAINER` title and one contained `TEXT`
  observation. Basic Text SR has dedicated generation validation and
  generated-root validation for SR flags, evidence references, root content,
  text content, and absence of Pixel Data. Coverage reports show the Basic Text
  SR row as a derived non-image object with null photometric/bits/frames
  metadata and
  `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`.
- 2026-06-14: Phase 5.4 Comprehensive SR is implemented for
  `derived/sr/comprehensive_measurement_explicit_le`. The Comprehensive SR
  recipe writes a non-image Explicit VR Little Endian Comprehensive SR Storage
  object after the generated Enhanced CT source, shares the source Study
  Instance UID, uses its own deterministic SR Series and SOP Instance UIDs,
  sets Synthetic Data to `YES`, and records a same-run `source_image` manifest
  reference to frames `[1, 2]` of the Enhanced CT object. The dataset includes
  SR Document Series Modality `SR`, SR Document General `Completion Flag`
  `COMPLETE` and `Verification Flag` `UNVERIFIED`, Current Requested Procedure
  Evidence referencing the source Study/Series/SOP Instance, and an SR Document
  Content tree with a root `CONTAINER`, one `NUM` measurement content item
  with Numeric Value `12.5` and UCUM millimeter units, and one `IMAGE` content
  item referencing the source SOP Instance and frame numbers. Generated-root
  validation now treats Basic Text SR and Comprehensive SR as SR-family
  non-image objects and validates the Comprehensive SR measurement and image
  reference shape from manifest semantics. Coverage reports show the
  Comprehensive SR row as a derived non-image object with null
  photometric/bits/frames metadata and
  `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`.
- 2026-06-14: Phase 5.4 Key Object Selection is implemented for
  `derived/sr/key_object_selection_explicit_le`. The KOS recipe writes a
  non-image Explicit VR Little Endian Key Object Selection Document Storage
  object after the generated Enhanced CT source and BINARY SEG source, shares
  the source Study Instance UID, uses its own deterministic KO Series and SOP
  Instance UIDs, sets Synthetic Data to `YES`, and records two same-run
  manifest references: `source_image` to Enhanced CT frames `[1, 2]` and
  `key_object_segmentation` to the generated BINARY SEG object. The dataset
  includes Key Object Document Series Modality `KO`, SR Document General
  `Completion Flag` `COMPLETE` and `Verification Flag` `UNVERIFIED`, Current
  Requested Procedure Evidence with both source series/SOP references, and an
  SR Document Content tree with a root `CONTAINER` title `Of Interest` and two
  contained `IMAGE` content items. Generated-root validation now treats Key
  Object Selection Document as an SR-family non-image object, compares modality
  from the manifest instead of assuming `SR`, and validates KOS key object
  content items against manifest references. Coverage reports show the KOS row
  as a derived non-image object with null photometric/bits/frames metadata and
  `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le",
  "derived/seg/binary_multiframe_explicit_le"]`.
- 2026-06-14: Phase 5.5 RT Structure Set is implemented for
  `non-image/rt/structure_set_single_roi_explicit_le`. The RT Structure Set
  recipe writes a non-image Explicit VR Little Endian RT Structure Set Storage
  object after the generated Enhanced CT source, shares the source Study
  Instance UID and Frame of Reference UID, uses its own deterministic RTSTRUCT
  Series and SOP Instance UIDs, sets Synthetic Data to `YES`, and records a
  same-run `source_image` manifest reference to frames `[1, 2]` of the source
  Enhanced CT object. The dataset includes RT Series Modality `RTSTRUCT`,
  Structure Set Label/Date/Time, Referenced Frame of Reference Sequence with RT
  Referenced Study/Series and Contour Image references, one Structure Set ROI
  Sequence item with ROI Generation Algorithm `MANUAL`, one ROI Contour
  Sequence item with a `CLOSED_PLANAR` contour, one RT ROI Observations
  Sequence item with interpreted type `ORGAN`, Common Instance Reference, and
  no Pixel Data. Generation-time validation and generated-root validation now
  check the RTSTRUCT module shape, ROI/contour/observation consistency, source
  references, and absence of Pixel Data. Coverage reports show the RT Structure
  Set row as a non-image reference object with null photometric/bits/frames
  metadata and
  `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`.

## Verification Results

- 2026-06-14 Phase 5.5 RT Structure Set slice:
  - `dicom-standard-kb` MCP lookups rechecked RT Structure Set Storage, the RT
    Structure Set IOD, IOD modules, RT Series, Structure Set, ROI Contour, RT
    ROI Observations, Frame of Reference, and Contour Data. Parsed
    defined/enumerated term lookup for ROI Generation Algorithm, Contour
    Geometric Type, and RT ROI Interpreted Type was unavailable, but the module
    rows returned the needed terms `MANUAL`, `CLOSED_PLANAR`, and `ORGAN`.
  - Initial `cargo fmt -- --check` failed on formatting in `src/generator.rs`,
    `src/lib.rs`, and `src/validation.rs`; `cargo fmt` was run and the
    repeated `cargo fmt -- --check` passed.
  - Initial focused
    `cargo test --test generate_cli --test validate_cli --test list_cases_cli --test project_artifacts`
    failed to compile because one RT validation sequence-count path treated an
    `Option` as a `Result`; the sequence handling was corrected.
  - Repeated focused
    `cargo test --test generate_cli --test validate_cli --test list_cases_cli --test project_artifacts`
    passed.
  - `cargo test` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /tmp/dts-rtstruct-slice --seed 1`
    passed, writing 15 files.
  - `cargo run -- validate /tmp/dts-rtstruct-slice` passed with 15 files
    checked and 0 validation failures.
  - `cargo run -- report /tmp/dts-rtstruct-slice --format json` passed with
    counts `generated=15`, `planned=2`, `skipped=0`, `blocked=0`; the RT
    Structure Set row reports
    `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`, SOP
    Class UID `1.2.840.10008.5.1.4.1.1.481.3`, null
    photometric/bits/frames fields, object type `non-image`, and validation
    status `passed`.
  - `cargo run -- report /tmp/dts-rtstruct-slice --format markdown` passed with
    the expected generated RT Structure Set row and 2 remaining Phase 5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5.4 Key Object Selection slice:
  - `dicom-standard-kb` MCP lookups rechecked Key Object Selection Document
    Storage, the Key Object Selection Document IOD, IOD modules, Current
    Requested Procedure Evidence Sequence, Content Sequence, and Completion
    Flag.
  - Initial `cargo fmt -- --check` failed on rustfmt wrapping in
    `src/generator.rs`, `src/lib.rs`, `src/validation.rs`,
    `tests/generate_cli.rs`, and `tests/project_artifacts.rs`; `cargo fmt` was
    run, and the repeated `cargo fmt -- --check` passed.
  - Initial `cargo test` failed on stale extended/all generated-file counts in
    `tests/generate_cli.rs`; tests were updated for 14 extended files, 36
    all-profile files, and 3 remaining planned Phase 5 cases.
  - Repeated `cargo test` failed on stale extended validation count in
    `tests/validate_cli.rs`; the test was updated for 14 checked extended
    files.
  - Repeated `cargo test` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /tmp/dts-kos-slice --seed 1`
    passed, writing 14 files.
  - `cargo run -- validate /tmp/dts-kos-slice` passed with 14 files checked and
    0 validation failures.
  - `cargo run -- report /tmp/dts-kos-slice --format json` passed with counts
    `generated=14`, `planned=3`, `skipped=0`, `blocked=0`; the KOS row reports
    `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le",
    "derived/seg/binary_multiframe_explicit_le"]`, SOP Class UID
    `1.2.840.10008.5.1.4.1.1.88.59`, null photometric/bits/frames fields, and
    validation status `passed`.
  - `cargo run -- report /tmp/dts-kos-slice --format markdown` passed with the
    expected generated KOS row and 3 remaining Phase 5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5.4 Comprehensive SR slice:
  - `dicom-standard-kb` MCP lookups rechecked Comprehensive SR Storage, the
    Comprehensive SR IOD, IOD modules, Measured Value Sequence, Numeric Value,
    Measurement Units Code Sequence, Referenced SOP Sequence, Relationship
    Type, Value Type, Concept Name Code Sequence, and SR Document Content text.
  - `cargo test --test generate_cli --test validate_cli --test list_cases_cli --test project_artifacts --no-run`
    passed.
  - `cargo test --test generate_cli --test validate_cli --test list_cases_cli --test project_artifacts`
    passed.
  - `cargo test --test report_cli` passed.
  - `cargo fmt -- --check` passed.
  - `cargo test` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /private/tmp/dts-comprehensive-sr-slice --seed 1`
    passed, writing 13 files.
  - `cargo run -- validate /private/tmp/dts-comprehensive-sr-slice` passed
    with 13 files checked and 0 validation failures.
  - `cargo run -- report /private/tmp/dts-comprehensive-sr-slice --format json`
    passed with counts `generated=13`, `planned=4`, `skipped=0`,
    `blocked=0`; the Comprehensive SR row reports
    `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`, SOP
    Class UID `1.2.840.10008.5.1.4.1.1.88.33`, null
    photometric/bits/frames fields, and validation status `passed`.
  - `cargo run -- report /private/tmp/dts-comprehensive-sr-slice --format markdown`
    passed with the expected generated Comprehensive SR row and 4 remaining
    Phase 5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5.4 Basic Text SR slice:
  - `dicom-standard-kb` MCP lookups rechecked Basic Text SR Storage, the Basic
    Text SR IOD, IOD modules, SR Document General, SR Document Content, Value
    Type, Content Sequence, Completion Flag, Verification Flag, Concept Name
    Code Sequence, and Text Value.
  - Initial
    `cargo test --test generate_cli --test list_cases_cli --test project_artifacts --test validate_cli`
    failed because stale tests still expected the extended/all file counts and
    skipped-case counts from before Basic Text SR was implemented; the tests
    were updated for 12 extended files, 34 all-profile files, and 5 remaining
    planned Phase 5 cases.
  - Repeated
    `cargo test --test generate_cli --test list_cases_cli --test project_artifacts --test validate_cli`
    passed.
  - `cargo fmt -- --check` passed.
  - `cargo test` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /private/tmp/dts-basic-text-sr-verification --seed 1`
    passed, writing 12 files.
  - `cargo run -- validate /private/tmp/dts-basic-text-sr-verification` passed
    with 12 files checked and 0 validation failures.
  - `cargo run -- report /private/tmp/dts-basic-text-sr-verification --format json`
    passed with counts `generated=12`, `planned=5`, `skipped=0`, `blocked=0`;
    the Basic Text SR row reports
    `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`, SOP
    Class UID `1.2.840.10008.5.1.4.1.1.88.11`, null photometric/bits/frames
    fields, and validation status `passed`.
  - `cargo run -- report /private/tmp/dts-basic-text-sr-verification --format markdown`
    passed with the expected generated Basic Text SR row and 5 remaining Phase
    5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5.3 Real World Value Mapping slice:
  - `dicom-standard-kb` MCP lookups rechecked Real World Value Mapping Storage,
    the Real World Value Mapping IOD, IOD modules, Real World Value Mapping
    Module attributes, Real World Value Mapping Series attributes, Real World
    Value Mapping Sequence, Measurement Units Code Sequence, and Real World
    Value Slope.
  - `cargo fmt -- --check` passed.
  - `cargo test --test generate_cli --test list_cases_cli --test project_artifacts`
    passed.
  - Initial `cargo test` failed because
    `tests/validate_cli.rs::validate_command_accepts_generated_extended_root`
    still expected `files_checked\t10`; the test was updated for the new 11th
    extended file.
  - `cargo test --test validate_cli` passed.
  - Repeated `cargo test` passed.
  - Repeated `cargo fmt -- --check` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /tmp/dts-slice-rwvm --seed 1`
    passed, writing 11 files.
  - `cargo run -- validate /tmp/dts-slice-rwvm` passed with 11 files checked
    and 0 validation failures.
  - `cargo run -- report /tmp/dts-slice-rwvm --format json` passed with counts
    `generated=11`, `planned=6`, `skipped=0`, `blocked=0`; the RWVM row
    reports
    `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`,
    SOP Class UID `1.2.840.10008.5.1.4.1.1.67`, null
    photometric/bits/frames fields, and validation status `passed`.
  - `cargo run -- report /tmp/dts-slice-rwvm --format markdown` passed with
    the expected generated RWVM row and 6 remaining Phase 5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5.3 Grayscale Softcopy Presentation State slice:
  - `dicom-standard-kb` MCP lookups rechecked Grayscale Softcopy Presentation
    State Storage, the Grayscale Softcopy Presentation State IOD, IOD modules,
    Presentation State Relationship with expanded Image SOP Instance Reference
    Macro, Presentation Series, Presentation State Identification, Displayed
    Area, Softcopy VOI LUT, Softcopy Presentation LUT, Presentation State
    Shutter, and Presentation State Mask.
  - Initial `cargo fmt -- --check` failed on rustfmt wrapping in
    `src/generator.rs`, `src/validation.rs`, and `tests/generate_cli.rs`;
    `cargo fmt` was run, and repeated `cargo fmt -- --check` passed.
  - Initial
    `cargo test --test generate_cli --test validate_cli --test list_cases_cli --test project_artifacts --test report_cli`
    failed on stale all-profile generated-file counts and a manifest schema
    helper that still required `uids.frame_of_reference_uid` for every file.
    Tests and `schemas/manifest.schema.json` were updated so non-image
    manifest rows may omit Frame of Reference UID, and the repeated focused
    command with `--test schema_artifacts` included passed.
  - `cargo test --test generate_cli --test validate_cli --test list_cases_cli --test project_artifacts --test report_cli --test schema_artifacts`
    passed.
  - `cargo test` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /private/tmp/dts-gsps-slice-20260614 --seed 1`
    passed, writing 10 files.
  - `cargo run -- validate /private/tmp/dts-gsps-slice-20260614` passed with
    10 files checked and 0 validation failures.
  - `cargo run -- report /private/tmp/dts-gsps-slice-20260614 --format json`
    passed with counts `generated=10`, `planned=7`, `skipped=0`,
    `blocked=0`; the GSPS row reports
    `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`,
    SOP Class UID `1.2.840.10008.5.1.4.1.1.11.1`, null photometric/bits/frames
    fields, and validation status `passed`.
  - `cargo run -- report /private/tmp/dts-gsps-slice-20260614 --format markdown`
    passed with the expected generated GSPS row and 7 remaining Phase 5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5.2 LABELMAP Segmentation Storage slice:
  - `dicom-standard-kb` MCP lookups rechecked Label Map Segmentation Storage,
    `LabelMapSegmentationStorage`, Segmentation IOD, Segmentation Type, Bits
    Allocated, and PS3.3 source text for `Segmentation Type LABELMAP`. Parsed
    enumerated-value lookup for Segmentation Type remains unavailable, matching
    the known SEG value lookup limitation, but source-text search returned
    PS3.3 `sect_C.8.20.2.3.3`, `sect_C.8.20.2`, and `table_C.8.20-2`.
  - `cargo fmt -- --check` passed.
  - Initial focused test-name commands with unmatched filters ran 0 tests and
    were superseded by the full focused binaries below.
  - `cargo test --test list_cases_cli` passed.
  - `cargo test --test project_artifacts` passed.
  - `cargo test --test generate_cli` initially failed on stale extended/all
    generated-file counts; tests were updated, and the repeated command passed.
  - `cargo test --test validate_cli` initially failed on the stale extended
    `files_checked` count; the test was updated, and the repeated command
    passed as part of full `cargo test`.
  - `cargo test --test report_cli` passed.
  - `cargo test` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /private/tmp/dts-labelmap-slice-20260614-0339 --seed 1`
    passed, writing 9 files.
  - `cargo run -- validate /private/tmp/dts-labelmap-slice-20260614-0339`
    passed with 9 files checked and 0 validation failures.
  - `cargo run -- report /private/tmp/dts-labelmap-slice-20260614-0339 --format json`
    passed with counts `generated=9`, `planned=8`, `skipped=0`, `blocked=0`;
    the LABELMAP SEG row reports
    `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`,
    SOP Class UID `1.2.840.10008.5.1.4.1.1.66.7`, and bit depth 8.
  - `cargo run -- report /private/tmp/dts-labelmap-slice-20260614-0339 --format markdown`
    passed with the expected generated LABELMAP SEG row and 8 remaining Phase 5
    gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5.2 FRACTIONAL Segmentation Storage slice:
  - `dicom-standard-kb` MCP lookups rechecked Segmentation Storage,
    Segmentation IOD, Segmentation Type, Segmentation Fractional Type, Maximum
    Fractional Value, Segmentation IOD modules, and PS3.3 source text for
    `FRACTIONAL`/`PROBABILITY`. Parsed term lookup for the fractional terms
    remains unavailable, matching the known plan limitation, but source-text
    search returned PS3.3 `sect_C.8.20.2.3.2` and `table_C.8.20-2`.
  - `cargo fmt -- --check` initially failed on rustfmt wrapping in
    `src/generator.rs`; `cargo fmt` was run, and the repeated
    `cargo fmt -- --check` passed.
  - `cargo test --test generate_cli --test validate_cli --test list_cases_cli --test project_artifacts`
    passed.
  - `cargo test --test project_artifacts --test list_cases_cli` passed again
    after correcting the fractional source-text evidence anchor to
    `sect_C.8.20.2.3.2`.
  - `cargo test` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- list-cases --profile extended --status planned` passed and
    listed 9 remaining planned Phase 5 rows, with LABELMAP SEG first.
  - `cargo run -- generate --profile extended --out /tmp/dts-fractional-seg --seed 1`
    passed, writing 8 files.
  - `cargo run -- validate /tmp/dts-fractional-seg` passed with 8 files
    checked and 0 validation failures.
  - `cargo run -- report /tmp/dts-fractional-seg --format json` passed with
    counts `generated=8`, `planned=9`, `skipped=0`, `blocked=0`; the
    fractional SEG row reports
    `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]` and
    bit depth 8.

- 2026-06-14 Phase 5.1 BINARY Segmentation Storage slice:
  - `cargo test --test generate_cli -- --nocapture` initially failed on stale
    extended/all generated-file counts and planned SEG assertions; tests were
    updated, and the repeated command passed.
  - `cargo test --test list_cases_cli -- --nocapture` passed.
  - `cargo test --test project_artifacts -- --nocapture` passed.
  - `cargo test --test validate_cli -- --nocapture` initially failed on the
    stale extended `files_checked` count; the test was updated, and the
    repeated command passed.
  - `cargo test --test report_cli -- --nocapture` passed.
  - `cargo test` initially failed on stale generator source-registry fixture
    metadata and in-library list-cases planned SEG assertions; tests were
    updated, and the repeated `cargo test` passed.
  - `cargo fmt -- --check` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /tmp/dts-slice-binary-seg --seed 1`
    passed, writing 7 files.
  - `cargo run -- validate /tmp/dts-slice-binary-seg` passed with 7 files
    checked and 0 validation failures.
  - `cargo run -- report /tmp/dts-slice-binary-seg --format json` passed with
    counts `generated=7`, `planned=10`, `skipped=0`, `blocked=0`; the SEG row
    reports `derived_refs=["enhanced/ct/multiframe_shared_perframe_explicit_le"]`.
  - `cargo run -- report /tmp/dts-slice-binary-seg --format markdown` passed
    with the expected generated SEG row and 10 remaining Phase 5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5 generation-time source registry slice:
  - `cargo fmt -- --check` initially failed on rustfmt wrapping in
    `src/generator.rs`; `cargo fmt` was run, and the repeated
    `cargo fmt -- --check` passed.
  - `cargo test generated_source_registry` passed.
  - `cargo test generation_context` passed.
  - `cargo test` passed with 80 tests plus doc tests.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /tmp/dts-slice-source-registry --seed 1`
    passed, writing 6 existing extended files and recording 11 planned Phase 5
    unavailable cases.
  - `cargo run -- validate /tmp/dts-slice-source-registry` passed with 6 files
    checked and 0 validation failures.
  - `cargo run -- report /tmp/dts-slice-source-registry --format json` passed
    with counts `generated=6`, `planned=11`, `skipped=0`, `blocked=0`.
  - `cargo run -- report /tmp/dts-slice-source-registry --format markdown`
    passed with the expected extended coverage matrix and planned Phase 5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5 object-aware validation and same-run reference validation
  slice:
  - `cargo fmt -- --check` initially failed on rustfmt wrapping in
    `src/lib.rs`; `cargo fmt` was run, and the repeated
    `cargo fmt -- --check` passed.
  - `cargo test --test validate_cli reference` passed with 2 focused reference
    validation tests.
  - `cargo test --test validate_cli` passed with 20 validation CLI tests.
  - `cargo test` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /tmp/dts-slice --seed 1`
    passed, writing 6 existing extended files and recording 11 planned Phase 5
    unavailable cases.
  - `cargo run -- validate /tmp/dts-slice` passed with 6 files checked and 0
    validation failures.
  - `cargo run -- report /tmp/dts-slice --format json` passed with counts
    `generated=6`, `planned=11`, `skipped=0`, `blocked=0`.
  - `cargo run -- report /tmp/dts-slice --format markdown` passed with the
    expected extended coverage matrix and planned Phase 5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5 manifest/report reference foundation slice:
  - `cargo fmt -- --check` initially failed on formatting in
    `tests/schema_artifacts.rs`; `cargo fmt` was run, and the repeated
    `cargo fmt -- --check` passed.
  - `cargo test --test schema_artifacts --test report_cli` passed.
  - `cargo test` passed.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.
  - `cargo run -- generate --profile extended --out /tmp/dts-slice --seed 1`
    passed, writing 6 existing extended files and recording 11 planned Phase 5
    unavailable cases.
  - `cargo run -- validate /tmp/dts-slice` passed with 6 files checked and 0
    validation failures.
  - `cargo run -- report /tmp/dts-slice --format json` passed with counts
    `generated=6`, `planned=11`, `skipped=0`, `blocked=0`.
  - `cargo run -- report /tmp/dts-slice --format markdown` passed with the
    expected extended coverage matrix and planned Phase 5 gaps.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.

- 2026-06-14 Phase 5 registry queue slice:
  - `jq empty cases/registry.json` passed.
  - `git diff --check` passed.
  - `cargo fmt -- --check` passed.
  - `cargo test list_cases` passed.
  - `cargo test registry_contains_initial_smoke_and_core_cases` passed.
  - `cargo test generate_command_writes_extended_enhanced_ct_multiframe_case`
    passed.
  - `cargo test generate_command_writes_all_profile_union_and_skips_planned_cases`
    passed.
  - `cargo test` passed.
  - `cargo run -- list-cases --profile extended --status planned` passed and
    listed 11 planned Phase 5 rows.
  - `cargo run -- standards gaps --profile extended` passed with no standards
    evidence gaps.
  - `cargo run -- generate --profile extended --out /tmp/dts-phase5-registry --seed 1`
    passed, writing 6 existing extended files and recording 11 planned Phase 5
    unavailable cases.
  - `cargo run -- validate /tmp/dts-phase5-registry` passed with 6 files
    checked and 0 validation failures.
  - `cargo run -- report /tmp/dts-phase5-registry --format json` passed with
    counts `generated=6`, `planned=11`, `skipped=0`, `blocked=0`.
  - `cargo run -- standards check-lock` passed with the existing documented
    unavailable-pin warnings.

## Current Blockers

None currently recorded for continuing Phase 5.5.

## Recommended Next Commit

Continue Phase 5.5 with `non-image/rt/dose_grid_u16_explicit_le`. Recheck RT
Dose Storage, the RT Dose IOD and modules, RT Dose Module attributes, grid dose
pixel requirements, Frame of Reference/Image Plane requirements, and references
to the generated RT Structure Set or Enhanced CT source with
`dicom-standard-kb` before adding the writer. Keep the commit limited to RT Dose
writer/validation/tests/registry status/progress unless a small shared RT
reference helper is required.

## Commit-Ready Summary

The current slice implements `non-image/rt/structure_set_single_roi_explicit_le`,
adds a non-image RT Structure Set writer and validation path, flips the RT
Structure Set registry row to `implemented`, updates focused tests and this
progress tracker, and leaves `non-image/rt/dose_grid_u16_explicit_le` as the
next work.

## Handoff Notes

- Use `rg`/`rg --files` first when inspecting the repository.
- Stage files selectively for each logical unit of work.
- After each completed task, run `git log --oneline -3` and confirm the new
  commit is present.
- If standards information is missing from `dicom-standard-kb`, add a local
  source note or mark the case blocked; do not make uncited assumptions.
- Keep smoke cases tiny, byte-stable, and free of optional codec requirements.
