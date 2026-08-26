# Coverage Expansion Baseline

This document records the Phase 0 comparison point for the coverage expansion
program. Generated JSON and Markdown reports remain build artifacts; this
committed note preserves only the stable source snapshot and headline metrics.

## Pre-expansion registry

The registry at commit `701d516` had SHA-256
`cc0bef6690dbd7a338608e8e2293e8b1b48eeb114c022cb0827a7fb74ca2483d`.
It contained:

- 106 implemented logical cases;
- 21 distinct SOP Classes;
- 19 distinct IOD names;
- 70 Secondary Capture Image Storage cases;
- 108 generated files in the all-features seed-1 `all` corpus.

The difference between 106 logical cases and 108 files is intentional evidence
that file count is not a coverage measure. Multi-instance recipes can produce
more than one file without adding a new compatibility axis.

## Phase 0 roadmap baseline

After the Phase 0 inventory, the registry SHA-256 is
`aad205f2f855c1c7e01029293bbb00aab852df8dff6ce2d9989ede874b6b9eeb` and the
standards lock SHA-256 is
`7da5898e512743de8bec2edc36c11827fa250c95d9a2617e8aeabf2526e21d31`.
The source inventory contains:

- 179 logical entries: 106 implemented and 73 planned;
- priorities of 2 `now`, 34 `next`, and 37 `later`;
- 175 DICOM-instance entries, one media File-set, and three transaction
  scenarios;
- 47 represented or planned SOP Classes;
- 28 represented or planned modalities;
- 15 object families and 20 compatibility axes.

The 106 implemented cases establish the initial axis baseline:

| Measure | Implemented baseline |
|---|---:|
| SOP Classes | 21 |
| Modalities | 16 |
| Object families | 9 |
| Compatibility axes | 11 |
| Secondary Capture logical cases | 70 |

The first dependency-ordered proof cases are:

- native: `geometry/ct/spatial_sort_conflicts_instance_number`;
- external: `derived/parametric-map/float32_ct_derived_explicit_le`.

## Reproducing the roadmap report

Run the deterministic registry-only report without generating DICOM files or
installing an external backend:

```sh
cargo run --locked --no-default-features -- report gaps --format json
cargo run --locked --no-default-features -- report gaps --format markdown
```

The report hashes the exact registry and standards-lock bytes, sorts cases and
dimension values, and omits wall-clock timestamps. Identical inputs therefore
produce byte-identical JSON. Compare expansion progress by implemented SOP
Classes, modalities, object families, and compatibility axes, then inspect the
per-case blocker and provider rows. Do not use generated file count as the
primary progress metric.

Protocol scenarios intentionally have no file-profile membership and no
invented SOP Class or Transfer Syntax. They remain visible roadmap gaps while
file, media, DIMSE, DICOMweb, and security outcomes stay separate.
