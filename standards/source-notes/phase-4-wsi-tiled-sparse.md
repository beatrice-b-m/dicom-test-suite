# Phase 4 Small TILED_SPARSE Whole Slide Microscopy Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `vl/wsi/tiled_sparse_small`
- Recipe ID: `vl_wsi_tiled_sparse_small`
- SOP Class: VL Whole Slide Microscopy Image Storage
  (`1.2.840.10008.5.1.4.1.1.77.1.6`)
- IOD: VL Whole Slide Microscopy Image
- Modality: `SM`
- Transfer Syntax: Explicit VR Little Endian
  (`1.2.840.10008.1.2.1`)
- Recommended provider: `rust_native`
- Recommended determinism: `byte_stable`
- Profile: `extended`
- Registry action during standards lock: remain `planned`

This is the deliberately incomplete counterpart to
`vl/wsi/tiled_full_small`. It is a small synthetic viewer-compatibility volume,
not a diagnostic pathology fixture. UIDs, dates, times, identity, specimen
metadata, optical-path metadata, geometry, pixels, Functional Groups, and
dimension indices shall be deterministic recipe inputs independent of the
host, locale, network, and clock. No generated slide is committed.

## Required Decision

Implement one native `TILED_SPARSE` VOLUME instance with two native RGB Frames.
Each Frame is a 2 by 2 tile in a 4 by 4 Total Pixel Matrix. There is one optical
path, one focal plane, no Concatenation, and exactly two deliberately absent
tile positions. The sparse object shall describe every encoded Frame with
Frame Content, Plane Position (Slide), and Optical Path Identification Macros
in its Per-Frame Functional Groups Item. A recipient shall use those explicit
values and shall not infer `TILED_FULL` Frame order or synthesize the absent
Frames.

The encoded tiles, in stored Frame order, are:

| Frame | Color | Column | Row | X mm | Y mm | Z | Frame SHA-256 |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | red | 1 | 1 | 0.0 | 0.0 | 0.0 | `fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8` |
| 2 | white | 3 | 3 | 1.0 | 1.0 | 0.0 | `8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21` |

The stored 24-byte interleaved payload SHA-256 is
`94a57aca44c4a97d424e8e546b2673fa91f711694de1ccb36f062aabbc9b55ee`.
The top-right tile at Column/Row `3/1` and the bottom-left tile at `1/3` are
absent. They have no stored pixels and no Per-Frame Functional Groups Items.

For an exact reconstruction oracle, allocate a 4 by 4 interleaved RGB matrix,
initialize missing locations to `[0, 0, 0]`, and place only the two encoded
Frames using their Plane Position (Slide) values. The resulting matrix
SHA-256 is
`d10a587875f14a0b74a9e4935ce83cdb73377bd7357a172db8e9f7347c030eb3`.
Black is an adapter-owned sentinel, not DICOM pixel content for an absent tile.
The reconstruction result shall also carry the tile occupancy mask
`[present, absent, absent, present]` in top-left, top-right, bottom-left,
bottom-right order so an absent tile cannot be confused with an encoded black
tile.

The exact pixel contract remains RGB, Samples per Pixel `3`, Planar
Configuration `0`, unsigned 8-bit samples, native OB Pixel Data, Bits
Allocated/Stored/High Bit `8/8/7`, Lossy Image Compression `00`, and Number of
Frames `2`. Image Type and shared Whole Slide Microscopy Image Frame Type are
both `ORIGINAL\PRIMARY\VOLUME\NONE`. Tiles Overlap is `NONE`.

## Locked Geometry And Explicit Dimension Indices

The geometry is carried unchanged from the qualified tiled-full counterpart:

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

Dimension Organization Type is exactly `TILED_SPARSE`. Dimension Organization
Sequence contains one Item with a deterministic Dimension Organization UID.
Dimension Index Sequence contains exactly two Items in this order:

1. Dimension Index Pointer `(0048,021E)`, Column Position In Total Image Pixel
   Matrix; Functional Group Pointer `(0048,021A)`, Plane Position (Slide)
   Sequence; and the same Dimension Organization UID.
2. Dimension Index Pointer `(0048,021F)`, Row Position In Total Image Pixel
   Matrix; Functional Group Pointer `(0048,021A)`, Plane Position (Slide)
   Sequence; and the same Dimension Organization UID.

Dimension Description Labels are respectively `Column Position` and
`Row Position`. No private-creator attributes are present. The single optical
path is invariant across the instance, so it is explicitly identified for
each Frame but is not an additional indexed dimension.

Per-Frame Functional Groups Sequence has exactly two Items in stored Frame
order. Each Item contains exactly these three Macros:

1. Frame Content Sequence, one Item, whose Dimension Index Values are `1\1`
   for Frame 1 and `2\2` for Frame 2. The Value order is bound to the two
   Dimension Index Sequence Items above; the values are logical ordinals for
   the distinct Column and Row positions.
2. Plane Position (Slide) Sequence, one Item, containing the exact one-based
   Column and Row and X/Y/Z offsets in the Frame table above.
3. Optical Path Identification Sequence, one Item, with Optical Path
   Identifier `RGB`, matching the sole Optical Path Sequence Item.

The shared Functional Groups Item contains exactly Pixel Measures and Whole
Slide Microscopy Image Frame Type. Frame Content shall not be shared. The
column and row positions remain on the locked 2-pixel tile grid. Their X/Y/Z
offsets shall agree with the origin, orientation, and spacing; merely
self-consistent index ordinals are not sufficient.

## Locked Specimen, Optical Path, And Slide Label Contract

The complete specimen, optical-path, ICC, and slide-label contract is carried
unchanged from `standards/source-notes/phase-4-wsi-tiled-full.md`:

- Container Identifier `DTS-SLIDE-001`, empty Type 2 Issuer of the Container
  Identifier Sequence, and empty Type 2 Container Type Code Sequence;
- one Specimen Description Item with Specimen Identifier
  `DTS-SPECIMEN-001`, a deterministic Specimen UID, empty Type 2 Issuer of the
  Specimen Identifier Sequence, and empty Type 2 Specimen Preparation
  Sequence;
- one Optical Path Sequence Item identified as `RGB`, Illumination Wave Length
  `550` nm, and Illumination Type `(111744, DCM, "Brightfield illumination")`;
- the nested locked 736-byte DCMTK sRGB input ICC Profile with SHA-256
  `8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef`
  and Color Space `SRGB`; and
- Barcode Value `DTS-SLIDE-001` and Label Text
  `DTS SYNTHETIC SLIDE 001`.

Specimen Label in Image is `NO`, Burned In Annotation is `NO`, Focus Method is
`AUTO`, Extended Depth of Field is `NO`, and Number of Optical Paths is `1`.
The optional Specimen Reference Functional Group remains absent because the
single specimen applies to the entire image.

Acquisition Context Sequence is present and empty. Referenced Series Sequence,
other references, Concatenation attributes, Multi-Resolution Pyramid,
extended-depth-of-field focal-plane count and distance, Lossy Image Compression
Ratio and Method, and top-level Image Pixel Description ICC Profile are absent.
Unlike `TILED_FULL`, Dimension Index Sequence and Per-Frame Functional Groups
Sequence are mandatory and non-empty for this exact case.

## Locked Standards Evidence

PS3.3 A.32.8 and Table A.32.8-1 define the VL Whole Slide Microscopy Image IOD.
Table A.32.8-2 makes Plane Position (Slide) and Optical Path Identification
required when Dimension Organization Type is not `TILED_FULL`, and prohibits
Frame Content as a Shared Functional Group. A.32.8.4.1.2 requires regular tile
grid positions, explicitly permits omitted tiles, places no ordering constraint
on sparse Frames, and requires each Frame to state its position.

PS3.3 C.7.6.17 defines the Multi-frame Dimension Module. Dimension Index
Sequence is required when Dimension Organization Type is absent or not
`TILED_FULL`; each Item binds one indexed attribute, its containing Functional
Group, and a Dimension Organization UID. C.7.6.17.1 binds the ordering of
Dimension Index Values to the ordering of Dimension Index Sequence Items and
defines those values as one-based logical ordinals. C.7.6.17.3 states that a
`TILED_SPARSE` recipient shall not assume Frame spatial position, optical path,
segment, or order and shall rely on the relevant per-frame Macros.

PS3.3 C.8.12.6.1 requires one Plane Position (Slide) Sequence Item with Column
and Row Position In Total Image Pixel Matrix and X, Y, and Z Offset in Slide
Coordinate System. C.8.12.6.2 requires one Optical Path Identification
Sequence Item referring to an Optical Path Identifier in Optical Path Sequence.
C.8.12.14 defines Total Pixel Matrix geometry. C.8.12.4, C.8.12.5, C.7.6.22,
C.8.12.8, C.7.4.1, and C.7.6.16.2.1 continue to bind the image, optical path,
specimen, label, slide Frame of Reference, and Pixel Measures content described
by the tiled-full source note.

The local `dicom-standard-kb` query
`dicom-kb lookup uid VLWholeSlideMicroscopyImageStorage --edition 2026b`
establishes the SOP Class UID but does not bind sparse dimension indices,
per-frame Macro placement, explicit tile positions, or absence rules. The
official locked 2026b evidence is therefore required. Its SHA-256 identities
are:

- PS3.3 DocBook:
  `4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`;
- PS3.4 DocBook:
  `8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`;
- PS3.6 DocBook:
  `512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`;
  and
- generated IOD definitions:
  `ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683`.

The repository lock records official source artifacts as
`unavailable_not_downloaded`; the separately provisioned validator cache
provides the hash-locked read-only copies used for this review. No official or
generated standard artifact is committed. This narrow gap does not currently
require a KB patch; a future reusable Multi-frame Dimension extraction would
supersede the hand-authored portion of this note.

## Prototype And Validator Disposition

A temporary pydicom prototype encoded the exact contract above. Its Part 10
file was 3,392 bytes with SHA-256
`8f73eccc7d2b604679f6bed14d7041baa8f532b3c6ee956ffbb5bff78d8fc4f1`.
Locked DCMTK 3.7.0 parsed it. The independently provisioned `uv`-locked
dicom-validator 0.8.2 loaded the exact hash-locked 2026b definitions, selected
VL Whole Slide Microscopy Image Storage, and returned `Passed` with zero IOD
errors.

The locked dicom3tools snapshot `1.00.snapshot.20260803085716`, executable
SHA-256
`1aeb75d6ccd3f193e3b322b6da77742cdce2e0604868eaf2a2669c786cbc27e5`,
recognized the IOD but emitted exactly one error:
`NumberOfFrames does not match expected value for tiled total pixel matrix`,
expecting four Frames. That calculation is the full-grid cardinality and is
incompatible with the standard's explicit permission for omitted sparse
tiles. The finding is not accepted or allowlisted, and the tool must not be
reported as passing this case.

Because the primary project-wide IOD validator cannot represent this valid
sparse cardinality, promotion requires the already authorized, independently
locked 2026b dicom-validator route to be the case-specific IOD authority for
`vl/wsi/tiled_sparse_small`. The route shall return zero errors and retain all
runtime, wheel, adapter, official DocBook, generated-definition, and lock
fingerprints. The dicom3tools result remains visible characterization evidence,
not a conformance success. DCMTK remains the independent Part 10 parser.

## Independent Reconstruction And Negative Controls

Promotion also requires a dedicated `uv`-locked highdicom 0.28.1, pydicom
3.0.2, and NumPy adapter that does not use the Rust generator. It shall reopen
the stored Frames through highdicom, independently read and cross-bind the
Dimension Index Sequence and all three required per-frame Macros, place only
the encoded Frames, emit the exact occupancy mask, and verify both Frame hashes,
the stored payload hash, and the sentinel-filled matrix hash. Pixel transforms,
including ICC application, shall remain disabled so this oracle validates
stored samples and sparse placement; strict validation separately binds the
nested ICC bytes.

At minimum, qualification shall reject controls that:

- replace `TILED_SPARSE` with `TILED_FULL` or omit it;
- add either absent tile, omit either encoded Frame, or change Number of Frames;
- omit, duplicate, reorder, or alter a Dimension Index Sequence Item, pointer,
  Functional Group pointer, Dimension Organization UID, or Dimension Index
  Value;
- omit Frame Content, Plane Position (Slide), or Optical Path Identification
  from either Frame;
- duplicate or swap tile positions, move a position off the locked grid or
  outside the matrix, or make an X/Y/Z offset disagree with the position,
  origin, orientation, or spacing;
- swap or change stored Frame bytes, or treat absent pixels as encoded black
  pixels without preserving the occupancy mask;
- change matrix extent, tile shape, optical-path identity or count, focal-plane
  count, specimen, slide label, or ICC profile; or
- relink the executable, adapter, source manifest, official standard inputs,
  generated definitions, or either `uv` lock.

No generator code may be imported into the reconstruction adapter. Strict Rust
validation owns the full manifest, module, cross-field, and absence contract.
No validator finding may be silently dropped or converted into an accepted
finding.

## Provider Decision

Use the native Rust writer and declare byte stability after two independent
same-seed roots compare byte-for-byte. The sparse structure is deterministic
native DICOM serialization, not a reason to make Python a generation-time
dependency of the `extended` profile. Python remains optional and independent
conformance infrastructure. This provider choice does not trigger a Phase 4
decision checkpoint.
