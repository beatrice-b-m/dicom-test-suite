# Remediation Plan

**Created:** 2026-06-13  
**Baseline:** review against `SYSTEM_SPEC.md` version 0.2.0  
**Purpose:** address implementation/spec gaps before expanding Phase 5 feature work

This plan is ordered so coding agents can resolve foundational contract issues before adding new DICOM recipes. Each phase should be completed as one or more granular commits, with `IMPLEMENTATION_PROGRESS.md` updated in the same commit whenever status, blockers, or next steps change.

## Remediation Principles

- Keep `SYSTEM_SPEC.md` as the source of truth. Do not weaken the spec to fit the current implementation unless the product decision is explicit.
- Keep `cases/registry.json` authoritative for planned, implemented, skipped, blocked, and deprecated cases.
- Preserve deterministic output. Every remediation that touches generation must keep smoke byte-stability green.
- Add regression tests before or with behavior changes.
- Continue excluding generated DICOM files, generated manifests, reports, caches, official DICOM artifacts, and generated KB databases from git.

## Phase R1: Restore Registry Authority

**Status:** complete as of 2026-06-13. Registry status now gates generation,
planned/skipped/blocked/deprecated cases have tested manifest/listing behavior,
the planned SEG case is restored, and generated file standards evidence is
deduplicated.

**Findings addressed:** generation does not honor registry status; planned SEG case absent from registry; skipped-case reporting is generic.

Tasks:

- Add the missing `derived/seg/binary_multiframe_explicit_le` planned registry entry with structured standards evidence or an explicit source-note/block reason.
- Introduce a typed case registry model or equivalent validated access layer so generation, listing, reporting, and skipped-case handling share one interpretation of registry entries.
- Change `generate` selection so only `status: implemented` cases are generated.
- Preserve `status: skipped` and `status: blocked` entries as skipped manifest rows using their registry `skip` object.
- Report `status: planned` entries as unavailable with accurate reason text and a current `recheck_phase`, not the hard-coded Phase 1 message.
- Decide and test behavior for `status: deprecated`; recommended default is to exclude from normal generation and show in `list-cases` unless filtered out.
- Deduplicate generated file `standards_evidence` when registry evidence and recipe evidence overlap.

Exit criteria:

- `generate --profile core` reports the two planned VL cases as unavailable with accurate phase/reason metadata.
- `generate --profile extended` reports the planned SEG case until implemented.
- A registry status change from `implemented` to `blocked` prevents generation of that case and produces a structured skipped manifest entry.
- Tests cover implemented, planned, skipped, blocked, and deprecated registry behavior.

## Phase R2: Complete Required CLI Contracts

**Status:** complete as of 2026-06-13. Required CLI contracts are implemented
or intentionally unavailable with a documented status: `list-cases --status`,
`validate <generated-root>`, `report <generated-root> --format json|markdown`,
`standards check-lock`, `standards gaps`, and `standards verify-kb --edition
2026b`.

**Findings addressed:** expected CLI surface is incomplete.

Tasks:

- Add `list-cases --status <status>` and allow combining it with `--profile`. Complete.
- Implement `validate <generated-root>` as a first-class command that reads `manifest.json`, reopens each generated file, reruns internal validation, and reports failures with non-zero exit status. Complete.
- Implement `report <generated-root> --format json` using `schemas/coverage-report.schema.json`. Complete.
- Implement `report <generated-root> --format markdown` with the same coverage counts and gaps in human-readable form. Complete.
- Implement `standards check-lock` to validate `standards.lock.json` shape, pin completeness, and current policy fields. Complete.
- Implement `standards gaps --profile <profile>` to list registry entries whose evidence is incomplete, blocked, or source-note-backed. Complete.
- Implement `standards verify-kb --edition 2026b` if the local MCP/CLI surface can support it; otherwise return a clear unavailable status and document the blocker. Complete with documented unavailable status.

Exit criteria:

- The CLI examples in `SYSTEM_SPEC.md` section 17 either work or intentionally return a documented unavailable status for external dependency reasons.
- Tests assert success paths for `list-cases --status`, `validate`, JSON report, Markdown report, and `standards check-lock`.
- Tests assert clear non-zero errors for malformed arguments and missing generated manifests.

## Phase R3: Harden Part 10 and Internal Validation

**Status:** in progress as of 2026-06-13. Raw Part 10 byte-level validation,
parsed cross-field image invariants, and generated manifest schema-conformance
checks are implemented; baseline Type 1/Type 2 standards-derived checks are
implemented; initial Secondary Capture and classic CT family-specific checks
are implemented; additional negative mutations and broader family-specific
standards-derived checks remain.

**Findings addressed:** validation is not yet the full Part 10 / standards-derived contract.

Tasks:

- Add raw byte validation for required Part 10 invariants. Complete for
  generated-root validation:
  - 128-byte preamble is present and all zero for normal profiles.
  - `DICM` prefix appears at byte offset 128.
  - File Meta Information Version `(0002,0001)` is present.
  - required File Meta elements are present.
  - File Meta Information is encoded as Explicit VR Little Endian.
  - File Meta group ends before dataset group `0008` and no group `0002` elements appear later in the dataset.
- Validate Implementation Version Name `(0002,0013)` when present and keep it deterministic. Complete for generated-root validation.
- Rework cross-field invariants so they compare actual parsed file values, not only recipe expectation values. At minimum cover `Bits Stored <= Bits Allocated`, `High Bit == Bits Stored - 1`, and native Pixel Data byte length from parsed rows/columns/frames/samples/bits. Complete for generated-root validation.
- Add manifest JSON Schema validation to generation tests. Complete for
  required-field and additional-property schema contracts in smoke/core/extended
  generation tests.
- Add targeted negative validator tests by mutating temporary generated files, without committing invalid DICOM fixtures. In progress: non-zero preamble, missing File Meta Information Version, unexpected group `0002`, inconsistent High Bit, missing Type 2 Patient's Name, missing SC Conversion Type, missing CT Image Type, missing MG Positioner Type, missing DX Presentation LUT Shape, missing US Image Type, missing CR Body Part Examined, and missing MR Scanning Sequence are covered.
- Expand standards-derived recipe validation incrementally for each implemented IOD family, starting with Type 1 and Type 2 attributes already in current recipes. In progress: baseline Patient, General Study, General Series, and General Image Type 1/Type 2 checks plus SC Equipment, classic CT, MG, DX, US, CR, classic MR, Image Plane, and Frame of Reference checks are covered for generated-root validation; Enhanced CT/MR family-specific checks remain.

Exit criteria:

- A malformed generated file with a missing File Meta Version, non-zero normal preamble, extra dataset group `0002` element, or inconsistent High Bit fails validation.
- All current generated profiles still pass internal validation.
- `cargo test` includes validator regression tests that would have failed under the previous self-referential checks.

## Phase R4: Close Reproducibility and CI Guard Gaps

**Status:** complete as of 2026-06-13. Core/extended byte-stability tests,
`all` profile union/schema/skipped-case coverage, generated-payload tracking
guards, and registry/schema consistency checks are implemented.

**Findings addressed:** test and CI gaps remain.

Tasks:

- Add two-run byte-stability tests for `core` and `extended` while all cases remain `byte_stable`. Complete.
- Add an `all` profile smoke test that verifies union behavior and skipped-case accounting without comparing full large outputs. Complete.
- Add a test or script that fails when generated DICOM-like payloads are tracked or staged. Complete.
- Add generated manifest schema validation to smoke, core, extended, and all generation tests. Complete for focused schema-conformance checks in generation CLI tests.
- Add registry/schema consistency tests:
  - every implemented registry case has a generator recipe. Complete.
  - every generator recipe has a registry case. Complete.
  - every planned initial-priority case from `SYSTEM_SPEC.md` section 21 is represented in the registry or explicitly deferred in progress notes. Complete.

Exit criteria:

- `cargo test` catches missing registry entries, orphan generator recipes, generated manifest schema drift, and accidental generated payload staging.
- Core and extended reproducibility are verified locally without external codecs.

## Phase R5: Resolve Standards Lock Pinning

**Status:** complete as of 2026-06-13. The committed lock has concrete
`source_manifest_sha256` pinning plus explicit non-fatal unavailable statuses
and reasons for the KB repository commit, local KB DB SHA-256, and official
source artifact hashes.

**Findings addressed:** standards lock is still partially unpinned.

Tasks:

- Determine whether `dicom-standard-kb` can expose the repository commit and local DB SHA-256 through MCP or CLI. Complete: the current Codex MCP query surface exposes query traces and `source_manifest_sha256`, but not the repository checkout commit or generated local DB SHA-256.
- Fill `dicom_standard_kb.commit` and `dicom_standard_kb.db_sha256` when verifiable. Complete with documented non-fatal unavailable statuses until those values are exposed or independently verified.
- Record official source artifact hashes only if those artifacts are acquired through an approved local workflow and kept out of git. Complete with documented `unavailable_not_downloaded` statuses because official source artifacts are not downloaded into this repository.
- If a hash cannot be obtained without redistributing or caching prohibited artifacts, document the precise blocker in `standards.lock.json`, `IMPLEMENTATION_PROGRESS.md`, and `standards/kb-integration.md`. Complete.
- Make `standards check-lock` distinguish between fatal missing lock data and documented unavailable lock data. Complete.

Exit criteria:

- `standards.lock.json` has either concrete pin values or explicit non-fatal unavailable statuses for every currently null reproducibility field.
- `standards check-lock` exits successfully only when the lock matches the documented policy.

## Phase R6: Resume Feature Work After Remediation

**Findings addressed:** aligns future work with the cleaned foundation.

Tasks:

- Re-evaluate Phase 5 readiness after R1 through R5 are complete.
- Start `derived/seg/binary_multiframe_explicit_le` only after the registry, CLI, validation, and reproducibility contracts are stable.
- Query `dicom-standard-kb` for Segmentation Storage, Segmentation Image IOD modules, BINARY segmentation type requirements, Derivation Image references, Source Image references, and Segment Sequence attributes.
- Add focused readback, validation, manifest, and report coverage for SEG.

Exit criteria:

- Phase 5 work begins from an authoritative registry and complete CLI/validation baseline.
- New derived-object implementation does not reopen the remediation findings closed in R1 through R5.

## Recommended Commit Sequence

1. `docs(remediation): add phased remediation plan`
2. `fix(registry): make case status control generation`
3. `fix(registry): add missing planned segmentation case`
4. `feat(cli): add status filtering to list-cases`
5. `feat(cli): add validate command`
6. `feat(report): add coverage report generation`
7. `feat(standards): add lock and gap commands`
8. `fix(validation): enforce Part 10 file meta invariants`
9. `fix(validation): compute pixel invariants from parsed files`
10. `test(generation): expand reproducibility and schema checks`
11. `chore(standards): resolve standards lock pinning`
12. `feat(seg): add binary segmentation case`

Agents may split any item further when a single step touches multiple modules, but should not batch unrelated phases into one commit.
