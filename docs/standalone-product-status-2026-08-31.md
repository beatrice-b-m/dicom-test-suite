# Standalone product status

**Recorded:** 2026-08-31

**Contract:** `docs/standalone-productization-plan.md`

**Release readiness:** not ready; S0 and S1 are complete and S2 automation is next

## Current gate state

| Gate | State | Current evidence or blocker |
| --- | --- | --- |
| S0 — product contract | Complete | ADR 0002 fixes CLI-primary/SDK-secondary integration and the three disjoint evidence workflows. The compatibility policy, CLI envelope/registry schemas and fixtures, current-command error mappings, structural-assembly design, and official-source foundation are committed and tested. |
| S1 — relocatable resources | Complete | Versioned embedded resources cover all first-party catalogs, recipes, schemas, locks, configs, and small assets. Production lookups are audited, explicit roots are integrity checked, manifests project resource identity, and the installed binary passed every resource-backed workflow from three unrelated directories. |
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

All S2 through S7 deliverables and every terminal acceptance row remain open.
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

### S1 — relocatable resources: complete

**Completed:** 2026-08-31

**Commits:**

- `4c8807a`, `d3907c7` — classify production filesystem access and enforce a
  zero-allowlist audit for ambient first-party resource lookups;
- `d98f550`, `ed5b87b`, `06abd91`, `6478c32` — embed the versioned product
  resource set and route generation, composition, reporting, conformance, and
  CLI defaults through `ProductResources`;
- `8960dd1`, `46bd0c8` — fail closed on explicit-root hash drift and project
  complete resource identity into curated and composition manifests;
- `8628ad0` — execute templates, compose, generate, validate, and report from
  three unrelated working directories using a copied installed binary with
  checkout/cache discovery inputs removed;
- `2393085`, `36ff1ed`, `b37f093`, `f757030`, `d533395`, `56709e6` — retain
  compatibility expectations, require explicit prepared external backends in
  qualification tests, preserve transaction failure coverage independently of
  repository files, and accept every still-supported manifest schema version.

**Resource identity:** embedded set version `1.0.0`, 224 logical resources,
resource-set SHA-256
`a2bed94aa5b3f30de8ecb94d4fe3531cba7c5763e196350f6db4ddd84a3c6809`.
An explicit root must contain the identical logical set and bytes. Drift fails
before materialization with `evidence.integrity.failed`. The prepared
highdicom/pydicom runtime remains an explicit external capability selected by
`DTS_HIGHDICOM_PYTHON`; its absence remains unavailable, not passed.

**Focused verification:**

```sh
cargo fmt --all -- --check
cargo test --locked --no-default-features \
  --test product_resources \
  --test product_resource_lookup_audit \
  --test standalone_generate_resources \
  --test standalone_compose_resources \
  --test installed_artifact_relocation \
  --test cli_contract_schema \
  --test schema_artifacts
cargo test --locked --no-default-features \
  --test composition_quantitative \
  --test composition_structured_reports \
  --test composition_p8_qualification \
  --test composition_curated_migration
cargo test --locked --no-default-features \
  --test curated_generate_integration
git diff --check
```

The relocation test passed from all three unrelated directories. Resource
identity/integrity, ambient-lookup audit, standalone generation/composition,
CLI schema, and all 73 schema-artifact tests passed. Explicit prepared-backend
qualification passed for the full catalog, quantitative content, structured
reports, and curated migration. The four curated generation tests passed,
including a post-planning publication failure that left no destination or
private staging.

**Regression closure and test modularity:**

The repository baseline was exercised across all targets. The exact command

```sh
cargo test --locked --all-targets --no-default-features
```

passed 472 library tests (2 ignored) and every integration target through
`general_ecg_waveform`, then exposed a compatibility-test helper that expected
the manifest version schema to remain a single `const`. After `56709e6`, the
affected `generate_cli` target passed 7/7, resource tests passed 6/6, schema
tests passed 73/73, standalone generation passed 1/1, and a lexical tail bundle
ran every integration target after `generate_cli` to completion. This is
segmented regression evidence; the exact all-targets command has not yet
produced one uninterrupted exit-zero run at this commit and remains mandatory
against the terminal release candidate.

Measured heavyweight tests are deliberately isolated:

- `case_recipe_catalog::data_first_sc_and_metadata_values_and_hashes_match_current_generator_bytes`
  — 683.69 seconds;
- `curated_stress_manifest::typed_stress_projection_matches_frozen_file_values_and_resources`
  — 681.02 seconds;
- `curated_stress_sc_integration::all_stress_sc_cases_execute_through_private_streaming_services`
  — 688.21 seconds;
- `generate_cli::generate_command_writes_all_profile_union_and_skips_planned_cases`
  — 686.18 seconds;
- `wsi_direct_plan::ordinary_wsi_direct_plans_match_fresh_seed_one_bytes_and_manifest_facts`
  — 691.07 seconds; and
- `wsi_pyramid::stress_profile_emits_complete_three_instance_wsi_pyramid`
  — 685.37 seconds.

Subsequent work uses dependency-aware tiers: formatting/diff checks and named
tests per commit; a small subsystem bundle per numbered plan item; a
phase-specific black-box or packaged gate at phase completion; and the
heavyweight/full matrix only for changes to its generation, execution,
projection, manifest, resource, stress, or WSI dependency surface and for the
exact S6/S7 release candidate. A passed broad gate is rerun only when later
changes intersect that gate's dependency surface. This preserves the terminal
acceptance matrix without spending roughly 68 minutes of heavyweight runtime
on unrelated CLI or SDK edits.

The builds emitted 38 existing dead-code warnings. They did not enter binary
runtime stdout, but clean machine stdout/stderr and elimination of compiler or
debug leakage remain explicit S2 work.

**Gate conclusion:** the binary resolves first-party runtime resources without
the checkout or Cargo cache, works after relocation for every S1 workflow, and
fails explicit resource drift closed. No release target or terminal matrix row
is claimed yet; packaging, external-consumer, exact release-candidate, and
platform qualification remain open.
