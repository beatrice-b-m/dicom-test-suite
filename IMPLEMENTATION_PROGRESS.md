# Implementation Progress

**Last updated:** 2026-06-13  
**Source specification:** `SYSTEM_SPEC.md` version 0.2.0  
**Current phase:** Phase 0 repository initialization, incomplete  
**Current implementation status:** planning documents only; no Rust workspace has been created yet

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
| `.gitignore` | missing | Must be added before generated output work begins. |
| `Cargo.toml` / Rust workspace | missing | Initial layout still undecided. |
| `rust-toolchain.toml` | missing | Toolchain must be pinned deliberately. |
| `standards.lock.json` | missing | Required before recipe implementation. |
| `schemas/` | missing | Manifest, case registry, coverage, and viewer report schemas are not created yet. |
| `cases/registry.json` | missing | Case registry must become authoritative for planned and implemented cases. |
| `standards/source-notes/` | missing | Needed for standards gaps not covered by `dicom-standard-kb`. |
| `src/` or `crates/` | missing | No generator implementation exists yet. |
| `tests/` | missing | No automated verification exists yet. |

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
| Phase 0: Repository initialization | in progress | README and system spec exist; workspace, ignore rules, toolchain, schemas, and dependency pins remain. |
| Phase 0.5: Standards and case registry foundation | not started | Required before real recipe work. |
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
- [ ] Add `.gitignore` rules for generated DICOM outputs, generated reports,
  caches, standards artifacts, and generated KB databases.
- [ ] Choose initial Rust edition and workspace layout.
- [ ] Add `Cargo.toml` and `Cargo.lock`.
- [ ] Add `rust-toolchain.toml`.
- [ ] Verify and pin the DICOM-rs crate family.
- [ ] Add initial `schemas/` directory.

Phase 0 is complete only when the repository has clear scope, output policy,
implementation plan, a Rust project skeleton, generated-artifact protections,
and initial schema placeholders.

## Phase 0.5 Checklist

- [ ] Add `standards.lock.json`.
- [ ] Decide explicitly whether the lock targets only the base DICOM edition or
  the base edition plus final-text supplements/correction items as of a fixed
  date.
- [ ] Add `schemas/manifest.schema.json`.
- [ ] Add `schemas/case-registry.schema.json`.
- [ ] Add `schemas/coverage-report.schema.json`.
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
| Rust workspace layout | open | Spec allows initial single-crate development if module boundaries match future crates. |
| Rust edition and toolchain | open | Pin with `rust-toolchain.toml` before implementation. |
| DICOM-rs versions | open | Spec notes `dicom` and `dicom-object` 0.9.1 as current on 2026-06-13; reverify before pinning. |
| Standards baseline | open | Recommended baseline is DICOM 2026b, but final-text inclusion policy must be explicit. |
| `dicom-standard-kb` pin | open | `standards.lock.json` needs repository commit and DB metadata when available. |
| Case registry storage shape | open | Use the required fields in `SYSTEM_SPEC.md` section 6.2. |
| Transfer syntax capability matrix format | open | Must report read, decode, write, encode, features, external libraries, and determinism. |

## Current Blockers

No implementation blocker has been proven yet. The immediate limitation is that
the repository does not yet contain the foundational files required by Phase 0
and Phase 0.5.

## Recommended Next Commit

Implement Phase 0 generated-artifact protections:

1. Add `.gitignore` entries for generated DICOM outputs, generated reports,
   caches, generated standards artifacts, and SQLite KB files.
2. Add a small CI or test placeholder only if the project skeleton exists;
   otherwise keep the commit scoped to ignore policy.
3. Update this progress file to mark the `.gitignore` Phase 0 item complete.
4. Commit as `chore(repo): ignore generated artifacts`.

After that, create the Rust skeleton and schema placeholders in separate
granular commits.

## Handoff Notes

- Use `rg`/`rg --files` first when inspecting the repository.
- Stage files selectively for each logical unit of work.
- After each completed task, run `git log --oneline -3` and confirm the new
  commit is present.
- If standards information is missing from `dicom-standard-kb`, add a local
  source note or mark the case blocked; do not make uncited assumptions.
- Keep smoke cases tiny, byte-stable, and free of optional codec requirements.
