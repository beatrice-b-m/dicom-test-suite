# Implementation Plan

**Last updated:** 2026-06-14  
**Source specification:** `SYSTEM_SPEC.md` version 0.2.0  
**Scope:** Phase 6, transfer syntax expansion
**Status:** Phase 5 complete; Phase 6 foundation started

This document translates the next implementation phases from `SYSTEM_SPEC.md`
into concrete, reviewable implementation increments. `SYSTEM_SPEC.md` remains
the requirements source of truth. `IMPLEMENTATION_PROGRESS.md` remains the
durable status ledger.

## Current Codebase Findings

- Generation is currently a single-package Rust implementation centered on
  `src/generator.rs`, with recipe arrays wired through `write_supported_cases`.
  Phase 1-5 recipes now include image, derived image, presentation, SR, RT, and
  encapsulated-document objects.
- `schemas/manifest.schema.json` supports nullable or absent image and
  `pixel_data` entries for non-image objects plus generated-file `references`
  arrays for same-run derived objects.
- `validate <generated-root>` performs generic Part 10 checks, object-family
  checks, optional image/pixel checks, and same-run reference resolution.
- Coverage reporting populates `derived_refs` from manifest references and
  preserves null photometric/bits/frames fields for non-image rows.
- Generation maintains an ordered source-object registry so derived recipes can
  reference only objects already generated in the same run.
- `cases/registry.json` marks all planned Phase 5 target rows implemented. The
  remaining planned rows are Phase 7 VL cases.

## Phase 6 Codebase Findings

- `dicom-transfer-syntax-registry` 0.9.1 is pinned with default features
  disabled in this crate. The current dependency graph includes the base
  DICOM-rs data-set encoders, but no optional compressed pixel codec crates.
- DICOM-rs 0.9.1 exposes built-in data-set read/write support for Implicit VR
  Little Endian, Explicit VR Little Endian, and retired Explicit VR Big
  Endian. The project capability matrix now records those three native
  syntaxes as `available`.
- Deflated Explicit VR Little Endian is present as a DICOM-rs transfer syntax,
  but this crate does not enable the `deflate` feature, so it remains
  feature-gated until a deliberate dependency-feature slice verifies dataset
  deflate generation, validation, and reproducibility.
- JPEG Baseline, JPEG-LS, JPEG XL, RLE, JPEG 2000, and HTJ2K transfer syntaxes
  are known to the DICOM-rs registry, but their write support depends on
  optional features and/or external codec adapters. Do not add generated cases
  for them until the capability matrix entry, feature flag, encoder behavior,
  encapsulated Pixel Data layout, and validation strategy are verified.

## Phase 6.0: Native Transfer Syntax Foundation

Goal: establish transfer-syntax capability truth in project artifacts and add
the first native legacy transfer syntax without disturbing Phase 1-5 profiles.

Tasks:

- Keep `transfer-syntax/capability-matrix.json` aligned with the pinned
  DICOM-rs registry for native uncompressed transfer syntaxes.
- Add tests that compare matrix availability for Implicit VR Little Endian,
  Explicit VR Little Endian, and Explicit VR Big Endian against DICOM-rs
  data-set encoder availability.
- Add a small transfer-syntax abstraction in generation so recipes select a
  named transfer syntax capability instead of passing raw UID/name pairs at
  every Part 10 write site.
- Add a `legacy` Explicit VR Big Endian Secondary Capture case after the writer
  abstraction is in place. Keep it out of `smoke`, `core`, `extended`, and
  `all` unless profile inclusion rules are explicitly updated.
- Extend generated-root validation messages for Explicit VR Big Endian and add
  a readback/mutation test proving File Meta remains Explicit VR Little Endian
  while the dataset uses Explicit VR Big Endian.

Exit criteria:

- `list-cases --profile legacy` shows the Big Endian case as implemented.
- `generate --profile legacy`, `validate`, JSON report, Markdown report, and
  two-run reproducibility pass for the Big Endian case.
- Existing `smoke`, `core`, `extended`, and `all` behavior remains unchanged.

## Phase 6.1: Deflated Dataset Feature Gate

Goal: introduce Deflated Explicit VR Little Endian only behind an explicit
project feature and only after DICOM-rs deflate support is verified locally.

Tasks:

- Add a `deflate` Cargo feature that enables the matching
  `dicom-transfer-syntax-registry` feature.
- Update the capability matrix and registry rows so Deflated Explicit VR Little
  Endian cases are generated only when the feature is enabled and are reported
  as unavailable otherwise.
- Add a tiny extended-profile deflated dataset case if DICOM-rs writes and
  reads it deterministically with the selected feature set.
- Add validation that distinguishes dataset deflate from encapsulated deflated
  image frame compression.

Exit criteria:

- Without the feature, deflated cases are reported unavailable with a clear
  feature-gated reason.
- With the feature, generation, validation, report, and reproducibility checks
  pass for the deflated dataset case.

## Phase 6.2: Encapsulated Pixel Data Foundation

Goal: prepare compressed image-frame recipes without adding unsupported codecs.

Tasks:

- Add manifest fields for encapsulated Pixel Data layout where needed: Basic
  Offset Table presence/population, fragment count per frame, Extended Offset
  Table presence, Extended Offset Table Lengths, and compressed frame hashes.
- Add validators for the valid and invalid offset-table combinations listed in
  `SYSTEM_SPEC.md` section 9.4.
- Add planned or feature-gated registry rows for JPEG Baseline 8-bit, JPEG-LS,
  JPEG XL, RLE, JPEG 2000, and HTJ2K only after their matrix entries document
  concrete local encoder support or an unavailable status.

Exit criteria:

- Encapsulated Pixel Data validators can be tested with synthetic manifest or
  mutation fixtures before any compressed image recipe is flipped to
  `implemented`.
- Reports identify compressed cases as generated, feature-gated unavailable, or
  blocked according to matrix-backed capability state.

## Standards Baseline Checked

The following Phase 5 SOP Classes and IODs were checked against the configured
2026b `dicom-standard-kb` MCP before writing this plan:

| Workstream | SOP Class | UID | IOD |
|---|---|---|---|
| SEG BINARY/FRACTIONAL | Segmentation Storage | `1.2.840.10008.5.1.4.1.1.66.4` | Segmentation |
| SEG LABELMAP | Label Map Segmentation Storage | `1.2.840.10008.5.1.4.1.1.66.7` | Segmentation |
| Presentation state | Grayscale Softcopy Presentation State Storage | `1.2.840.10008.5.1.4.1.1.11.1` | Grayscale Softcopy Presentation State |
| SR | Basic Text SR Storage | `1.2.840.10008.5.1.4.1.1.88.11` | Basic Text SR |
| SR | Comprehensive SR Storage | `1.2.840.10008.5.1.4.1.1.88.33` | Comprehensive SR |
| KOS | Key Object Selection Document Storage | `1.2.840.10008.5.1.4.1.1.88.59` | Key Object Selection Document |
| RWVM | Real World Value Mapping Storage | `1.2.840.10008.5.1.4.1.1.67` | Real World Value Mapping |
| RT | RT Dose Storage | `1.2.840.10008.5.1.4.1.1.481.2` | RT Dose |
| RT | RT Structure Set Storage | `1.2.840.10008.5.1.4.1.1.481.3` | RT Structure Set |
| Encapsulated document | Encapsulated PDF Storage | `1.2.840.10008.5.1.4.1.1.104.1` | Encapsulated PDF |

Implementation notes from the standards review:

- BINARY and FRACTIONAL segmentation use Segmentation Storage. LABELMAP uses
  Label Map Segmentation Storage and must be a separate registry/SOP Class
  path, not just another descriptor on the existing binary SEG row.
- Segmentation, RWVM, RT Dose, and RT Structure Set use Common Instance
  Reference or reference-related modules in relevant contexts. Reference
  validation is therefore Phase 5 foundation work, not a late hardening task.
- GSPS uses Presentation State Relationship, Displayed Area, and Softcopy
  Presentation LUT modules. The first GSPS case should be intentionally small
  and avoid optional mask/overlay/rotation behavior until the required
  relationship path is stable.
- Basic Text SR, Comprehensive SR, and KOS share SR Document Content machinery.
  Implementing a small SR content tree builder once is preferable to writing
  one-off sequence construction in each recipe.
- Parsed SEG value lookup has a known limitation: direct enumerated-value lookup
  for Segmentation Type surfaced fractional subtype terms. Before implementing
  each SEG variant, recheck PS3.3 text anchors for `BINARY`, `FRACTIONAL`, and
  `LABELMAP`, and add a local source note if the needed value evidence cannot
  be represented cleanly in `cases/registry.json`.

## Phase 5.0: Object And Reference Foundation

Goal: make the core data model able to describe non-image Part 10 objects and
same-run references without weakening existing image validation.

Tasks:

- Add planned registry rows for the full Phase 5 target queue with 2026b
  standards evidence and `phase-5` recheck metadata.
- Update the manifest schema so `image` and `pixel_data` can be explicitly
  absent or null for non-image objects while preserving existing image entries.
- Add a `references` array to each generated file manifest entry. Each reference
  should record relationship name, source case ID, source path, SOP Class UID,
  SOP Instance UID, Series Instance UID when applicable, and frame numbers when
  applicable.
- Refactor generation to maintain a same-run source object map keyed by
  `case_id` and expose only already-generated source instances to later derived
  recipes.
- Split validation into generic Part 10 checks, optional image/pixel checks,
  object-family standards checks, and reference-resolution checks.
- Update coverage reports to populate `derived_refs` from manifest references
  and to handle non-image rows without image metadata.
- Add tests proving existing smoke/core/extended generation, validation,
  reproducibility, and schema checks still pass with the new manifest shape.

Exit criteria:

- Existing Phase 1-4 cases continue to pass `generate`, `validate`, report, and
  two-run reproducibility tests.
- A synthetic test manifest with a non-image file shape passes schema-focused
  tests without forcing image or Pixel Data fields.
- A generated manifest reference can be validated against another generated file
  in the same run.

## Phase 5.1: Binary Segmentation

Target case:

| Case ID | Profile | Source object | SOP Class |
|---|---|---|---|
| `derived/seg/binary_multiframe_explicit_le` | `extended` | `enhanced/ct/multiframe_shared_perframe_explicit_le` | Segmentation Storage |

Tasks:

- Implement a `SegmentationRecipe` and shared SEG writer for tiny multi-frame
  Explicit VR Little Endian output.
- Use source rows, columns, frame count, Study UID, Frame of Reference UID, and
  source SOP references from the generated Enhanced CT case in the same run.
- Encode a minimal BINARY segmentation with deterministic bit-packed Pixel
  Data, Segment Sequence, Segmentation Image Module fields, Multi-frame
  Functional Groups, Multi-frame Dimension metadata, and Common Instance
  Reference or Derivation Image references as required by the selected design.
- Add SEG-specific validation for Segmentation Type, Segment Sequence,
  frame/segment counts, bit-packed Pixel Data length, Dimension Index metadata,
  and source reference resolution.
- Change the registry row from `planned` to `implemented` in the same commit as
  the working recipe.

Exit criteria:

- `generate --profile extended` writes the binary SEG object and no longer
  reports that case as planned.
- `validate <extended-root>` confirms SEG object semantics and source
  references.
- Reproducibility tests remain byte-stable for `extended` and `all`.

## Phase 5.2: Fractional And Labelmap Segmentation

Target cases:

| Case ID | Profile | Source object | SOP Class |
|---|---|---|---|
| `derived/seg/fractional_probability_multiframe_explicit_le` | `extended` | `enhanced/ct/multiframe_shared_perframe_explicit_le` | Segmentation Storage |
| `derived/seg/labelmap_multiframe_explicit_le` | `extended` | `enhanced/ct/multiframe_shared_perframe_explicit_le` | Label Map Segmentation Storage |

Tasks:

- Extend the SEG writer to support FRACTIONAL with Segmentation Fractional Type
  `PROBABILITY`, Maximum Fractional Value, and deterministic 8-bit occupancy
  pattern.
- Add the LABELMAP path using Label Map Segmentation Storage and source-text
  verified LABELMAP attribute requirements.
- Add registry rows, standards evidence, manifest expectations, and tests for
  both variants.
- Add validation for fractional subtype/max value and for LABELMAP SOP Class
  and segmentation type compatibility.

Exit criteria:

- The extended profile contains BINARY, FRACTIONAL, and LABELMAP SEG coverage.
- SEG validation catches mismatched SOP Class versus Segmentation Type in
  mutation tests.

## Phase 5.3: Presentation State And RWVM

Target cases:

| Case ID | Profile | Source object | SOP Class |
|---|---|---|---|
| `derived/presentation-state/grayscale_softcopy_ct_window_explicit_le` | `extended` | `enhanced/ct/multiframe_shared_perframe_explicit_le` | Grayscale Softcopy Presentation State Storage |
| `derived/rwvm/linear_ct_mapping_explicit_le` | `extended` | `enhanced/ct/multiframe_shared_perframe_explicit_le` | Real World Value Mapping Storage |

Tasks:

- Add a GSPS writer with Presentation Series, Presentation State
  Identification, Presentation State Relationship, Displayed Area, Softcopy
  Presentation LUT, and a conservative VOI/window presentation expectation.
- Add an RWVM writer with Real World Value Mapping Series, Real World Value
  Mapping Sequence, units code sequence, linear slope/intercept mapping, and
  Common Instance Reference to the source image.
- Add object-family validators for GSPS required references and RWVM mapping
  sequence contents.
- Ensure coverage reports show both cases as derived/reference objects, not
  renderable source images.

Exit criteria:

- Viewers can be tested for recognizing presentation state and quantitative
  mapping objects that reference generated source images.
- `report --format markdown` and JSON expose the source case IDs in
  `derived_refs`.

## Phase 5.4: SR And KOS

Target cases:

| Case ID | Profile | Source object | SOP Class |
|---|---|---|---|
| `derived/sr/basic_text_observation_explicit_le` | `extended` | `enhanced/ct/multiframe_shared_perframe_explicit_le` | Basic Text SR Storage |
| `derived/sr/comprehensive_measurement_explicit_le` | `extended` | `enhanced/ct/multiframe_shared_perframe_explicit_le` | Comprehensive SR Storage |
| `derived/sr/key_object_selection_explicit_le` | `extended` | generated Enhanced CT and SEG objects | Key Object Selection Document Storage |

Tasks:

- Implement a small SR content tree helper for container, text, code, numeric,
  and image-reference content items as required by the selected SR cases.
- Add Basic Text SR with a deterministic synthetic observation and evidence
  reference to the source image.
- Add Comprehensive SR with a minimal image measurement pattern and explicit
  content relationship validation.
- Add KOS with a key-object title and references to generated image and derived
  objects from the same run.
- Add validators for SR Document Series, SR Document General, SR Document
  Content, Completion/Verification flags, Value Type, Concept Name Code
  Sequence, and referenced evidence.

Exit criteria:

- The extended profile tests recognition of Basic Text SR, Comprehensive SR,
  and KOS objects.
- Mutation tests catch a broken SR evidence reference and a missing required
  content item.

## Phase 5.5: RT Detection Cases

Target cases:

| Case ID | Profile | Source object | SOP Class |
|---|---|---|---|
| `non-image/rt/structure_set_single_roi_explicit_le` | `extended` | `enhanced/ct/multiframe_shared_perframe_explicit_le` | RT Structure Set Storage |
| `non-image/rt/dose_grid_u16_explicit_le` | `extended` | RT Structure Set and generated CT frame of reference | RT Dose Storage |

Tasks:

- Add RT Structure Set with RT Series, Structure Set, ROI Contour, and RT ROI
  Observations modules using a single deterministic ROI.
- Add RT Dose as a tiny grid-based dose detection case with RT Dose Module,
  Image Pixel/Image Plane fields required for grid dose, and references to the
  generated RT Structure Set or shared Frame of Reference as supported by the
  standards evidence.
- Add validation for RT Structure Set ROI sequence consistency, RT Dose grid
  pixel length, dose grid scaling, and reference resolution.
- Keep the expected capability as recognition/metadata handling, not image
  rendering.

Exit criteria:

- Viewers can be tested for graceful handling of RT Dose and RT Structure Set
  SOP Classes.
- RT Dose remains byte-stable and uses only native Explicit VR Little Endian.

## Phase 5.6: Encapsulated PDF And Completion Hardening

Target case:

| Case ID | Profile | Source object | SOP Class |
|---|---|---|---|
| `non-image/encapsulated-document/pdf_minimal_explicit_le` | `extended` | none required | Encapsulated PDF Storage |

Tasks:

- Add a deterministic minimal PDF byte payload and Encapsulated Document Module
  fields including MIME Type of Encapsulated Document and Encapsulated
  Document.
- Add validation for encapsulated document presence, MIME type, payload hash,
  and absence of image-only manifest requirements.
- Run a Phase 5 completion audit against `SYSTEM_SPEC.md` exit criteria.
- Update `IMPLEMENTATION_PROGRESS.md` to mark Phase 5 complete only after all
  target cases and validation gates pass.

Exit criteria:

- Extended/all profiles include all Phase 5 target cases.
- There are no Phase 5 planned skipped cases in `generate --profile extended`
  or `generate --profile all`.
- Derived objects resolve references to source objects generated in the same
  run.
- Generated manifests and coverage reports distinguish renderable images from
  unsupported-but-recognized derived/non-image objects.

## Required Verification Per Increment

Run the narrowest useful command set after each commit, and run the full suite
before marking Phase 5 complete:

```sh
cargo fmt -- --check
cargo test
cargo run -- standards check-lock
cargo run -- generate --profile extended --out /tmp/dts-phase5 --seed 1
cargo run -- validate /tmp/dts-phase5
cargo run -- report /tmp/dts-phase5 --format json
cargo run -- report /tmp/dts-phase5 --format markdown
cargo run -- standards gaps --profile extended
```

If any command is unavailable in the local environment, record the exact command
and failure in `IMPLEMENTATION_PROGRESS.md`.

## Commit Boundaries

- Commit Phase 5.0 registry/schema/foundation changes separately from the first
  SEG recipe.
- Commit each object-family writer and validator as a coherent unit.
- Commit registry status flips in the same commit as the recipe and tests that
  make the status true.
- After every distinct logical unit, run `git log --oneline -3` and confirm the
  commit was recorded.
