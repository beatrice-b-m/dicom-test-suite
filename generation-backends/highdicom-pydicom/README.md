# highdicom/pydicom Generation Backend

This is an optional, locked Python backend for complex DICOM objects. Rust
remains responsible for selection, identities, source staging, output
verification, manifests, validation, and reporting. The backend is not required
by smoke, core, or ordinary Rust tests.

## Provisioning

The committed environment was resolved with `uv 0.11.26`. Provisioning is an
explicit development action; corpus generation never runs `uv`, downloads an
interpreter, synchronizes packages, or accesses the network.

```sh
uv python install 3.12.12
uv sync \
  --project generation-backends/highdicom-pydicom \
  --locked \
  --no-editable \
  --python 3.12.12
```

The project configuration sets `python-downloads = "manual"` and
`python-preference = "only-managed"`. The default prepared interpreter is:

- Unix: `generation-backends/highdicom-pydicom/.venv/bin/python`
- Windows: `generation-backends/highdicom-pydicom/.venv/Scripts/python.exe`

Set `DTS_HIGHDICOM_PYTHON` to an explicit prepared interpreter path when the
environment is located elsewhere. Discovery verifies the exact CPython version,
ABI, locked distribution versions and installed files, backend entrypoint bytes,
and executable bytes before invocation. Absolute environment paths are not part
of the deterministic identity.

## Supported Recipes

The backend supports the paired CT-derived Parametric Map recipes below. Both
use three staged CT source images and Explicit VR Little Endian; Rust remains
responsible for independently checking the emitted object and payload.

| Case | Sample type | Pixel element | VR |
| --- | --- | --- | --- |
| `derived/parametric-map/float32_ct_derived_explicit_le` | binary32 | Float Pixel Data `(7FE0,0008)` | `OF` |
| `derived/parametric-map/float64_ct_derived_explicit_le` | binary64 | Double Float Pixel Data `(7FE0,0009)` | `OD` |

Requests select a recipe by case and recipe ID and may repeat the expected type
in the `sample_type` parameter. A conflicting type is rejected. The float64
recipe uses the exact spatial-rank increment `2^-30` so it retains distinctions
below binary32 precision. Payload expectations contain the serialized
little-endian IEEE 754 bit patterns and a SHA-256 digest for each frame.

The backend also supports two Comprehensive 3D SR recipes. The TID 1500 volume
report links an Enhanced CT and binary SEG through a TID 1411 measurement
group. The SCOORD3D report uses TID 1501 with a 2.5 mm Distance NUM whose
`INFERRED FROM` POLYLINE joins the patient-space positions of Enhanced CT
frames 1 and 2. Rust supplies every UID and verifies the source geometry before
invocation; PixelMed is the mandatory independent template validator.

## Locked Runtime Licenses

The environment is downloaded from upstream package indexes and is not vendored
or redistributed by this repository. The exact locked distribution metadata was
reviewed on 2026-08-26:

| Distribution | Version | License expression |
| --- | --- | --- |
| highdicom | 0.28.1 | MIT |
| NumPy | 2.5.2 | BSD-3-Clause AND 0BSD AND MIT AND Zlib AND CC0-1.0 |
| packaging | 26.3 | Apache-2.0 OR BSD-2-Clause |
| Pillow | 12.3.0 | MIT-CMU |
| pydicom | 3.0.2 | MIT |
| pyjpegls | 1.5.1 | MIT |
| typing-extensions | 4.16.0 | PSF-2.0 |

The local backend package uses the repository license.
