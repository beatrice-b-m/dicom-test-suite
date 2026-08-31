# Standalone product status

**Recorded:** 2026-08-31

**Contract:** `docs/standalone-productization-plan.md`

**Release readiness:** not ready; S0 is complete and S1 relocation is next

## Current gate state

| Gate | State | Current evidence or blocker |
| --- | --- | --- |
| S0 — product contract | Complete | ADR 0002 fixes CLI-primary/SDK-secondary integration and the three disjoint evidence workflows. The compatibility policy, CLI envelope/registry schemas and fixtures, current-command error mappings, structural-assembly design, and official-source foundation are committed and tested. |
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

All S1 through S7 deliverables and every terminal acceptance row remain open.
Linux x86_64 and macOS arm64 cannot be claimed as standalone release targets
until the exact target archives pass the required external-consumer matrix.
Optional runtimes are accepted only when discovered and fingerprint-qualified;
their absence will remain an unavailable result rather than a waived gate.

## Update rule

This record is updated at each phase gate with the exact commit, commands,
artifact identities, results, unsupported targets, and remaining blockers. A
gate is marked complete only after its acceptance criteria pass without
narrowing the product contract.

## Gate evidence

### S0 — product contract: complete

**Completed:** 2026-08-31

**Commits:**

- `2ec2791` — accepted standalone product ADR with the supported compatibility
  surface, non-goals, and change-classification test;
- `6bf6da3` — independent version domains, additive/breaking examples, support
  windows, negotiation, deprecation, and upgrade evidence policy;
- `47d16d9` — CLI success/error/registry schemas, append-only code meanings,
  six exit classes, every current public command's failure-stage mapping, and
  positive/adversarial fixtures; and
- `3ae4002` — structural request/manifest design plus the locked DICOM
  standards foundation, including protected fields, typed bulk, validation
  ceiling, and permanent `iod_conformance = "not_assessed"` semantics.

**Verification:**

```sh
cargo fmt --check
cargo test --locked --no-default-features \
  --test cli_contract_schema --test schema_artifacts
git diff --check
```

The focused run passed 4 CLI contract tests and 73 schema artifact tests.
Envelope adversarial fixtures rejected wrong API versions, unknown top-level
fields, non-namespaced codes, and nested context objects that could expose
private staging details. The error-registry test proved unique registered codes,
the exact `0/2/3/4/5/6` exit set, referentially valid failure mappings, and
coverage for all 17 current public command/subcommand forms. Existing library
dead-code warnings were present during the Rust test build; they are not
machine-command stdout, and eliminating compiler/debug leakage remains an S2
acceptance item.

**Gate conclusion:** a proposed change can be classified as public compatible,
versioned-breaking, internal, curated, qualified composition, or structural
assembly by the accepted ADR and normative policy examples. No undocumented
judgment or IOD inference is needed. This gate does not claim that any S1-S7 or
terminal release-candidate criterion has passed.
