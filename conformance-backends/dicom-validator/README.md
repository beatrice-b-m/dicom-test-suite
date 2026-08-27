# dicom-validator Independent IOD Adapter

This optional conformance backend runs `dicom-validator` 0.8.2 against an
exact, externally provisioned DICOM 2026b definition cache. It is independent
of the Rust generator and does not become a smoke, core, extended, or `all`
profile runtime dependency.

The adapter deliberately loads the locked JSON definitions directly rather
than calling the package's download-aware edition resolver. Every official
DocBook input and derived JSON definition is checked against
`standard-lock.json` before validation, so conformance runs are offline and
cannot silently follow a newer standard edition. Installed wheel contents are
also checked against their RECORD hashes on every invocation.

## Provisioning

The committed environment is resolved with `uv` and CPython 3.12.12:

```sh
uv python install 3.12.12
uv sync \
  --project conformance-backends/dicom-validator \
  --locked \
  --no-editable \
  --python 3.12.12
```

Provision the official 2026b DocBook parts 3, 4, and 6 and the generated JSON
definitions outside git, then set `DTS_DICOM_VALIDATOR_STANDARD_HOME` to that
cache root. The expected relative paths, byte lengths, and SHA-256 identities
are committed in `standard-lock.json`. Generated and upstream standards
artifacts must not be added to the repository.

The prepared interpreter is normally
`conformance-backends/dicom-validator/.venv/bin/python` on Unix or
`.venv\\Scripts\\python.exe` on Windows. Conformance discovery fingerprints the
interpreter and committed adapter inputs before execution.

Set both variables when running the case-specific conformance route:

```sh
export DTS_DICOM_VALIDATOR_PYTHON="$PWD/conformance-backends/dicom-validator/.venv/bin/python"
export DTS_DICOM_VALIDATOR_STANDARD_HOME=/path/to/locked/dicom-validator-cache
```

Adapter version 0.3.0 exposes `--pixel-u32`. It reads the native OW value
through pydicom, requires the locked 32/32/31 unsigned MONOCHROME2 shape, and
emits canonical JSON containing dimensions, attributes, exact stored values,
the Pixel Data hash, and frame hashes. It does not use NumPy or a project pixel
decoder.

The same locked runtime exposes `--nonsquare-spacing` for
`classic/sc/nonsquare_pixel_spacing`. It requires exactly one of the two
independent spatial declarations: Pixel Spacing `0.6\\0.3` with matching
Nominal Scanned Pixel Spacing, or Pixel Aspect Ratio `2\\1`. The other
declaration must be absent. Both variants must retain their exact DS/IS VR and
VM 2, the 4x6 MONOCHROME2 native OB payload and hash, no calibration metadata,
and no patient-space geometry. The canonical semantic result is linked to the
same composite uv/runtime/standard fingerprint as the IOD result.

## Locked Runtime Licenses

| Distribution | Version | License |
| --- | --- | --- |
| dicom-validator | 0.8.2 | MIT |
| lxml | 6.1.2 | BSD-3-Clause |
| pydicom | 3.0.2 | MIT |
| pyparsing | 3.3.2 | MIT |

The local adapter uses the repository license.
