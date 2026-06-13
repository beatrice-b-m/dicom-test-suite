# dicom-test-suite

`dicom-test-suite` is a Rust project for generating a comprehensive local corpus of synthetic DICOM files for viewer compatibility testing.

The suite is standard-first: it is not designed around the current behavior of any one viewer. Its generated files should expose compatibility gaps in DICOM parsers and viewers across legacy single-frame images, enhanced multi-frame images, mammography, color and palette images, overlays, presentation states, segmentations, structured reports, and relevant transfer syntaxes.

Generated DICOM files are intentionally not committed. The repository should contain deterministic generation code, case recipes, validation rules, manifests, and reports. Local output belongs under ignored paths such as `generated/`, `out/`, or `target/`.

## Planned Commands

```sh
cargo run -- list-cases
cargo run -- list-cases --profile smoke
cargo run -- generate --profile smoke --out generated/smoke
cargo run -- generate --profile core --out generated/core
cargo run -- validate generated/core
cargo run -- report generated/core --format markdown
```

## Design Principles

- Use the current DICOM standard as the authority for IODs, modules, attributes, SOP Class UIDs, and transfer syntax behavior.
- Use DICOM-rs for writing valid Part 10 files, starting with the latest verified `dicom` crate family version.
- Generate synthetic, deterministic, non-PHI data.
- Store expected behavior in machine-readable manifests next to generated output.
- Cover orthogonal compatibility axes, not only common happy-path examples.
- Keep `dcmview` and other viewers as consumers of this suite, not as constraints on what the suite can generate.

See [SYSTEM_SPEC.md](SYSTEM_SPEC.md) for the full architecture and phased implementation plan.
