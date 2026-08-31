# Standalone product status

**Recorded:** 2026-08-31

**Contract:** `docs/standalone-productization-plan.md`

**Release readiness:** not ready; execution is in the initial contract audit

## Current gate state

| Gate | State | Current evidence or blocker |
| --- | --- | --- |
| S0 — product contract | In progress | The approved composition ADR defines curated and qualified-composition boundaries, but no standalone-product ADR, compatibility policy, machine-envelope schemas, stable error registry, or structural-assembly specification has been promoted. |
| S1 — relocatable resources | Not started | Production defaults still open repository-relative registry, template, schema, lock, and recipe paths. `src/composition/run.rs` also derives a production repository root from `CARGO_MANIFEST_DIR`. |
| S2 — automation protocol | Not started | The executable has no JSON `version` or `capabilities` discovery commands and returns human strings with a single generic failure exit. |
| S3 — Rust SDK | Not started | Public internal modules exist, but there is no supported `dicom_test_suite::sdk` facade or shared stable public error model. |
| S4 — structural assembly | Not started | There is no `assemble` CLI/SDK workflow, assembly request schema, or structural-assembly manifest branch. |
| S5 — packaging and guides | Not started | Cargo metadata and public quick starts do not yet satisfy the release-archive and installed-product contract. |
| S6 — release qualification | Not started | Existing source-tree qualification is strong, but no exact packaged release candidate has passed the black-box, relocation, SDK, assembly, or terminal security matrix. |
| S7 — promotion | Not started | The README still leads with `cargo run`; standalone release gates and compatibility ownership are not promoted. |

No terminal acceptance gate has passed against a standalone release candidate.
Existing curated and qualified-composition evidence remains valid only within
its recorded scope and is not being reinterpreted as standalone-product
evidence.

## Baseline audit evidence

The following read-only checks established the initial state on 2026-08-31:

```sh
git status --short
git log --oneline -12
rg -n "ProductResources|cli_api_version|structural_assembly|assemble|standalone|capabilities|version --format|resource-root|sdk" \
  src tests docs schemas Cargo.toml .github
rg -n "CARGO_MANIFEST_DIR|cases/|templates/|schemas/|standards\\.lock|generation-backends\\.lock|transfer-syntax/" \
  src build.rs
```

The worktree was clean before productization work began. The newest commit was
`85c14d7`, which added the proposed standalone plan. The searches found the
existing plan terminology but no implemented standalone public contracts. They
also identified ambient production lookups in `src/main.rs`, `src/lib.rs`,
`src/conformance.rs`, `src/composition/run.rs`, and related defaults. Tests and
test-only fixtures are not classified by this initial production audit.

## Preserved boundaries

- `generate` remains registry-led curated evidence.
- `compose` remains catalog-qualified and does not project curated case or
  profile credit.
- Planned, feature-gated, backend-unavailable, validator-unavailable, and
  peer-unavailable outcomes remain explicit.
- `negative`, `fuzz`, opt-in `stress`, media, and protocol evidence retain
  their isolation rules.
- Same-project validation is not independent conformance evidence.
- No structural-assembly output may claim IOD conformance or qualified
  template coverage.

## Remaining blockers

All S0 through S7 deliverables and every terminal acceptance row remain open.
Linux x86_64 and macOS arm64 cannot be claimed as standalone release targets
until the exact target archives pass the required external-consumer matrix.
Optional runtimes are accepted only when discovered and fingerprint-qualified;
their absence will remain an unavailable result rather than a waived gate.

## Update rule

This record is updated at each phase gate with the exact commit, commands,
artifact identities, results, unsupported targets, and remaining blockers. A
gate is marked complete only after its acceptance criteria pass without
narrowing the product contract.
