# Phase 7 Bounded Fuzz Status

Status date: 2026-08-28

## Qualified profile

The opt-in `fuzz` profile is implemented as a payload-free runtime
qualification. It resolves two committed seed descriptions to privately
generated, known-good Explicit VR Little Endian and RLE Lossless Secondary
Capture instances. Source and candidate bytes are removed before the staged
run is promoted; the generated root contains only `manifest.json`.

Each seed uses the deterministic mutation substrate with a shared aggregate
ceiling of 64 candidates, 512 mutations, 8 MiB input/output, 256 minimization
attempts per reproducer, and 100,000,000 target operations. The checked Part
10 target is explicitly labeled `same_project`; it is bounded by input bytes
and fixed element, depth, item, and Fragment limits. Its evidence is useful
robustness qualification, not independent parser conformance.

## Reproducibility and outcomes

Two seed-7 executions produce identical qualification records. The qualified
run generated 64 candidates and 294 mutations. Outcomes were 4 accepted, 20
clean rejections, and 40 parse failures. The first interesting result from
each seed was deterministically minimized, retaining only a fingerprint and
reproduction metadata rather than payload bytes.

Crash, hang, timeout, and resource-limit outcomes are distinct and
unconditionally unacceptable. All four counts were zero. Strict validation
rejects any passing record that contains one of those outcomes, exceeds a
declared budget, leaks into another profile, or retains a generated DICOM
payload.

JSON and Markdown reports expose fuzz evidence separately from both the valid
coverage matrix and expected-invalid negative coverage. A minimized input is
not a regression merely because it is interesting: promotion requires a
named reproducible `negative/...` recipe. Fuzz-generated DICOM payloads remain
uncommitted.

Phase 7 is complete. The deterministic negative corpus remains documented in
`docs/phase-7-negative-status.md`.
