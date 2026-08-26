# Independent Conformance Validation

This directory defines the reproducible, viewer-neutral validation framework for
generated DICOM corpora. Results are engineering evidence produced by named
independent tools; they are not official DICOM certification.

## Baseline recorded 2026-08-26

The locked default suite passed with 150 tests. An all-features seed-1 `all`
corpus generated 108 files and internal validation reported zero failures. The
all-features seed-1 `legacy` corpus generated one file and also reported zero
failures. These temporary corpora were not committed.

On the baseline arm64 macOS host, DCMTK 3.7.0 `dcmdump` and `dcmdjpeg`,
`ojph_compress`, and `dcmcjpeg` were installed. `dciodvfy`, `dcentvfy`, GDCM,
and PixelMed were absent. Consequently, real IOD/entity acceptance is blocked
until a pinned dicom3tools package is approved and installed; this does not
block the hermetic framework implementation.

## Adapter decision matrix

| Role | Adapter | Initial command | Requirement | Acquisition and constraints |
| --- | --- | --- | --- | --- |
| Per-instance IOD | `dicom3tools-dciodvfy` | `dciodvfy -new` | Required | dicom3tools BSD license; pin source snapshot/package and executable hash. Homebrew does not currently provide it on this host. Debian packages both validator commands; upstream publishes source and platform builds. Validator definitions evolve, so the snapshot/definition baseline must remain visible. |
| Corpus entity consistency | `dicom3tools-dcentvfy` | `dcentvfy -f <file-list>` | Required | Same dicom3tools identity and acquisition decision as `dciodvfy`; pass files through its one-path-per-line file-list option to avoid argument limits. |
| Independent parse | `dcmtk-dcmdump` | `dcmdump +fo` | Required | DCMTK is BSD-style licensed and cross-platform. Baseline is Homebrew DCMTK 3.7.0. Dictionary and character mapping data affect behavior and must be noted with the fingerprint. |
| Independent lossless decode | `dcmtk-dcmdjpeg` | `dcmdjpeg` | Capability-based | Suitable for JPEG families supported by the installed DCMTK build. It is not independent for cases encoded by the project's DCMTK `dcmcjpeg` path. Raw native-byte normalization still needs a proven adapter. |
| Independent lossless decode | `gdcm-decode` | `gdcmconv`/`gdcmraw` | Optional candidate | GDCM is cross-platform and BSD licensed. Not installed; exact command behavior and native-byte normalization remain research targets. |
| SR second validation | `pixelmed-sr-validator` | Java `DicomSRValidator` | Optional milestone | PixelMed Java toolkit is source-distributed under its own BSD-style license. Package/JAR pinning and template identity must be resolved before enabling it. Unavailability never weakens primary IOD acceptance. |

The primary validator and entity checker cannot be replaced by a parser. Pixel
decode adapters must declare independence from the generator encoder and report
`same_implementation` or `unsupported` when that invariant is not met.

## Configuration

`validators.json` is the committed command policy. Paths can be overridden in a
run-specific config; arguments are always arrays and are executed directly,
never through a shell. `validator-lock.json` contains accepted real fingerprints
once acquired. `accepted-findings.json` contains only exact, reviewed findings.

Generated bundles belong below ignored `reports/conformance/`. Every run is
driven solely by `manifest.json` file entries and uses manifest-relative paths.

## Pixel evidence

`pixel-decoders.json` records the transfer-syntax independence matrix and exact
blockers. The first promoted adapter is DCMTK `dcmdrle`: it decompresses RLE
independently of the project-owned encoder, then `dcmdump +W` extracts native
Pixel Data. The adapter normalizes 16-bit sample byte order and planar color to
the manifest's interleaved frame-hash convention before comparison. A real
seed-1 all-profile run on the locked arm64 macOS tools matched all 58 RLE files.
Set `DTS_REAL_CONFORMANCE=1` to exercise that conditional integration test.

## Remaining acquisition decision

Choose one immutable dicom3tools distribution per supported CI platform. A
source snapshot is preferable for Linux; the upstream macOS binary or a frozen,
license-preserving mirror can be used for arm64 macOS after its checksum and
definition vintage are reviewed. Record the selected identity in
`validator-lock.json`; do not fetch validators during ordinary tests.
