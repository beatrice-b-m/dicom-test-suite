# Current Plan

**Last updated:** 2026-08-26
**Active goal:** implement independent conformance evidence collection
**Source specification:** `SYSTEM_SPEC.md` version 0.2.0
**Planning status:** conformance Phases 0-2 complete; instance collection next

## Independent Conformance Framework

The assignment in `docs/conformance-validation-agent-brief.md` is active.
Phase 0 is complete: the portable suite passed, all-features seed-1 `all` and
`legacy` corpora generated and validated internally, installed tools were
inventoried, and `conformance/README.md` records the adapter decision matrix.

Real IOD/entity acceptance is blocked on choosing and installing an immutable
dicom3tools distribution providing `dciodvfy` and `dcentvfy`. DCMTK 3.7.0 is
available for the independent parser. Framework development proceeds with fake
executables and no network/runtime dependency in the default build.

Phase 1 is complete: strict Draft 2020-12 schemas define run evidence and exact
finding dispositions; committed fixtures are validated with a full schema
engine; the accepted-finding set starts empty; and the DCMTK parser baseline is
locked by version, source tag, package identity, platform, and executable hash.

Phase 2 is complete: `conformance check-tools` resolves configured paths before
`PATH`, executes argv directly, fingerprints bytes, captures stdout/stderr and
nonzero version probes, times out hung probes, represents versionless commands,
and compares tools to the committed lock.

Recommended next step: implement manifest-driven, bounded, deterministic
per-instance `dciodvfy -new` collection with raw byte preservation and finding
normalization.

This document replaces the previous historical implementation plan and progress
ledger. `SYSTEM_SPEC.md` remains the architecture and requirements source of
truth. This file should stay focused on the active project goal, the major
implementation phases, and the decisions that must be researched before a
specific backend or codec strategy is locked in.

## Viewer-Review Readiness Term

The completed codec corpus is entering a short readiness term before an
independent viewer project consumes it. This term does not add viewer-specific
behavior to the generator and does not prescribe how a viewer must be tested.
It establishes a reliable corpus-selection contract, neutral consumption
instructions, and a tractable handoff for independent DICOM conformance
evidence.

Current phases:

1. **Profile contract:** complete. `list-cases --profile all` uses the same
   smoke/core/extended union as generation and keeps legacy opt-in.
2. **Corpus consumption guide:** complete. `docs/corpus-consumption.md`
   documents complete and reduced corpus generation, prerequisites, validation,
   reports, manifests, neutral consumer responsibilities, and scope.
3. **Independent conformance handoff:** complete.
   `docs/conformance-validation-agent-brief.md` defines implementable evidence
   artifacts, validator pinning, finding disposition, corpus-level checks,
   independent pixel decoding, phased commits, and acceptance criteria without
   prescribing viewer behavior.

Exit criteria:

- Listing and generation agree on the meaning of `all`.
- A consuming agent can generate every available case without discovering
  hidden feature or external-command requirements.
- A validation-framework agent has a self-contained brief with phased work,
  acceptance criteria, and explicit evidence outputs.

## Current Baseline

The generator already has a stable foundation for synthetic DICOM corpus
generation:

- Native, derived, presentation, SR, RT, and encapsulated-document cases through
  the previous Phase 5 scope are implemented.
- Native transfer syntax infrastructure is present, including Implicit VR
  Little Endian, Explicit VR Little Endian, Explicit VR Big Endian, and a
  feature-gated Deflated Explicit VR Little Endian dataset case.
- Encapsulated Pixel Data manifest and validation metadata exist for offset
  table state, fragments per frame, Extended Offset Table state, Extended
  Offset Table Lengths state, and compressed frame hashes.
- RLE Lossless generation is implemented for 8-bit and 16-bit Secondary Capture
  cases through the native project-owned encoder, with decoded-frame validation
  and byte-stable reproducibility.
- JPEG Baseline 8-bit generation is implemented as a feature-gated `jpeg`
  Secondary Capture case through the pinned `dicom-rs` adapter path, with
  decoded-frame tolerance validation and semantic-stable reproducibility.
- JPEG-LS Lossless has an implement-now decision, a project-level `charls`
  feature, a DICOM-rs CharLS wrapper, and a feature-gated generated corpus row
  with validation, reporting, and reproducibility coverage. JPEG-LS
  Near-Lossless is explicitly deferred until lossy semantics and validation
  policy are selected.
- JPEG 2000 Lossless has a project `jpeg2000` feature, a project-owned
  `jpeg2k`/`openjp2` adapter, a feature-gated generated Secondary Capture case,
  validation, reporting, and reproducibility evidence. JPEG 2000 lossy remains
  deferred until lossy semantics and validation policy are selected.
- HTJ2K Lossless has a project `htj2k_openjph` feature, an OpenJPH
  external-command wrapper, a feature-gated generated Secondary Capture case,
  validation, reporting, and reproducibility evidence. HTJ2K lossy/RPCL
  variants remain deferred.
- JPEG XL lossy, JPEG 2000 lossy, and HTJ2K lossy/RPCL remain unavailable
  until their backend and standards decisions are resolved and proven.
- Deflated Image Frame Compression has a project `deflate` feature, a pinned
  DICOM-rs adapter wrapper, and a generated binary Segmentation multi-frame
  case with one fragment per frame, exact decoded-frame validation, report
  coverage, and reproducibility evidence.
- `dicom-rs` 0.9.1 provides the useful integration surface:
  `PixelDataReader`, `PixelDataWriter`, transfer syntax descriptors, and
  optional codec features. It does not currently provide verified writers for
  every codec family required by this project.

## Goal

Make compressed image transfer syntax coverage a first-class capability of the
project. The intended outcome is a generator-owned codec integration layer that
can produce, validate, report, and reproduce DICOM files for all reasonably
expected still-image compressed transfer syntax families.

The work should prefer existing mature encoders and upstreamable `dicom-rs`
adapters over from-scratch codec implementations. A project-owned native
encoder is acceptable where the format is small and bounded, such as RLE
Lossless.

## Current-Term Completion Boundary

The current term ends when the existing lossless/native codec corpus has:

- generation, validation, reporting, and reproducibility coverage;
- one representative empty Basic Offset Table RLE Lossless case and one
  multi-fragment-per-frame JPEG Baseline case in addition to the existing
  populated-table, single-fragment cases;
- automated default-build and feature-gated codec verification; and
- current user documentation plus a completed durable plan handoff.

Do not extend this term with additional RLE pixel permutations or report fields
unless they are necessary to satisfy those exit criteria.

Extended Offset Table generation is deferred to the future large-object/stress
scope because the current small corpus does not require 64-bit frame offsets.
Lossy JPEG-LS, JPEG XL, JPEG 2000, and HTJ2K variants remain deferred until
their metadata, tolerance, validation, and reproducibility policies are
selected. JPEG Extended 12-bit remains deferred until an independent 12-bit
decode path is available.

## Scope

Initial target families:

- RLE Lossless.
- JPEG Baseline 8-bit.
- JPEG-LS Lossless and Near-Lossless.
- JPEG XL Lossless and JPEG XL.
- JPEG 2000 Lossless and Lossy.
- HTJ2K Lossless, RPCL, and lossy variants where practical.
- JPEG Lossless SV1 and JPEG Lossless Process 14 have project
  `legacy_jpeg_dcmtk` feature-gated generated Secondary Capture cases through
  DCMTK `dcmcjpeg`, with manifest runtime identity capture, exact decoded-frame
  validation, report coverage, and reproducibility evidence. JPEG Extended
  12-bit has DCMTK encode and reproducibility spike evidence, but generated-case
  promotion is deferred until an independent 12-bit validation path is selected.
- Deflated Image Frame Compression using the pinned DICOM-rs `deflate` adapter
  for a first binary Segmentation multi-frame case, with one fragment per frame
  and exact decoded-frame validation.

Out of scope for this immediate goal:

- Video transfer syntaxes.
- Whole Slide Imaging scale/stress coverage beyond what is needed to prove
  compressed frame handling.
- Negative/fuzz malformed compressed codestreams.

## Phase 0: Research And Decisions

Goal: turn the current uncertainty into explicit backend decisions before code
is written.

Deliverables:

- A codec backend decision matrix covering each target transfer syntax family.
- A licensing and redistribution decision for every external library or tool.
- A build portability decision for macOS, Linux, and Windows.
- A determinism policy per codec: byte-stable, semantic-stable, or unsupported.
- A validation strategy per codec, including at least one independent decode or
  conformance check where practical.
- Updated `transfer-syntax/capability-matrix.json` entries only after evidence
  supports the new claim.

Research questions needing one clear option:

- Should heavyweight codecs be integrated through Rust/FFI adapters,
  subprocess tools, upstream `dicom-rs` patches, or a hybrid approach?
- Is AGPL/GPL tooling acceptable for optional local generation, or must all
  default-supported backends be permissively licensed?
- Which JPEG 2000 backend should be primary: OpenJPEG through the existing
  Rust `jpeg2k` crate, a lower-level OpenJPEG binding, DCMTK/GDCM tooling, or
  another option?
- Which HTJ2K backend should be primary: OpenJPH, Grok, another library, or
  deferred support?
- Which backend can reliably produce legacy JPEG Lossless, JPEG Lossless SV1,
  and JPEG Extended 12-bit DICOM-compatible codestreams?
- Which codecs can be expected to produce byte-stable output across the pinned
  toolchain and target platforms?
- What exact DICOM attribute updates are required per codec for photometric
  interpretation, planar configuration, lossy image compression metadata,
  derivation metadata, and signed/unsigned sample representation?

Exit criteria:

- Each target codec family is classified as `implement_now`, `research_more`,
  `defer`, or `reject`.
- Every `implement_now` family has one selected backend, feature gate,
  validation path, determinism classification, and first case target.

## Phase 1: Codec Integration Architecture

Goal: add a project-owned integration layer that can support multiple codec
backends without coupling generator recipes directly to external tools.

Major work:

- Define codec traits around frame-level encode/decode inputs and outputs.
- Record backend identity, version, feature flags, encoder options, and
  determinism class in generated manifests.
- Provide a common error model for unavailable features, unsupported image
  shapes, codec failures, and validation failures.
- Keep backend selection explicit and reportable through `list-cases`,
  generated manifests, validation, and reports.
- Preserve compatibility with `dicom-rs` `PixelDataWriter` and
  `PixelDataReader` so mature adapters can be upstreamed or reused later.

Areas to solidify:

- Whether the integration layer lives inside the current package, a workspace
  crate, or a separate repository consumed by this project.
- Whether external command execution is allowed in normal generator runs or
  only behind explicit opt-in features/profiles.
- How backend versions are discovered and pinned for reproducibility.

## Phase 2: Encapsulated Pixel Data Substrate

Goal: make DICOM encapsulation independent of any specific codec.

Major work:

- Build reusable frame-to-fragment layout code for one-fragment-per-frame and
  multi-fragment frames.
- Support empty and populated Basic Offset Tables.
- Add Extended Offset Table and Extended Offset Table Length support where
  needed for larger or multi-fragment cases.
- Handle item padding for odd compressed frame lengths without corrupting frame
  length metadata.
- Provide frame indexing utilities for future partial decode work.
- Validate compressed frame hashes by decoding back to native frame hashes when
  a decoder is available.

Areas to solidify:

- Which first cases require populated Basic Offset Tables versus empty Basic
  Offset Tables.
- Whether Extended Offset Table support should be implemented immediately or
  only when a large/multi-fragment case needs it.
- How to represent partial decode indexes in the manifest without overfitting
  to one codec backend.

## Phase 3: Low-Risk Codec Enablement

Goal: establish the complete generation/validation/reporting loop on codecs
with bounded implementation risk.

Major work:

- Implement or integrate RLE Lossless encoding first. A native project-owned
  RLE encoder is preferred because the format is small and deterministic.
- Verify JPEG Baseline 8-bit through the existing `dicom-rs` optional writer or
  a selected external backend.
- Verify JPEG-LS Lossless and Near-Lossless through the selected CharLS or
  tool-backed path.
- Verify JPEG XL Lossless and JPEG XL through the selected `dicom-rs` optional
  writer or external backend.
- Add one tiny Secondary Capture case per codec family before expanding to
  modality-specific IODs.

Areas to solidify:

- Whether JPEG Baseline, JPEG-LS, and JPEG XL should be enabled by exposing
  existing `dicom-rs` features, by wrapping external tools, or by adding
  project-owned adapters around their underlying libraries.
- Which photometric interpretations are valid and useful for the first JPEG and
  JPEG-LS cases.
- Whether lossy cases should be byte-stable or semantic-stable from the start.

Exit criteria:

- Each enabled codec has at least one generated file, validation, report
  coverage, reproducibility coverage, and skipped/unavailable behavior when the
  feature is absent.
- Codec failures are surfaced as capability-gated skips or validation failures,
  not panics or silent fallback to native Pixel Data.

## Phase 4: JPEG 2000 And HTJ2K

Goal: add the most important advanced still-image compressed syntaxes with a
clear backend strategy and independent validation.

Major work:

- Implement JPEG 2000 Lossless first, then JPEG 2000 lossy.
- Implement HTJ2K Lossless first, then RPCL/lossy variants where practical.
- Support 8-bit and 16-bit unsigned samples before expanding to signed samples.
- Verify codestream compatibility with DICOM transfer syntax requirements, not
  just generic image codec decode success.
- Add semantic reproducibility based on decoded frame hashes when encoded bytes
  are not stable.

Areas to solidify:

- Whether the existing Rust `jpeg2k` crate can encode directly from in-memory
  native pixel frames, or whether lower-level OpenJPEG access or subprocess
  tooling is required.
- Whether OpenJPH can cover the required HTJ2K cases with acceptable licensing,
  platform support, and DICOM compatibility.
- Whether Grok is acceptable for any optional path given its licensing and
  deployment implications.
- Which independent validator should be used for JPEG 2000 and HTJ2K output.

Exit criteria:

- JPEG 2000 and HTJ2K lossless cases round-trip to the original decoded native
  frame hashes.
- Lossy cases record deterministic semantic expectations and lossy metadata.
- Unsupported backends or platforms produce explicit skipped-case reports.

## Phase 5: Legacy And Specialty Compressed Syntaxes

Goal: cover remaining still-image compressed syntaxes that are expected in
viewer compatibility testing but require more specialized backend support.

Major work:

- Add JPEG Extended 12-bit if a reliable encoder backend is selected.
- Add JPEG Lossless and JPEG Lossless SV1 if a reliable encoder backend is
  selected.
- Add Deflated Image Frame Compression for the selected binary Segmentation
  target now that standards and IOD suitability are resolved.
- Add additional case dimensions for multi-frame, odd compressed frame lengths,
  non-empty offset tables, and selected modality IODs.

Areas to solidify:

- Which tool or library is acceptable for legacy JPEG family encoding.
- Which exact registry ID and source-pixel reuse path should be used for the
  first binary Segmentation Deflated Image Frame generated case.
- Which modality IODs are most valuable for compressed transfer syntax coverage
  beyond Secondary Capture.

## Phase 6: Corpus Expansion And Reporting

Goal: turn individual codec support into a coherent compressed-image corpus.

Major work:

- Expand from one smoke-sized case per codec to a deliberate matrix of bit
  depth, photometric interpretation, frame count, fragment layout, and IOD.
- Keep profile inclusion explicit so default runs remain practical.
- Update registry rows from skipped to implemented only in the same commit as
  working generation, validation, reports, and tests.
- Extend coverage reports to summarize codec family, transfer syntax, backend,
  determinism class, and skipped/unavailable reasons.
- Add fixture-generation or compatibility notes for external viewers without
  making viewer behavior a generator constraint.

Areas to solidify:

- The exact compressed case matrix for `core`, `extended`, `legacy`, and any
  new codec-specific profile.
- Whether `all` should include optional external-codec cases by default when
  features are enabled.
- How large stress cases should be gated to avoid slowing normal validation.

Current-term exit work:

- Add one byte-stable multi-frame RLE Lossless case with an empty Basic Offset
  Table while retaining one fragment per frame as required by PS3.5
  Section A.4.2.
- Add one feature-gated JPEG Baseline case with multiple fragments per frame,
  which PS3.5 Section A.4.1 explicitly permits.
- Verify that generation-time and CLI validation reassemble each compressed
  frame before exact native-frame hash comparison.
- Add default-build and in-process codec feature CI jobs.
- Give external-command HTJ2K and legacy JPEG backends either pinned execution
  jobs or an explicit scheduled/manual verification policy.
- Run the final profile acceptance matrix and update user-facing commands.

## Phase 7: Upstreaming And Maintenance

Goal: reduce long-term project ownership where codec support belongs in shared
libraries.

Major work:

- Upstream small, general-purpose `dicom-rs` adapter improvements where
  possible.
- Keep project-specific corpus recipes in this repository.
- Maintain backend version pinning, CI coverage, and capability matrix evidence.
- Document unsupported combinations clearly rather than letting stale matrix
  entries imply support.

Areas to solidify:

- Which adapters should be proposed upstream versus kept project-owned.
- How often codec backend support should be reverified against newer
  `dicom-rs` releases.
- Whether the codec integration layer should be published independently for the
  future partial compressed pixel-array project.

Phase 7 is a long-term maintenance roadmap and is not a current-term completion
blocker.

## Completion Status

The current-term exit criteria were completed on 2026-07-28:

- The default extended corpus generates 75 files and reports nine unavailable
  feature-gated rows with no blocked or planned rows.
- The existing two-frame RLE Lossless case covers an empty Basic Offset Table
  while retaining the required one-fragment-per-frame layout.
- The JPEG Baseline case covers a frame split across two fragments and is
  reassembled before decoded-sample tolerance validation.
- Default and in-process codec feature paths have automated CI execution.
- External-command paths have compile CI plus a documented runtime verification
  policy; local OpenJPH and DCMTK acceptance runs generated and validated their
  cases successfully.
- README commands, feature requirements, profiles, and verification entry
  points reflect the implemented system.

## Deferred Long-Term Roadmap

- JPEG Extended 12-bit and deferred lossy codec variants.
- Whole Slide Microscopy, video transfer syntaxes, and large stress objects.
- Viewer runner adapters and viewer-specific regression workflows.
- Negative and fuzz profiles.
- Publishing or upstreaming codec adapters.
- Extended Offset Table cases that require genuinely large frame offsets.

## Immediate Next Step

Execute `docs/conformance-validation-agent-brief.md` from Phase 0 through the
real-tool acceptance phase. Do not give the independent viewer-review go signal
until strict conformance evidence passes or every remaining limitation is
recorded as an explicit blocker.
