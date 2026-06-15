# Current Progress

**Last updated:** 2026-06-15  
**Active goal:** comprehensive compressed image codec generation support  
**Current phase:** Phase 1 - Codec Integration Architecture  
**Repo state source:** reconstructed from `SYSTEM_SPEC.md`, `CURRENT_PLAN.md`, `transfer-syntax/capability-matrix.json`, and tests because this file was missing.

## Phase Status

- Phase 0 - Research And Decisions: in progress for non-RLE codecs; RLE has an implement-now decision.
- Phase 1 - Codec Integration Architecture: in progress.
- Phase 2 - Encapsulated Pixel Data Substrate: not started.
- Phase 3 - Low-Risk Codec Enablement: not started.
- Phase 4 - JPEG 2000 And HTJ2K: not started.
- Phase 5 - Legacy And Specialty Compressed Syntaxes: not started.
- Phase 6+ - Corpus expansion and maintenance: blocked until Phase 5 scope is complete.

## Completed Work

- Recreated this durable progress tracker after it was absent from the working tree.
- Added `transfer-syntax/backend-decisions.json` as the Phase 0 codec backend decision record.
- Classified RLE Lossless as the first `implement_now` codec family using a native project-owned encoder.
- Kept JPEG Baseline, JPEG-LS, JPEG XL, JPEG 2000, HTJ2K, and legacy JPEG families in `research_more`.
- Deferred Deflated Image Frame Compression until standards/IOD suitability is resolved.
- Preserved the existing invariant that compressed registry rows remain skipped and `transfer-syntax/capability-matrix.json` remains unavailable until generation, validation, reporting, and reproducibility are proven.
- Added `src/codecs.rs` with a minimal frame encoder API for compressed still-image backends.
- Added backend identity, backend kind, feature gate, version, transfer syntax UID, and determinism metadata through `CodecBackendInfo`.
- Added typed codec errors for unavailable, unsupported, encode-failed, and validation-failed outcomes.
- Added a native project-owned RLE Lossless frame encoder that emits a DICOM RLE frame header and deterministic PackBits-style segment payloads for byte-aligned frames up to the 15-segment RLE limit.
- Kept `classic/sc/mono2_u8_rle_lossless` skipped and kept `transfer-syntax/capability-matrix.json` unavailable for RLE until encapsulated Pixel Data generation, validation, report output, and reproducibility are proven.

## Blockers

- No current blocker for the next implementation slice.
- JPEG 2000, HTJ2K, and legacy JPEG backend selection remain unresolved research items.
- Deflated Image Frame Compression requires a standards/IOD suitability decision before any implementation.

## Open Decisions

- Whether the initial codec API needs separate decode traits before JPEG/JPEG-LS/JPEG XL work begins.
- How codec backend versions and encoder options will be represented in generated manifests once compressed cases are emitted.
- Whether JPEG/JPEG-LS/JPEG XL project feature gates should directly expose DICOM-rs optional features or wrap them behind project-owned adapters.
- Which independent validators should be used for JPEG 2000 and HTJ2K.

## Verification Results

- `cargo test codec_backend_decisions --test project_artifacts`: passed, 2 tests.
- `cargo fmt -- --check`: initially reported rustfmt-only changes in `tests/project_artifacts.rs`; passed after running `cargo fmt`.
- `cargo test --test project_artifacts`: initially failed because a new test incorrectly required deferred Deflated Image Frame Compression to already be represented in `transfer-syntax/capability-matrix.json`; passed after narrowing that assertion to `implement_now` families, 17 tests.
- `cargo test`: initially failed for the same `project_artifacts` assertion; passed after the fix, full suite clean.
- `cargo run -- standards check-lock`: passed with existing documented lock warnings.
- `dicom-standard-kb` MCP `lookup_uid RLELossless`: passed; confirmed UID `1.2.840.10008.1.2.5` as a PS3.6 Transfer Syntax.
- `dicom-standard-kb` MCP `search_standard_text "RLE Compression"` in PS3.5: passed; found anchors for PS3.5 Section 8.2.2, Section 10.4, Annex A.4.2, Annex G.1, and Table 8.2.2-1.
- `dicom-standard-kb` MCP `retrieve_standard_text` for PS3.5 Annex G sections: could not run because the local SQLite text cache `/Users/beatrice/.cache/dicom-standard-kb/db/2026b.sqlite` is absent.
- `cargo fmt -- --check`: initially reported rustfmt-only layout changes in the new `src/codecs.rs`; passed after running `cargo fmt`.
- `cargo test codecs`: passed, 6 focused codec tests.
- `cargo test`: passed, full suite clean.

## Commit-Ready Summary

- Phase 1 now has the first codec integration boundary and native RLE frame encoder needed for later RLE generation work.
- The RLE encoder reports backend identity and byte-stable determinism without requiring a feature gate or external codec.
- Unsupported frame shapes are reported as typed codec errors rather than panics or silent fallback.
- No generator recipe, registry status, capability-matrix availability, manifest output, validation behavior, or report output was changed.

## Recommended Next Commit

Add the reusable encapsulated Pixel Data substrate needed for compressed frames: one-fragment-per-frame layout, empty Basic Offset Table support, item padding behavior, and focused tests using the native RLE encoded frame. Do not flip `classic/sc/mono2_u8_rle_lossless` from skipped until generation, validation, report output, and reproducibility all pass in a later slice.
