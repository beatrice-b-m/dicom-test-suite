# Documentation Map

This directory contains current operating guides, policy documents, dated
qualification records, and historical implementation plans. Use this map to
avoid treating a milestone snapshot as the current capability contract.

## Start Here

- [Repository README](../README.md): concise capability overview, quick start,
  profiles, optional runtimes, and command map.
- [Generation guide](generation-guide.md): complete user/agent workflow for
  selecting, generating, validating, reporting, and consuming representative
  DICOM test corpora.
- [Composition guide](composition-guide.md): caller-defined template, raw
  content, attribute, dry-run, validation, and evidence workflow.
- [Corpus consumption](corpus-consumption.md): reproducible downstream handoff
  procedure and evidence checklist.
- [Case taxonomy](../cases/taxonomy.md): stable case ID and profile-selection
  rules.
- [Agent guidelines](../AGENTS.md): mandatory implementation, documentation,
  verification, and commit workflow.

## Current Sources Of Truth

When documents disagree, determine current behavior from:

1. the executable and versioned schemas;
2. `cases/registry.json` for logical case status and requirements;
3. a generated run's `manifest.json` for actually emitted and skipped cases;
4. `transfer-syntax/capability-matrix.json` for codec claims; and
5. the newest applicable dated qualification record.

Plans and status documents provide rationale and evidence. They do not override
the registry or a fresh run report.

## Operating And Policy Guides

- [Deterministic build policy](deterministic-build-policy.md): byte-stable and
  semantic-stable contracts, UID derivation, controlled metadata, and artifact
  handling.
- [External codec verification](external-codec-verification.md): OpenJPH and
  DCMTK executable qualification and fingerprint policy.
- [Independent conformance framework](../conformance/README.md): validator
  configuration, exact-case routing, evidence collection, and acceptance.
- [Conformance acceptance](conformance-acceptance.md): dated independent-tool
  acceptance state and blockers.
- [Viewer testing handoff](viewer-testing-handoff.md): a prepared, dated corpus
  review snapshot. Regenerate and re-report before treating its counts as
  current.

## Capability And Qualification Status

These are dated evidence records for implemented vertical slices:

- [Phase 1 proof](phase-1-proof-status.md): backend platform and initial native
  and external proofs.
- [Phase 2 native coverage](phase-2-native-status.md): geometry, metadata,
  clinical families, native pixels, and ICC.
- [Phase 3 derived objects](phase-3-derived-status.md) and
  [complex objects](phase-3-complex-object-status.md): quantitative, SR,
  registration, presentation, waveform, RT, and mesh coverage.
- [Phase 4 pathology](phase-4-pathology-status.md): visible light, tiled/sparse
  WSI, pyramids, optical paths, and tile Segmentation.
- [Phase 5 encapsulation](phase-5-encapsulation-status.md) and
  [lossy codecs](phase-5-lossy-status.md): offset-table/Fragment behavior and
  bounded lossy qualifications.
- [Phase 6 stress](phase-6-stress-status.md): promoted reduced-scale boundaries
  and explicit full-scale unavailability.
- [Phase 7 negative](phase-7-negative-status.md) and
  [bounded fuzz](phase-7-fuzz-status.md): expected-invalid instances and
  payload-free robustness qualification.
- [Phase 8 interoperability](phase-8-interoperability-status.md): DICOMDIR,
  protocol, and security availability boundaries.
- [Arbitrary composition status](arbitrary-dicom-composition-status.md): dated
  gates and evidence for the shared standards-aware composition program.

## Planning And Decision Records

- [Arbitrary DICOM composition plan](arbitrary-dicom-composition-plan.md):
  active phased execution contract for the standards-aware caller-supplied
  attribute and content composition engine. Only capabilities promoted in the
  current guide and status record are public; later phase descriptions remain
  planned.
- [Coverage expansion plan](coverage-expansion-plan.md): historical phased plan
  and acceptance model; consult its completion section and the registry for
  current state.
- [Coverage baseline](coverage-baseline.md): pre-expansion inventory.
- [Coverage expansion decisions](coverage-expansion-decisions-2026-08-28.md):
  resource, lossy, media/protocol, and synthetic PKI decisions.
- [Conformance validation agent brief](conformance-validation-agent-brief.md):
  completed design/acceptance brief retained for framework maintenance.
- [RLE Annex G correction handoff](rle-annex-g-correction-handoff.md): dated
  correction and regeneration evidence.

## Standards Evidence

Case-level standards evidence is stored in `cases/registry.json`. Detailed
source notes live under `standards/source-notes/`; use them when an implemented
recipe depends on a standard detail that cannot be reconstructed from a short
registry query trace. The gap and knowledge-base workflows are documented in
`standards/gap-workflow.md` and `standards/kb-integration.md`.

The standalone commands are:

```sh
cargo run --locked -- standards check-lock
cargo run --locked -- standards gaps --profile extended
cargo run --locked -- report gaps --format markdown
```

## Generated Documentation

Coverage reports and conformance/interoperability evidence are run artifacts.
Write them under ignored output roots such as `generated/` or `reports/`, keep
them with their source manifest, and do not commit ordinary run output. A report
is meaningful only together with its repository revision, manifest hash,
features, external-tool identities, and unavailable-case rows.
