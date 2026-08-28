# Phase 4 Multiple-Optical-Path Whole Slide Microscopy Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `vl/wsi/multiple_optical_paths`
- Recipe ID: `vl_wsi_multiple_optical_paths`
- SOP Class: VL Whole Slide Microscopy Image Storage
  (`1.2.840.10008.5.1.4.1.1.77.1.6`)
- IOD: VL Whole Slide Microscopy Image
- Modality: `SM`
- Transfer Syntax: Explicit VR Little Endian
  (`1.2.840.10008.1.2.1`)
- Recommended provider: `rust_native`
- Recommended determinism: `byte_stable`
- Profile: `extended`

This is a deliberately small synthetic viewer-compatibility volume, not a
diagnostic pathology fixture. UIDs, dates, times, identity, specimen metadata,
optical-path metadata, geometry, pixels, and equipment values shall be
deterministic recipe inputs independent of the host, locale, network, and
clock. No generated slide is committed.

## Required Optical-Path Decision

Implement one native `TILED_FULL` VOLUME instance with two ordered optical
paths and one separately encoded focal plane. Do not add a second focal plane
to this case. Phase 4 milestone 5 permits multiple optical paths or focal
planes, and this case ID specifically selects the optical-path branch. Keeping
the focal-plane count at one makes optical-path order independently testable
without combining two dimensions in the first compatibility slice.

Multiple separately encoded focal planes are distinct from focus stacking.
Total Pixel Matrix Focal Planes (0048,0303) counts separately encoded Z
locations. Number of Focal Planes (0048,0013) and Distance Between Focal
Planes (0048,0014) instead describe acquisition planes combined into one
encoded plane when Extended Depth of Field (0048,0012) is `YES`. This case
sets Extended Depth of Field to `NO`, Total Pixel Matrix Focal Planes to `1`,
and omits Number of Focal Planes, Distance Between Focal Planes, and Spacing
Between Slices (0018,0088).

Each Frame is a 2 by 2 native RGB tile. The Total Pixel Matrix is 4 by 4
pixels, so each optical path contributes four spatial tiles and the instance
contains exactly eight Frames. Number of Optical Paths (0048,0302) is `2`.
Optical Path Sequence (0048,0105) contains these exact ordered Items:

| Ordinal | Identifier | Description | Illumination wavelength | Illumination type |
| ---: | --- | --- | ---: | --- |
| 1 | `BRIGHTFIELD` | `Deterministic brightfield path` | 550 nm | `(111744, DCM, "Brightfield illumination")` |
| 2 | `ALTERNATE` | `Deterministic alternate path` | 650 nm | `(111744, DCM, "Brightfield illumination")` |

Each Item contains the existing locked 736-byte DCMTK sRGB input ICC Profile
with SHA-256
`8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef`
and Color Space `SRGB`. Reusing the same input-device profile is intentional:
both paths encode stored RGB values in the same color space. A top-level Image
Pixel Description ICC Profile shall be absent because the Optical Path Module
owns the per-path profiles. Optical Path Identifiers are Type 1, distinct, and
their Sequence order is semantic.

The exact pixel contract is RGB, Samples per Pixel `3`, Planar Configuration
`0`, unsigned 8-bit samples, native OB Pixel Data, Bits
Allocated/Stored/High Bit `8/8/7`, Pixel Representation `0`, Lossy Image
Compression `00`, and Number of Frames `8`. Image Type and the shared Whole
Slide Microscopy Image Frame Type are both
`ORIGINAL\PRIMARY\VOLUME\NONE`.

## Locked Implicit Frame Order And Pixels

For `TILED_FULL`, spatial position varies first from left to right and then
top to bottom, followed by focal plane and finally by successive Optical Path
Sequence Items. The locked one-based positions and path assignments are:

| Frame | Optical path | Focal plane | Column | Row | X mm | Y mm | Z mm | Tile |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | `BRIGHTFIELD` | 1 | 1 | 1 | 0.0 | 0.0 | 0.0 | red |
| 2 | `BRIGHTFIELD` | 1 | 3 | 1 | 1.0 | 0.0 | 0.0 | green |
| 3 | `BRIGHTFIELD` | 1 | 1 | 3 | 0.0 | 1.0 | 0.0 | blue |
| 4 | `BRIGHTFIELD` | 1 | 3 | 3 | 1.0 | 1.0 | 0.0 | white |
| 5 | `ALTERNATE` | 1 | 1 | 1 | 0.0 | 0.0 | 0.0 | cyan |
| 6 | `ALTERNATE` | 1 | 3 | 1 | 1.0 | 0.0 | 0.0 | magenta |
| 7 | `ALTERNATE` | 1 | 1 | 3 | 0.0 | 1.0 | 0.0 | yellow |
| 8 | `ALTERNATE` | 1 | 3 | 3 | 1.0 | 1.0 | 0.0 | black |

The ordered Frame SHA-256 values are:

1. `fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8`;
2. `6c8f6d772829d493618e079a099cf4f20d8524ed3656f49db234f5bbf60a4e65`;
3. `7263ad3fd60c6620abd423516d748baedf5e393b1fbdaaf780ff5803a443cc4f`;
4. `8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21`;
5. `f7606fde280d9577c963618cc2a8fa52b15315ff63ec185029cf66bda64435ab`;
6. `81fd180e1f66d28018580f37d46188c02fd6709f875b3b620090718a8847c282`;
7. `745598fdcfa2650299b59b42f40c0750087e117d6bc236c66486087cd264ebd8`;
   and
8. `15ec7bf0b50732b49f8228e07d24365338f9e3ab994b00af08e5a3bffe55fd8b`.

The exact ordered 96-byte Pixel Data payload SHA-256 is
`831fe6e50cbc3f3d82e3f57c984d3c273cdb18dd3bd3ab511b3633dc293f708f`.
Independent per-path reconstruction shall produce two 4 by 4 interleaved RGB
matrices in Optical Path Sequence order:

1. `BRIGHTFIELD`:
   `62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a`;
   and
2. `ALTERNATE`:
   `caa1a1abb84ec283bbf92a0f00d5bd89650420d0b1fa911e191ddb368f50e09f`.

The matrices shall be reconstructed separately. Combining both optical paths
into one 4 by 4 matrix would discard a required dimension.

## Locked Geometry, Functional Groups, And Absences

The physical geometry is inherited from the qualified small tiled-full case:

- Rows and Columns: `2` and `2`;
- Total Pixel Matrix Rows and Columns: `4` and `4`;
- Total Pixel Matrix Focal Planes: `1`;
- Pixel Spacing: `0.5\0.5` mm;
- Slice Thickness: `0.001` mm;
- Imaged Volume Width and Height: `2.0` mm and `2.0` mm;
- Imaged Volume Depth: `1.0` micrometer;
- Total Pixel Matrix Origin X/Y/Z: `0.0/0.0/0.0`;
- Image Orientation (Slide): `1\0\0\0\1\0`; and
- Position Reference Indicator: `SLIDE_CORNER`.

Dimension Organization Type is `TILED_FULL`. Dimension Organization Sequence
contains one deterministic Dimension Organization UID. Dimension Index
Sequence and Per-Frame Functional Groups Sequence are absent. The shared
Functional Groups Item contains exactly Pixel Measures and Whole Slide
Microscopy Image Frame Type. Plane Position (Slide) and Optical Path
Identification Macros may be present for `TILED_FULL`, but the standard permits
them to be omitted because position and path are implicit. Their absence is
locked here so that independent reconstruction actually tests the normative
order rather than reading redundant per-frame assertions.

The specimen, container, Frame of Reference, Slide Label, acquisition context,
and deterministic identity contract is inherited unchanged from
`phase-4-wsi-tiled-full.md`. Specimen Label in Image is `NO`, Burned In
Annotation is `NO`, Focus Method is `AUTO`, and Extended Depth of Field is
`NO`. Multi-Resolution Pyramid, Concatenation attributes, references, lossy
ratio and method, top-level ICC, Dimension Index Sequence, and Per-Frame
Functional Groups Sequence are absent.

## Manifest, Validation, And Report Contract

The manifest shall carry an exact `expected_wsi_multiple_optical_paths`
contract that binds:

- the SOP identity, transfer syntax, one-instance membership, and deterministic
  Frame of Reference, specimen, and Dimension Organization UIDs;
- the eight-Frame, 2 by 2 tile, 4 by 4 total-matrix, one-focal-plane geometry;
- the exact ordered two-item Optical Path Sequence, descriptions, wavelengths,
  illumination codes, ICC hashes, and `Number of Optical Paths=2`;
- all eight implicit positions and their optical-path ordinals and identifiers;
- the ordered Frame hashes, aggregate payload hash, and two per-path matrix
  hashes;
- the complete presence and absence contract; and
- the exact instance, Frame, total-byte, and generation-time budgets.

Strict validation shall reopen the file and compare the dataset, stored Pixel
Data, implicit positions, and manifest contract. It shall not infer path order
from colors, filenames, or manifest ordering alone. JSON and Markdown reports
shall expose the optical-path count, ordered identifiers, per-path Frame ranges,
one focal plane, organization type, ICC disposition, payload hash, both matrix
hashes, independent reconstruction status, IOD-validator dispositions, and
budget status.

## Locked Standards Evidence

PS3.3 A.32.8 and Table A.32.8-1 define the VL Whole Slide Microscopy Image
IOD. Table A.32.8-2 makes Pixel Measures and Whole Slide Microscopy Image Frame
Type shared Functional Groups mandatory and makes Plane Position (Slide) and
Optical Path Identification conditional when Dimension Organization Type is
not `TILED_FULL`.

PS3.3 C.7.6.17 and Table C.7.6.17-1 define the Multi-frame Dimension Module.
C.7.6.17.3, especially paragraphs
`para_08fcce55-df5f-49f2-b8a2-978d0468872c` through
`para_e553a9fe-2e1b-42ed-aa5f-a27f3c7c00e6`, requires complete
`TILED_FULL` Frames to vary first along rows from left to right, then columns
from top to bottom, then depth, and then successive Optical Path Sequence
Items. Paragraph `para_6d73468e-28c6-4aee-a708-dab0b3a14702` contrasts the
explicit per-frame semantics of absent or `TILED_SPARSE` organization.

PS3.3 C.8.12.4 and Table C.8.12.4-1 define the WSI image and pixel attributes.
They distinguish Total Pixel Matrix Focal Planes, which counts separately
encoded Z locations, from Number of Focal Planes and Distance Between Focal
Planes, which describe focus stacking when Extended Depth of Field is `YES`.
PS3.3 C.7.6.16.2.1, Table C.7.6.16-2, paragraph
`para_2a27a0ca-e588-4dd3-8eec-5a225b88a3ab` requires Spacing Between Slices
for `TILED_FULL` only when Total Pixel Matrix Focal Planes is greater than one;
paragraph `para_46d23721-f2aa-485f-a5a5-817268ce8789` repeats the distinction
from focus stacking.

PS3.3 C.8.12.5 and Table C.8.12.5-1 require Number of Optical Paths for
`TILED_FULL`, require one or more Optical Path Sequence Items, require every
Optical Path Identifier to be unique within the Sequence, and require a nested
ICC Profile for RGB. C.8.12.5.1.1 requires the Sequence to include an Item for
every optical path used in the current image and defines Sequence order as the
reference basis for multi-frame images. C.8.12.5.1.4 binds the nested ICC
Profile. C.8.12.6.2 and Table C.8.12.6.2-1 define the per-frame Optical Path
Identification Macro used when explicit identification is required.

PS3.3 C.8.12.14 and Table C.8.12.14-1 define Total Pixel Matrix rows,
columns, focal planes, origin, and orientation. PS3.4 Table B.5-1 identifies
the composite storage SOP Class. PS3.6 Tables A-1 and 6-1 identify the SOP
Class, Transfer Syntax, and attribute registry properties.

The local `dicom-standard-kb` query
`dicom-kb lookup uid VLWholeSlideMicroscopyImageStorage --edition 2026b`
establishes the SOP Class UID but does not bind multi-path cardinality,
normative Frame order, ICC placement, focal-plane distinctions, or exact
manifest semantics. The official locked 2026b evidence is therefore required.
Its SHA-256 identities are:

- PS3.3 DocBook:
  `4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`;
- PS3.4 DocBook:
  `8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`;
- PS3.6 DocBook:
  `512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`;
- generated IOD definitions:
  `ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683`;
  and
- generated module definitions:
  `9f4853924ef520dd9b97ada0f14abd206fb15e6d8622e4d24a90f8b404a3e8c3`.

The repository lock records official source artifacts as
`unavailable_not_downloaded`; the separately provisioned validator cache
provides the hash-locked read-only copies used for this review. No official or
generated standard artifact is committed. This narrow typed-contract gap does
not currently require a `dicom-standard-kb` patch.

## Prototype And Independent Qualification Evidence

A temporary pydicom prototype encoded the exact contract twice. Both outputs
were byte-identical. Each file was 3,934 bytes with SHA-256
`501df067edae9c0fc22c478e66d9fb11b489980c01349449bb641b919447a283`.
The measured generation times were `0.002518249995773658` and
`0.0012716250057565048` seconds.
The prototype Pixel Data was 96 bytes and reproduced the exact aggregate,
Frame, and per-path matrix hashes above.

Locked `dciodvfy` selected the VL Whole Slide Microscopy Image IOD and reported
zero errors for the positive prototype. The separately implemented,
`uv`-locked dicom-validator against the generated 2026b definitions also
reported `status=Passed errors=0`. These are complementary IOD opinions, not
substitutes for the project-owned optical-path and pixel contract.

The isolated `uv`-locked highdicom 0.28.1/pydicom 3.0.2 stack derived the exact
eight tuples `(optical path ordinal, focal plane, column, row, X, Y, Z)` from
`iter_tiled_full_frame_data()`. Its unfiltered `get_total_pixel_matrix()`
correctly rejected the object with `RuntimeError` because spatial dimensions
alone do not uniquely identify Frames when two optical paths occupy the same
positions. Filtering by each optical-path dimension separately produced two
`(4, 4, 3)` matrices with the locked hashes. The final independent adapter
shall therefore reconstruct and report both per-path matrices; it shall not
collapse the paths or treat the unfiltered API limitation as a conformance
failure. It shall import no generator code.

Prototype negative controls exposed the necessary ownership boundary.
`dciodvfy` rejected a seven-Frame object and a false path count because their
calculated `TILED_FULL` cardinalities were not eight. The secondary IOD
validator did not detect those cardinality errors. Neither generic IOD
validator detected duplicate Optical Path Identifiers, path-Sequence-only
swaps, swapped path pixel blocks, or reordered Frames within a path. Strict
Rust validation and the independent reconstruction adapter must own those
exact semantic failures; no finding may be silently dropped or allowlisted.

At minimum, qualification shall reject controls that:

- omit or add a Frame, or disagree about Number of Frames, Number of Optical
  Paths, tile-grid cardinality, or focal-plane cardinality;
- omit, duplicate, reorder, or change an Optical Path Sequence Item or
  identifier;
- remove or alter either nested ICC Profile, or move a profile to the top
  level;
- swap path pixel blocks, reorder Frames within either path, or change a
  Frame, payload, implicit position, or per-path matrix hash;
- add separately encoded focal planes, focus-stacking attributes, Dimension
  Index Sequence, Per-Frame Functional Groups, Concatenation, Pyramid, or
  references;
- substitute `TILED_SPARSE`, change origin, orientation, spacing, tile or
  total-matrix dimensions, or break the inherited specimen and identity
  contract; or
- relink the executable, adapter, source manifest, official standard inputs,
  generated definitions, or a `uv` lock.

## Provider, Budget, And Checkpoint Decision

Use the native Rust writer and declare byte stability only after two
independent same-seed generated roots compare byte-for-byte. The native route
reuses the already qualified WSI, ICC, specimen, geometry, Pixel Data, and
deterministic UID machinery. Python remains optional independent conformance
infrastructure rather than becoming a generation-time dependency.

The qualification ceiling is exactly one instance, exactly eight Frames, no
more than 16,384 total DICOM bytes, and no more than 5 seconds of generation
wall time on the qualification host. These limits provide broad headroom over
the 3,934-byte, sub-0.003-second prototype without defining a large-slide
workload. Any breach fails the slice rather than silently reducing the number
of paths, Frames, tiles, or required metadata.

This is a small WSI and belongs in `extended`, consistent with the Phase 4
gate. It is neither a pyramid nor a large-slide workload. Promoting it changes
the normal extended/all corpus by one small instance but adds no mandatory
runtime, codec, network source, or generated artifact to git.

Proceeding with this milestone triggers no explicit decision checkpoint in
`docs/coverage-expansion-plan.md`. The remaining Phase 4 checkpoint concerns
selecting dimensions, budgets, and CI scheduling for a future full-size
pyramid. This small multiple-optical-path slice neither authorizes nor
prejudges that decision.
