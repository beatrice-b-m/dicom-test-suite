# Phase 3 Integer Parametric Map Provider Qualification

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `derived/parametric-map/integer_ct_derived_explicit_le`
- Intended provider: `dcmqi`
- Intended value: an integer Parametric Map produced by an implementation that
  is independent of the existing highdicom/pydicom floating-point backend

## Standards Contract

PS3.3 A.75.1 and Table A.75-1 permit the Parametric Map IOD to use the Image
Pixel Module rather than either floating-point pixel module. The planned
unsigned 16-bit variant therefore requires native Pixel Data `(7FE0,0010)`,
Bits Allocated/Stored/High Bit `(0028,0100-0102)` equal to `16/16/15`, Pixel
Representation `(0028,0103)` equal to zero, Samples per Pixel equal to one,
and MONOCHROME2. Float Pixel Data `(7FE0,0008)` and Double Float Pixel Data
`(7FE0,0009)` must be absent.

The locked DICOM source manifest SHA-256 remains
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`.
Locked dicom3tools `dciodvfy` empirically recognizes this integer module form,
and locked DCMTK can extract and hash its OW payload. Independent validation is
therefore available and is not the current blocker.

## Provider Qualification

The official dcmqi v1.5.7 arm64 macOS release was inspected and executed:

- release tag: `v1.5.7`
- repository revision: `506306a`
- archive: `dcmqi-1.5.7-mac-arm64.tar.gz`
- archive SHA-256:
  `ec17425d3eaa7b58db0924138569508c833e9774ef48052ca85d3e5a1b6cf9b9`
- converter identity: `itkimage2paramap version 1.0`

The upstream `libsrc/ParaMapConverter.cpp` path accepts `FloatImageType`,
constructs `DPMParametricMapIOD` with `IODFloatingPointImagePixelModule`, and
adds floating-point frames. The released `itkimage2paramap` path does not
offer the integer Image Pixel Module required by this case.

## Project Decision

Keep the registry row planned with provider `dcmqi` and the explicit
`provider_capability_unavailable` blocker. Do not silently substitute the
already-used highdicom backend: doing so would remove the cross-implementation
purpose of this case while making coverage appear complete.

Recheck when dcmqi exposes integer Parametric Map generation or when the
project deliberately selects another independent integer provider. The
float64 highdicom variant may proceed independently because it does not claim
to satisfy this planned row.
