# Composing DICOM objects

`compose` creates DICOM from a caller-owned declarative specification. It uses
the same standards-locked plan, Part 10 writer, manifest projection, and generic
validator as the shared composition library. It is separate from `generate`:
the latter selects curated registry cases and retains their case-specific
qualification contracts; composition runs do not claim registry coverage.

The catalog currently qualifies the Phase P2 Secondary Capture templates and
the Phase P3.3 classic modality lane:

- `classic/secondary-capture/monochrome@1.0.0`: native unsigned 8- or 16-bit
  MONOCHROME1/MONOCHROME2;
- `classic/secondary-capture/rgb@1.0.0`: native unsigned 8-bit RGB with planar
  configuration 0 or 1.
- `classic/cr@1.0.0`: native unsigned 12-bit-in-16-bit CR;
- `classic/ct@1.0.0`: native signed 12-bit-in-16-bit CT with axial geometry
  and HU rescale defaults;
- `classic/mr@1.0.0`: native unsigned 12-bit-in-16-bit MR with axial geometry
  and deterministic acquisition defaults.
- `classic/dx/for-presentation@1.0.0`: native unsigned MONOCHROME2
  12-bit-in-16-bit DX with presentation display semantics;
- `classic/mammography/for-presentation@1.0.0`: native unsigned MONOCHROME1
  12-bit-in-16-bit mammography with inverse presentation LUT semantics;
- `classic/mammography/for-processing@1.0.0`: native unsigned MONOCHROME2
  12-bit-in-16-bit mammography with processing intent and no presentation
  window defaults.

Inspect the current descriptors rather than copying this summary as an
inventory invariant:

```sh
cargo run --locked -- templates list
cargo run --locked -- templates list --format json
cargo run --locked -- templates describe \
  classic/secondary-capture/monochrome --format json
```

## Default object

Save this as `examples/composition/sc-default.json` (or use the committed
`tests/fixtures/composition/valid/template-only.json` fixture):

```json
{
  "composition_spec_schema_version": "0.1.0",
  "instances": [{
    "instance_id": "primary",
    "template": { "id": "classic/secondary-capture/monochrome" }
  }]
}
```

Resolve it without retaining output, then publish it to a new root:

```sh
cargo run --locked -- compose \
  --spec tests/fixtures/composition/valid/template-only.json \
  --out generated/composition-sc-dry-run --seed 1 --dry-run

cargo run --locked -- compose \
  --spec tests/fixtures/composition/valid/template-only.json \
  --out generated/composition-sc --seed 1

cargo run --locked -- validate generated/composition-sc
cargo run --locked -- report generated/composition-sc --format markdown
```

The dry run prints canonical resolved plans and never creates the named output
root. A normal run stages inputs and outputs privately, validates every file,
writes `manifest.json`, removes private staged inputs, and atomically promotes
the complete root. The output path must not exist.

## Caller-owned raw pixels

Local paths are relative to the specification file, never to the process
working directory. A local pixel source requires an exact shape. Supplying its
SHA-256 is strongly recommended and makes source drift fail before publication:

```json
{
  "composition_spec_schema_version": "0.1.0",
  "instances": [{
    "instance_id": "primary",
    "template": { "id": "classic/secondary-capture/rgb" },
    "content": [{
      "slot": "pixels",
      "source": {
        "kind": "local_file",
        "path": "assets/rgb-2x2.raw",
        "sha256": "REPLACE_WITH_64_LOWERCASE_HEX_DIGITS",
        "pixel": {
          "rows": 2,
          "columns": 2,
          "frames": 1,
          "samples_per_pixel": 3,
          "photometric_interpretation": "RGB",
          "sample_type": "uint",
          "bits_allocated": 8,
          "bits_stored": 8,
          "high_bit": 7,
          "byte_order": "little",
          "planar_configuration": 0
        }
      }
    }]
  }]
}
```

Raw values must contain exactly `rows × columns × frames × samples × bytes per
sample` bytes. Each family descriptor fixes its permitted photometric,
sample-type, stored-bit, and frame model; rows and columns remain caller-sized
within the bounded resource policy. The manifest records the source-relative
path, whole-value hash, pixel shape, and exact per-frame hashes. Symlinks,
absolute paths, traversal, changing files, wrong hashes, and resource overruns
are rejected.

## Attribute operations

Caller operations support standard tags or keywords, explicit private creators,
typed primitive and multi-values, empty, remove, binary values, and recursive
Sequences. Precedence is template defaults, run defaults, instance overrides,
then derived structural values. The final layer protects SOP Class, SOP/Study/
Series identities, transfer syntax, Rows, Columns, pixel shape, and Pixel Data.
A contradiction is an error; caller content is never silently reconciled with a
contradictory attribute.

The committed
`tests/fixtures/composition/valid/typed-local-content.json` fixture demonstrates
all P2 attribute forms. Private elements require an odd group, an element in the
private data range, an explicit VR, and `private_creator`.

## Limits and evidence

The optional `resource_limits` block bounds instance count, input file count,
per-file bytes, total input bytes, and total output bytes. Defaults are finite
and are recorded in the composition manifest. Network content and provider
execution are not currently available.

`validate` reconstructs each resolved plan from the manifest, verifies file
size and SHA-256, reopens Part 10 and data elements, checks content hashes, and
detects undeclared instance files. `report` groups only composition templates
and transfer syntaxes. It deliberately has no registry `case_id`, profile, or
coverage projection.

These are strong same-project checks. The qualified P2 and P3.3 defaults also
have finding-free evidence from the pinned `dicom3tools-dciodvfy` route in
`docs/arbitrary-dicom-composition-status.md`. That evidence applies only to the
documented template versions and native pixel domains.
