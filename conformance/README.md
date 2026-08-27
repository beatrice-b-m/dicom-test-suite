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
`ojph_compress`, and `dcmcjpeg` were installed. On 2026-08-26 the upstream
universal macOS dicom3tools snapshot `1.00.snapshot.20260803085716` was installed
under a versioned Homebrew prefix with its matching BSD license. The two
validator executables and both source/binary archives are pinned by SHA-256 in
`validator-lock.json`. GDCM remains absent. PixelMed release 20260608 was
acquired and characterized for SR validation with its Java and classpath
artifacts locked as one composite adapter identity.
The pydicom `dicom-validator` 0.8.2 runtime is also locked through `uv` with
official DICOM 2026b DocBook inputs and derived definitions for the unsigned
32-bit Secondary Capture case that dicom3tools cannot evaluate.

## Adapter decision matrix

| Role | Adapter | Initial command | Requirement | Acquisition and constraints |
| --- | --- | --- | --- | --- |
| Per-instance IOD | `dicom3tools-dciodvfy` | `dciodvfy -new` | Required | dicom3tools BSD license; pin source snapshot/package and executable hash. Homebrew does not currently provide it on this host. Debian packages both validator commands; upstream publishes source and platform builds. Validator definitions evolve, so the snapshot/definition baseline must remain visible. |
| U32 SC per-instance IOD | `pydicom-dicom-validator-u32` | `python -m dts_dicom_validator_adapter` | Required for its declared case only | `uv` locks CPython 3.12.12, `dicom-validator` 0.8.2, pydicom 3.0.2, and transitive packages. `DTS_DICOM_VALIDATOR_PYTHON` selects the prepared interpreter and `DTS_DICOM_VALIDATOR_STANDARD_HOME` selects the external hash-locked 2026b cache. It is not a generation-profile runtime. |
| Corpus entity consistency | `dicom3tools-dcentvfy` | `dcentvfy -f <file-list>` | Required | Same dicom3tools identity and acquisition decision as `dciodvfy`; pass files through its one-path-per-line file-list option to avoid argument limits. |
| Independent parse | `dcmtk-dcmdump` | `dcmdump +fo` | Required | DCMTK is BSD-style licensed and cross-platform. Baseline is Homebrew DCMTK 3.7.0. Dictionary and character mapping data affect behavior and must be noted with the fingerprint. |
| Independent lossless decode | `dcmtk-dcmdjpeg` | `dcmdjpeg` | Capability-based | Suitable for JPEG families supported by the installed DCMTK build. It is not independent for cases encoded by the project's DCMTK `dcmcjpeg` path. Raw native-byte normalization still needs a proven adapter. |
| Independent lossless decode | `gdcm-decode` | `gdcmconv`/`gdcmraw` | Optional candidate | GDCM is cross-platform and BSD licensed. Not installed; exact command behavior and native-byte normalization remain research targets. |
| SR second validation | `pixelmed-sr-validator` | Java `DicomSRValidator` | Optional milestone | PixelMed 20260608 validates the three generated SR SOP Classes. Set `DTS_PIXELMED_HOME` to the extracted binary/dependency release root. Java, JARs, and embedded definition resources are fingerprinted together. Unavailability never weakens primary IOD acceptance. |

The primary validator and entity checker cannot be replaced by a parser. Pixel
decode adapters must declare independence from the generator encoder and report
`same_implementation` or `unsupported` when that invariant is not met.

## Configuration

`validators.json` is the committed command policy. Paths can be overridden in a
run-specific config; arguments are always arrays and are executed directly,
never through a shell. `validator-lock.json` contains accepted real fingerprints
once acquired. `accepted-findings.json` contains only exact, reviewed findings.

Primary IOD routing is exact-case-first. A validator with
`supported_case_ids` supersedes the unrestricted primary only for those IDs;
overlapping declarations are rejected. The U32 route therefore cannot alter
acceptance for existing cases, and strict verification requires the optional
tool to be available and lock-matched whenever it produced evidence.

The U32 payload path uses adapter version 0.2.0 to read raw OW bytes through
pydicom and unpack exact little-endian unsigned 32-bit words without NumPy.
Its deterministic sidecar is cross-linked to the locked adapter, all image
attributes, the four expected values, and every manifest frame hash.

The locked dicom3tools entity checker asserts if it reads the original U32
Pixel Data element. For entity consistency only, the runner creates a
hash-linked projection by copying every source byte before the terminal
`(7FE0,0010) OW` element and omitting that complete element. The source copy,
removed offset/length/value hash, projected input, and file list are preserved
in evidence and reconstructed during strict verification. IOD and pixel checks
always use the untouched original; an ineligible layout or tampered projection
fails rather than falling back.

The committed PixelMed dispositions apply only to the Basic Text and
Comprehensive SR cases, the locked composite validator fingerprint, and the
exact template-less warnings observed on 2026-08-26. They confirm that those
two generic recipes were not created from a named root template; they do not
suppress SR IOD errors or findings from the TID 2010 Key Object Selection case.

The committed dicom3tools disposition applies only to the zero-length
Instance Number in
`geometry/ct/duplicate_missing_instance_number/slice-003.dcm`, the locked
`dciodvfy` fingerprint, and its exact DICOMDIR-usability warning. PS3.3 Table
C.7-9 defines Instance Number as Type 2 for this CT Image, so an empty value is
intentional and valid. The disposition does not accept omission of the element,
an IOD error, or any other Instance Number warning.

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

## Installation identity

The arm64 macOS validator acquisition decision is complete. Other acceptance
platforms still require their own immutable source or binary identity and
fingerprints. The executables do not expose a version flag, so the framework
uses their exact hashes and the immutable upstream snapshot name. Ordinary tests
never fetch or require these external validators.
