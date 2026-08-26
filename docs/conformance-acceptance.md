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

It also used the upstream universal macOS dicom3tools snapshot
`1.00.snapshot.20260803085716`, installed under a versioned Homebrew prefix:

- `dciodvfy` SHA-256
  `1aeb75d6ccd3f193e3b322b6da77742cdce2e0604868eaf2a2669c786cbc27e5`;
- `dcentvfy` SHA-256
  `1b96e598f28f66deee1bfc1cb52ff460c316ab6b0625dae575d701f20c836e2c`.

The binary archive SHA-256 is
`c1d1feb60a1b206862c52db5a4e3115987c134467332f3397957050e4a83e5e1`;
the matching source archive SHA-256 is
`7e8937c2e8a5c61fd9ec1a0405330782abf41ba18efa459327de5c5071be0160`.
The matching BSD `COPYRIGHT` is installed beside the executables.

SR-specific validation used PixelMed release `20260608` with Oracle Java
25.0.3. The composite adapter SHA-256 is
`f10b9b06f8d665af35c17af915f0544f21757f8fe51d3cbf2f69d431b5834f50`;
it binds the Java executable and the exact PixelMed, Commons Codec, Saxon HE,
and XML Resolver JAR hashes recorded in `conformance/validator-lock.json`. The
upstream binary, dependency, and source archive hashes are also locked there.

Results on the 108-instance all-features corpus:

- independent parser completed for 108/108 manifest files;
- independent RLE decode matched every expected frame hash for 58/58 RLE
  instances, including 8/16-bit, signed/unsigned, monochrome/color,
  planar/interleaved, and single/multi-frame cases;
- primary IOD validation completed for 108/108 instances;
- one instance had no primary finding and 107 had at least one finding;
- primary validation reported 200 errors and 72 warnings;
- corpus entity validation completed and reported 13 errors and three warnings;
- PixelMed completed SR IOD/template validation for all three SR instances;
- the Key Object Selection instance passed PixelMed cleanly after its root was
  identified as DCMR TID 2010 and its Row 8 IMAGE Concept Names were removed;
- Basic Text and Comprehensive SR produced seven warnings because those generic
  recipes intentionally do not claim a named root template; the warnings have
  exact `generator_intent_confirmed` dispositions citing PS3.3 C.18.8.1.2;
- strict verification accepted all seven PixelMed warnings and still reported
  the original 288 primary/entity findings unresolved; and
- the separate legacy instance completed primary validation with one unresolved
  `Laterality` Type 2C error.

The generated evidence bundle remains an ignored local artifact and was not
committed.

## Exact blocker

The arm64 macOS acquisition blocker is resolved. Acceptance is now blocked on
standards-led finding triage. The first run exposed clear generator defects,
including incorrect VR/value use for `FieldOfViewDimensions`, missing required
or conditional attributes, and inconsistent study-level entity values. It also
reported findings that may reflect validator definition gaps, such as rejecting
multi-frame Secondary Capture and warning about current attributes as absent
from the validator's IOD definitions.

Next actions:

1. group findings by affected recipe and DICOM requirement;
2. fix generator defects before considering exact finding dispositions;
3. investigate the two entity parse failures by exact transfer syntax;
4. reconcile the validator's compiled definition snapshot with the project
   2026b standards lock;
5. confirm every generated SOP Class and transfer syntax is recognized;
6. enable a manual/scheduled CI acceptance job pinned to the installed tool
   artifacts and upload the ignored evidence bundle.

Do not call the corpus conformance-ready until strict verification succeeds.

## Repeatable commands

```sh
cargo run --locked -- conformance check-tools
cargo run --locked --all-features -- generate \
  --profile all --out generated/conformance-all --seed 1
cargo run --locked --all-features -- validate generated/conformance-all
DTS_PIXELMED_HOME=/path/to/pixelmed-20260608 \
  cargo run --locked --all-features -- conformance run \
    generated/conformance-all --out reports/conformance/all-seed-1
cargo run --locked --all-features -- conformance verify \
  reports/conformance/all-seed-1
```

Set `DTS_REAL_CONFORMANCE=1` when running the conditional real DCMTK RLE
integration test. Ordinary tests remain hermetic and need no external validator.
