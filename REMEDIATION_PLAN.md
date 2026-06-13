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

**Status:** in progress as of 2026-06-13. `list-cases --status`,
`validate <generated-root>`, and `report <generated-root> --format
json|markdown` are implemented and covered by CLI tests; standards commands
remain open.

**Findings addressed:** expected CLI surface is incomplete.

Tasks:

- Add `list-cases --status <status>` and allow combining it with `--profile`. Complete.
- Implement `validate <generated-root>` as a first-class command that reads `manifest.json`, reopens each generated file, reruns internal validation, and reports failures with non-zero exit status. Complete.
- Implement `report <generated-root> --format json` using `schemas/coverage-report.schema.json`. Complete.
- Implement `report <generated-root> --format markdown` with the same coverage counts and gaps in human-readable form. Complete.
- Implement `standards check-lock` to validate `standards.lock.json` shape, pin completeness, and current policy fields.
- Implement `standards gaps --profile <profile>` to list registry entries whose evidence is incomplete, blocked, or source-note-backed.
- Implement `standards verify-kb --edition 2026b` if the local MCP/CLI surface can support it; otherwise return a clear unavailable status and document the blocker.

Exit criteria:

- The CLI examples in `SYSTEM_SPEC.md` section 17 either work or intentionally return a documented unavailable status for external dependency reasons.
- Tests assert success paths for `list-cases --status`, `validate`, JSON report, Markdown report, and `standards check-lock`.
- Tests assert clear non-zero errors for malformed arguments and missing generated manifests.

## Phase R3: Harden Part 10 and Internal Validation

**Findings addressed:** validation is not yet the full Part 10 / standards-derived contract.

Tasks:

- Add raw byte validation for required Part 10 invariants:
  - 128-byte preamble is present and all zero for normal profiles.
  - `DICM` prefix appears at byte offset 128.
  - File Meta Information Version `(0002,0001)` is present.
  - required File Meta elements are present.
  - File Meta Information is encoded as Explicit VR Little Endian.
  - File Meta group ends before dataset group `0008` and no group `0002` elements appear later in the dataset.
- Validate Implementation Version Name `(0002,0013)` when present and keep it deterministic.
- Rework cross-field invariants so they compare actual parsed file values, not only recipe expectation values. At minimum cover `Bits Stored <= Bits Allocated`, `High Bit == Bits Stored - 1`, and native Pixel Data byte length from parsed rows/columns/frames/samples/bits.
- Add manifest JSON Schema validation to generation tests.
- Add targeted negative validator tests by mutating temporary generated files, without committing invalid DICOM fixtures.
- Expand standards-derived recipe validation incrementally for each implemented IOD family, starting with Type 1 and Type 2 attributes already in current recipes.

Exit criteria:

- A malformed generated file with a missing File Meta Version, non-zero normal preamble, extra dataset group `0002` element, or inconsistent High Bit fails validation.
- All current generated profiles still pass internal validation.
- `cargo test` includes validator regression tests that would have failed under the previous self-referential checks.

## Phase R4: Close Reproducibility and CI Guard Gaps

**Findings addressed:** test and CI gaps remain.

Tasks:

- Add two-run byte-stability tests for `core` and `extended` while all cases remain `byte_stable`.
- Add an `all` profile smoke test that verifies union behavior and skipped-case accounting without comparing full large outputs.
- Add a test or script that fails when generated DICOM-like payloads are tracked or staged.
- Add generated manifest schema validation to smoke, core, extended, and all generation tests.
- Add registry/schema consistency tests:
  - every implemented registry case has a generator recipe;
  - every generator recipe has a registry case;
  - every planned initial-priority case from `SYSTEM_SPEC.md` section 21 is represented in the registry or explicitly deferred in progress notes.

Exit criteria:

- `cargo test` catches missing registry entries, orphan generator recipes, generated manifest schema drift, and accidental generated payload staging.
- Core and extended reproducibility are verified locally without external codecs.

## Phase R5: Resolve Standards Lock Pinning

**Findings addressed:** standards lock is still partially unpinned.

Tasks:

- Determine whether `dicom-standard-kb` can expose the repository commit and local DB SHA-256 through MCP or CLI.
- Fill `dicom_standard_kb.commit` and `dicom_standard_kb.db_sha256` when verifiable.
- Record official source artifact hashes only if those artifacts are acquired through an approved local workflow and kept out of git.
- If a hash cannot be obtained without redistributing or caching prohibited artifacts, document the precise blocker in `standards.lock.json`, `IMPLEMENTATION_PROGRESS.md`, and `standards/kb-integration.md`.
- Make `standards check-lock` distinguish between fatal missing lock data and documented unavailable lock data.

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
