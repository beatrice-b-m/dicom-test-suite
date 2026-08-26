# Independent Conformance Acceptance Status

**Run date:** 2026-08-26  
**Status:** blocked; not accepted  
**Claim boundary:** reproducible engineering assessment, not official DICOM certification

## Reproduced baseline

The default locked suite passes. An all-features seed-1 `all` corpus generated
108 instances and internal validation reported zero failures. An all-features
seed-1 `legacy` corpus generated one instance and also reported zero failures.

The current arm64 macOS conformance run used locked DCMTK 3.7.0 executables:

- `dcmdump` SHA-256
  `d2261944ea1ceb6743df9866f2237014b284fa39119c8a5eee226ae922ead45f`;
- `dcmdrle` SHA-256
  `d63743af7ec1dc8f0af0dc7562e2c502e81c3af9f38a7b51de30e822de7c8daf`.

Results on the 108-instance all-features corpus:

- independent parser completed for 108/108 manifest files;
- independent RLE decode matched every expected frame hash for 58/58 RLE
  instances, including 8/16-bit, signed/unsigned, monochrome/color,
  planar/interleaved, and single/multi-frame cases;
- primary IOD validation completed for 0/108 because `dciodvfy` is absent;
- corpus entity validation is unsupported because `dcentvfy` is absent; and
- strict verification correctly failed with 115 failures: six required-tool
  identity gaps, 108 incomplete primary results, and one incomplete entity run.

The generated evidence bundle remains an ignored local artifact and was not
committed.

## Exact blocker

Select and approve one immutable dicom3tools distribution for each acceptance
platform. The decision must cover its BSD license attribution, upstream source
snapshot or package identity, executable SHA-256 values, validator-definition
vintage, and CI acquisition method. After installation:

1. add `dciodvfy` and `dcentvfy` entries to `validator-lock.json`;
2. characterize their real exit and stdout/stderr behavior with controlled
   fixtures;
3. confirm every generated SOP Class is recognized by that definition build;
4. run all-features `all` and `legacy` collections;
5. fix generator defects before considering exact finding dispositions;
6. evaluate PixelMed `DicomSRValidator` for generated SR cases; and
7. enable a manual/scheduled CI acceptance job pinned to the approved tool
   artifacts and upload the ignored evidence bundle.

Do not call the corpus conformance-ready until strict verification succeeds.

## Repeatable commands

```sh
cargo run --locked -- conformance check-tools
cargo run --locked --all-features -- generate \
  --profile all --out generated/conformance-all --seed 1
cargo run --locked --all-features -- validate generated/conformance-all
cargo run --locked --all-features -- conformance run \
  generated/conformance-all --out reports/conformance/all-seed-1
cargo run --locked --all-features -- conformance verify \
  reports/conformance/all-seed-1
```

Set `DTS_REAL_CONFORMANCE=1` when running the conditional real DCMTK RLE
integration test. Ordinary tests remain hermetic and need no external validator.

