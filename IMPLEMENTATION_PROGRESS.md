# Implementation Progress

**Last updated:** 2026-06-13  
**Source specification:** `SYSTEM_SPEC.md` version 0.2.0  
**Current phase:** Phase 0.5 standards and case registry foundation, incomplete  
**Current implementation status:** Phase 0 repository initialization is complete; standards baseline lock plus manifest, case registry, and coverage report schemas are committed

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
| `README.md` | present | Contains project purpose and planned commands. |
| `SYSTEM_SPEC.md` | present | Version 0.2.0 planning baseline. |
| `AGENTS.md` | present | Requires descriptive granular commits for completed work. |
| `IMPLEMENTATION_PROGRESS.md` | present after this task | Hand-off ledger for implementation state. |
| `.gitignore` | present | Covers generated DICOM outputs, reports, sidecars, caches, generated standards artifacts, and SQLite KB files. |
| `Cargo.toml` / Rust workspace | present | Single package named `dicom-test-suite`, using Rust 2024 edition; pins minimal DICOM-rs crates for Phase 1 object and transfer syntax work. |
| `rust-toolchain.toml` | present | Pins Rust 1.85.0 with `rustfmt` and `clippy`, matching an installed local toolchain. |
| `standards.lock.json` | present | Locks to DICOM 2026b base edition only using the pinned `dicom-standard-kb` MCP source manifest; local DB and source artifact hashes remain pending. |
| `schemas/` | present | Manifest, case registry, and coverage report schemas have initial structured coverage; viewer report schema remains a placeholder. |
| `cases/registry.json` | missing | Case registry must become authoritative for planned and implemented cases. |
| `standards/source-notes/` | missing | Needed for standards gaps not covered by `dicom-standard-kb`. |
| `src/` or `crates/` | present | Minimal `src/lib.rs` and `src/main.rs` exist; generator implementation has not started. |
| `tests/` | present | Includes schema artifact integration tests; generator behavior tests have not started. |

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
| Phase 0.5: Standards and case registry foundation | in progress | Standards base edition plus manifest/case-registry/coverage schemas are in place; case registry data, viewer report schema, profile definitions, capability matrix, KB repository/DB pin, source artifact hashes, and standards workflow remain. |
| Phase 1: Generator core | not started | CLI, UID generation, manifest writing, Part 10 writing, and file validation pending. |
| Phase 2: Native pixel matrix | not started | Pixel generators and photometric validators pending. |
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
- [ ] Add `schemas/viewer-report.schema.json`.
- [ ] Add normalized case ID taxonomy to committed project artifacts.
- [ ] Add explicit profile definitions and inclusion rules.
- [ ] Add initial `cases/registry.json` with planned smoke/core cases.
- [ ] Add transfer syntax capability matrix.
- [ ] Add deterministic build policy.
- [ ] Add `dicom-standard-kb` integration instructions.
- [ ] Add standards gap/patch workflow documentation.

Phase 0.5 is complete only when `list-cases` can show all planned smoke/core
cases with structured status and planned Phase 1/2 cases have standards
evidence through `dicom-standard-kb` or local source notes.

## Initial Priority Case Queue

These case IDs come from `SYSTEM_SPEC.md` section 21 and should seed
`cases/registry.json` before implementation:

| Case ID | Profile | Implementation status |
|---|---|---|
| `classic/sc/mono2_u8_explicit_le` | `smoke` | planned |
| `classic/sc/mono1_u8_explicit_le` | `smoke` | planned |
| `classic/sc/rgb_planar0_explicit_le` | `smoke` | planned |
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
| Case registry storage shape | open | Use the required fields in `SYSTEM_SPEC.md` section 6.2. |
| Transfer syntax capability matrix format | open | Must report read, decode, write, encode, features, external libraries, and determinism. |

## Current Blockers

No implementation blocker has been proven yet. The immediate limitations are
that the local `dicom-standard-kb` repository commit/DB SHA-256 and official
source artifact hashes have not yet been verified, and the Phase 0.5
registry/schema/policy artifacts are still placeholders or missing.

## Recommended Next Commit

Expand the viewer report schema:

1. Replace the placeholder `schemas/viewer-report.schema.json` with the initial
   structured schema for optional viewer compatibility results: file open,
   object recognition, metadata extraction, pixel rendering, timing,
   errors/warnings, screenshots/artifacts, and per-case status.
2. Keep it viewer-agnostic and aligned with `SYSTEM_SPEC.md` section 18.
3. Extend schema artifact tests for the viewer report contract.
4. Update this progress file with completed schema scope and the next
   recommended Phase 0.5 task.
5. Commit as `feat(schemas): define viewer report schema`.

## Handoff Notes

- Use `rg`/`rg --files` first when inspecting the repository.
- Stage files selectively for each logical unit of work.
- After each completed task, run `git log --oneline -3` and confirm the new
  commit is present.
- If standards information is missing from `dicom-standard-kb`, add a local
  source note or mark the case blocked; do not make uncited assumptions.
- Keep smoke cases tiny, byte-stable, and free of optional codec requirements.
