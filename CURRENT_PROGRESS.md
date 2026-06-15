# Current Progress

**Last updated:** 2026-06-15  
**Active goal:** comprehensive compressed image codec generation support  
**Current phase:** Phase 3 - Low-Risk Codec Enablement
**Repo state source:** reconstructed from `SYSTEM_SPEC.md`, `CURRENT_PLAN.md`, `transfer-syntax/capability-matrix.json`, and tests because this file was missing.

## Phase Status

- Phase 0 - Research And Decisions: in progress for non-RLE codecs; RLE has an implement-now decision.
- Phase 1 - Codec Integration Architecture: in progress; minimal codec API plus native RLE frame encode/decode support are present.
- Phase 2 - Encapsulated Pixel Data Substrate: complete for the first RLE one-fragment case; Extended Offset Table and future multi-fragment generation remain later substrate expansion.
- Phase 3 - Low-Risk Codec Enablement: in progress; first generated RLE Lossless Secondary Capture case is implemented and round-trip validated.
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
- Previously kept `classic/sc/mono2_u8_rle_lossless` skipped until encapsulated Pixel Data generation, validation, report output, and reproducibility were proven; this slice now flips that row to implemented.
- Added `src/encapsulation.rs` with reusable encapsulated Pixel Data item stream construction.
- Added one-fragment-per-frame encapsulation and configurable empty or populated Basic Offset Table item support.
- Recorded Basic Offset Table offsets relative to the first fragment item while keeping internal fragment metadata offsets relative to the encoded Pixel Data value bytes.
- Added deterministic item padding for odd compressed frame lengths and tracked compressed frame hashes.
- Added multi-fragment frame layout support for later empty Basic Offset Table cases.
- Added focused encapsulation tests, including wrapping a native RLE encoded frame as a single fragment.
- Implemented `classic/sc/mono2_u8_rle_lossless` as the first compressed generated Secondary Capture case.
- Used the native project-owned RLE frame encoder and DICOM-rs `PixelFragmentSequence` to write true encapsulated Pixel Data with RLE Lossless transfer syntax `1.2.840.10008.1.2.5`.
- Used a populated Basic Offset Table for the single-fragment-per-frame RLE case so the case satisfies the project's encapsulated Pixel Data combination validator without requiring Extended Offset Table metadata.
- Added file manifest codec backend metadata, native decoded frame hashes, compressed frame hashes, fragment layout metadata, and encapsulated Pixel Data schema support.
- Extended Part 10 generation-time validation to validate encapsulated Pixel Data fragment sequence shape without applying native byte-length checks to compressed data.
- Flipped only the RLE Lossless registry row and capability matrix entry to implemented/available/byte-stable; JPEG Baseline, JPEG-LS, JPEG XL, JPEG 2000, and HTJ2K remain skipped/unavailable.
- Added focused CLI/artifact/schema tests for RLE generation, list-cases status, validation counts, and capability matrix state.
- Added a minimal `FrameDecoder` API and native project-owned RLE Lossless decoder for DICOM RLE frame headers, PackBits-style segment payloads, and byte-plane reconstruction.
- Added generation-time RLE round-trip validation that decodes encapsulated RLE fragments back to native frame bytes and compares decoded SHA-256 hashes to the expected native frame hashes.
- Added CLI `validate` RLE round-trip validation that decodes generated RLE fragments and compares the decoded native frame hashes to manifest `/pixel_data/frame_hashes`.
- Added focused tests for RLE decode behavior, generated manifest validation results, and CLI rejection of RLE decoded-frame hash mismatches.

## Blockers

- No current blocker for the next implementation slice.
- JPEG 2000, HTJ2K, and legacy JPEG backend selection remain unresolved research items.
- Deflated Image Frame Compression requires a standards/IOD suitability decision before any implementation.

## Open Decisions

- How codec backend versions and encoder options will be represented in generated manifests once compressed cases are emitted.
- Whether JPEG/JPEG-LS/JPEG XL project feature gates should directly expose DICOM-rs optional features or wrap them behind project-owned adapters.
- Which independent validators should be used for JPEG 2000 and HTJ2K.
- Whether the current project-owned RLE decoder should support multi-fragment frame reassembly in generation-time validation before a multi-fragment RLE case is added.

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
- `dicom-standard-kb` MCP `search_standard_text "Basic Offset Table encapsulated Pixel Data Item padding Extended Offset Table"` in PS3.5: passed; confirmed PS3.5 Section A.4 as the local KB anchor for encapsulated Pixel Data Basic Offset Table and trailing NULL padding behavior.
- `cargo test encapsulation`: passed, 5 focused encapsulation tests.
- `cargo fmt -- --check`: initially reported rustfmt-only layout changes in the new `src/encapsulation.rs`; passed after running `cargo fmt`.
- `cargo test`: passed, full suite clean.
- `dicom-standard-kb` MCP `lookup_uid RLELossless`: passed; confirmed UID `1.2.840.10008.1.2.5` as a PS3.6 Transfer Syntax.
- `dicom-standard-kb` MCP `lookup_sop_class "Secondary Capture Image Storage"`: passed; confirmed Secondary Capture Image Storage UID `1.2.840.10008.5.1.4.1.1.7` and linked Secondary Capture Image IOD.
- `dicom-standard-kb` MCP `search_standard_text "RLE Lossless Transfer Syntax encapsulated Pixel Data"` in PS3.5: passed; found PS3.5 Section 8.2.2 as the RLE compression anchor.
- `cargo test codecs encapsulation --test generate_cli --test validate_cli --test list_cases_cli --test project_artifacts --test schema_artifacts --test report_cli`: failed because the command used two test filters; reran as separate valid cargo test commands.
- `cargo test codecs`: passed, 6 focused codec tests.
- `cargo test encapsulation`: passed, 5 focused encapsulation tests.
- `cargo test --test generate_cli --test validate_cli --test list_cases_cli --test project_artifacts --test schema_artifacts --test report_cli`: initially failed because the RLE case used an empty Basic Offset Table for a one-fragment frame without Extended Offset Table; passed after switching to a populated Basic Offset Table.
- `cargo test --test validate_cli validate_command_accepts_generated_extended_root`: passed after the populated Basic Offset Table fix.
- `cargo test --test generate_cli generate_command_writes_extended_enhanced_ct_multiframe_case`: passed after updating RLE manifest expectations.
- `cargo fmt -- --check`: passed.
- `cargo test`: passed, full suite clean.
- `cargo run -- standards check-lock`: passed with the existing documented lock warnings.
- `cargo run -- generate --profile extended --out /tmp/dts-rle-slice-ext-0615 --seed 1`: passed, 18 files written in the no-deflate build.
- `cargo run -- validate /tmp/dts-rle-slice-ext-0615`: passed, 18 files checked and 0 validation failures.
- `cargo run -- report /tmp/dts-rle-slice-ext-0615 --format json`: passed; report counted 18 generated rows and included `classic/sc/mono2_u8_rle_lossless` as generated with transfer syntax `1.2.840.10008.1.2.5`.
- `cargo run -- generate --profile extended --out /tmp/dts-rle-repro-a-0615 --seed 1`: passed, 18 files written.
- `cargo run -- generate --profile extended --out /tmp/dts-rle-repro-b-0615 --seed 1`: passed, 18 files written.
- `diff -r /tmp/dts-rle-repro-a-0615 /tmp/dts-rle-repro-b-0615`: passed with no differences.
- `cargo test codecs`: initially failed because adding `FrameDecoder::backend` made the existing backend identity test ambiguous; passed after disambiguating the test, 10 focused codec tests.
- `cargo test --test generate_cli generate_command_writes_extended_enhanced_ct_multiframe_case`: passed after adding the RLE decoded-frame validation assertion.
- `cargo test --test validate_cli validate_command_reports_rle_decoded_frame_hash_mismatch`: passed, proving CLI validation rejects a manifest native-frame hash that does not match decoded RLE bytes.
- `cargo fmt -- --check`: initially reported rustfmt-only layout changes in touched Rust files; passed after running `cargo fmt`.
- `cargo test`: passed, full suite clean.
- `cargo run -- standards check-lock`: passed with the existing documented lock warnings.
- `cargo run -- generate --profile extended --out /tmp/dts-rle-decode-slice-0615 --seed 1`: passed, 18 files written in the no-deflate build.
- `cargo run -- validate /tmp/dts-rle-decode-slice-0615`: passed, 18 files checked and 0 validation failures.
- `cargo run -- report /tmp/dts-rle-decode-slice-0615 --format json`: passed; report counted 18 generated rows and included `classic/sc/mono2_u8_rle_lossless` as generated with validation status `passed`.

## Commit-Ready Summary

- Phase 3 now has RLE round-trip validation for the first generated compressed image file: `classic/sc/mono2_u8_rle_lossless`.
- The case is byte-stable, uses the native project RLE backend, writes encapsulated Pixel Data with a populated Basic Offset Table, records codec/fragment metadata in the manifest, decodes back to the expected native frame hash during generation and CLI validation, appears in JSON reports, and is byte-identical across two extended runs.
- Only the RLE Lossless registry row and capability matrix entry were flipped to implemented/available; other compressed transfer syntax rows remain skipped/unavailable.

## Recommended Next Commit

Add a second small RLE Lossless Secondary Capture case, preferably 16-bit MONOCHROME2, to exercise multi-segment byte-plane generation and decoded-frame hash validation before starting another low-risk codec family.
