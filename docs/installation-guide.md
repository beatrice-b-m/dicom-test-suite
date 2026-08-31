# Install and upgrade dicom-test-suite

The supported primary channel is the target-specific native release archive.
It contains one relocatable executable, project and dependency license notices,
machine-readable version/capability documents, operating guides, and a
schema-bound release manifest. A source checkout, Rust toolchain, and repository
working directory are not runtime dependencies.

## Choose a qualified archive

Use only an archive whose target has a dated qualification row in the current
[standalone product status](standalone-product-status-2026-08-31.md). A builder
existing for another target is not evidence that its artifact works. At the
current gate, macOS arm64 is qualified; Linux x86_64 remains unclaimed until its
own extracted artifact passes the same matrix.

Set the downloaded filenames, verify the published checksum from the directory
containing both files, and extract the archive:

```sh
ARCHIVE=dicom-test-suite-0.1.0-aarch64-apple-darwin.tar.gz
shasum -a 256 -c "$ARCHIVE.sha256"
tar -xzf "$ARCHIVE"
DTS="$PWD/dicom-test-suite-0.1.0-aarch64-apple-darwin/bin/dicom-test-suite"
"$DTS" version --format json
"$DTS" capabilities --format json
```

On Linux, use `sha256sum -c` for a Linux archive that has actually been
qualified. Do not rename one target's binary or infer cross-target support.

The extracted directory may be moved as a unit. The binary embeds immutable
first-party resources, so it can also be copied alone. Preserve the archive's
`release-manifest.json`, `version.json`, `capabilities.json`, licenses, and
checksum with qualification evidence even when only the binary is placed on
`PATH`.

## Verify an installation

The two discovery calls above must succeed with empty stderr. Confirm:

- `result.product.version` is the intended product version;
- `result.target` matches the downloaded target;
- `result.enabled_features` matches the release notes;
- `result.product_resources.resource_set_sha256` matches
  `release-manifest.json`; and
- required workflows and transfer syntaxes are `available`.

An optional runtime reported as unavailable remains unavailable. Installing the
main binary does not install codecs, Python providers, validators, or peers.

## Upgrade safely

Install a new archive beside the existing version; do not overwrite it in
place. Verify its checksum and discovery results before changing `PATH`. Read
the changelog and migration notes, then compare the independent version domains
under `capabilities.result.supported_versions` with the requests, manifests,
reports, templates, and provider protocols retained by the consumer.

Dry-run representative composition and assembly requests with the new binary,
write qualification output to fresh roots, validate it, and compare canonical
results under the documented determinism class. Keep the old binary until the
consumer's supported-version fixtures pass. Unsupported input receives a
version error and migration action; it must not be silently rewritten.

Downgrades follow the same process. A lower product version may not accept a
schema or template version emitted by a newer product.

## Secondary source installation

`cargo install` is not a claimed release channel yet. The verified Cargo crate
is package evidence, but a generally supported source installation requires
the same outside-checkout relocation and release matrix as a native archive.
Contributors may still build from a clone as documented in the repository
README; that is a development workflow, not the consumer installation path.
