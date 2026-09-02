# External Codec Verification Policy

The `htj2k_openjph` and `legacy_jpeg_dcmtk` features execute tools discovered
on `PATH`. Normal CI compiles these feature paths but does not claim runtime
codec verification unless the required executables are installed.

## Required tools

| Feature | Required executable | Generated coverage |
|---|---|---|
| `htj2k_openjph` | `ojph_compress` | HTJ2K Lossless |
| `legacy_jpeg_dcmtk` | `dcmcjpeg` | JPEG Lossless Process 14 and SV1 |

The generator records the resolved executable path and SHA-256 fingerprint in
the run manifest. DCMTK also supplies a version banner. A successful
compile-only CI job is not a substitute for this runtime identity evidence.

## Required cadence

Run the applicable verification:

- before a release;
- after changing the Rust toolchain, DICOM-rs version, external codec version,
  wrapper options, capability matrix, or generated recipe; and
- at least once per calendar quarter while the backend is advertised as
  feature-gated.

Record the tool identity, platform, commands, generated/validated counts, and
result in `CURRENT_PROGRESS.md`. Update the capability matrix verification date
when the run revalidates its claims.

## HTJ2K verification

```sh
command -v ojph_compress
cargo test --locked --all-targets --features htj2k_openjph
cargo run --locked --features htj2k_openjph -- generate \
  --profile extended --out /tmp/synth-dicom-gen-htj2k --seed 1
cargo run --locked --features htj2k_openjph -- validate /tmp/synth-dicom-gen-htj2k
cargo run --locked --features htj2k_openjph -- report \
  /tmp/synth-dicom-gen-htj2k --format json
```

The run passes only if the HTJ2K case is generated rather than skipped,
validation reports zero failures, the report marks the case `generated`, and
the manifest contains the executable fingerprint.

## Legacy JPEG verification

```sh
command -v dcmcjpeg
cargo test --locked --all-targets --features legacy_jpeg_dcmtk
cargo run --locked --features legacy_jpeg_dcmtk -- generate \
  --profile extended --out /tmp/synth-dicom-gen-legacy-jpeg --seed 1
cargo run --locked --features legacy_jpeg_dcmtk -- validate \
  /tmp/synth-dicom-gen-legacy-jpeg
cargo run --locked --features legacy_jpeg_dcmtk -- report \
  /tmp/synth-dicom-gen-legacy-jpeg --format json
```

The run passes only if both legacy JPEG Lossless cases are generated rather
than skipped, validation reports zero failures, the report marks both cases
`generated`, and each manifest entry contains the executable identity.

## Reproducibility

For either backend, repeat generation with the same tool binary, target,
toolchain, feature set, profile, and seed in two output roots. Compare manifest
semantics and decoded native frame hashes. Exact file equality may be recorded
as stronger local evidence, but these cases remain `semantic_stable` across
backend versions and platforms.
