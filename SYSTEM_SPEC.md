# System Specification: `dicom-test-suite`

**Status:** architecture baseline; implementation status is tracked elsewhere

**Spec version:** 0.2.1

**Last reviewed:** 2026-08-28

**Primary consumer today:** `dcmview`  
**Design stance:** viewer-agnostic, standards-led, deterministic, synthetic

## 1. Purpose

`dicom-test-suite` generates a broad, deterministic, synthetic DICOM corpus for validating DICOM viewer compatibility and DICOM handling behavior. The initial downstream use case is `dcmview`, but this repository must remain viewer-agnostic. A failing viewer is useful signal; generated cases should reflect the DICOM standard, common interoperability risks, and real-world compatibility stressors rather than the current capabilities of any one implementation.

Generated DICOM payloads are build artifacts. They must not be committed. The repository commits code, case recipes, metadata schemas, expected results, validation logic, coverage reports, and compatibility report schemas.

> **Current-state note:** This specification preserves the original architecture
> and phased requirements. Several sections intentionally describe initial or
> historical milestones. For current commands and implemented capability, use
> `README.md`, `docs/generation-guide.md`, `cases/registry.json`, and a fresh
> generated coverage report. Those artifacts supersede future-tense phase text
> when determining what the executable can do today.

The project should become useful in three modes:

1. **Developer smoke testing:** small generated corpus for fast local validation.
2. **Conformance-valid compatibility testing:** a wider corpus of valid DICOM Part 10 files covering viewer-relevant axes.
3. **Explicit robustness testing:** opt-in negative/fuzz profiles, kept separate from valid conformance profiles.

## 2. Scope and Non-Goals

### In scope

The suite shall generate valid synthetic DICOM Part 10 files across standard IOD families and compatibility axes. It shall support clearly labeled legacy and stress profiles for files that are valid but uncommon, retired, or historically problematic.

The suite shall generate:

- classic single-frame image objects;
- enhanced multi-frame image objects;
- mammography and projection X-Ray variants;
- visible light, pathology, and color objects;
- selected derived, annotation, structured-report, radiotherapy, waveform, and encapsulated-document objects;
- transfer-syntax variants when encoder support is available;
- deterministic manifests and coverage reports.

### Out of scope for initial implementation

The initial implementation excluded intentionally invalid DICOM files. The
current implementation provides explicit `negative` and `fuzz` profiles, which
remain separated from conformance-valid cases as required by this architecture.

The suite shall not ship generated DICOM data, a redistributed DICOM Standard corpus, or a prebuilt `dicom-standard-kb` database.

The suite shall not encode assumptions around `dcmview` in generator logic. Viewer-specific behavior belongs only in optional runner adapters, compatibility reports, and downstream issue triage.

## 3. Standards Reference Policy

### 3.1 Primary reference source

The primary machine-queryable standards reference for this project is [`dicom-standard-kb`](https://github.com/beatrice-b-m/dicom-standard-kb). Its command-line and MCP surfaces should be used first for DICOM data elements, UIDs, IODs, modules, SOP Classes, defined terms, enumerated values, and cited standard text.

`dicom-standard-kb` is a builder/query layer for local, edition-pinned knowledge bases. It does not redistribute the DICOM Standard or a prebuilt knowledge base. Generated KB artifacts must remain outside this repository.

The current `dicom-standard-kb` parser surface is focused on PS3.3, PS3.4, and PS3.6. This is sufficient for many IOD/module/SOP Class/UID lookups, but it is not complete coverage of the DICOM Standard. The suite must therefore have an explicit fallback and patching process.

### 3.2 Authority hierarchy

For standards decisions, use this order:

1. **Official DICOM Standard PDFs** for authoritative text when exact wording or conflict resolution matters.
2. **Official DICOM Standard HTML, CHTML, DocBook XML, or TargetDB artifacts** for convenient lookup, citation, and machine-readable extraction.
3. **`dicom-standard-kb` query results** as the default project interface to covered standard content.
4. **Implementation-library documentation** only for implementation feasibility, not for defining DICOM conformance.
5. **Viewer behavior** only as compatibility evidence, never as the definition of valid DICOM.

When `dicom-standard-kb` and official source artifacts disagree, treat the official source as authoritative and file a KB patch or project issue.

### 3.3 Standards lock

The repository shall include a committed `standards.lock.json` describing the standards baseline used to generate and validate recipes. The lock is a reproducibility artifact, not a replacement for source standards.

Recommended shape:

```json
{
  "schema_version": "0.1.0",
  "dicom_base_edition": "2026b",
  "include_final_text_after_base": false,
  "verified_at": "2026-06-13",
  "official_source_policy": "PDF authoritative; HTML/CHTML/DocBook/TargetDB convenience",
  "dicom_standard_kb": {
    "repository": "https://github.com/beatrice-b-m/dicom-standard-kb",
    "commit": "<pinned commit>",
    "db_edition": "2026b",
    "db_sha256": "<local db sha256, if available>",
    "parser_surface": ["PS3.3", "PS3.4", "PS3.6"]
  },
  "source_artifacts": [
    {"part": "PS3.3", "format": "docbook_xml", "sha256": "<sha256>"},
    {"part": "PS3.4", "format": "docbook_xml", "sha256": "<sha256>"},
    {"part": "PS3.5", "format": "pdf", "sha256": "<sha256>"},
    {"part": "PS3.6", "format": "docbook_xml", "sha256": "<sha256>"},
    {"part": "PS3.10", "format": "pdf", "sha256": "<sha256>"}
  ],
  "notes": [
    "Use official source artifacts for areas not covered by dicom-standard-kb.",
    "Patch dicom-standard-kb for repeatable lookup gaps that affect recipe generation."
  ]
}
```

The project must decide explicitly whether it targets only the base DICOM edition named in the lock or the base edition plus final-text supplements and correction items approved as of a specific date. Do not rely on a plain edition string alone when exact reproducibility matters.

### 3.4 Standards evidence per recipe

Every implemented recipe shall include standards evidence. Evidence may be encoded in the case registry, recipe source, or sidecar metadata, but it must be available to developers and reports.

Minimum recipe evidence:

```json
{
  "standards_evidence": [
    {
      "source": "dicom-standard-kb",
      "edition": "2026b",
      "query": "dicom-kb iod modules 'CT Image' --edition 2026b",
      "covered": true
    },
    {
      "source": "official-dicom-standard",
      "part": "PS3.10",
      "anchor": "Table 7.1-1 DICOM File Meta Information",
      "reason": "Part 10 file meta validation is outside the current primary KB parser surface."
    }
  ]
}
```

### 3.5 Gap and patch process

When a recipe requires standard content not represented in `dicom-standard-kb`, the developer shall do one of the following:

1. **Patch upstream or locally:** add support or extracted content to `dicom-standard-kb` when the gap is systematic, repeatable, and useful to future recipes.
2. **Add a local standards note:** create `standards/source-notes/<topic>.md` when the gap is narrow or the implementation is not ready to patch the KB.
3. **Add a blocked/skipped case:** keep the planned case in the registry with a clear skip reason when neither source evidence nor implementation support is ready.

A local standards note must include:

- the affected case or recipe IDs;
- the DICOM part and section/table/anchor used;
- the reason the KB was insufficient;
- whether the gap should become a KB patch;
- date checked and source artifact identity from `standards.lock.json`.

### 3.6 Do not redistribute standards artifacts

Do not commit official DICOM source artifacts, generated full-standard JSON, generated full-text indexes, or generated KB databases. Caches belong under user cache directories or ignored project paths.

Allowed repository artifacts include:

- standards lock metadata;
- concise anchor references;
- small hand-authored notes that cite sections/tables without copying large amounts of standard text;
- tests for project-specific standards lookup behavior;
- patches to `dicom-standard-kb` when permitted by that project.

## 4. Core Requirements

### 4.1 Deterministic generation

The same recipe version, seed, profile, standards lock, generator version, feature flags, Rust toolchain, target triple, Cargo lock, DICOM-rs crate versions, and codec versions shall produce reproducible output according to each case's declared determinism level.

Supported determinism levels:

- `byte_stable`: exact file SHA-256 reproducibility is expected.
- `semantic_stable`: decoded pixel/frame hashes, manifest content, and semantic expectations are stable, but encoded bytes may vary by codec version.
- `unstable`: the case is allowed only with explicit warning and manifest metadata; avoid this in `smoke` and `core`.

Deterministic generation must control:

- UID inputs and UID generation algorithm;
- Study/Series/SOP Instance UID roles;
- timestamps and dates used in generated metadata;
- attribute ordering where the writer permits control;
- sequence item ordering;
- file preamble content;
- Implementation Class UID and Implementation Version Name;
- private creator strings;
- synthetic pixel pattern seed;
- codec feature flags and external codec versions.

CI shall include at least one two-run reproducibility check:

```sh
dicom-test-suite generate --profile smoke --out /tmp/dts-a --seed 1
dicom-test-suite generate --profile smoke --out /tmp/dts-b --seed 1
diff -r /tmp/dts-a /tmp/dts-b
```

For compressed cases declared `semantic_stable`, CI shall compare decoded frame hashes and manifest semantics rather than raw file bytes.

### 4.2 Synthetic data and PHI policy

All generated SOP Instances are artificial test data. Every generated SOP Instance shall set `Synthetic Data (0008,001C)` to `YES` unless a recipe explicitly documents a standards-based reason not to do so.

Derived objects produced from generated synthetic instances shall also set `Synthetic Data (0008,001C)` to `YES`.

The generator shall avoid PHI-like content. Use deterministic synthetic names and IDs such as:

- `DTS^Synthetic^Patient001`
- `DTS-STUDY-<case-hash>`
- `DTS-SERIES-<case-hash>`

Recommended additional metadata:

- `Contributing Equipment Sequence (0018,A001)` identifying the generator as synthesizing equipment where practical;
- deterministic `Manufacturer`, `Manufacturer's Model Name`, and `Software Versions` values that identify the generator without implying a real scanner created the instance.

### 4.3 DICOM Part 10 file contract

DICOM Part 10 output is the default. Every generated `.dcm` file shall contain exactly one SOP Instance and a valid File Meta Information header.

Required file-level invariants:

- 128-byte preamble, all zero unless a specific valid case exercises preamble behavior;
- uppercase `DICM` prefix;
- File Meta Information encoded as Explicit VR Little Endian, regardless of the dataset transfer syntax;
- File Meta Information Version `(0002,0001)`;
- Media Storage SOP Class UID `(0002,0002)`;
- Media Storage SOP Instance UID `(0002,0003)`;
- Transfer Syntax UID `(0002,0010)`;
- Implementation Class UID `(0002,0012)`;
- optional but deterministic Implementation Version Name `(0002,0013)`;
- file meta SOP Class UID and SOP Instance UID matching dataset `SOP Class UID (0008,0016)` and `SOP Instance UID (0008,0018)`;
- no group `0002` elements outside File Meta Information.

Plain datasets without Part 10 wrapping may be added only as explicit later cases and must not appear in normal `smoke`, `core`, or `all` conformance profiles.

### 4.4 Manifest per run

Every generation run shall emit `manifest.json` at the output root. The manifest must describe:

- generator and environment metadata;
- standards lock metadata;
- generated files;
- skipped/planned-but-unavailable cases;
- validation results;
- expected semantic and visual interpretation;
- profile and coverage membership;
- deterministic hashes.

The manifest must be versioned by `manifest_schema_version`, and CI shall validate it against `schemas/manifest.schema.json`.

### 4.5 Profiles

Supported profiles:

- `smoke`: fastest sanity set; only small, byte-stable files; no optional external codecs required.
- `core`: common valid viewer-relevant cases; local-friendly size and runtime.
- `extended`: broader valid coverage, including enhanced multi-frame and derived objects.
- `legacy`: valid retired or uncommon behavior, excluded from `core`.
- `stress`: valid but large, slow, or expensive cases; explicit opt-in only.
- `all`: `smoke + core + extended`, excluding `legacy` and excluding `stress`
  unless `--include-stress` is passed.
- `negative`: deterministic invalid or malformed files; never included in
  `all`.
- `fuzz`: bounded payload-free robustness qualification; never included in
  `all`.

Recommended profile budgets:

| Profile | Target count | Target size | Runtime goal | Notes |
|---|---:|---:|---:|---|
| `smoke` | 5-10 files | < 5 MB | seconds | Always generated in CI. |
| `core` | 50-150 files | < 100 MB | local-friendly | No huge WSI/video. |
| `extended` | 200-500 files | configurable | slower | Broader but still valid. |
| `legacy` | configurable | configurable | slower | Retired/uncommon valid cases. |
| `stress` | opt-in | may be large | slow | May include WSI/video/large multi-frame. |

A case may belong to multiple profiles. The case registry shall define membership explicitly.

### 4.6 Viewer independence

Generator logic and validation logic must be standards-led. Viewer-specific behavior may be captured only in:

- optional viewer runner adapters;
- per-viewer compatibility reports;
- issue triage notes;
- downstream viewer-specific expected-failure files.

Do not change a valid recipe merely because `dcmview` cannot yet read it. Instead, mark viewer status in a report.

### 4.7 Generated artifacts must stay out of git

Generated DICOM outputs, generated manifests, sidecars, reports, caches, official DICOM source artifacts, and generated KB databases must remain under ignored paths or ignored file extensions.

Minimum `.gitignore` expectations:

```gitignore
generated/
reports/
.cache/
*.dcm
*.dicom
*.ima
*.part10
*.validation.json
*.expected.json
*.coverage.json
*.sqlite
*.sqlite3
```

CI shall fail if generated DICOM-like payloads are accidentally staged.

## 5. Repository Artifacts

Recommended committed layout:

```text
.
  README.md
  SYSTEM_SPECS.md
  standards.lock.json
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  .gitignore
  schemas/
    manifest.schema.json
    case-registry.schema.json
    coverage-report.schema.json
    viewer-report.schema.json
  cases/
    registry.json
    smoke.json
    core.json
    extended.json
  standards/
    source-notes/
  src/ or crates/
  tests/
```

Generated artifacts shall be written outside committed paths by default, typically under `generated/<profile>/`.

## 6. Case ID Taxonomy and Case Registry

### 6.1 Case ID format

Case IDs shall be stable, human-readable, and path-safe.

Recommended format:

```text
<domain>/<iod_family>/<descriptor>
```

Examples:

```text
classic/sc/mono2_u8_explicit_le
classic/sc/mono1_u8_explicit_le
classic/sc/rgb_planar0_explicit_le
classic/ct/mono2_i16_rescale_12bit_explicit_le
classic/mg/for_presentation_mono1_u16_12bit_explicit_le
classic/mg/for_processing_mono2_u16_12bit_implicit_le
classic/cr/overlay_modality_voi_explicit_le
classic/mr/multislice_oblique_explicit_le
enhanced/ct/multiframe_shared_perframe_explicit_le
derived/seg/binary_multiframe_explicit_le
vl/photo/rgb_planar0_explicit_le
vl/photo/palette_color_explicit_le
```

Avoid mixing styles such as `ct/classic/...` and `classic/ct/...`.

### 6.2 Case registry

The repository shall include a case registry describing planned, implemented, skipped, and blocked cases. `list-cases` shall read from this registry.

Required registry fields:

```json
{
  "case_id": "classic/ct/mono2_i16_rescale_12bit_explicit_le",
  "status": "implemented",
  "profiles": ["core"],
  "recipe_id": "ct_classic_rescale",
  "recipe_version": "1.0.0",
  "iod_name": "CT Image",
  "sop_class_name": "CT Image Storage",
  "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2",
  "transfer_syntax_uid": "1.2.840.10008.1.2.1",
  "determinism": "byte_stable",
  "requirements": {
    "features": [],
    "external_codecs": [],
    "external_validators": []
  },
  "skip": null,
  "standards_evidence": []
}
```

Allowed `status` values:

- `planned`
- `implemented`
- `skipped`
- `blocked`
- `deprecated`

Skip and block reasons shall be structured:

```json
{
  "status": "blocked",
  "skip": {
    "reason_code": "codec_encode_unavailable",
    "message": "JPEG 2000 encoding is not available through the selected feature set.",
    "recheck_phase": "phase-6"
  }
}
```

## 7. Test Coverage Model

Coverage is built from two layers:

1. **IOD families:** representative SOP Classes and module combinations that viewers encounter.
2. **Orthogonal axes:** pixel layout, photometric interpretation, transfer syntax, geometry, sequences, overlays, annotations, references, character sets, and metadata edge cases.

The project shall maintain a generated or committed coverage matrix with at least:

```text
case_id | profile | status | IOD | SOP Class UID | transfer syntax | photometric | bits | frames | geometry | derived refs | validation status
```

The matrix prevents combinatorial explosion. Do not generate every cross-product. Prefer small, high-signal cases that isolate one or two compatibility risks.

Each generated file should be small enough for local iteration unless it specifically tests large images, tiled images, video, or multi-frame behavior.

## 8. IOD and SOP Class Families

### 8.1 Classic single-frame image

Required early coverage:

- Secondary Capture Image Storage.
- Computed Radiography Image Storage.
- CT Image Storage.
- MR Image Storage.
- Ultrasound Image Storage.
- Digital X-Ray Image Storage, For Presentation and For Processing.
- Digital Mammography X-Ray Image Storage, For Presentation and For Processing.
- X-Ray Angiographic and X-Ray Radiofluoroscopic image families.
- Nuclear Medicine Image Storage.
- Positron Emission Tomography Image Storage.

Important compatibility features:

- single-file images and multi-file series;
- instance sorting by `Instance Number`, `Image Position Patient`, and `Image Orientation Patient`;
- signed and unsigned monochrome pixels;
- modality LUT or rescale slope/intercept;
- VOI LUT and window center/width;
- MONOCHROME1 inversion, especially mammography;
- pixel padding value and pixel padding range;
- overlays and display shutters;
- missing or duplicated `Instance Number` while geometry remains valid.

### 8.2 Enhanced multi-frame image

Required coverage:

- Enhanced CT Image Storage.
- Enhanced MR Image Storage.
- Enhanced PET Image Storage.
- Enhanced US Volume Storage.
- Enhanced XA/XRF where codec and generator support allow.

Important compatibility features:

- `Number of Frames`;
- Shared Functional Groups Sequence;
- Per-Frame Functional Groups Sequence;
- Multi-frame Dimension Module;
- frame-specific position, orientation, pixel spacing, temporal position, echo, phase, and cardiac/respiratory timing;
- concatenation metadata for large logical multi-frame objects;
- empty versus populated per-frame groups where allowed;
- frame-level references for derived objects.

### 8.3 Mammography and projection X-Ray

Required coverage:

- Digital Mammography X-Ray Image Storage - For Presentation.
- Digital Mammography X-Ray Image Storage - For Processing.
- CR-like older mammography-compatible images, including CBIS-DDSM-style 12-bit monochrome patterns.

Important compatibility features:

- MONOCHROME1 and MONOCHROME2;
- 12 bits stored in 16 bits allocated;
- High Bit equal to Bits Stored minus one;
- laterality, view position, image laterality, breast compression, detector metadata, and imager pixel spacing;
- Presentation Intent Type differences between For Presentation and For Processing;
- windowing, VOI LUT, modality LUT, and overlays.

### 8.4 Visible light, pathology, and color

Required coverage:

- VL Photographic Image Storage.
- VL Endoscopic Image Storage.
- VL Microscopic Image Storage.
- VL Whole Slide Microscopy Image Storage.

Important compatibility features:

- RGB planar configuration 0 and 1;
- YBR_FULL and YBR_FULL_422;
- PALETTE COLOR with palette lookup tables;
- ICC profiles where required or useful;
- tiled full and tiled sparse whole slide images;
- optical path, slide label, specimen, pyramid, and thumbnail cases.
- derived segmentation with exact source-Frame and total-matrix tile closure.

### 8.5 Derived, annotation, and non-image objects

Required coverage:

- Segmentation Storage: BINARY, FRACTIONAL, and LABELMAP.
- Parametric Map Storage.
- Real World Value Mapping Storage.
- Grayscale, Color, and Advanced Blending Presentation State objects.
- Basic Text SR, Enhanced SR, Comprehensive SR, and Comprehensive 3D SR.
- Key Object Selection Document.
- Encapsulated PDF and CDA where useful.
- RT Dose, RT Structure Set, RT Plan, RT Image, and RT Radiation Set objects as detection and metadata-read cases.
- Waveform objects such as 12-lead ECG as non-image read cases.

Viewer expectations shall distinguish:

- `render_image`;
- `render_overlay_or_annotation`;
- `show_metadata`;
- `show_unsupported_but_recognized`;
- `reject_with_clear_unsupported_object_status`.

## 9. Pixel and Encoding Axes

### 9.1 Native pixel data

Generate representative combinations of:

- Bits Allocated: 1, 8, 16, and 32 where the IOD allows.
- Bits Stored: 1, 8, 10, 12, 14, 16, and 32 where the IOD allows.
- Pixel Representation: unsigned `0` and signed `1`.
- High Bit: always one less than Bits Stored for valid conformance cases.
- Samples per Pixel: 1 and 3.
- Planar Configuration: absent for one sample; 0 and 1 for color where permitted.
- Rows and columns: square, rectangular, odd-sized, large, and very small.
- Pixel Aspect Ratio without Pixel Spacing, and Pixel Spacing with non-square values.
- Pixel Data with even-length value-field padding.

Each recipe shall validate the pixel axis against the selected IOD and transfer syntax. Do not assume every axis is valid for every IOD.

Pixel-level invariants for valid native cases:

- `Bits Stored <= Bits Allocated`.
- `Bits Allocated` is 1 or a multiple of 8.
- `High Bit == Bits Stored - 1`.
- signed pixel values use two's-complement interpretation.
- unused high bits are deterministic but receivers must not be expected to rely on their value.
- the complete Pixel Data Value Field has even byte length.
- for native multi-frame Pixel Data, individual frames are concatenated and padding applies to the complete Value Field, not to each frame.

### 9.2 Photometric interpretation

Generate:

- MONOCHROME1.
- MONOCHROME2.
- PALETTE COLOR with Red, Green, and Blue palette descriptors and data.
- RGB.
- YBR_FULL.
- YBR_FULL_422.

Rules:

- MONOCHROME and PALETTE COLOR cases shall use Samples per Pixel = 1 unless the selected IOD says otherwise.
- RGB and YBR cases shall use Samples per Pixel = 3.
- Planar Configuration shall be present when Samples per Pixel > 1 and absent otherwise.
- RGB may use Planar Configuration 0 or 1 where permitted.
- YBR_FULL may use Planar Configuration 0 or 1 where permitted.
- YBR_FULL_422 shall use Planar Configuration 0.
- PALETTE COLOR shall include valid palette lookup table descriptors and data.

Native `YBR_FULL_422` is a special case. Samples per Pixel remains nominally 3, but chrominance is horizontally downsampled. The native Pixel Data byte length is not the generic `rows * columns * samples_per_pixel * bytes_per_sample` formula. Validators shall use the YBR_FULL_422-specific length formula:

```text
rows * columns * frames * 2 * bytes_per_sample
```

then apply even Value Field padding if required.

### 9.3 Transfer syntaxes

Transfer syntax generation shall be capability-gated. `list-cases` and `manifest.json` shall report generated, skipped, and unavailable transfer syntax cases.

Baseline native support:

| Transfer Syntax | UID | Profile | Requirement |
|---|---|---|---|
| Implicit VR Little Endian | `1.2.840.10008.1.2` | `smoke`, `core` | Required. |
| Explicit VR Little Endian | `1.2.840.10008.1.2.1` | `smoke`, `core` | Required. |
| Explicit VR Big Endian | `1.2.840.10008.1.2.2` | `legacy` | Required once writer support is verified. |
| Deflated Explicit VR Little Endian | `1.2.840.10008.1.2.1.99` | `extended` | Feature-gated. |

Compressed and encapsulated support shall be organized by implementation feasibility:

#### Tier A: expected writable with DICOM-rs support or simple native implementation after feature verification

- JPEG Baseline 8-bit, if `jpeg` feature support is enabled and verified.
- JPEG-LS Lossless and JPEG-LS Near-Lossless, if CharLS feature/build support is enabled and verified.
- JPEG XL Lossless and JPEG XL, if JPEG XL feature/build support is enabled and verified.
- Deflated Image Frame Compression / JPIP referenced deflate cases only when the selected transfer syntax and IOD are appropriate.

#### Tier B: possible with custom encoder or external codec adapter

- RLE Lossless.
- JPEG Extended 12-bit.
- JPEG Lossless and JPEG Lossless SV1.
- JPEG 2000 Lossless and Lossy.
- HTJ2K transfer syntaxes.

#### Tier C: later video or fixture-generation work

- MPEG-2.
- H.264 / MPEG-4 AVC.
- HEVC / H.265.
- SMPTE ST 2110-related transfer syntaxes.

Implementation notes:

- Do not assume a transfer syntax can be encoded merely because it can be registered or decoded.
- Maintain a transfer syntax capability matrix with `read_dataset`, `decode_pixel`, `write_dataset`, `encode_pixel`, `feature_flags`, `external_libraries`, and `determinism` fields.
- Reverify DICOM-rs and codec support during Phase 0.5 and before Phase 6.
- Distinguish Deflated Explicit VR Little Endian, which deflates the dataset, from Deflated Image Frame Compression, which compresses image frames in encapsulated form.

### 9.4 Encapsulated pixel data

Encapsulated pixel data cases should include:

- empty Basic Offset Table;
- populated Basic Offset Table;
- Extended Offset Table and Extended Offset Table Lengths;
- single fragment per frame;
- multiple fragments per frame;
- odd compressed frame lengths requiring item padding.

Valid combinations are constrained. The recipe validator shall enforce:

Valid:

- empty Basic Offset Table + one fragment per frame + Extended Offset Table + Extended Offset Table Lengths;
- populated Basic Offset Table + one or more fragments per frame + no Extended Offset Table;
- empty Basic Offset Table + multiple fragments per frame + no Extended Offset Table.

Invalid for conformance profiles:

- Extended Offset Table with populated Basic Offset Table;
- Extended Offset Table with multiple fragments per frame;
- Extended Offset Table present but empty;
- Extended Offset Table without Extended Offset Table Lengths;
- Extended Offset Table when Pixel Data is native/unencapsulated.

Odd compressed frame length handling:

- Item Value Fields shall be padded to even length as required by DICOM encoding.
- Extended Offset Table Lengths shall record the compressed frame lengths, not item padding bytes.

## 10. Geometry and Series Axes

Generate series and multi-frame cases for:

- axial stack with regular spacing;
- oblique stack;
- gantry tilt;
- missing or duplicated Instance Number while geometry remains valid;
- non-uniform slice spacing;
- multi-echo MR;
- dynamic contrast or temporal frames;
- PET/NM frames with energy windows, time slots, and detector dimensions;
- Frame of Reference shared across series;
- referenced source images for derived objects;
- multiple series in one study;
- sorting disagreement between Instance Number and spatial position.

Expected geometry metadata shall state how consumers should sort or interpret frames/instances:

```json
{
  "expected_geometry": {
    "sort_key": "image_position_patient_along_orientation_normal",
    "slice_spacing_mm": [1.0, 1.0, 1.5],
    "orientation": "oblique",
    "frame_of_reference_shared": true
  }
}
```

## 11. Metadata and Value Representation Axes

Generate cases covering:

- empty Type 2 attributes;
- optional Type 3 attributes present and absent;
- long text values;
- person names;
- date, time, datetime, and timezone offset;
- decimal strings, integer strings, and multi-valued attributes;
- Specific Character Set with UTF-8 and selected ISO 2022 cases;
- private creator blocks and private data elements;
- retired group length elements in `legacy` profile only;
- explicit and implicit VR parsing;
- definite and indefinite length sequences where the writer supports them;
- nested sequences used by SR, functional groups, derivation, and code sequences;
- empty sequences where allowed;
- unknown private tags that are validly scoped by private creator blocks.

Metadata cases shall avoid PHI-like content even when exercising person-name or text parsing.

## 12. Manifest Schema

Each generation run writes `manifest.json` at the output root. The manifest shall be machine-readable, JSON Schema validated, and stable enough to support downstream automation.

Recommended top-level shape:

```json
{
  "manifest_schema_version": "0.2.0",
  "generated_at": "20000101T000000Z",
  "generator": {
    "name": "dicom-test-suite",
    "version": "0.1.0",
    "git_sha": "optional",
    "rustc_version": "1.xx.x",
    "target_triple": "x86_64-unknown-linux-gnu",
    "cargo_lock_sha256": "...",
    "feature_flags": ["native"]
  },
  "standards": {
    "dicom_base_edition": "2026b",
    "include_final_text_after_base": false,
    "standards_lock_sha256": "...",
    "dicom_standard_kb": {
      "commit": "...",
      "db_edition": "2026b",
      "db_sha256": "..."
    }
  },
  "dependencies": {
    "dicom_rs_versions": {
      "dicom": "0.9.1",
      "dicom-object": "0.9.1",
      "dicom-transfer-syntax-registry": "0.9.1"
    },
    "codec_versions": {}
  },
  "run": {
    "profile": "smoke",
    "seed": 1,
    "include_stress": false
  },
  "files": [],
  "skipped_cases": []
}
```

Recommended file entry shape:

```json
{
  "case_id": "classic/sc/mono2_u8_explicit_le",
  "profile_membership": ["smoke", "core"],
  "path": "classic/sc/mono2_u8_explicit_le.dcm",
  "sha256": "...",
  "size_bytes": 12345,
  "determinism": "byte_stable",
  "recipe": {
    "recipe_id": "sc_mono2_u8",
    "recipe_version": "1.0.0",
    "recipe_parameters": {}
  },
  "dicom": {
    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7",
    "sop_class_name": "Secondary Capture Image Storage",
    "iod_name": "Secondary Capture Image",
    "modality": "OT",
    "transfer_syntax_uid": "1.2.840.10008.1.2.1",
    "transfer_syntax_name": "Explicit VR Little Endian"
  },
  "uids": {
    "study_instance_uid": "2.25...",
    "series_instance_uid": "2.25...",
    "sop_instance_uid": "2.25...",
    "frame_of_reference_uid": null,
    "implementation_class_uid": "2.25..."
  },
  "image": {
    "rows": 64,
    "columns": 64,
    "frames": 1,
    "samples_per_pixel": 1,
    "photometric_interpretation": "MONOCHROME2",
    "bits_allocated": 8,
    "bits_stored": 8,
    "high_bit": 7,
    "pixel_representation": 0,
    "planar_configuration": null
  },
  "pixel_data": {
    "vr": "OB",
    "native_or_encapsulated": "native",
    "value_length": 4096,
    "frame_count": 1,
    "frame_hashes": ["..."]
  },
  "expected_capabilities": ["read_part10", "decode_native_pixels", "render_grayscale"],
  "expected_semantics": {
    "object_handling": "render_image",
    "monochrome_polarity": "MONOCHROME2",
    "rescale": null,
    "voi": null,
    "overlays": []
  },
  "expected_visual_checks": {
    "decoded_pixel_hash": "...",
    "normalized_preview_hash": "...",
    "dimensions": [64, 64],
    "tolerance": "exact"
  },
  "validation": {
    "status": "passed",
    "internal": [],
    "standards": [],
    "external": []
  },
  "known_stressors": ["minimal_part10", "native_ob_pixels"],
  "standards_evidence": []
}
```

`expected_semantics` and `expected_visual_checks` are deliberately separate. Some cases only require recognition or metadata display; others require pixel-perfect or visually normalized rendering checks.

## 13. UID Strategy

Use deterministic UUID-derived DICOM UIDs in the `2.25.<decimal uuid>` form. Inputs shall include:

- project namespace UUID;
- standards lock hash;
- case ID;
- recipe version;
- role;
- seed;
- file index;
- frame or referenced-object index where applicable.

UID roles should include:

- `study_instance_uid`;
- `series_instance_uid`;
- `sop_instance_uid`;
- `frame_of_reference_uid`;
- `implementation_class_uid`;
- referenced source SOP Instance UIDs for derived objects.

Generated UIDs shall satisfy DICOM UID length constraints. The UID generator shall have unit tests for stability and maximum length.

## 14. Output Layout

Recommended generated layout:

```text
generated/<profile>/
  manifest.json
  coverage.json
  classic/
    sc/
    ct/
    mr/
    cr/
    mg/
    dx/
    us/
  enhanced/
    ct/
    mr/
    pet/
    us/
  derived/
    seg/
    presentation-state/
    sr/
    rwvm/
  vl/
    photo/
    endoscopic/
    microscopic/
    wsi/
  non-image/
    rt/
    waveform/
    encapsulated-document/
  reports/
```

The generator may also write per-case sidecars:

```text
<case_id>.expected.json
<case_id>.validation.json
```

Sidecars are generated artifacts, not committed fixtures.

## 15. Validation

Validation has four layers.

### 15.1 File-level validation

Required checks:

- file has 128-byte preamble and `DICM` prefix;
- file meta required fields are present;
- file meta is encoded as Explicit VR Little Endian;
- file meta Transfer Syntax UID is consistent with dataset encoding;
- file meta SOP Class/Instance UIDs match dataset SOP Class/Instance UIDs;
- Implementation Class UID is present and deterministic;
- no group `0002` elements appear in the dataset outside File Meta Information;
- generated file can be reopened by the selected DICOM-rs version.

### 15.2 Standards-derived recipe validation

Each recipe shall validate:

- required modules and Type 1/1C/2/2C attributes it is responsible for;
- conditional attributes intentionally satisfied or intentionally absent;
- enumerated values and defined terms used by the case;
- SOP Class UID and IOD compatibility;
- Transfer Syntax compatibility;
- references to source images, presentation states, segmentation frames, and SR evidence where applicable.

Recipe validation shall be based first on `dicom-standard-kb` where covered, and on official source artifacts otherwise.

### 15.3 Pixel and encapsulation validation

Required checks:

- `Bits Stored <= Bits Allocated`;
- `High Bit == Bits Stored - 1`;
- pixel byte length formula by photometric interpretation;
- native Pixel Data padding only at the complete Value Field level;
- signed values in expected two's-complement range;
- Planar Configuration presence/absence rules;
- palette LUT descriptor/data consistency;
- encapsulated item padding;
- Basic Offset Table and Extended Offset Table consistency;
- fragment structure by transfer syntax and recipe;
- decoded frame hashes where decoder support is available.

### 15.4 Optional external validation

If tools such as `dciodvfy`, `dcmdump`, `gdcmdump`, Orthanc, or other validators are installed, adapters may add results to the manifest and reports.

External validators are not mandatory by default. Their results shall include:

- tool name;
- tool version;
- command line;
- status;
- warnings/errors;
- whether the project configuration treats failures as fatal.

Generation shall fail if internal validation or standards-derived recipe validation fails. Optional external validator failure shall fail generation only when explicitly configured.

## 16. Architecture

The repository should be a Rust workspace with these crates or modules:

- `dicom-test-suite`: CLI binary.
- `suite-core`: shared case model, case registry, manifest schema, UID generation, deterministic seeding, output layout.
- `suite-standards`: `dicom-standard-kb` integration, standards lock handling, source-note lookup, evidence recording.
- `suite-iod`: IOD builders and module builders.
- `suite-pixel`: synthetic pixel generators, LUT generators, color conversion helpers, compression adapters.
- `suite-validate`: internal validation and optional external validator adapters.
- `suite-report`: coverage and compatibility report generation.

Initial single-crate development is acceptable if module boundaries match the future workspace layout.

Recommended internal traits:

```rust
trait CaseRecipe {
    fn id(&self) -> CaseId;
    fn profiles(&self) -> &'static [Profile];
    fn requirements(&self) -> CaseRequirements;
    fn standards_evidence(&self) -> StandardsEvidence;
    fn generate(&self, ctx: &GenerationContext) -> Result<Vec<GeneratedInstance>>;
}

trait IodBuilder {
    fn sop_class_uid(&self) -> &'static str;
    fn iod_name(&self) -> &'static str;
    fn required_modules(&self) -> ModuleSet;
    fn build(&self, attrs: AttributePlan) -> Result<InMemDicomObject>;
}

trait TransferSyntaxWriter {
    fn uid(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn capability(&self, features: &FeatureSet) -> TransferSyntaxCapability;
    fn encode_pixel_data(&self, frame_data: &[Frame]) -> Result<PixelDataEncoding>;
}

trait Validator {
    fn name(&self) -> &'static str;
    fn validate(&self, instance: &GeneratedInstance) -> ValidationResult;
}
```

The DICOM-rs family shall be pinned coherently in `Cargo.lock`. As of 2026-06-13, the current docs.rs versions for the umbrella `dicom` and `dicom-object` crates are `0.9.1`; recheck before the first real implementation phase and whenever updating dependencies.

## 17. CLI

Current command surface:

```sh
dicom-test-suite generate --profile smoke --out generated/smoke
dicom-test-suite generate --profile core --out generated/core --seed 1
dicom-test-suite generate --profile all --out generated/all --seed 1
dicom-test-suite generate --profile all --include-stress --out generated/all-plus-stress
dicom-test-suite generate --profile legacy --out generated/legacy --seed 1
dicom-test-suite generate --profile negative --out generated/negative --seed 1
dicom-test-suite generate --profile fuzz --out generated/fuzz --seed 1

dicom-test-suite list-cases
dicom-test-suite list-cases --profile extended
dicom-test-suite list-cases --status blocked

dicom-test-suite validate generated/core
dicom-test-suite report generated/core --format json
dicom-test-suite report generated/core --format markdown
dicom-test-suite report gaps --format markdown

dicom-test-suite conformance check-tools
dicom-test-suite conformance run generated/all --out reports/conformance/all
dicom-test-suite conformance verify reports/conformance/all

dicom-test-suite interoperate media-dicomdir GENERATED_ROOT --dcmmkdir PATH --dcmdump PATH --dciodvfy PATH --format json
dicom-test-suite interoperate protocol-baseline GENERATED_ROOT --format markdown
```

Standards-related commands:

```sh
dicom-test-suite standards check-lock
dicom-test-suite standards verify-kb --edition 2026b
dicom-test-suite standards gaps --profile core
```

An optional viewer-runner command remains unimplemented:

```sh
dicom-test-suite run-viewer generated/core --viewer 'dcmview {file}' --report reports/dcmview.json
```

Viewer runners are adapters. They must not be required for corpus generation or validation.

## 18. Reporting and Viewer Integration

Reports shall include:

- coverage by profile, IOD, SOP Class, transfer syntax, photometric interpretation, bit depth, geometry, and object type;
- generated/skipped/blocked case counts;
- validation status;
- determinism status;
- optional external validator output;
- optional viewer compatibility results.

Viewer compatibility result schema shall distinguish:

- file open success/failure;
- object recognition;
- unsupported-object handling;
- pixel decode;
- rendered dimensions;
- frame navigation;
- window/rescale behavior;
- MONOCHROME1 inversion;
- color interpretation;
- overlay/presentation-state behavior;
- crash/hang/timeouts;
- viewer-specific notes.

Viewer failures shall be reported against stable `case_id` values. Generator recipes must not be changed solely to accommodate a viewer bug.

## 19. Phased Implementation Plan

This is the original dependency-ordered implementation plan. Consult the
registry and dated phase status documents for completion state; task lists below
must not be read as current unimplemented capability.

### Phase 0: Repository initialization

Tasks:

- Add README and this system spec.
- Add `.gitignore` rules for generated DICOM outputs and standards caches.
- Choose initial Rust edition and workspace layout.
- Add `rust-toolchain.toml`.
- Pin the DICOM-rs crate family after verifying latest compatible releases.
- Add initial schemas directory.

Exit criteria:

- Repository has clear scope, output policy, and implementation plan.

### Phase 0.5: Standards and case registry foundation

Tasks:

- Add `standards.lock.json`.
- Add `schemas/manifest.schema.json`.
- Add `schemas/case-registry.schema.json`.
- Add normalized case ID taxonomy.
- Add initial `cases/registry.json` with planned smoke/core cases.
- Add explicit profile definitions and inclusion rules.
- Add transfer syntax capability matrix.
- Add deterministic build policy.
- Add `suite-standards` integration plan for `dicom-standard-kb`.
- Add standards gap/patch workflow documentation.

Exit criteria:

- `list-cases` can show all planned smoke/core cases with status `planned`, `implemented`, `skipped`, or `blocked`.
- Standards references for planned Phase 1 and Phase 2 cases are represented through `dicom-standard-kb` or source notes.

### Phase 1: Generator core

Tasks:

- Create Rust CLI and core case model.
- Implement deterministic UID generation.
- Implement deterministic date/time policy.
- Implement output directory creation and manifest writing.
- Implement minimal Part 10 writing through DICOM-rs.
- Implement file-level validation that reopens generated files.

Exit criteria:

`generate --profile smoke` writes:

- `classic/sc/mono2_u8_explicit_le`;
- `classic/sc/mono1_u8_explicit_le`;
- `classic/sc/rgb_planar0_explicit_le`;
- a valid manifest with hashes, file meta UIDs, pixel metadata, and validation results;
- byte-stable output across two identical runs.

### Phase 2: Native pixel matrix

Tasks:

- Implement monochrome pixel patterns for 8-bit and 16-bit unsigned and signed data.
- Implement MONOCHROME1 and MONOCHROME2 rendering expectation metadata.
- Implement RGB planar configuration 0 and 1.
- Implement PALETTE COLOR with palette LUT descriptors and data.
- Implement native YBR_FULL and YBR_FULL_422.
- Add cases for odd dimensions, rectangular images, very small images, and pixel padding.
- Add pixel byte-length and photometric validators.

Exit criteria:

- Smoke and core profiles cover key Image Pixel combinations with byte-length validation.
- YBR_FULL_422 uses the correct special native byte-length validator.

### Phase 3: Classic radiology IODs

Tasks:

- Add CT, MR, CR, US, Secondary Capture, DX, and Digital Mammography builders.
- Add CT rescale slope/intercept and windowing cases.
- Add mammography For Presentation and For Processing cases, including MONOCHROME1 12-bit data.
- Add overlays, display shutters, Modality LUT, and VOI LUT cases.
- Add multi-file series generation with stable Study/Series/Frame of Reference UIDs.

Exit criteria:

- Core profile includes classic single-frame cases known to challenge older viewers and CBIS-DDSM-like mammography behavior.
- Standards evidence exists for each implemented IOD/module builder.

### Phase 4: Enhanced multi-frame

Tasks:

- Add Enhanced CT and Enhanced MR builders.
- Add Shared and Per-Frame Functional Groups.
- Add Multi-frame Dimension metadata.
- Add frame-varying position, temporal position, echo, and phase cases.
- Add concatenation cases for extended profile.

Exit criteria:

- Extended profile contains valid multi-frame CT/MR cases and reports expected frame counts and geometry.

### Phase 5: Derived, presentation, and non-image objects

Tasks:

- Add Segmentation Storage for BINARY, FRACTIONAL, and LABELMAP.
- Add Grayscale Softcopy Presentation State and references to source images.
- Add Basic Text SR and Comprehensive SR.
- Add Key Object Selection.
- Add Real World Value Mapping.
- Add RT Dose and RT Structure Set detection cases.
- Add Encapsulated PDF detection case.

Exit criteria:

- Viewers can be tested for graceful handling of common non-image and derived SOP Classes.
- Derived objects resolve references to source objects generated in the same run.

### Phase 6: Transfer syntax expansion

Tasks:

- Implement transfer syntax abstraction.
- Reverify DICOM-rs transfer syntax write/encode capabilities.
- Add feature-gated compressed cases according to the capability matrix.
- Add JPEG Baseline 8-bit, JPEG-LS, and JPEG XL cases where supported.
- Add RLE, JPEG 12-bit, JPEG Lossless, JPEG 2000, and HTJ2K only when encoder support is available or an adapter is implemented.
- Add encapsulated pixel data offset table variants.
- Add legacy Explicit VR Big Endian profile.

Exit criteria:

- Reports identify which transfer syntax cases were generated, skipped, or unavailable due to build features.
- Encapsulated Pixel Data validators enforce Basic Offset Table and Extended Offset Table rules.

### Phase 7: Pathology, video, and large object profiles

Tasks:

- Add VL Photographic and selected VL endoscopic/microscopic cases.
- Add small VL Whole Slide Microscopy tiled examples.
- Add pyramid, thumbnail, label, and optical path cases.
- Add video transfer syntax cases where practical.
- Add large-file stress cases behind explicit profile selection.

Exit criteria:

- Extended and stress profiles exercise color, tiled, multi-frame, and large-object behavior without bloating git.

### Phase 8: Reporting and viewer integration

Tasks:

- Implement JSON and Markdown coverage reports.
- Add optional viewer runner adapters.
- Add per-viewer compatibility result schema.
- Add CI checks for generator determinism and manifest validity.
- Add regression workflow for linking viewer failures to case IDs.

Exit criteria:

- A viewer project can generate a corpus, run itself against cases, and record compatibility results without this repository depending on that viewer.

### Phase 9: Negative and fuzz profiles

Tasks:

- Add intentionally invalid or malformed files only under `negative` or `fuzz` profiles.
- Separate conformance failures from viewer compatibility failures.
- Include truncated data, mismatched metadata, invalid VR, missing required attributes, bad sequence lengths, and invalid transfer syntax metadata.

Exit criteria:

- Consumers can opt into robustness testing without mixing invalid cases into standard conformance suites.

## 20. Agent Development Guidance

Coding agents working in this repository shall:

- Query `dicom-standard-kb` before adding or modifying an IOD builder, module builder, SOP Class mapping, UID assumption, enumerated value, or defined-term assumption.
- Use official DICOM source artifacts when `dicom-standard-kb` does not cover the needed part or section.
- Patch `dicom-standard-kb` or add a local standards source note for uncovered portions.
- Prefer small deterministic cases over large fixtures.
- Add a manifest expectation for every generated case.
- Add validation before adding broad case counts.
- Keep generated DICOM files, standards caches, and generated KB databases out of git.
- Avoid hard-coding behavior around `dcmview`; add viewer-specific observations only to generated reports or optional compatibility data.
- Treat optional external codecs and validators as feature-gated capabilities with clear skipped-case reporting.
- Keep `cases/registry.json` authoritative for planned, implemented, skipped, and blocked cases.
- Update `standards.lock.json` deliberately, with a review note, whenever the standards baseline changes.

## 21. Initial Priority Cases

The first high-value cases for `dcmview` development and general viewer compatibility are:

| Case ID | Profile | Purpose |
|---|---|---|
| `classic/sc/mono2_u8_explicit_le` | `smoke` | Simplest known-good grayscale image. |
| `classic/sc/mono1_u8_explicit_le` | `smoke` | MONOCHROME1 inversion sanity case. |
| `classic/sc/rgb_planar0_explicit_le` | `smoke` | Basic color support. |
| `classic/ct/mono2_i16_rescale_12bit_explicit_le` | `core` | Signed CT-like pixels with rescale. |
| `classic/mg/for_presentation_mono1_u16_12bit_explicit_le` | `core` | CBIS-DDSM-like mammography stressor. |
| `classic/mg/for_processing_mono2_u16_12bit_implicit_le` | `core` | Processing-intent mammography variant. |
| `classic/cr/overlay_modality_voi_explicit_le` | `core` | Overlay plus LUT handling. |
| `classic/mr/multislice_oblique_explicit_le` | `core` | Geometry sorting. |
| `enhanced/ct/multiframe_shared_perframe_explicit_le` | `extended` | Functional group parsing. |
| `derived/seg/binary_multiframe_explicit_le` | `extended` | Derived object recognition and references. |
| `vl/photo/rgb_planar0_explicit_le` | `core` | VL color handling. |
| `vl/photo/palette_color_explicit_le` | `core` | Palette LUT support. |

## 22. Pre-Implementation Acceptance Criteria

Before broad implementation, the repository should have:

- `standards.lock.json` committed and reviewed;
- explicit policy for base edition versus post-base final-text supplements/correction items;
- `dicom-standard-kb` integration instructions;
- standards gap/patch workflow;
- normalized case ID taxonomy;
- profile inclusion rules and size budgets;
- case registry schema and initial registry;
- manifest schema;
- transfer syntax capability matrix;
- deterministic build policy;
- validation architecture;
- `.gitignore` and CI checks preventing generated DICOM payloads from being committed.

These contracts should be in place before expanding IOD coverage. The project should prefer a small, well-validated, deterministic corpus over a large under-specified corpus.
