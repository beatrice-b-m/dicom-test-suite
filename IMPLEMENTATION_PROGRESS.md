# Implementation Progress

**Last updated:** 2026-06-13  
**Source specification:** `SYSTEM_SPEC.md` version 0.2.0  
**Current phase:** Phase 2 native pixel matrix, in progress

**Current implementation status:** Phase 0, Phase 0.5, and Phase 1 are complete; Phase 2 has unsigned and signed 16-bit core Secondary Capture native pixel cases plus RGB planar configuration 1, PALETTE COLOR, YBR_FULL, and YBR_FULL_422 coverage

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
| `cases/registry.json` | present | Seeds planned smoke/core cases with SOP Class and transfer syntax evidence from `dicom-standard-kb` MCP lookups. |
| `transfer-syntax/capability-matrix.json` | present | Records initial read/decode/write/encode, feature, external library, and determinism capabilities for baseline native transfer syntaxes. |
| `docs/deterministic-build-policy.md` | present | Documents determinism levels, reproducibility inputs, UID derivation, metadata controls, hashes, and two-run verification. |
| `standards/kb-integration.md` | present | Documents the pinned 2026b `dicom-standard-kb` MCP query workflow, evidence fields, and fallback path. |
| `standards/gap-workflow.md` | present | Documents standards gap handling, local source notes, blocked/skipped registry actions, and KB patch criteria. |
| `standards/source-notes/` | present | Contains a README/template plus `uid-2-25.md` for the PS3.5 UID root gap not covered by `dicom-standard-kb`. |
| `src/` or `crates/` | present | Single-package implementation now includes initial `list-cases`, `generate`, deterministic UID, run manifest, and smoke Part 10 writer paths. |
| `tests/` | present | Includes schema artifact, `list-cases` CLI, `generate` CLI, UID, manifest, Part 10 readback, and smoke reproducibility tests. |

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
| Phase 2: Native pixel matrix | in progress | Core native monochrome 16-bit unsigned/signed MONOCHROME2 OW Pixel Data, RGB planar configuration 1, PALETTE COLOR, YBR_FULL, and YBR_FULL_422 cases are implemented; odd dimensions, rectangular images, tiny images, and pixel padding remain. |
| Phase 3: Classic radiology IODs | not started | CT/MR/CR/US/DX/MG builders pending. |
| Phase 4: Enhanced multi-frame | not started | Enhanced CT/MR and functional groups pending. |
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
- [ ] Add odd-dimension, rectangular, very small image, and pixel padding cases.
- [ ] Broaden pixel byte-length and photometric validators for Phase 2 cases.

Phase 2 is complete only when smoke and core profiles cover key Image Pixel
combinations with byte-length validation, and YBR_FULL_422 uses the required
special native byte-length validator.

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
| `classic/ct/mono2_i16_rescale_12bit_explicit_le` | `core` | planned |
| `classic/mg/for_presentation_mono1_u16_12bit_explicit_le` | `core` | planned |
| `classic/mg/for_processing_mono2_u16_12bit_implicit_le` | `core` | planned |
| `classic/cr/overlay_modality_voi_explicit_le` | `core` | planned |
| `classic/mr/multislice_oblique_explicit_le` | `core` | planned |
| `enhanced/ct/multiframe_shared_perframe_explicit_le` | `extended` | planned |
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

## Current Blockers

No implementation blocker has been proven yet. The immediate limitations are
that the local `dicom-standard-kb` repository commit/DB SHA-256 and official
source artifact hashes have not yet been verified. Phase 2 can continue with
odd-dimension, rectangular, tiny-image, and pixel-padding coverage.

## Recommended Next Commit

Add native geometry and padding pixel coverage:

1. Query the 2026b `dicom-standard-kb` for Rows, Columns, Pixel Aspect Ratio,
   Pixel Spacing, Pixel Padding Value, and Pixel Padding Range Limit attributes.
2. Add focused core Secondary Capture cases for odd dimensions, rectangular
   dimensions, very small images, and pixel padding.
3. Generalize recipe geometry away from the current fixed 2x2 defaults and
   extend validation/test coverage for the new dimensions and padding
   attributes.
4. Commit as one or more granular `feat(pixels): ...` commits, split by
   geometry versus padding if both changes are non-trivial.

## Handoff Notes

- Use `rg`/`rg --files` first when inspecting the repository.
- Stage files selectively for each logical unit of work.
- After each completed task, run `git log --oneline -3` and confirm the new
  commit is present.
- If standards information is missing from `dicom-standard-kb`, add a local
  source note or mark the case blocked; do not make uncited assumptions.
- Keep smoke cases tiny, byte-stable, and free of optional codec requirements.
