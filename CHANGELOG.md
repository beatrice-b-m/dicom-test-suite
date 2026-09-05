# Changelog

All notable standalone-product changes are recorded here. The project follows
the independent compatibility/version domains in
`docs/compatibility-policy.md`; a product-version change does not silently
change a CLI API, request, manifest, report, template, or provider version.

## [Unreleased]

### Added

- Additive coverage report `1.1.0` for current curated manifests, preserving
  frozen readers and truthful non-generated nonsquare observations. This is
  an unreleased `0.2.0` source capability, not a qualified release artifact.
- Relocatable native archives with checksums, target/feature identity,
  dependency notices, and a schema-bound payload manifest.
- Versioned machine discovery, result/error envelopes, stable exit classes,
  and a supported typed Rust SDK facade.
- Qualified composition, curated generation, and bounded structural assembly
  as separate workflows with separate evidence semantics.
- Installed operating guides and self-contained synthetic examples for raw
  grayscale/RGB, metadata/private/Sequence values, references, and assembly.
- Caller-defined classic CT corpus recipes through a fail-closed capability
  tuple that is independent of embedded names, planning order, and output
  paths, available through the external corpus CLI and supported Rust SDK.
- Caller-defined DX/MG corpus recipes through matching native capability tuples,
  with explicit caller paths, strict partial-tuple rejection, CLI/SDK parity and
  preserved historical payload hashes. Malformed dimension overflow returns a
  contract error.
- Caller-defined UTF-8 Person Name, qualified empty Type 2 and private-creator
  SC metadata recipes, with complete typed admission, fixed CLI/SDK payload
  oracles, preserved historical bytes and overflow-safe high-bit validation.

### Changed

- First-party catalogs, schemas, recipes, locks, configs, and small assets are
  embedded and integrity identified instead of requiring repository-relative
  runtime lookup.
- Automation should select installed-product JSON contracts and discover
  artifacts through `manifest.json`, not parse human output or repository
  layout.

### Migration notes

- This is the initial standalone-product candidate, not a promoted `1.0.0`
  release. Only targets with a completed dated qualification row may be used.
- Replace consumer-side `cargo run -- ...` invocation with the executable from
  a checksummed native archive. Contributor commands remain supported for
  repository development but are not an installation contract.
- Existing human CLI forms remain available. New durable integrations should
  use `--format json`; `report` integrations should select
  `--cli-api 1.0.0` explicitly.
- Curated `generate`, qualified `compose`, and structural `assemble` results
  are not interchangeable. Assembly always retains
  `iod_conformance = "not_assessed"`.
- Install upgrades side by side, compare
  `capabilities.result.supported_versions`, dry-run representative requests,
  and retain the prior binary until compatible-input fixtures pass. An
  unsupported version must produce its documented error and migration action;
  do not rewrite it silently.

[Unreleased]: https://github.com/beatrice-b-m/synth-dicom-gen/compare/HEAD...HEAD
