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
LittleCMS 2.19 is locked as a composite ICC adapter over `transicc` and its
dynamically linked `liblcms2.2` implementation.

## Adapter decision matrix

| Role | Adapter | Initial command | Requirement | Acquisition and constraints |
| --- | --- | --- | --- | --- |
| Per-instance IOD | `dicom3tools-dciodvfy` | `dciodvfy -new` | Required | dicom3tools BSD license; pin source snapshot/package and executable hash. Homebrew does not currently provide it on this host. Debian packages both validator commands; upstream publishes source and platform builds. Validator definitions evolve, so the snapshot/definition baseline must remain visible. |
| U32 and non-square SC per-instance IOD | `pydicom-dicom-validator-u32` | `python -m dts_dicom_validator_adapter` | Required for its declared cases only | `uv` locks CPython 3.12.12, `dicom-validator` 0.8.2, pydicom 3.0.2, and transitive packages. `DTS_DICOM_VALIDATOR_PYTHON` selects the prepared interpreter and `DTS_DICOM_VALIDATOR_STANDARD_HOME` selects the external hash-locked 2026b cache. It is not a generation-profile runtime. |
| Registration second IOD opinion | `pydicom-dicom-validator-registration` | `python -m dts_dicom_validator_adapter` | Required for its declared cases only | Runs in addition to, never instead of, locked `dciodvfy` for Spatial Registration and Deformable Spatial Registration. The same `uv` runtime and exact 2026b definitions are independently fingerprinted under the case-scoped secondary adapter. |
| Presentation-state second IOD opinion | `pydicom-dicom-validator-presentation-state` | `python -m dts_dicom_validator_adapter` | Required for its declared cases only | Runs additively for Color Softcopy, Advanced Blending, and Blending Softcopy Presentation States. It reuses the independently implemented, `uv`-locked runtime and hash-locked 2026b definitions under a separate case-scoped adapter identity. |
| Linked RT second IOD opinion | `pydicom-dicom-validator-rt` | `python -m dts_dicom_validator_adapter` | Required for its declared cases only | Runs additively for the linked RT Plan and RT Image. It reuses the unchanged `uv`-locked adapter and exact 2026b definitions under a separate qualification identity; primary IOD validation remains locked `dciodvfy`. |
| Waveform second IOD and payload opinion | `pydicom-dicom-validator-waveform` | `python -m dts_dicom_validator_adapter` / `--waveform` | Required for its declared cases only | Runs additively for Twelve-lead and General ECG. The normal route validates the 2026b IOD; the waveform route independently extracts each ordered raw OW group with pydicom and decodes signed samples with Python `struct`, without NumPy or generator code. |
| U1 SC independent pixels | `dcmtk-dcm2img-u1` | `dcm2img +Fa +Fn -M -W +Pid -O +opn 1` | Required when collecting evidence for its declared case | DCMTK 3.7.0 emits one PGM per frame. The collector requires P2, dimensions 3 by 3, maximum value one, exact samples, and exact frame hashes; `dcmdump +W` separately binds the packed Pixel Data bytes. Primary IOD validation remains `dciodvfy`. |
| Linked RT Image independent pixels | `dcmtk-dcm2img-rt-image` | `dcm2img +F 1 -S -bs -M -W +Pid -O +opn 8` | Required when collecting evidence for its declared case | DCMTK 3.7.0 emits one P2 PGM. The collector requires the exact 4 by 4 gradient, maximum value 255, and decoded SHA-256 `a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811`; isolated `dcmdump +W` must emit exactly one 16-byte native OB value with the same hash. Primary IOD validation remains `dciodvfy`. |
| ICC profile processing | `littlecms-transicc-icc` | `transicc -n -i<profile> -o*XYZ -t0` | Required when collecting evidence for its declared case | Set `DTS_LCMS_HOME` to the immutable LittleCMS prefix. Locked DCMTK reconstructs the complete ICC OB value, strict checks enforce the DICOM input-profile header and `SRGB` label, and LittleCMS 2.19 must reproduce four fixed RGB-to-XYZ vectors. Primary IOD validation remains `dciodvfy`. |
| Corpus entity consistency | `dicom3tools-dcentvfy` | `dcentvfy -f <file-list>` | Required | Same dicom3tools identity and acquisition decision as `dciodvfy`; pass files through its one-path-per-line file-list option to avoid argument limits. |
| Independent parse | `dcmtk-dcmdump` | `dcmdump +fo` | Required | DCMTK is BSD-style licensed and cross-platform. Baseline is Homebrew DCMTK 3.7.0. Dictionary and character mapping data affect behavior and must be noted with the fingerprint. |
| Native floating payloads | `dcmtk-dcmdump` | `dcmdump +L +P 7fe0,0008/0009` | Required for native OF/OD cases | The collector parses complete decimal values as `f32` or `f64`, reconstructs exact little-endian IEEE 754 bytes, and compares every manifest frame hash. This payload proof is independent of the highdicom/pydicom generator; primary IOD validation remains locked `dciodvfy`. |
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
overlapping declarations are rejected. The U32 and non-square routes therefore cannot alter
acceptance for existing cases, and strict verification requires the optional
tool to be available and lock-matched whenever it produced evidence.

Secondary IOD routing is additive and exact-case-only. The registration
adapter runs after the selected primary for only
`derived/registration/spatial_ct_pair` and
`derived/registration/deformable_ct_pair`. Strict verification requires its
tool and evidence to be available, lock-matched, and complete for both cases.
Qualification against the locked 2026b definitions accepted the valid Spatial
Registration prototype and rejected a missing Type 1 Matrix Registration
Sequence. It did not reject a VM 15 transformation matrix, non-orthonormal
`RIGID` values, or a dangling referenced SOP Instance UID. Consequently it
cannot replace `dciodvfy`, strict registration semantics, or `dcentvfy`
reference closure, and no finding is allowlisted for this route.

The presentation-state secondary adapter is likewise exact-case-only for
`derived/presentation-state/color_softcopy`,
`derived/presentation-state/advanced_blending`, and
`derived/presentation-state/blending`. Qualification recognized all three
2026b IODs and added a Type 1 finding when Content Label was removed. It also
identified the target-specific ICC, blending, display, frame-of-reference, and
common-reference modules. A dangling referenced SOP Instance UID did not alter
its Color Softcopy findings, and it missed absent conditional palette LUT data
that `dciodvfy` reported for the Blending probe. Strict verification therefore
requires this additive evidence without replacing `dciodvfy`, project-owned
presentation semantics, or `dcentvfy` reference closure; no finding is
allowlisted for this route.

The linked RT secondary adapter is exact-case-only for
`non-image/rt/plan_linked` and `non-image/rt/image_linked`. Feasibility probes
using the exact SOP Class UIDs selected the locked 2026b `RT Plan IOD` and
`RT Image IOD` definitions and produced the corresponding mandatory RT module
findings rather than an unsupported-SOP result. The corrected RT Plan
prototype, SHA-256
`e9337a6c46fe85b56f1f563120dd3caf56ea1335355792db42386db959be6db2`,
uses Study ID `DTS-RTSTRUCT` to align with the existing Structure Set. Locked
`dciodvfy -new` identified `RTPlan`; the uv-locked secondary selected the 2026b
RT Plan IOD and returned `Passed` with zero errors; and `dcmdump +fo` parsed the
exact file. The separately qualified RT Image prototype has SHA-256
`460d525ab06aaf74df963029f3ab39c2536e4e1c5bf4b75fcf16b500382db20c`.
Both IOD validators accepted it with zero errors, `dcmdump +fo` parsed it, and
the exact DCMTK pixel route proved one 4 by 4 P2 raster plus one 16-byte native
OB value with decoded and raw SHA-256
`a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811`.

The generated 2026b RT Plan definition does not provide a sufficiently
trustworthy standalone condition for omission of the whole RT Beams Module
when Number of Beams is one. Strict Rust therefore owns that conditional
presence, exact fraction/beam/control-point semantics, cardinality, and order.
Across all 20 locked Plan mutations, both IOD validators detected missing Plan
Label, `PATIENT` geometry without the Structure Set, and all four missing zero
accessory counts. `dciodvfy` alone detected a one-item Control Point Sequence.
Isolated `dcentvfy` alone added a missing-referenced-SOP finding for the
dangling Structure Set UID. Both IOD validators missed the wrong Structure SOP
Class; dangling, duplicated, or swapped Structure/Dose identities; fraction
and beam mismatch; dangling or duplicate Beam Number; device order; jaw
positions; control-point index or order; isocenter; first or final meterset;
and wrong Study or Frame of Reference. The secondary validator also missed the
wrong control-point count. Strict Rust owns every semantic miss; `dcentvfy`
owns dangling instance closure. All mutations parsed with `dcmdump`, which is
not semantic detection.

The exact CT/Structure Set/Dose/Plan entity run retains two immutable upstream
Study ID diagnostics: Dose `DTS-RTDOSE` versus Plan/Structure
`DTS-RTSTRUCT`, and enhanced CT `DTS-ECT` versus Plan/Structure
`DTS-RTSTRUCT`. They remain visible and unallowlisted. RT Plan entity
acceptance means no additive missing or dangling reference finding beyond
those two baseline diagnostics; it does not mean a silent `dcentvfy` run or a
zero exit code. The secondary validator cannot replace `dciodvfy`, this exact
entity-closure rule, strict Rust semantics, or the separate RT Image pixel
decoder. No linked RT finding is allowlisted.

Across all 20 locked Image mutations, `dciodvfy` detected 10, the uv-locked
IOD adapter detected 6, and the exact DCMTK pixel route detected 6. Both IOD
validators detected missing Image Type, Label, Plane, the `NON_NORMAL`
orientation condition, Bits Stored, and Pixel Representation. `dciodvfy`
alone also detected the `PORTAL` origin condition, High Bit, pixel length, and
shape. The pixel route detected shape, length, Bits Stored, High Bit, Pixel
Representation, and a changed payload byte. Isolated `dcentvfy` alone added a
missing-SOP finding for the wrong Plan UID. Strict Rust owns wrong beam and
fraction linkage, spacing, position, SAD, SID, Study, and Frame of Reference,
as well as the complete exact contract. Every mutation parsed with `dcmdump`.

The five-object baseline retains the same two visible Study ID diagnostics and
no missing or dangling reference finding. Removing CT, Structure Set, Dose, or
Plan adds missing-SOP evidence. Strict validation also rejects a syntactically
valid but stale Plan source digest after reopening the generated Plan. The
integrated qualification run ID is
`d0d78ffccf44218a27944cf1b80dec63c8afa7162b0e085532feb51706a04714`;
its RT Image IOD, parsing, and pixel routes are clean. Qualification does not
promote the planned RT Image registry row.

The waveform secondary adapter is exact-case-only for
`non-image/waveform/twelve_lead_ecg` and
`non-image/waveform/general_ecg`. General ECG qualification used the exact
prototype with instance SHA-256
`a656720538672c95aacdf068ba89b0c6d6f78042610f3a665d55065d0a4ab40c`.
Locked `dciodvfy` identified it as `GeneralECG`; the uv-locked secondary
validator identified the 2026b General ECG IOD with zero errors; `dcmdump +fo`
parsed it; and isolated `dcentvfy` produced no findings. The separate
`--waveform` route observed two ordered groups, 16 total channels, 56,000
payload bytes, and aggregate SHA-256
`c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea`.

Qualification mutations established the external boundary. Both IOD
validators rejected a channel missing both conditional skew attributes.
Neither IOD validator rejected five groups, 25 channels in one group, 199 or
1,001 Hz, `US` sample interpretation, a wrong or duplicated CID 3001 source,
sample-count/payload disagreement, reversed groups, or a changed payload byte.
The group-aware raw route rejected every one of those mutations, including the
wrong and duplicated sources and missing skews. DCMTK parsed every mutation,
and isolated entity checks reported no findings, as expected for syntactically
valid single-instance probes. Strict Rust validation therefore remains
authoritative for exact IOD constraints and manifest semantics, while the raw
route independently enforces
ordered groups, locked shapes, formula, channel-then-sample interleave,
channel definitions, group and per-channel hashes, aggregate hash, and value
range. Strict verification requires both additive IOD and payload evidence; no
waveform finding is allowlisted.

The U1 case stays on the unrestricted, locked `dciodvfy` primary route because
that validator recognizes Multi-frame Single Bit Secondary Capture and its
PS3.3 A.8.2.4 content constraints. The `uv`-locked pydicom validator was
evaluated but did not reject an invalid `8/8/7` Bits
Allocated/Stored/High Bit control, so it is not an acceptance oracle for U1.
Normalized findings remain authoritative because `dciodvfy` reports forbidden
Planar Configuration while returning zero for that control.

The ICC VL Photographic case also stays on the unrestricted `dciodvfy` route.
That validator enforces selection and non-empty presence of the ICC Profile
Module but did not reject empirical `acsp`, `scnr`, or Color Space mismatch
controls. The case-scoped composite therefore uses complete `dcmdump +L`
extraction, exact bytes and header/tag-table checks, and an operational
LittleCMS transform. Strict verification requires both external tools to be
available and lock-matched and rejects relinked sidecars; no ICC failure can be
converted to ordinary unsupported native-pixel coverage.

The U32 payload path uses adapter version 0.6.0 to read raw OW bytes through
pydicom and unpack exact little-endian unsigned 32-bit words without NumPy.
Its deterministic sidecar is cross-linked to the locked adapter, all image
attributes, the four expected values, and every manifest frame hash.

Native float32 and float64 Parametric Maps stay on the unrestricted, locked
`dciodvfy` primary route. The independent DCMTK parser selects `(7FE0,0008)`
OF or `(7FE0,0009)` OD from manifest sample type, reconstructs the corresponding
binary width, and writes separate `dcmtk-native-float32` or
`dcmtk-native-float64` evidence. Strict completeness requires passing,
lock-matched independent evidence for every native floating payload.

For `classic/sc/nonsquare_pixel_spacing`, adapter version 0.4.0 also performs
case-scoped semantic extraction with pydicom. It proves that the two files use
mutually exclusive physical-spacing and integer-aspect-ratio declarations,
checks exact VR, VM, lexical values and required absences, and binds the 4x6
native OB payload to the manifest hash. Strict verification requires one
independent, hash-linked sidecar for each variant.

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

Native pixel coverage remains case-scoped rather than implied for every native
shape. U32 uses the `uv`-locked pydicom adapter described above. U1 uses locked
DCMTK 3.7.0 `dcm2img`, executable SHA-256
`6a6103a7c516814b5eb44f53d198b111cbaf1678de5952ab7d31961732f112d5`,
to decode both non-byte-aligned frames and locked `dcmdump` to extract the raw
four-byte Value Field. Strict verification rejects a missing frame, any PGM
shape/maxval/sample mismatch, manifest relinking, payload mismatch, lock
mismatch, or policy mismatch. Every other native shape remains explicitly
unsupported in `pixel-decoders.json`.

The linked RT Image uses a separate `dcmtk-dcm2img-rt-image` route over that
same locked DCMTK 3.7.0 executable. The exact `+F 1 -S -bs -M -W +Pid -O +opn
8` policy produces a P2 raster whose 16 samples must be `0, 17, ... 255` and
hash to `a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811`.
A separate `dcmdump +W` invocation must produce exactly one raw file containing
the same 16-byte native OB value. This evidence is additive to both the primary
IOD validator and the linked-RT secondary validator.

## Installation identity

The arm64 macOS validator acquisition decision is complete. Other acceptance
platforms still require their own immutable source or binary identity and
fingerprints. The executables do not expose a version flag, so the framework
uses their exact hashes and the immutable upstream snapshot name. Ordinary tests
never fetch or require these external validators.
