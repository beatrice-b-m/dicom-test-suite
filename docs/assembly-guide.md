# Structural assembly guide

`assemble` is the expert, schema-driven route for deterministic DICOM Part 10
when no qualified composition template matches the requested structure. It
accepts arbitrary supported elements and typed bulk content, but it does not
assess DICOM IOD conformance. Every structural manifest and report records
`iod_conformance = "not_assessed"` and is excluded from curated and qualified
coverage matrices.

## Discover the contract

Inspect the installed product before constructing a request:

```sh
synth-dicom-gen version --format json
synth-dicom-gen capabilities --format json
```

The capability result lists supported assembly request, result, and manifest
versions; resource ceilings; content kinds; and transfer syntaxes. The initial
qualified set supports Implicit and Explicit VR Little Endian plus standard,
unknown explicit-VR, managed private, recursive Sequence, integer/float/double
pixel, waveform, PDF document, mesh, and general binary content.

## Minimal request

Save this synthetic request as `assembly.json`:

```json
{
  "assembly_request_schema_version": "1.0.0",
  "instances": [
    {
      "instance_id": "primary",
      "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7",
      "elements": [
        {
          "address": { "keyword": "PatientName" },
          "value": { "kind": "string", "value": "SYNTHETIC^ASSEMBLY" }
        },
        {
          "address": {
            "private_group": "0011",
            "private_creator": "DTS_EXAMPLE",
            "private_offset": "10"
          },
          "vr": "LO",
          "value": { "kind": "string", "value": "STRUCTURAL" }
        },
        {
          "address": { "keyword": "ReferencedImageSequence" },
          "value": {
            "kind": "sequence",
            "items": [
              {
                "elements": [
                  {
                    "address": { "keyword": "ReferencedSOPClassUID" },
                    "value": { "kind": "string", "value": "1.2.840.10008.5.1.4.1.1.7" }
                  }
                ]
              }
            ]
          }
        }
      ],
      "bulk": [
        {
          "kind": "integer_pixel_data",
          "source": { "kind": "inline_base64", "base64": "AAECAw==" },
          "rows": 2,
          "columns": 2,
          "bits_allocated": 8,
          "bits_stored": 8
        }
      ]
    }
  ]
}
```

Dry-run and publication use the same result schema:

```sh
synth-dicom-gen assemble --request assembly.json \
  --out generated/assembly-preview --dry-run --format json
synth-dicom-gen assemble --request assembly.json \
  --out generated/assembly --seed 1 --format json
synth-dicom-gen validate generated/assembly --format json
synth-dicom-gen report generated/assembly --format json --cli-api 1.0.0
```

Every output root must be new. Dry-run creates no requested root. Publication
uses private staging, strict generation-time checks, and atomic no-overwrite
promotion. `manifest.json` is the authority for resolved private blocks,
identities and their provenance, references, element projections, bulk hashes,
padding, validation, resource use, and the no-IOD-claim boundary.

## Files and resource limits

A file bulk source is relative to an explicit caller-asset root and requires a
lowercase SHA-256:

```json
{
  "kind": "file",
  "path": "pixels/frame.raw",
  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
}
```

The CLI defaults the asset root to the request file's parent; `--asset-root`
overrides it. The SDK requires the root for byte requests. Traversal, absolute
paths, backslashes, symlinks in any path component, non-regular files, and hash
mismatches fail before publication. Request limits may lower discoverable
product ceilings but never raise them.

## Choosing a workflow

- Use `generate` for registry-led curated cases and profile coverage.
- Use `compose` for standards-aware qualified templates and bundles.
- Use `assemble` for caller-owned structure when Part 10 correctness is useful
  but IOD conformance is intentionally unassessed.

Passing `validate` proves the structural request was reproduced exactly and
the Part 10 object reopens consistently. It is same-project evidence, not an
independent conformance result and not permission to relabel the output as a
qualified template or curated case.
