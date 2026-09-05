# Case Taxonomy and Profiles

This document defines the committed naming and profile rules used by
`cases/registry.json`, generated manifests, coverage reports, and the
`list-cases` command.

Composition template IDs are a separate namespace. They identify qualified
object contracts in `templates/catalog.json`; they are not case IDs, do not
belong to profiles, and never add registry coverage. For current composition
support, query `templates list` or `templates describe`, then use the generated
composition manifest as the authority for what a particular spec emitted.

Every implemented registry row binds exactly one `recipe_id` plus
`recipe_version` from the modular documents under `cases/recipes/`. Static
scenario differences belong in those schema-validated documents; bounded Rust
plan providers are named only for algorithmic construction. A binding resolves
to a plan before any retained file is written. This recipe namespace is also
distinct from both case IDs and caller-visible template IDs.

These naming conventions describe the committed registry. The documented
caller-defined capabilities use complete typed contracts rather than interpreting
a caller's case-name segments. External file profile membership is bound to the
captured corpus definition; a historical-looking name does not grant stress or
other qualification evidence. Schema identity and path rules still apply. The
bounded native PET capability likewise preserves its fixed synthetic activity
and metadata tuple while accepting caller identities, orders and safe paths;
its local standards note remains part of the submitted bundle. See the
[PET contract](../docs/generation-guide.md#caller-defined-native-pet).

The bounded native XA/XRF capabilities also accept caller identities, unique
orders and safe paths under their fixed synthetic projection contracts. Both
local source notes remain in the submitted bundle; naming does not add cine,
calibrated geometry or enhanced-object evidence. See the
[XA/XRF contract](../docs/generation-guide.md#caller-defined-native-xa-and-xrf).

The two fixed native photographic contracts accept caller case/recipe IDs,
unique planning/projection orders and safe paths. Their four-member fixture
contains only a descriptor, registry and two recipes; no note or asset is
required. Canonical selector/ledger order is palette then RGB, while recipe
orders emit RGB then palette. These naming freedoms preserve the source-fixed
2×2 pixel and metadata contracts; they do not extend photographic composition
beyond its qualified RGB8 default. See the
[photographic contract](../docs/generation-guide.md#caller-defined-native-photographic-images).

The bounded native MR contract accepts caller case/recipe IDs, metadata, MR
acquisition values, logical IDs, roles, unique orders and paths, geometry and
U16 pixels for a consistent multi-instance series. Those freedoms are admitted
by the complete typed capability, never by an MR-looking name, and do not widen
RLE, enhanced/multiframe, independent-conformance or viewer evidence. See the
[MR contract](../docs/generation-guide.md#caller-defined-native-mr-series).

The bounded native US multiframe contract likewise accepts caller names,
metadata, identities, timing, orders, paths and ordered U8 frames only when the
complete typed template/content/algorithm/projection tuple is present. A
US-looking name grants neither profile membership nor historical, codec,
independent-conformance or viewer evidence. See the
[US contract](../docs/generation-guide.md#caller-defined-native-ultrasound-multiframe).

The bounded native Nuclear Medicine multiframe contract accepts caller names,
metadata, identities, spacing, dimension sequences and vectors, detector
geometry, timing and U16 pixels only through the complete typed tuple. An
NM-looking name grants no profile, codec, clinical, independent-conformance or
viewer evidence. See the
[NM contract](../docs/generation-guide.md#caller-defined-native-nuclear-medicine-multiframe).

## Case ID Format

Case IDs are stable, human-readable, path-safe identifiers:

```text
<domain>/<iod_family>/<descriptor>
```

Use lowercase ASCII letters, digits, underscores, and hyphens. Do not use
spaces, uppercase letters, file extensions, viewer names, generated file hashes,
or transient implementation details.

The path order is canonical. Do not invert segments such as `ct/classic/...`
when the normalized form is `classic/ct/...`.

## Domains

- `classic`: classic single-frame image IODs and related legacy single-frame
  image behavior.
- `enhanced`: enhanced multi-frame image IODs.
- `derived`: derived image, segmentation, presentation state, structured
  report, key object, and related reference objects.
- `vl`: visible light, photographic, endoscopic, microscopic, and whole-slide
  imaging objects.
- `non-image`: waveform, radiotherapy, encapsulated document, and other
  non-image objects.
- `geometry`: multi-instance ordering, spacing, tilt, and Frame of Reference
  relationships that are best described as study/series behavior.
- `metadata`: character set, date/time, sequence-length, private-creator, and
  value-boundary cases whose primary axis is not a new IOD family.
- `encapsulation`: encapsulated Pixel Data offset-table and Fragment-layout
  behavior.
- `negative`: deterministic expected-invalid mutations, organized by failure
  axis rather than by a valid IOD family.
- `stress`: valid reduced-scale resource-boundary cases.
- `fuzz`: bounded runtime robustness qualifications; these need not retain a
  DICOM artifact.
- `qualification`: non-instance arithmetic or substrate qualifications that
  record evidence without pretending to be a DICOM SOP Instance.
- `media`: DICOM File-set and instance-security scenarios.
- `protocol`: DIMSE, DICOMweb, TLS, and user-identity transactions.
- `video`: encapsulated video transfer-syntax cases.

## IOD Family Segments

Use the shortest unambiguous lowercase segment for the IOD family or object
family. Initial segments include:

- `sc`, `ct`, `mr`, `cr`, `mg`, `dx`, `us`
- `seg`, `presentation-state`, `sr`, `rwvm`
- `photo`, `endoscopic`, `microscopic`, `wsi`
- `rt`, `waveform`, `encapsulated-document`

## Descriptor Conventions

Descriptors should name the compatibility axis under test rather than the
implementation strategy. Prefer ordered tokens for:

1. image/object variant;
2. photometric interpretation or color organization;
3. sample type and bit depth;
4. notable semantic behavior;
5. transfer syntax.

Examples:

```text
classic/sc/mono2_u8_explicit_le
classic/sc/mono1_u8_explicit_le
classic/sc/rgb_planar0_explicit_le
classic/ct/mono2_i16_rescale_12bit_explicit_le
classic/mg/for_presentation_mono1_u16_12bit_explicit_le
classic/mg/for_processing_mono2_u16_12bit_implicit_le
classic/cr/overlay_modality_voi_explicit_le
classic/mr/multislice_oblique_explicit_le
enhanced/ct/multiframe_shared_perframe_explicit_le
enhanced/ct/concatenation_two_part_explicit_le
derived/seg/binary_multiframe_explicit_le
vl/photo/rgb_planar0_explicit_le
vl/photo/palette_color_explicit_le
```

## Profiles

Profile membership is explicit in `cases/registry.json`; do not infer it only
from the case ID.

- `smoke`: fastest sanity set; only small, byte-stable files; no optional
  external codecs required.
- `core`: common valid viewer-relevant cases; local-friendly size and runtime.
- `extended`: broader valid coverage, including enhanced multi-frame and
  derived objects.
- `legacy`: valid retired or uncommon behavior, excluded from `core`.
- `stress`: valid but large, slow, or expensive cases; explicit opt-in only.
- `all`: includes `smoke`, `core`, and `extended`; excludes `legacy` and
  excludes `stress` unless `--include-stress` is passed.
- `negative`: deterministic invalid or malformed files with mutation evidence;
  never included in `all`.
- `fuzz`: bounded, deterministic, payload-free robustness qualification; never
  included in `all`.

## Inclusion Rules

- A case may belong to multiple profiles.
- `smoke` cases must be byte-stable, small, and free of optional external codec
  requirements.
- `core` excludes large WSI/video/stress cases and intentionally invalid data.
- `legacy` is opt-in by profile; it is not part of `core` or `all`.
- `stress` is excluded from `all` unless an explicit stress flag is enabled.
- `negative` and `fuzz` are never included in `all` and are not conformance
  profiles.
- Expected-invalid files may only be emitted in a `negative` run and must use
  only `negative` profile membership.
- Fuzz qualifications may only be emitted in a `fuzz` run and must not retain
  source or candidate DICOM payloads.
- Stress qualifications are isolated to a `stress` run. When stress cases are
  selected into `all` with `--include-stress`, their DICOM files are included
  but stress-profile qualification records remain isolated to the dedicated
  `stress` run.
- Viewer-specific behavior must not influence profile membership; it belongs in
  optional viewer reports.

Profile membership is selection metadata, not a claim that a case will be
generated by every binary. The registry `status`, required Cargo features,
external runtime availability, and provider capability jointly determine
whether a selected case is emitted or appears in manifest `skipped_cases`.
