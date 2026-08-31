# Product resource lookup audit

**Recorded:** 2026-08-31

**Scope:** production filesystem access before S1 migration

## Classification

Every production filesystem lookup belongs to exactly one of these classes:

| Class | Required product behavior | Current subsystems |
| --- | --- | --- |
| Embedded first-party resource | Resolve through `ProductResources`; never use ambient CWD or a compile-time checkout path. | Registry and recipe catalog, template catalog/inventory/qualification evidence, JSON schemas, standards/backend/validator/transfer-syntax locks and configs, security fixture identity, small built-in materializer assets, product error/version metadata. |
| Explicit caller asset | Resolve beneath an explicit caller-asset root with traversal, symlink, count, byte, and hash controls. | Composition request files and local/inline content, provider payloads, future assembly content, explicit custom catalogs/resource roots, generated roots supplied to validate/report/conformance/interoperability. |
| Explicit external tool | Use an explicit path or qualified `PATH` discovery and record executable/version/hash evidence; never infer a repository tool. | Optional codecs, generation/content providers, independent validators, DICOMDIR peers, prepared highdicom/pydicom runtime. |
| Output or private staging | Derive only from an explicit output root or an owned temporary parent; enforce bounded cleanup and atomic no-overwrite publication. | Corpus/manifest/report/evidence roots, executor staging, provider sandboxes, backend staging, media qualification staging. |

Network fetching is not a fifth class. It remains prohibited for generation.

## Ambient first-party lookups requiring S1 removal

The regression test `product_resource_lookup_audit` derives exact occurrences
from production source before each file's `#[cfg(test)] mod tests` body. The
initial inventory includes:

- `src/main.rs`: default template catalog, case registry, standards lock,
  coverage-gap inputs, conformance configuration/allowlist, and protocol
  fixture-lock paths;
- `src/lib.rs`: curated generation's `CARGO_MANIFEST_DIR` root and default
  registry/standards paths used by generation, standards gaps, and listing;
- `src/composition/run.rs`: `CARGO_MANIFEST_DIR` and repository-root-derived
  recipes, registry, template, and standards dependencies;
- `src/conformance.rs`: default validator/configuration artifacts and ambient
  conformance schemas;
- `src/generation_backends/mod.rs`: ambient backend lock default;
- `src/curated_plan.rs`: repository-root joins for recipes, registry, template
  catalog, and standards lock; and
- composition advanced-default modules: standards-lock paths inherited from
  the repository root.

Compile-time `include_str!`/`include_bytes!` resources are classified as
embedded, not ambient reads, but S1.2 must expose them through the same resource
identity abstraction. Manifest dependency strings and source-note identifiers
are provenance labels rather than filesystem opens and remain distinguishable
from resource resolution.

## Explicit caller, tool, and output access

The following access is intentionally retained but must be routed through typed
boundaries:

- CLI request paths, explicit `--registry`, `--catalog`, `--lock`, `--config`,
  `--allowlist`, `--fixtures`, and the future `--resource-root`;
- composition asset-root resolution, content staging, provider request/output,
  and hash verification;
- generated-root reads for validate, report, conformance, and interoperability;
- optional executable discovery and fingerprint reads in runtime capabilities,
  codec wrappers, providers, validators, and media tools; and
- transaction-owned staging, materialization, manifests, evidence, reports,
  cleanup, and final rename operations.

These paths are not permitted to become implicit first-party-resource fallbacks.
S1/S6 tests retain their existing path, symlink, resource, provider, cleanup,
and destination-race contracts.

## Audit invariant

`tests/product_resource_lookup_audit.rs` fails if a new ambient repository
resource lookup or `CARGO_MANIFEST_DIR` dependency appears in production source.
During S1.2-S1.3, known findings are removed from its explicit allowlist until
the list is empty. New first-party resources must be added to
`ProductResources` and its immutable inventory instead of this allowlist.
