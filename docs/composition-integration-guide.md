# Composition integration guide

This guide is for external producers that need deterministic DICOM objects
without adding a curated case or maintaining a repository fork. The file-backed
CLI, in-memory Rust API, and external content provider all use the same
versioned spec, qualified catalog, identity allocator, resolver, Part 10
materializer, validation, manifest, and atomic publication boundary.

Composition is not curated generation. `compose` entries have an `instance_id`
and template identity, never a registry `case_id`, profile, or coverage credit.
Use `generate --profile ...` when the registry's exact case recipe and evidence
are the intended contract.

## Discover the supported contract

Do not infer support from a SOP Class UID or the curated registry. Inspect the
live qualified catalog:

```sh
cargo run --locked -- templates list
cargo run --locked -- templates describe classic/xa --format json
cargo run --locked -- templates reference --format markdown
```

The JSON descriptor is in `result.templates[0]` inside CLI API `1.0.0`'s
common success envelope. Use `capabilities --format json` to discover the live
CLI, result-schema, request-schema, template, transfer-syntax, and runtime
availability versions before submitting a request.

The descriptor is authoritative for template version, defaults, protected and
conditional attributes, content and reference slots, transfer syntaxes,
requirements, limitations, determinism, and independent-validator routes. An
unlisted transfer syntax, content model, or semantic parameter is unavailable;
the composer does not fall back to an unqualified generic SOP writer.

## File-backed CLI

A template-only spec creates deterministic neutral synthetic defaults:

```json
{
  "composition_spec_schema_version": "0.1.0",
  "parallelism": 1,
  "instances": [{
    "instance_id": "primary",
    "template": { "id": "classic/secondary-capture/monochrome" }
  }]
}
```

Use a new output path for every run:

```sh
cargo run --locked -- compose \
  --spec tests/fixtures/composition/valid/template-only.json \
  --out generated/composition-seed-1 --seed 1
cargo run --locked -- validate generated/composition-seed-1
cargo run --locked -- report \
  generated/composition-seed-1 --format markdown
```

Local content paths are relative to the spec file and must remain beneath that
directory without symlinks. Large content belongs in a local file, not inline
JSON. Declare the exact native pixel shape or typed bulk slot required by the
descriptor and include a lowercase SHA-256 whenever the bytes are known.
Small fixtures may use `inline_small_fixture`; they remain hash checked and
resource accounted. Only XA/XRF descriptors expose `encoded_frames`, and only
for RLE Lossless with per-frame hashing and decode qualification. Providers are
available only when the descriptor lists `provider` in `allowed_sources`.

`parallelism` is file-level and bounded from 1 through 64. Identity and plan
resolution occur before workers start. Sequential and parallel runs therefore
have identical paths, UIDs, references, resolved-plan hashes, and output hashes
for byte-stable templates. The manifest records requested and used workers.

## Rust API

The file API is `compose`. `compose_from_bytes` accepts the exact JSON bytes and
an explicit root for relative local assets:

```rust,no_run
use synth_dicom_gen::composition::{ComposeBytesOptions, compose_from_bytes};

let spec = br#"{
  "composition_spec_schema_version":"0.1.0",
  "instances":[{"instance_id":"primary","template":{
    "id":"classic/secondary-capture/monochrome"
  }}]
}"#;
let options = ComposeBytesOptions {
    spec_root: "fixtures".into(),
    out_dir: "generated/from-rust".into(),
    seed: 1,
    catalog_path: "templates/catalog.json".into(),
    dry_run: false,
};
let (_summary, _manifest) = compose_from_bytes(spec, &options)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Long-running hosts can use `ComposeCancellationToken` with
`compose_with_cancellation` or `compose_from_bytes_with_cancellation`. The token
is cloneable and thread-safe. Cancellation is cooperative during resolution
and file materialization and forcefully terminates an active provider process
group. A cancelled run removes its exact private staging root and never
publishes the requested output directory.

## External content providers

A provider supplies one opaque content-slot payload; it never writes DICOM.
The shared materializer remains the only Part 10 writer. Provider protocol
`1.0.0` is defined by:

- `schemas/composition-provider-request.schema.json`; and
- `schemas/composition-provider-response.schema.json`.

Provider use is explicit in the spec. The executable must be an absolute,
non-symlink regular file with a precomputed SHA-256. The fixed argument vector,
provider version, output size and hash, timeout, media type, pixel declaration,
and typed provider parameters are part of the request contract:

```json
{
  "slot": "pixels",
  "source": {
    "kind": "provider",
    "provider_id": "example.neutral",
    "provider_version": "1.0.0",
    "executable": "/absolute/prepared/provider",
    "executable_sha256": "REPLACE_WITH_64_LOWERCASE_HEX_DIGITS",
    "arguments": ["--fixture", "neutral-2x2"],
    "timeout_ms": 2000,
    "size_bytes": 4,
    "sha256": "88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589",
    "media_type": "application/octet-stream",
    "pixel": {
      "rows": 2,
      "columns": 2,
      "frames": 1,
      "samples_per_pixel": 1,
      "photometric_interpretation": "MONOCHROME2",
      "sample_type": "uint",
      "bits_allocated": 8,
      "bits_stored": 8,
      "high_bit": 7,
      "byte_order": "little"
    },
    "parameters": { "fixture": "neutral-2x2" }
  }
}
```

The runner allocates all UIDs before invocation and writes a canonical request
to `SYNTH_DICOM_GEN_COMPOSITION_PROVIDER_REQUEST`. It also sets:

- `SYNTH_DICOM_GEN_COMPOSITION_PROVIDER_RESPONSE` for the response JSON;
- `SYNTH_DICOM_GEN_COMPOSITION_PROVIDER_OUTPUTS` for the one declared regular file; and
- `SYNTH_DICOM_GEN_COMPOSITION_PROVIDER_NETWORK=disabled`.

The inherited environment is cleared, stdin is closed, diagnostics and
response size are bounded, the working/output directories are private, and a
wall-clock timeout owns the full process group. The provider must echo protocol,
request, provider, executable, argument-vector, slot, size, and hash identity:

```json
{
  "protocol_version": "1.0.0",
  "request_id": "REQUEST_SHA256_FROM_THE_REQUEST",
  "provider_id": "example.neutral",
  "provider_version": "1.0.0",
  "executable_sha256": "EXECUTABLE_SHA256_FROM_THE_REQUEST_ENVIRONMENT",
  "argument_sha256": "ARGUMENT_SHA256_FROM_THE_REQUEST",
  "output": {
    "slot": "pixels",
    "relative_path": "pixels.raw",
    "size_bytes": 4,
    "sha256": "88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589"
  }
}
```

Only that file may exist in the provider output directory. It is hash-checked
with a fixed-size buffer, copied into resolver-owned read-only staging, checked
again against composition file-count and byte envelopes, and removed with all
provider-private files before publication.

`network_policy = disabled` means the protocol supplies no network source,
credentials, proxy settings, or inherited environment. The runner does not
claim a portable OS-level socket sandbox. Run only a prepared, fingerprinted
offline provider; apply an external operating-system sandbox if the executable
is not trusted to honor this contract.

## Provenance and reproducibility

The manifest binds the exact input spec bytes, catalog and standards locks,
resolved plans, inputs, identities, references, output hashes, validation,
parallelism, and resource limits. Provider-backed content additionally records
protocol, provider ID/version, executable and argument hashes, request and
response hashes, disabled-network policy, resource outcome, termination status,
and exact payload hashes. RLE composition records backend
kind/version/feature availability, compressed hashes, and decoded native frame
hashes.

For a byte-stable template, run the same spec and seed into two new roots and
compare each entry's `instance_id`, UIDs, resolved-plan SHA-256, and output
SHA-256. For a semantic-stable codec, compare decoded frame or bulk hashes and
the descriptor's bounded semantic metrics. Staging names and provider paths are
operational host state and never enter UID allocation.

Composition's generic and codec/provider checks are same-project evidence.
Independent conformance remains the pinned route named by each descriptor and
status record. An unavailable optional tool is recorded as unavailable; it is
never converted into a pass.
