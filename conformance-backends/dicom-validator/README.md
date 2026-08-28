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

Adapter 0.7.0 applies two RT-only, fail-closed compatibility corrections after
verifying the complete locked module/include/tag shapes. It restores the
generated alternative branch for Recorded RT Control Point DateTime and maps
the two Device Alternate Identifier "has a Value" conditions to an equivalent
not-equal-empty operation because dicom-validator 0.8.2 treats an empty string
as `NotEmpty`. The original standard artifacts remain unchanged and contribute
to the composite fingerprint; no finding is allowlisted.

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

The unchanged normal IOD entry point is also registered as
`pydicom-dicom-validator-visible-light` for exactly
`vl/endoscopic/rgb_explicit_le`, `vl/microscopic/rgb_explicit_le`,
`vl/wsi/tiled_full_small`, every instance of
`vl/wsi/pyramid_multiresolution`, and `vl/wsi/multiple_optical_paths`. This is
an additive second IOD opinion over the same locked runtime and 2026b
definitions; `dciodvfy` remains the primary validator and strict validation
and the WSI reconstruction adapter retain ownership of optical-path order and
pixels.

The case-scoped `dts_dicom_validator_adapter.wsi_tile_segmentation` entry point
provides the additive IOD opinion for `derived/seg/wsi_tile_reference`. The
locked 2026b extraction has empty Segmentation IOD `group_macros` and no
generated C.8.20.3.1 module even though the locked official PS3.3 DocBook
contains Table A.51-2 and the Segmentation Macro. Adapter 0.2.0 restores those
two structures in memory only after checking the exact omission, both affected
Segmentation SOP Class definitions, their mandatory modules, and every retained
macro reference. It fails closed if definitions drift, never rewrites the
external cache, and never allowlists the original `TagUnexpected` errors.
Qualification of the two-frame FRACTIONAL OCCUPANCY prototype selected
Segmentation Storage and completed with zero errors; removing Type 1
Segmentation Type produced a `TagMissing` error and exit code one. Removing
each of shared Pixel Measures, shared Segment Identification, per-frame Frame
Content, per-frame Plane Position Slide, and per-frame Derivation Image also
failed the exact-case preflight. Table A.51-2 makes Segmentation mandatory when
Dimension Organization Type is not `TILED_FULL` and Segmentation Type is not
`LABELMAP`; that condition is encoded in the restored definition. Its other
macro rules depend on coordinate system, derivation presence, or non-empty
dimension content, so the M6 entry point additionally requires the locked
placements and one-item cardinalities before invoking general IOD validation.
The injected C.8.20.3.1 structure is exactly Table C.8.20-3: one Type 1 Segment
Identification Sequence (0062,000A) item containing Type 1 Referenced Segment
Number (0062,000B).
Exact references,
dimension order, slide positions, pixel values, frame hashes, and matrix
reconstruction remain outside this IOD adapter's authority.

Adapter version 0.4.0 exposes `--pixel-u32`. It reads the native OW value
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

The `--waveform` route independently reads the Twelve-lead ECG Waveform
Storage OW value with pydicom and decodes signed little-endian samples with
Python `struct`, without NumPy or generator code. It requires the locked
12-channel, 500-sample, 500 Hz metadata and CID 3001 lead order, verifies the
deterministic sample formula, and emits canonical JSON binding the full
payload hash, deinterleaved per-channel hashes, metadata, value range, and
channel-then-sample interleave.

## Locked Runtime Licenses

| Distribution | Version | License |
| --- | --- | --- |
| dicom-validator | 0.8.2 | MIT |
| lxml | 6.1.2 | BSD-3-Clause |
| pydicom | 3.0.2 | MIT |
| pyparsing | 3.3.2 | MIT |

The local adapter uses the repository license.
