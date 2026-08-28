# DICOM Coverage Expansion Implementation Plan

**Status:** proposed execution baseline

**Prepared:** 2026-08-26

**Applies to:** post-current-term corpus expansion

**Goal:** broaden the suite beyond the current Rust/DICOM-rs generation surface
without weakening determinism, standards evidence, or independent validation.

## 1. Outcome

The project will retain Rust as the suite orchestrator and native generator, but
will no longer require every case to be constructed by DICOM-rs. A versioned
generation-backend contract will allow deterministic external generators and
codecs to contribute Part 10 instances. Rust will remain authoritative for case
selection, deterministic identity inputs, manifests, validation orchestration,
reporting, and profile isolation.

The completed expansion should provide meaningful coverage in these areas:

- multi-instance geometry, sorting, and metadata encoding;
- additional clinical image families;
- quantitative, derived, SR, RT, and waveform objects;
- whole-slide microscopy and other visible-light objects;
- additional lossy, legacy, video, and encapsulation layouts;
- large valid stress cases;
- intentionally invalid and fuzzed files in isolated profiles;
- media interchange and, as a separate harness, DIMSE and DICOMweb behavior.

The target is not one example of every DICOM SOP Class. Coverage is selected by
viewer relevance, interoperability risk, independent validation feasibility,
and the ability to state an unambiguous expected result.

## 2. Baseline And Constraints

The current registry has 106 implemented logical cases across 21 SOP Classes.
Seventy cases use Secondary Capture Image Storage, so raw case count overstates
object-family breadth. The current all-features seed-1 baseline produces 108
files and passes the independent conformance framework recorded in
`conformance/README.md`.

All expansion work must preserve these existing contracts:

- valid profiles contain only standards-conformant files;
- `negative` and `fuzz` never enter `all` or another conformance profile;
- generated payloads and reports remain uncommitted build artifacts;
- every case has standards evidence and explicit expected semantics;
- deterministic UIDs, dates, ordering, seeds, and backend identities are
  controlled inputs;
- external commands are invoked directly, never through a shell;
- a generator is not its own independent conformance authority;
- unavailable tooling is reported rather than silently reducing coverage;
- viewer behavior does not define whether a generated object is valid.

No expansion phase may be declared complete merely because DICOM-rs can reopen
its own output.

## 3. Sequencing Principles

1. **Represent the gaps before filling them.** Planned registry rows and a
   coverage matrix must make omitted domains visible.
2. **Build shared infrastructure once.** External backends, provenance,
   semantic comparison, and profile selection precede backend-specific cases.
3. **Take low-dependency wins first.** Geometry and metadata cases can expand
   viewer coverage while the external backend substrate is being completed.
4. **Add vertical slices.** Each milestone includes recipes, generation,
   manifest data, validation, reports, tests, and documentation.
5. **Validate by risk.** Complex IODs require IOD/template validators;
   compressed pixels require an independent decoder; negative cases require an
   exact expected parser or validator outcome.
6. **Keep optional ecosystems optional.** Default smoke and core generation
   must remain local-friendly and must not require Python, Java, DCMTK, or video
   tooling.
7. **Measure axes, not volume.** Progress is reported by SOP Class, geometry,
   metadata, pixel, transfer syntax, encapsulation, and robustness axes rather
   than total file count alone.

## 4. Target Architecture

### 4.1 Rust-owned orchestration

Rust continues to own:

- CLI and profile resolution;
- the case registry and recipe version selection;
- deterministic UID, date/time, synthetic identity, and seed derivation;
- native DICOM-rs cases and project-owned encapsulation;
- backend discovery, locking, invocation, and timeout handling;
- output staging and atomic promotion into the run directory;
- manifest assembly, schemas, internal validation, reporting, and conformance
  dispatch.

The current generator should be separated by concern as it is touched. The
intended boundaries are `generator/native`, `generator/backends`,
`generator/recipes`, `generator/manifest`, and `generator/mutation`. This is an
incremental refactor; a large prerequisite rewrite is explicitly avoided.

### 4.2 Versioned external generation contract

Add a JSON request/response protocol, initially `0.1.0`. The Rust process
creates a private staging directory and invokes a configured backend with an
argument array. The request contains:

- case and recipe IDs and versions;
- selected profile, seed, and standards-lock identity;
- a staging output path;
- all pre-derived Study, Series, SOP Instance, and Frame of Reference UIDs;
- controlled patient, equipment, date, time, and timezone values;
- source-instance paths and reference roles;
- case-specific parameters and requested determinism level.

The response contains:

- generated relative paths and SOP Class/Instance UIDs;
- source and derived reference relationships;
- semantic expectations and expected frame/pixel hashes or tolerances;
- backend name, version, dependency-lock hash, and executable or environment
  fingerprint;
- warnings and an explicit generated, unavailable, or failed status.

Rust must reject path traversal, undeclared files, UID disagreement, missing
provenance, malformed responses, or output outside the staging directory. It
must reopen every returned Part 10 file before promotion.

### 4.3 Backend lock and policy

Add a committed backend policy artifact distinct from the transfer-syntax
matrix. Each backend entry records:

- stable backend ID and protocol version;
- implementation kind (`rust_native`, `python`, `java`, or
  `external_command`);
- discovery command and fixed arguments;
- dependency/version lock and fingerprint method;
- supported case families and platforms;
- determinism classification;
- independent validation requirements;
- redistribution and license notes.

Initial candidates are:

| Backend | Initial responsibility | Required independent check |
| --- | --- | --- |
| Rust/DICOM-rs | Native images, geometry, metadata, encapsulation | dicom3tools and DCMTK |
| highdicom/pydicom | Parametric Map, advanced SR, selected derived objects | dicom3tools and PixelMed where applicable |
| dcmqi | SEG, Parametric Map, TID 1500 cross-implementation cases | dicom3tools plus a backend-independent parser |
| DCMTK | RT, waveform/presentation utilities, selected legacy codecs | dicom3tools; never use the matching DCMTK decoder as sole pixel evidence |
| dcm4che | DICOMDIR, charset/JSON/network fixtures | dicom3tools and DCMTK |
| video codec adapter | MPEG-2, AVC/H.264, HEVC/H.265 codestreams | independent probe/decode plus DICOM IOD validation |

An external backend is an optional capability, not a mandatory development
environment dependency, unless a later decision explicitly promotes it.

## 5. Workstreams

The program is divided into six workstreams that converge at phase gates.

### A. Registry, standards, and reporting

- add planned rows for acknowledged gaps;
- define coverage dimensions and priorities;
- add backend and robustness fields to schemas;
- report coverage by meaningful axis and provider;
- maintain standards evidence and source notes.

### B. Generation platform

- split touched generator responsibilities into modules;
- implement the backend protocol and lock;
- stage and verify external output safely;
- capture reproducibility and provenance;
- support feature/capability-gated cases uniformly.

### C. Native breadth

- add geometry, series, metadata, and clinical image cases that DICOM-rs can
  express cleanly;
- reuse module and functional-group builders;
- avoid multiplying Secondary Capture cases where another IOD is more useful.

### D. Complex objects and payloads

- add quantitative, SR, RT, waveform, pathology, and video cases using the
  most suitable backend;
- preserve cross-object references and shared identity;
- independently validate templates, pixels, and payloads.

### E. Robustness and scale

- add Extended Offset Tables and large valid cases;
- implement deterministic byte-level mutation;
- isolate expected-invalid results from conformance results;
- add bounded fuzz seed corpora and reproducible minimization.

### F. Media and protocol harnesses

- generate DICOMDIR file sets and secure-media samples;
- exercise DIMSE association/storage/query/retrieve separately from file
  generation;
- exercise STOW-RS, QIDO-RS, and WADO-RS with recorded transactions;
- keep protocol results distinct from per-file conformance.

## 6. Phased Delivery

### Phase 0 — Make the roadmap executable

**Purpose:** establish honest coverage accounting and select the first vertical
slices before adding generators.

Tasks:

1. Add planned registry entries grouped into `now`, `next`, and `later`
   priorities for every domain in Section 7.
2. Extend the registry schema with a small, stable `provider` requirement and
   structured blocker codes; do not store host-specific paths.
3. Add a coverage-gap report that distinguishes logical cases, SOP Classes,
   modalities, object families, and compatibility axes.
4. Record a baseline report from the existing registry so later gains can be
   compared without relying on file count.
5. Select one native and one external proof case:
   - native: a CT series with spatial ordering that conflicts with Instance
     Number;
   - external: a floating-point Parametric Map derived from generated CT.

Gate:

- all acknowledged areas have visible planned or deliberately deferred rows;
- existing profile outputs and hashes are unchanged;
- schemas, list output, and reports represent provider requirements and gaps;
- no external runtime is required by the default test suite.

### Phase 1 — Shared platform and two proof slices

**Purpose:** prove the architecture without committing to a large polyglot
surface.

This phase has two parallel lanes after the schemas are accepted.

**Lane 1A: native geometry proof**

- add reusable series/geometry recipe types;
- generate a spatially ordered CT stack whose Instance Numbers disagree with
  geometric order;
- add expected sorting and spacing metadata;
- extend entity validation and reporting for multi-instance expectations.

**Lane 1B: external backend proof**

- implement protocol `0.1.0`, backend discovery, staging, timeouts, and
  fingerprints;
- provide a locked highdicom/pydicom development backend;
- generate a floating-point Parametric Map referencing the CT source series;
- validate Float Pixel Data, dimensions, real-world value mapping, references,
  decoded values, and deterministic semantics.

Gate:

- native and external cases use the same manifest/report pipeline;
- an absent external runtime produces an explicit unavailable row;
- malformed or malicious backend responses are rejected by tests;
- repeated runs satisfy declared byte or semantic determinism;
- dicom3tools validates both vertical slices with no unreviewed errors.

### Phase 2 — High-value native compatibility breadth

**Purpose:** deliver broad viewer value with minimal new operational
dependencies.

Implement these groups as separate milestones:

1. **Geometry and series:** gantry tilt, non-uniform spacing, duplicated and
   missing Instance Number, shared Frame of Reference across series, multiple
   series in one study, and temporal/dynamic frames.
2. **Metadata and VR:** UTF-8, selected ISO 2022 repertoires, PN component
   groups, DA/TM/DT and timezone boundaries, empty Type 2 attributes, long and
   multi-valued text/numeric strings, private creator blocks, and defined versus
   undefined sequence lengths where writer control is available.
3. **Clinical families:** one representative valid case each for Nuclear
   Medicine, PET, XA/XRF, multi-frame Ultrasound, and one additional enhanced
   modality selected by downstream viewer value.
4. **Pixels:** 32-bit integer pixels, 1-bit native pixels where permitted, ICC
   profile handling, and non-square spacing/aspect-ratio cases.

Status on 2026-08-27: milestones 1 through 3 are complete. The clinical-family
milestone closes with the independently conformant Enhanced PET multi-frame
representative. In milestone 4, the unsigned 32-bit native Secondary Capture
slice is complete with a case-specific `uv`-locked independent IOD/payload
validator. The 1-bit native Multi-frame Secondary Capture slice is also
complete with clean dicom3tools IOD/entity results and a locked DCMTK
independent frame decoder. ICC profile handling is complete with exact profile
bytes, strict DICOM header and label checks, and an operational LittleCMS
transform. Non-square spacing/aspect-ratio is complete as two mutually
exclusive Secondary Capture variants with `uv`-locked IOD and semantic
validation. All four dependency-ordered Phase 2 milestones are complete.

Gate:

- every group has focused internal tests and at least one independently
  validated generated corpus;
- geometry reports state the expected sort order rather than only listing tags;
- charset cases round-trip through at least two independent implementations;
- core remains within its documented runtime and size budget.

### Phase 3 — Quantitative, derived, SR, RT, and waveform breadth

**Purpose:** cover high-value objects for which domain constructors are more
reliable than handwritten generic datasets.

Deliver independent vertical slices in this order:

1. Parametric Map: integer, 32-bit float, and 64-bit float variants.
2. TID 1500 Measurement Report referencing source images and SEG or Parametric
   Map; then Comprehensive 3D SR with SCOORD3D.
3. Spatial Registration and Deformable Spatial Registration.
4. Color Softcopy, Advanced Blending, and Blending Presentation States.
5. Twelve-lead ECG waveform and one additional representative waveform.
6. RT Plan and RT Image linked to existing RT Structure Set and RT Dose;
   evaluate a minimal current RT Radiation Set slice after those references are
   proven.
7. Surface Segmentation or Encapsulated STL as the first mesh-oriented case.

Each slice must choose and record one primary generator and at least one
independent validator. Similar outputs from two backends are useful
cross-implementation cases, but do not count as independent cases unless their
semantic intent differs.

Gate:

- all source/derived references resolve within the corpus;
- template-driven SR passes PixelMed and primary IOD validation;
- quantitative pixel values are compared after independent decode;
- reports distinguish renderable, metadata-only, annotation, and recognized-
  unsupported expectations.

Status on 2026-08-27: Phase 3 milestones 1 through 4 are complete. Spatial and
Deformable Spatial Registration are byte-stable native vertical slices with
two-run reproducibility, exact source/transform/grid/reference closure, strict
manifest-driven validation, registration-specific reports, clean locked
primary IOD validation, independently implemented `uv`-locked secondary IOD
validation, independent parsing, and clean isolated entity-reference
validation. In milestone 4, Color Softcopy Presentation State is complete as a
byte-stable native vertical slice with automatic cross-profile RGB source
materialization, exact reference/display/ICC closure, strict manifest-driven
validation, report coverage, clean locked primary and `uv`-locked secondary
IOD validation, independent parsing, and silent isolated entity-reference
validation. Advanced Blending Presentation State is also complete as a
byte-stable native vertical slice with automatic four-CT source
materialization, exact two-input and final-display graph closure, mirrored
Common Instance References, strict manifest-driven validation, report
coverage, clean independent parsing, zero-error `uv`-locked secondary IOD
validation, and silent isolated entity-reference validation. Its two
contradictory `dciodvfy` Frame-of-Reference findings remain visible and
unallowlisted against the locked mandatory module contract. Blending Softcopy
Presentation State completes milestone 4 as a byte-stable native vertical
slice over four CT sources in two registered Series, with exact
underlying/superimposed positions, complete ordered references, per-item
rescale, relative opacity, global displayed area, mandatory palette and ICC
payloads, strict manifest-driven validation, dedicated report coverage, clean
locked primary IOD validation and independent parsing, zero-error
`uv`-locked secondary IOD validation, and silent isolated entity-reference
validation. Two seed-7 extended roots each contained 101 strictly valid files
with byte-identical manifest and Blending instance hashes; integrated
conformance kept `accepted_findings` at zero and Blending added no finding.
Milestone 5 is now in progress: Twelve-lead ECG Waveform Storage is complete
as a byte-stable native extended-profile slice with one ordered 12-channel,
500-sample, 500 Hz group, an exact deterministic 12,000-byte signed waveform
payload, typed manifest semantics, strict formula and interleave validation,
dedicated reports, and locked independent IOD and payload validation. Two
seed-7 extended roots each contained 102 strictly valid files with
byte-identical manifest and ECG instance hashes; integrated conformance kept
`accepted_findings` at zero and the waveform added no finding. General ECG
Waveform Storage completes milestone 5 as a byte-stable native slice with two
ordered heterogeneous groups (`12x1000@250Hz; 4x4000@1000Hz`), sixteen
channels, a 56,000-byte ordered payload aggregate, typed manifest/report
closure, strict validator-owned group arithmetic, and locked independent IOD
and raw waveform validation. Two seed-7 extended roots each contained 103
strictly valid files with byte-identical manifest and General ECG instance
hashes; integrated conformance kept `accepted_findings` at zero and General
ECG added no finding. Phase 3 milestone 5 is complete. In milestone 6, linked
RT Plan and RT Image are complete as byte-stable native slices over the
existing RT Structure Set and RT Dose graph. Two promoted seed-7 extended
roots each contain 105 strictly valid files and have byte-identical Plan,
Image, and manifest bytes. Both IODs pass locked `dciodvfy` and a separately
implemented `uv`-locked secondary IOD route; the Image also passes exact
independent native-pixel decoding. Strict validation closes the Image over the
generated Plan digest and identity, and integrated conformance adds no linked
RT finding or accepted finding. The explicit milestone-6 decision checkpoint
is now authorized. Standards review requires the minimal RT Radiation Set to
be implemented with a registered C-Arm Photon-Electron Radiation companion;
their paired native generation, strict graph closure, and corrected
`uv`-locked independent IOD route are now qualified. Two seed-7 extended
roots each contain 107 strictly valid files with byte-identical Radiation,
Set, and manifest bytes. Both exact IOD routes report zero errors, DCMTK parses
both files, integrated conformance accepts no finding, and the registry now
contains 143 implemented and 39 planned cases. `dcentvfy` visibly reports the
current SOP Classes as unrecognized, so strict Rust retains graph ownership.
Phase 3 milestone 6 is complete.

### Phase 4 — Pathology and tiled microscopy

**Purpose:** establish a small but semantically complete WSI corpus before
attempting large slides.

Milestones:

1. VL Endoscopic and VL Microscopic single-frame cases.
2. Small VL WSI `TILED_FULL` volume with specimen, optical path, slide label,
   total pixel matrix, and plane-position metadata.
3. `TILED_SPARSE` counterpart with deliberately absent tiles.
4. Multi-resolution pyramid with thumbnail and label instances.
5. Multiple optical paths or focal planes.
6. SEG, Parametric Map, or annotation object referencing WSI tiles.

Tile source images must be deterministic synthetic patterns. The project should
not add identifiable pathology fixtures or commit generated slides.

Gate:

- frame-to-total-pixel-matrix mapping is independently reconstructed and
  compared;
- tiled full/sparse expectations are present in the manifest;
- small WSI remains in `extended`; pyramid and large-slide cases are placed in
  `stress` according to measured output size and runtime.

Milestones 1 and 2 are complete. VL Endoscopic and direct-patient VL
Microscopic are native byte-stable extended-profile slices with exact LUNG/R
and EYE/R anatomy/laterality contracts, empty Acquisition Context, planar-0 2
by 2 RGB, and explicit specimen, optical-path, ICC, frame-count, and
frame-of-reference absences. Two seed-7 extended roots each contain 109
strictly valid files and have byte-identical manifests and instances. Locked
`dciodvfy` and the authorized `uv`-locked secondary IOD route report zero
errors for both exact SOP Classes; DCMTK parses both, and independent binary P6
plus native OB extraction reconstructs the exact 12-byte RGB payload.
Integrated conformance accepts no finding, while unrelated whole-corpus
failures remain visible. The
single native VL Whole Slide Microscopy Image in milestone 2 adds a byte-stable
small `TILED_FULL` VOLUME with four 2 by 2 RGB Frames, a 4 by 4 total pixel
matrix, and locked specimen, optical-path, slide-label, physical-geometry, and
implicit tile-order contracts. Two seed-7 extended roots each contain 110
strictly valid files and have byte-identical manifests and WSI instances.
Locked `dciodvfy` and the authorized `uv`-locked secondary IOD route report
zero errors for the exact WSI SOP Class, DCMTK parses the instance cleanly,
and an isolated, non-generation `uv`-locked highdicom/pydicom route
independently reconstructs all implicit positions and the exact total pixel
matrix. Integrated case-scoped conformance accepts no finding. The 229
unrelated whole-corpus conformance failures remain visible and unallowlisted.
The registry now contains 146 implemented and 36 planned cases. Milestone 3,
the deliberately incomplete `TILED_SPARSE` counterpart, is complete as a
native byte-stable extended-profile slice. It encodes two diagonal tiles and
two deliberate absences with exact dimension indices, per-frame positions,
payload and occupancy contracts. Two seed-7 extended roots each contain 111
strictly valid files and compare byte-for-byte. The authorized case-specific
`uv`-locked dicom-validator reports zero IOD errors, DCMTK parses the instance,
and the isolated `uv`-locked highdicom adapter independently reconstructs the
exact sparse matrix. dicom3tools' incompatible full-grid cardinality result
remains visible unallowlisted characterization, and the 229 unrelated
whole-corpus failures remain visible. The registry then contained 147
implemented and 35 planned cases. Milestone 4 is complete as the opt-in,
native, byte-stable `vl/wsi/pyramid_multiresolution` stress slice. Its one
logical case emits an ordered VOLUME, THUMBNAIL, and LABEL group totaling three
instances, six Frames, and 8,694 bytes. Two seed-7 stress roots passed strict
validation for all three instances and were byte-identical as complete trees;
generation completed in 0.55 and 0.59 seconds, below the locked five-second
ceiling. Locked `dciodvfy` and the independent `uv`-locked dicom-validator
reported zero IOD errors for every role, while the isolated `uv`-locked
highdicom 0.28.1/pydicom 3.0.2 adapter version 0.3.0 reconstructed and bound
the exact three-member group. Integrated run
`0188fc12678acf82e29f27c139d531dd060ec8e2f36363c9927d4d673d869f6d`
records zero entity findings, passing independent pixel evidence, zero
accepted findings, and zero verification failures against an empty
exact-slice findings set. The registry then contained 148 implemented and 34
planned cases. Ordinary `all` remains unchanged because stress cases require
explicit selection. Milestone 5 is complete as the native byte-stable
`vl/wsi/multiple_optical_paths` extended slice. It encodes eight path-major
Frames over ordered `BRIGHTFIELD` and `ALTERNATE` paths and reconstructs two
separate 4 by 4 by 3 matrices. Two seed-7 extended roots each contained 112
strictly valid files, completed in 4.50 and 4.54 seconds, and compared
byte-for-byte. Locked `dciodvfy`, the authorized `uv`-locked dicom-validator,
and DCMTK parsing all passed. The isolated highdicom 0.28.1/pydicom 3.0.2
adapter version 0.4.0 reproduced the exact aggregate, per-Frame, per-path,
matrix, and nested ICC evidence while rejecting ambiguous unfiltered matrix
access. Exact-slice run
`c2203223e9d8ce0b716175329769b7f3bb947ac48da44a510843d5a82d8b3dcc`
has silent entity validation, passing independent pixels, zero accepted
findings, and zero verification failures. The registry now contains 149
implemented and 33 planned cases. Phase 4 milestone 6, a derived object
referencing WSI tiles, is next. Selecting dimensions, budgets, and CI
scheduling for a full-size pyramid remains the plan's explicit decision
checkpoint, and neither small-slice promotion authorizes it for ordinary CI.

Milestone 6 is complete as the semantic-stable external-backend
`derived/seg/wsi_tile_reference` extended slice. Its two FRACTIONAL OCCUPANCY
Frames reference exactly Frames 1 and 4 of the small `TILED_FULL` source and
reconstruct a 4 by 4 `TILED_SPARSE` matrix. Two seed-7 extended roots each
contained 113 strictly valid files and produced the same semantic projection
and byte-identical 4,140-byte SEG. Backend invocation took 448 and 387
milliseconds, within the locked five-second and 16 KiB ceilings. Locked
`dciodvfy`, the authorized independent `uv`-locked dicom-validator 0.8.2 route,
DCMTK parsing, strict Rust graph and matrix validation, and independent source
WSI pixel evidence all passed. Exact-slice run
`973749c21773cd7e66aae6c8377600f4a7ca839c4d88ec0fe45b256e9684c9bf`
has zero accepted findings and zero verification failures. The registry now
contains 150 implemented and 32 planned cases.

Full-corpus run
`d9252ca2fa2edaad6d2c445f5cb0076a0a2b7558355ca9635a228bd0d3ca037d`
retains zero accepted findings and exposes 220 pre-existing or unrelated
verification failures. All six Phase 4 milestones are complete. Ordinary
`all` profile semantics remain unchanged: it includes promoted extended cases
but does not opt into the stress-only full-size pyramid. No unavailable route
or unrelated whole-corpus finding has been hidden. The next action is the
explicit full-size-pyramid decision checkpoint: dimensions, resource budgets,
and ordinary-CI scheduling require project authorization before implementation
proceeds.

### Phase 5 — Encapsulation, lossy codecs, and video

**Purpose:** close the highest-value remaining transfer-syntax and frame-layout
gaps without making codec availability implicit.

Order:

1. Extended Offset Table and Extended Offset Table Lengths, first with small
   synthetic frames and simulated overflow checks, then with a genuine large
   stress object.
2. JPEG-LS Near-Lossless with a declared maximum sample error and lossy metadata.
3. JPEG 2000 lossy and JPEG XL lossy with metric/tolerance policies.
4. JPEG Extended 12-bit only after an independent decoder is proven.
5. HTJ2K lossy variants where encoder and decoder independence is available.
6. MPEG-2, H.264/AVC, and H.265/HEVC, starting with one short deterministic
   monochrome or color cine loop per family.

Lossy acceptance must define per-channel maximum error and an aggregate metric;
visual similarity alone is insufficient. Video validation must separately
check the elementary stream, DICOM encapsulation, frame/time metadata, and
independent decode.

Gate:

- unavailable codecs remain explicit in manifests and reports;
- independent decoding and tolerance checks exist for every promoted codec;
- external codec versions and executable fingerprints are locked;
- no lossy or video case is promoted to `core` without a separate policy
  decision.

Status on 2026-08-28: milestone 1 is complete for both EOT evidence forms that
do not require a large allocation. The promoted RLE instance has exact EOT
Values `[0, 78, 152]` and Lengths `[69, 66, 69]`; the virtual
`0x1_0000_0006` crossing remains an honest non-file qualification fixture.
Milestone 2 is complete for the selected JPEG XL and HTJ2K lossy cases. Their
fixed encoders, executable fingerprints, independent full-frame decoders,
numeric metrics, lossy metadata, manifest validation, and report projections
are locked. A feature-enabled 120-file extended corpus passed strict validation
with zero failures. JPEG-LS Near-Lossless, JPEG 2000 lossy, JPEG Extended
12-bit, and all three video families remain explicit unavailable coverage under
their independent-tool blockers. A genuine large EOT object remains stress-
only. See `docs/phase-5-encapsulation-status.md` and
`docs/phase-5-lossy-status.md`.

Decision update on 2026-08-28: the shared lossy thresholds, reduced/full stress
envelopes, optional DCMTK-first media/protocol baseline, dcm4che independent-
peer target, and repository-owned synthetic PKI fixtures are approved. Exact
values and scope are locked in
`docs/coverage-expansion-decisions-2026-08-28.md`. These approvals remove the
policy checkpoints but do not promote a case until its complete vertical gate
passes.

### Phase 6 — Stress and large-object behavior

**Purpose:** test resource and offset behavior without slowing normal work.

Add parameterized, opt-in recipes for:

- high instance-count studies;
- many-frame enhanced and cine objects;
- large WSI pyramids;
- large encapsulated frames and multi-fragment layouts;
- concatenations and EOT offsets crossing 32-bit Basic Offset Table limits;
- deeply nested but valid sequences;
- long-value and large bulk-data handling.

Each recipe has explicit byte, frame, instance, memory, and runtime budgets.
CI runs reduced boundary variants; scheduled or release jobs run full sizes.

Gate:

- `stress` is functional and excluded unless explicitly selected;
- generation is streaming or bounded where practical;
- interruption leaves no promoted partial run;
- reports record requested and actual scale parameters.

Status on 2026-08-28: the reduced-scale gate is complete. Seven promoted
stress cases emit seven isolated `stress_case_run` qualifications. A final
seed-7 root contained 139 DICOM files and 160,213,322 qualified stress-case
bytes; its summed case time was 19,376 ms, peak RSS was unavailable on the
platform, and strict validation checked all files with zero failures. Two
repeated roots contained byte-identical DICOM files. The encapsulated case uses
64 Fragments per Frame, 16,384 total, and its 64 MiB measurement is native
decoded Pixel Data. These built-in checks are same-project evidence, not
independent conformance. Every qualification explicitly records the `full`
scale as unavailable under `full_scale_runner_unimplemented`; no full-scale
job was added to ordinary CI. See `docs/phase-6-stress-status.md`.

### Phase 7 — Negative and fuzz profiles

**Purpose:** test parser robustness using reproducible failures that conforming
writers cannot naturally emit.

Implement a mutation layer that starts from a known-good case and applies one
named mutation after Part 10 writing. Initial deterministic mutations are:

- truncated file meta, dataset, sequence, item, fragment, and pixel value;
- incorrect explicit-VR length fields and illegal VR bytes;
- transfer-syntax mismatch between file meta and dataset encoding;
- SOP Class/Instance disagreement between file meta and dataset;
- missing Type 1 attributes;
- invalid Bits Stored/High Bit and pixel byte length;
- broken Basic or Extended Offset Tables;
- undefined length without delimitation and invalid nested item lengths;
- invalid character-set declarations and malformed encoded text.

Every negative case records its valid source hash, mutation ID and parameters,
byte offsets changed, expected failure layer, and acceptable bounded outcomes.
Crashes, hangs, timeouts, and unbounded resource usage are never acceptable.

After deterministic mutations are stable, add a bounded fuzz harness with a
small committed seed description, reproducible RNG seeds, automatic
minimization, and promotion of valuable minimized inputs into named negative
recipes. Fuzz-generated DICOM payloads remain uncommitted.

Gate:

- `negative` and `fuzz` cannot be selected through `all`;
- conformance failure is the expected result and is not counted as a suite
  failure when it matches the case contract;
- timeouts and crashes are reported distinctly from clean rejection;
- every promoted regression is reproducible from recipe plus seed.

Status on 2026-08-28: the deterministic mutation milestone is complete. All
15 registered negative cases generate only under `negative`, two seed-7 roots
were byte-identical, and strict validation checked 15 files with zero contract
failures. Normal coverage rows remain empty for the profile; negative outcomes
are reported separately and the built-in classifier is labeled same-project.
Bounded DCMTK and dicom3tools exercises produced no timeout, crash, or signal.
DCMTK safely ignored the corrupt Extended Offset Table in one case, an explicit
interoperability result recorded without treating that tool as independent
proof of the suite's classifier. The bounded `fuzz` profile is also complete:
two committed seed descriptions resolve to private generated sources, 64
seed-7 candidates produce deterministic outcome and minimization evidence, and
the promoted root retains no DICOM payloads. Strict validation and separate
JSON/Markdown reporting keep fuzz evidence out of both valid and negative
coverage. Phase 7 is complete. See `docs/phase-7-negative-status.md` and
`docs/phase-7-fuzz-status.md`.

### Phase 8 — Media and protocol interoperability

**Purpose:** supplement file compatibility with interchange behavior while
keeping results separable.

Milestones:

1. DICOMDIR file sets containing mixed image, derived, and non-image objects.
2. Secure DICOM media samples and digital-signature verification where a
   maintainable independent toolchain exists.
3. DIMSE association negotiation matrix, followed by C-STORE and C-ECHO.
4. C-FIND, C-GET, and C-MOVE scenarios with deterministic expected responses.
5. STOW-RS, QIDO-RS, and WADO-RS transaction scenarios, including multipart,
   metadata, frame, and bulk-data responses.
6. TLS and user-identity negotiation as an opt-in security suite.

Protocol scenarios consume generated cases but produce transaction reports, not
additional file-conformance rows. Server implementations must be replaceable so
that the harness does not define conformance by testing only itself.

Gate:

- file, media, DIMSE, DICOMweb, and security outcomes are reported separately;
- all peers and servers are fingerprinted;
- captured logs contain only synthetic identities and deterministic request
  data;
- protocol failures link back to stable case IDs and transaction IDs.

Status on 2026-08-28: the Phase 8 interoperability substrate is complete at
the currently available independent-tool boundary. A fresh 115-file extended
seed-7 root supplied Enhanced CT, binary SEG, and General ECG members to the
bounded DICOMDIR runner; DCMTK `dcmmkdir` 3.7.0, the Rust closure walk,
`dciodvfy`, `dcentvfy`, and a same-provider DCMTK parser check all passed with
zero warnings after the SEG Study ID correction. The dcm4che peer remains
unavailable, so that media result is explicitly non-promotable. Dedicated
media and transaction schemas plus JSON/Markdown CLI commands keep these
results separate from file coverage. DIMSE, DICOMweb, and TLS/user-identity
each emit a deterministic unavailable transaction with a precise blocker;
reports include only public synthetic-PKI fingerprints. Secure media and
digital signatures also remain explicitly unavailable pending independent
creator/verifier toolchains. See `docs/phase-8-interoperability-status.md`.

## 7. Planned Coverage Inventory

Phase 0 should create planned registry rows for at least these groups. Exact
case IDs and SOP Class UIDs require standards verification before commit.

| Group | Initial representatives | Delivery phase |
| --- | --- | --- |
| Geometry | sorting conflict, gantry tilt, non-uniform spacing, multi-series FoR | 1-2 |
| Metadata | UTF-8, ISO 2022, PN, timezone, private blocks, sequence lengths | 2 |
| Modalities | NM, PET, XA/XRF, multi-frame US, selected enhanced modality | 2 |
| Quantitative | integer/float/double Parametric Map | 1, 3 |
| Registration | spatial and deformable registration | 3 |
| Presentation | color, blending, advanced blending | 3 |
| SR | TID 1500 and Comprehensive 3D | 3 |
| RT | RT Plan, RT Image, evaluated RT Radiation Set | 3 |
| Waveform | 12-lead ECG and one additional waveform | 3 |
| Mesh | surface segmentation or Encapsulated STL | 3 |
| Visible light | endoscopic, microscopic, WSI full/sparse/pyramid | 4 |
| Encapsulation | EOT/EOT Lengths and large offsets | 5-6 |
| Compression | near-lossless/lossy JPEG families and video | 5 |
| Scale | large WSI, frame counts, instances, nesting, bulk data | 6 |
| Robustness | named structural and semantic mutations | 7 |
| Media/protocol | DICOMDIR, DIMSE, DICOMweb, optional security | 8 |

Planned rows must state why a case matters and what blocks it. They must not be
promoted because a backend merely emitted a parseable file.

## 8. Acceptance Contract For Every Vertical Slice

A case milestone is complete only when all applicable items below are present:

1. standards evidence or a source note;
2. registry entry, stable case ID, profile, recipe version, provider, and
   determinism declaration;
3. deterministic recipe and synthetic payload source;
4. generated Part 10 file or deliberately isolated negative output;
5. manifest semantics, relationships, hashes, and provenance;
6. internal structural and semantic validation;
7. independent IOD validation;
8. independent pixel, waveform, mesh, document, or video payload validation;
9. schema and CLI tests;
10. JSON and Markdown report coverage;
11. two-run reproducibility evidence appropriate to the determinism level;
12. documentation of feature gates, external dependencies, and known limits.

If no independent validator supports an otherwise valuable object, retain it as
planned or experimental and record the blocker. Do not weaken the general gate.

## 9. Efficient Parallelization And Merge Order

The safe dependency and concurrency structure is:

```text
Phase 0 registry/schema baseline
              |
              v
Phase 1 backend protocol + native geometry proof
       |                         |
       v                         v
Phase 2 native breadth      Phase 3 complex objects
       |                         |
       +------------+------------+
                    v
             Phase 4 pathology
                    |
                    v
         Phase 5 codecs and video
                    |
                    v
             Phase 6 stress

Phase 7 negative/fuzz may start after Phase 1 staging and manifest contracts.
Phase 8 media/protocol may start after Phase 1 identity and backend provenance.
Their promotion gates remain independent from valid-file corpus expansion.
```

Within a phase, parallel work should operate on different files or modules and
converge only after shared schemas land. Registry/schema changes merge first,
then infrastructure, then one case family per commit. Avoid parallel edits to
the current monolithic `src/generator.rs`; create the target module boundary
before assigning independent case-family work.

## 10. Commit And Review Units

Follow the repository commit policy with one coherent unit per commit. A normal
vertical slice will use several commits, for example:

1. `docs(standards): record parametric map generation evidence`
2. `feat(types): add external generation backend contract`
3. `feat(generator): invoke fingerprinted external backends`
4. `feat(parametric-map): generate floating-point derived map`
5. `test(parametric-map): verify references values and determinism`
6. `docs(corpus): document parametric map capability`

Do not combine backend infrastructure, several SOP Classes, codec changes, and
report changes in one commit. Do not amend a commit once recorded in the
project history.

## 11. Decision Checkpoints

Pause for an explicit project decision at these points:

- before making Python or Java mandatory for any existing profile;
- before selecting a long-term dependency/runtime manager for external
  backends;
- before accepting a generator without an independent IOD validator;
- before promoting a lossy codec without a numeric tolerance policy;
- before committing certificates, keys, or cryptographic fixtures;
- before adding a full-size stress job to ordinary CI;
- before changing the meaning or inclusion rules of `all`;
- before treating protocol conformance as part of file-corpus completion.

## 12. First Execution Milestone

The next implementation milestone should be Phase 0 followed by the two Phase 1
proof slices. Its concrete deliverables are:

- planned coverage inventory and coverage-gap reporting;
- external backend schema and lock format;
- safe backend staging/invocation with fake-backend contract tests;
- one native CT sorting-conflict series;
- one external floating-point Parametric Map referencing that source series;
- independent validation, reports, and reproducibility evidence for both.

This milestone tests the architectural assumptions at low corpus volume. If it
succeeds, the remaining phases can add object families in parallel without
reworking identity, provenance, manifest, or validation contracts.
