# Installed examples

Every release archive includes small, deterministic JSON examples under
`examples/`. They contain only synthetic identifiers and inline fixture bytes;
they do not require network access, patient data, or files outside the archive.

Set `DTS` to the extracted executable and `EXAMPLES` to the extracted examples
directory. Each output root below must be a new path.

```sh
DTS=/path/to/dicom-test-suite/bin/dicom-test-suite
EXAMPLES=/path/to/dicom-test-suite/examples
```

## Qualified composition

The grayscale and RGB examples supply raw, uncompressed pixel bytes together
with their dimensions, sample layout, and SHA-256 identity:

```sh
"$DTS" compose --spec "$EXAMPLES/compose-raw-grayscale.json" \
  --out generated/grayscale --seed 1 --format json
"$DTS" compose --spec "$EXAMPLES/compose-raw-rgb.json" \
  --out generated/rgb --seed 1 --format json
```

Typed standard metadata, a managed private value, and a Sequence value are
demonstrated separately from the pixel examples:

```sh
"$DTS" compose --spec "$EXAMPLES/compose-metadata-private-sequence.json" \
  --out generated/metadata --seed 1 --format json
```

The multi-instance example resolves a presentation-state reference to the SOP
Instance UID generated for its source image:

```sh
"$DTS" compose --spec "$EXAMPLES/compose-multi-instance-reference.json" \
  --out generated/reference --seed 1 --format json
```

These are qualified-template outputs. Their manifests record template identity,
resolved content and references, hashes, and determinism evidence.

## Structural assembly

The assembly example combines a standard value, a managed private value, a
recursive Sequence, and inline integer pixel data:

```sh
"$DTS" assemble --request "$EXAMPLES/assemble-structural.json" \
  --out generated/assembly --seed 1 --format json
```

Assembly proves deterministic Part 10 materialization but does not assess IOD
conformance. Its manifest and report therefore retain
`iod_conformance = "not_assessed"`.

## Verify every result

For each output root, run the same-project structural validator and create a
versioned report:

```sh
"$DTS" validate generated/grayscale --format json
"$DTS" report generated/grayscale --format json --cli-api 1.0.0
```

Use a fresh root and the same seed to reproduce an example. Compare artifacts
according to the determinism classification in the manifest, and preserve the
manifest and report with any downstream test evidence.
