# Current Progress

**Last updated:** 2026-06-15  
**Active goal:** comprehensive compressed image codec generation support  
**Current phase:** Phase 0 - Research And Decisions  
**Repo state source:** reconstructed from `SYSTEM_SPEC.md`, `CURRENT_PLAN.md`, `transfer-syntax/capability-matrix.json`, and tests because this file was missing.

## Phase Status

- Phase 0 - Research And Decisions: in progress.
- Phase 1 - Codec Integration Architecture: not started.
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

## Blockers

- No current blocker for the next implementation slice.
- JPEG 2000, HTJ2K, and legacy JPEG backend selection remain unresolved research items.
- Deflated Image Frame Compression requires a standards/IOD suitability decision before any implementation.

## Open Decisions

- Exact Phase 1 codec trait shape.
- How backend versions and encoder options will be represented in generated manifests.
- Whether JPEG/JPEG-LS/JPEG XL project feature gates should directly expose DICOM-rs optional features or wrap them behind project-owned adapters.
- Which independent validators should be used for JPEG 2000 and HTJ2K.

## Verification Results

- `cargo test codec_backend_decisions --test project_artifacts`: passed, 2 tests.
- `cargo fmt -- --check`: initially reported rustfmt-only changes in `tests/project_artifacts.rs`; passed after running `cargo fmt`.
- `cargo test --test project_artifacts`: initially failed because a new test incorrectly required deferred Deflated Image Frame Compression to already be represented in `transfer-syntax/capability-matrix.json`; passed after narrowing that assertion to `implement_now` families, 17 tests.
- `cargo test`: initially failed for the same `project_artifacts` assertion; passed after the fix, full suite clean.
- `cargo run -- standards check-lock`: passed with existing documented lock warnings.

## Commit-Ready Summary

- Phase 0 now has a committed backend decision matrix for compressed codec generation.
- RLE Lossless is the only codec family classified as `implement_now`.
- Higher-risk codec families remain `research_more` or `defer` and no generator, registry status, or capability-matrix availability was changed.
- Focused artifact tests enforce the decision matrix shape and the RLE-first implementation boundary.

## Recommended Next Commit

Implement the Phase 1 minimal codec adapter API needed by native RLE Lossless encoding, including backend identity, determinism, and unavailable/unsupported error reporting. Do not flip `classic/sc/mono2_u8_rle_lossless` from skipped until a later slice proves generation, validation, report output, and reproducibility.
