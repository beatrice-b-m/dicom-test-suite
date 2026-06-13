# System Specification

## Purpose

`dicom-test-suite` generates a broad, deterministic, synthetic DICOM corpus for validating DICOM viewer compatibility. The initial downstream use case is `dcmview`, but this repository must remain viewer-agnostic. A failing viewer is useful signal; generated cases should reflect the DICOM standard and real-world interoperability risk rather than the current capabilities of any one implementation.

Generated DICOM payloads are build artifacts and must not be committed. The repository commits code, case recipes, metadata schemas, expected results, and reports.

## Standards Baseline

The first specification pass was based on DICOM Standard edition 2026b through the `dicom-kb` MCP tool. Representative queried anchors include:

- PS3.3 CT Image IOD Modules, table A.3-1.
- PS3.3 Enhanced CT Image IOD Modules, table A.38-1.
- PS3.3 MR Image and Enhanced MR Image IOD Modules, tables A.4-1 and A.36-1.
- PS3.3 Computed Radiography Image IOD Modules, table A.2-1.
- PS3.3 Digital Mammography X-Ray Image IOD Modules, table A.27-1.
- PS3.3 Image Pixel Module and Image Pixel Description Macro, tables C.7-11a and C.7-11c.
- PS3.3 Multi-frame Functional Groups Module, table C.7.6.16-1.
- PS3.3 Segmentation IOD and Segmentation Image Module, tables A.51-1 and C.8.20-2.
- PS3.3 VL Whole Slide Microscopy Image IOD Modules, table A.32.8-1.
- PS3.3 Overlay Plane, Modality LUT, VOI LUT, Palette Color Lookup Table, and SOP Common modules.
- PS3.4 Standard SOP Class mappings for CT, Enhanced CT, CR, Digital Mammography, MR, Enhanced MR, US, Secondary Capture, Segmentation, and VL Whole Slide Microscopy.
- PS3.6 UID registry entries for Transfer Syntax UID, Implicit VR Little Endian, Explicit VR Little Endian, JPEG Baseline 8-bit, and RLE Lossless.

As of 2026-06-13, docs.rs lists the latest `dicom` and `dicom-object` crate versions as `0.9.1`. The implementation should pin the DICOM-rs family coherently and revisit this before the first real code phase.

## Scope

The suite should generate valid synthetic DICOM Part 10 files across standard IOD families and compatibility axes. It should also support clearly labeled legacy and stress profiles for files that are valid but uncommon, retired, or historically problematic.

The suite should not initially generate intentionally invalid DICOM files. Negative/fuzz cases belong in a later phase and must be separated from conformance-valid cases.

## Core Requirements

- Deterministic generation: the same recipe version, seed, profile, and crate versions produce byte-stable files where feasible.
- Synthetic data marking: set `Synthetic Data (0008,001C)` to `YES` where appropriate and avoid PHI.
- DICOM Part 10 output by default: include file meta information and Transfer Syntax UID.
- Manifest per run: emit machine-readable metadata describing every generated case, expected interpretation, and validation status.
- Profiles: support `smoke`, `core`, `extended`, `legacy`, `stress`, and `all`.
- Case IDs: stable, human-readable IDs such as `ct/classic/mono2_i16_rescale_explicit_le`.
- No generated DICOMs in git: generated outputs must remain under ignored directories or ignored file extensions.
- Viewer independence: optional viewer runners may consume the corpus but must not shape generation logic.

## Test Coverage Model

Coverage should be built from two layers:

1. IOD families: representative SOP Classes and module combinations that viewers encounter.
2. Orthogonal axes: pixel layout, photometric interpretation, transfer syntax, geometry, sequences, overlays, annotations, and metadata edge cases.

Each generated file should be small enough for local iteration unless a case specifically tests large images, tiled images, or multi-frame behavior.

## IOD and SOP Class Families

### Classic Single-frame Image

Required early coverage:

- Computed Radiography Image Storage.
- CT Image Storage.
- MR Image Storage.
- Ultrasound Image Storage.
- Secondary Capture Image Storage.
- Digital X-Ray and Digital Mammography X-Ray Image Storage, both For Presentation and For Processing.
- X-Ray Angiographic and X-Ray Radiofluoroscopic image families.
- Nuclear Medicine Image Storage.
- Positron Emission Tomography Image Storage.

Important compatibility features:

- Single file images and multi-file series.
- Instance sorting by `Instance Number`, `Image Position Patient`, and `Image Orientation Patient`.
- Signed and unsigned monochrome pixels.
- Modality LUT or rescale slope/intercept.
- VOI LUT and window center/width.
- MONOCHROME1 inversion, especially mammography.
- Pixel padding value and pixel padding range.
- Optional overlays and display shutters.

### Enhanced Multi-frame Image

Required coverage:

- Enhanced CT Image Storage.
- Enhanced MR Image Storage.
- Enhanced PET Image Storage.
- Enhanced US Volume Storage.
- Enhanced XA/XRF where codec and generator support allow.

Important compatibility features:

- `Number of Frames`.
- Shared Functional Groups Sequence.
- Per-Frame Functional Groups Sequence.
- Multi-frame Dimension Module.
- Frame-specific position, orientation, pixel spacing, temporal position, echo, phase, and cardiac or respiratory timing.
- Concatenation metadata for large logical multi-frame objects.
- Empty versus populated per-frame groups where allowed.

### Mammography and Projection X-Ray

Required coverage:

- Digital Mammography X-Ray Image Storage - For Presentation.
- Digital Mammography X-Ray Image Storage - For Processing.
- CR-like older mammography-compatible images, including CBIS-DDSM-style 12-bit monochrome patterns.

Important compatibility features:

- MONOCHROME1 and MONOCHROME2.
- 12 bits stored in 16 bits allocated.
- High Bit equals Bits Stored minus one.
- Laterality, view position, image laterality, breast compression, detector metadata, and imager pixel spacing.
- Presentation Intent Type differences between For Presentation and For Processing.
- Windowing, VOI LUT, modality LUT, and overlays.

### Visible Light, Pathology, and Color

Required coverage:

- VL Photographic Image Storage.
- VL Endoscopic and Microscopic Image Storage.
- VL Whole Slide Microscopy Image Storage.

Important compatibility features:

- RGB planar configuration 0 and 1.
- YBR_FULL and YBR_FULL_422.
- PALETTE COLOR with palette lookup tables.
- ICC profiles where required or useful.
- Tiled full and tiled sparse whole slide images.
- Optical path, slide label, specimen, pyramid, and thumbnail cases.

### Derived, Annotation, and Non-Image Objects

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

Viewer expectations should distinguish "render image", "render overlay/annotation", "show metadata", "show unsupported but recognized object", and "reject with clear unsupported-object status".

## Pixel and Encoding Axes

### Native Pixel Data

Generate representative combinations of:

- Bits Allocated: 1, 8, 16, and 32 where the IOD allows.
- Bits Stored: 1, 8, 10, 12, 14, 16, and 32 where the IOD allows.
- Pixel Representation: unsigned `0` and signed `1`.
- High Bit: always one less than Bits Stored for valid conformance cases.
- Samples per Pixel: 1 and 3.
- Planar Configuration: absent for one sample, 0 and 1 for color.
- Rows and columns: square, rectangular, odd-sized, large, and very small.
- Pixel Aspect Ratio without Pixel Spacing, and Pixel Spacing with non-square values.
- Pixel Data with even-length padding.

### Photometric Interpretation

Generate:

- MONOCHROME1.
- MONOCHROME2.
- PALETTE COLOR with Red, Green, and Blue palette descriptors and data.
- RGB.
- YBR_FULL.
- YBR_FULL_422 with even dimensions and Planar Configuration 0.

The standard constrains MONOCHROME and PALETTE COLOR to Samples per Pixel 1, RGB/YBR to Samples per Pixel 3, and requires palette lookup tables for PALETTE COLOR.

### Transfer Syntaxes

Baseline support:

- Implicit VR Little Endian: `1.2.840.10008.1.2`.
- Explicit VR Little Endian: `1.2.840.10008.1.2.1`.
- Explicit VR Big Endian as a legacy profile case.
- Deflated Explicit VR Little Endian if DICOM-rs supports writing it cleanly.

Compressed support, gated by available encoders:

- RLE Lossless.
- JPEG Baseline 8-bit.
- JPEG Extended 12-bit.
- JPEG Lossless and JPEG Lossless SV1.
- JPEG-LS lossless and near-lossless.
- JPEG 2000 lossless and lossy.
- HTJ2K transfer syntaxes where practical.
- MPEG-2, H.264, and HEVC for video IODs where practical.

Encapsulated pixel data cases should include:

- Empty Basic Offset Table.
- Populated Basic Offset Table.
- Extended Offset Table and Extended Offset Table Lengths.
- Single fragment per frame.
- Multiple fragments per frame.
- Odd compressed frame lengths requiring item padding.

## Geometry and Series Axes

Generate series and multi-frame cases for:

- Axial stack with regular spacing.
- Oblique stack.
- Gantry tilt.
- Missing or duplicated Instance Number while geometry remains valid.
- Non-uniform slice spacing.
- Multi-echo MR.
- Dynamic contrast or temporal frames.
- PET/NM frames with energy windows, time slots, and detector dimensions.
- Frame of Reference shared across series.
- Referenced source images for derived objects.

## Metadata and Value Representation Axes

Generate cases covering:

- Empty Type 2 attributes.
- Optional Type 3 attributes present and absent.
- Long text values, person names, date/time/timezone, decimal strings, integer strings, and multi-valued attributes.
- Specific Character Set with UTF-8 and selected ISO 2022 cases.
- Private creator blocks and private data elements.
- Retired group length elements in legacy profile.
- Explicit and implicit VR parsing.
- Definite and indefinite length sequences where DICOM-rs can write them.
- Nested sequences used by SR, functional groups, derivation, and code sequences.

## Architecture

The repository should be a Rust workspace with these crates or modules:

- `dicom-test-suite`: CLI binary.
- `suite-core`: shared case model, manifest schema, UID generation, deterministic seeding, output layout.
- `suite-iod`: IOD builders and module builders.
- `suite-pixel`: synthetic pixel generators, LUT generators, color conversion helpers, compression adapters.
- `suite-validate`: internal validation and optional external validator adapters.
- `suite-report`: matrix and compatibility report generation.

Initial single-crate development is acceptable if module boundaries match the future workspace layout.

## CLI

Expected commands:

```sh
dicom-test-suite generate --profile smoke --out generated/smoke
dicom-test-suite generate --profile core --out generated/core --seed 1
dicom-test-suite list-cases --profile extended
dicom-test-suite validate generated/core
dicom-test-suite report generated/core --format json
dicom-test-suite report generated/core --format markdown
```

Optional later command:

```sh
dicom-test-suite run-viewer generated/core --viewer 'dcmview {file}' --report reports/dcmview.json
```

Viewer runners should be adapters. They must not be required for corpus generation or validation.

## Manifest Schema

Each generation run writes `manifest.json` at the output root. Each file entry should include:

- `case_id`
- `profile`
- `path`
- `recipe_version`
- `dicom_standard_edition`
- `sop_class_uid`
- `sop_class_name`
- `iod_name`
- `transfer_syntax_uid`
- `transfer_syntax_name`
- `modality`
- `rows`
- `columns`
- `frames`
- `samples_per_pixel`
- `photometric_interpretation`
- `bits_allocated`
- `bits_stored`
- `high_bit`
- `pixel_representation`
- `planar_configuration`
- `expected_capabilities`
- `expected_rendering`
- `validation`
- `known_stressors`

`expected_rendering` should be precise enough for automated viewer checks later: grayscale inversion, rescale behavior, windowing, color model, number of frames, expected dimensions, overlays, and unsupported-object status.

## UID Strategy

Use deterministic UUID-derived DICOM UIDs in the `2.25.<decimal uuid>` form. Inputs should include case ID, recipe version, role, seed, and file index. This avoids requiring a project-owned OID root while preserving reproducibility.

## Output Layout

Recommended layout:

```text
generated/<profile>/
  manifest.json
  classic/
    ct/
    mr/
    cr/
    mammography/
  enhanced/
    ct/
    mr/
    pet/
  derived/
    segmentation/
    presentation-state/
    sr/
  pathology/
    vl/
    wsi/
  reports/
```

The generator may also write per-case sidecars:

```text
<case_id>.expected.json
<case_id>.validation.json
```

Sidecars are generated artifacts, not committed fixtures.

## Validation

Validation has three layers:

1. Internal structural validation: DICOM-rs can re-open the file; required file meta and key attributes are present; Pixel Data byte length or encapsulated structure matches the manifest.
2. Standards-derived validation: each recipe asserts required module attributes and conditional attributes it intentionally exercises.
3. Optional external validation: if tools such as `dciodvfy`, `dcmdump`, `gdcmdump`, or Orthanc are installed, adapters can add results to the report without making them mandatory.

The first implementation should fail generation if internal validation fails.

## Phased Implementation Plan

### Phase 0: Repository Initialization

- Add README and system spec.
- Add `.gitignore` rules for generated DICOM outputs.
- Choose initial Rust edition and workspace layout.
- Pin DICOM-rs crate family after verifying the latest compatible release.

Exit criteria:

- Repository has clear scope, output policy, and implementation plan.

### Phase 1: Generator Core

- Create the Rust CLI and core case model.
- Implement deterministic UID generation.
- Implement output directory creation and manifest writing.
- Implement minimal Part 10 writing through DICOM-rs.
- Add internal validation that re-opens generated files.

Exit criteria:

- `generate --profile smoke` writes at least one valid Secondary Capture image and a manifest.

### Phase 2: Native Pixel Matrix

- Implement monochrome pixel patterns for 8-bit and 16-bit unsigned and signed data.
- Implement MONOCHROME1 and MONOCHROME2 rendering expectation metadata.
- Implement RGB planar configuration 0 and 1.
- Implement PALETTE COLOR with palette LUT descriptors and data.
- Add cases for odd dimensions, rectangular images, and pixel padding.

Exit criteria:

- Smoke and core profiles cover key Image Pixel combinations with byte length validation.

### Phase 3: Classic Radiology IODs

- Add CT, MR, CR, US, Secondary Capture, DX, and Digital Mammography builders.
- Add CT rescale slope/intercept and windowing cases.
- Add mammography For Presentation and For Processing cases, including MONOCHROME1 12-bit data.
- Add overlays, display shutters, Modality LUT, and VOI LUT cases.
- Add multi-file series generation with stable Study/Series/Frame of Reference UIDs.

Exit criteria:

- Core profile includes classic single-frame cases known to challenge older viewers and CBIS-DDSM-like mammography behavior.

### Phase 4: Enhanced Multi-frame

- Add Enhanced CT and Enhanced MR builders.
- Add Shared and Per-Frame Functional Groups.
- Add Multi-frame Dimension metadata.
- Add frame-varying position, temporal position, echo, and phase cases.
- Add concatenation cases for extended profile.

Exit criteria:

- Extended profile contains valid multi-frame CT/MR cases and reports expected frame counts and geometry.

### Phase 5: Derived, Presentation, and Non-Image Objects

- Add Segmentation Storage for BINARY, FRACTIONAL, and LABELMAP.
- Add Grayscale Softcopy Presentation State and references to source images.
- Add Basic Text SR and Comprehensive SR.
- Add Key Object Selection.
- Add RT Dose and RT Structure Set detection cases.
- Add Encapsulated PDF detection case.

Exit criteria:

- Viewers can be tested for graceful handling of common non-image and derived SOP Classes.

### Phase 6: Transfer Syntax Expansion

- Add transfer syntax abstraction.
- Add RLE Lossless if encoder support is available.
- Add JPEG Baseline 8-bit and JPEG 12-bit paths.
- Add JPEG-LS, JPEG 2000, and HTJ2K as optional feature-gated codecs.
- Add encapsulated pixel data offset table variants.
- Add legacy Explicit VR Big Endian profile.

Exit criteria:

- Reports identify which transfer syntax cases were generated, skipped, or unavailable due to build features.

### Phase 7: Pathology, Video, and Large Object Profiles

- Add VL Photographic and selected VL endoscopic/microscopic cases.
- Add small VL Whole Slide Microscopy tiled examples.
- Add pyramid, thumbnail, label, and optical path cases.
- Add video transfer syntax cases where practical.
- Add large-file stress cases behind explicit profile selection.

Exit criteria:

- Extended and stress profiles exercise color, tiled, multi-frame, and large-object behavior without bloating git.

### Phase 8: Reporting and Viewer Integration

- Implement JSON and Markdown coverage reports.
- Add optional viewer runner adapters.
- Add per-viewer compatibility result schema.
- Add CI checks for generator determinism and manifest validity.
- Add regression workflow for linking viewer failures to case IDs.

Exit criteria:

- A viewer project can generate a corpus, run itself against cases, and record compatibility results without this repository depending on that viewer.

### Phase 9: Negative and Fuzz Profiles

- Add explicitly invalid or malformed files only under `negative` or `fuzz` profiles.
- Separate conformance failures from viewer compatibility failures.
- Include truncated data, mismatched metadata, invalid VR, missing required attributes, bad sequence lengths, and invalid transfer syntax metadata.

Exit criteria:

- Consumers can opt into robustness testing without mixing invalid cases into standard conformance suites.

## Agent Development Guidance

Coding agents working in this repository should:

- Query `dicom-kb` before adding or modifying an IOD builder, module builder, or enumerated-value assumption.
- Prefer small deterministic cases over large fixtures.
- Add a manifest expectation for every generated case.
- Add validation before adding broad case counts.
- Keep generated DICOM files out of git.
- Avoid hard-coding behavior around `dcmview`; add viewer-specific observations only to generated reports or optional compatibility data.
- Treat optional external codecs and validators as feature-gated capabilities with clear skipped-case reporting.

## Initial Priority Cases

The first high-value cases for `dcmview` development should be:

- `sc/mono2_u8_explicit_le`: simplest known-good image.
- `ct/classic_mono2_i16_rescale_12bit_explicit_le`: signed CT-like pixels with rescale.
- `mg/for_presentation_mono1_u16_12bit_explicit_le`: CBIS-DDSM-like mammography stressor.
- `mg/for_processing_mono2_u16_12bit_implicit_le`: processing-intent mammography variant.
- `cr/overlay_modality_voi_explicit_le`: overlay plus LUT handling.
- `mr/classic_multislice_oblique_explicit_le`: geometry sorting.
- `enhanced_ct/multiframe_shared_perframe_explicit_le`: functional group parsing.
- `seg/binary_multiframe_explicit_le`: derived object recognition.
- `vl/rgb_planar0_explicit_le`: basic color support.
- `vl/palette_color_explicit_le`: palette LUT support.
