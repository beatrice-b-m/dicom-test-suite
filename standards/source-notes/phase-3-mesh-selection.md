# Phase 3 Mesh Representative Selection

Checked: 2026-08-28
Standards baseline: DICOM 2026b, `standards.lock.json`

## Decision

Select **Encapsulated STL Storage** as the first Phase 3 mesh-oriented vertical
slice. Defer Surface Segmentation Storage until a domain constructor or a
separately qualified handwritten surface builder is available.

This is a representative-selection decision, not an assertion that
Encapsulated STL and Surface Segmentation have equivalent semantics.
Encapsulated STL preserves a manufacturing mesh byte stream in an
Encapsulated Document IE. Surface Segmentation represents classified surfaces
within the DICOM Surface IE and can associate one or more surfaces with
segments. The first slice should therefore promise manufacturing-model import
and payload recognition, not segmentation overlays or per-segment interaction.

## Official Standards Evidence

- PS3.3 Section A.85.1 defines the Encapsulated STL IOD. Its mandatory modules
  are the standard Patient, Study, Encapsulated Document Series, Frame of
  Reference, Equipment, Enhanced General Equipment, Encapsulated Document,
  Manufacturing 3D Model, and SOP Common modules. The content constraint
  requires a binary STL byte stream, MIME type `model/stl`, and modality `M3D`.
  The Frame of Reference UID identifies the origin and axes implicit in the STL
  data. See the official
  [Encapsulated 3D Manufacturing Model IODs](https://dicom.nema.org/medical/dicom/2026b/output/chtml/part03/sect_A.85.html).
- PS3.3 Section C.24.2 requires the encapsulated document stream and its MIME
  type, and permits Encapsulated Document Length `(0042,0015)` to preserve the
  unpadded stream length. A source sequence becomes required when the document
  is derived from DICOM instances. See the official
  [Encapsulated Document Module](https://dicom.nema.org/medical/dicom/2026b/output/chtml/part03/sect_C.24.2.html).
- PS3.3 Section C.35.1 requires one Measurement Units Code Sequence item for
  the manufacturing-model coordinate system. Optional model modification,
  mirroring, usage, description, preview, algorithm, and group identity can be
  added without changing the minimal payload contract. See the official
  [Manufacturing 3D Model Module](https://dicom.nema.org/medical/dicom/2026b/output/chtml/part03/sect_C.35.html).
- PS3.6 registers Encapsulated STL Storage as SOP Class UID
  `1.2.840.10008.5.1.4.1.1.104.3`. See the official
  [UID registry](https://dicom.nema.org/medical/dicom/2026b/output/chtml/part06/chapter_A.html).
- PS3.3 Sections A.57 and C.8.23 define Surface Segmentation Storage. That IOD
  requires segmentation semantics in addition to Surface IE point and mesh
  primitive data, including Segment Sequence, surface counts and references,
  point-coordinate OF payloads, and primitive-index OW payloads. See the
  official [Surface Segmentation IOD](https://dicom.nema.org/medical/dicom/2026b/output/chtml/part03/sect_A.57.html)
  and [Surface Segmentation Module](https://dicom.nema.org/medical/dicom/2026b/output/chtml/part03/sect_C.8.23.html).

The official 2026b source directory was checked directly. No official source
artifact was downloaded or committed; source identity remains the locked
`source_manifest_sha256` in `standards.lock.json`.

## Local Dependency Assessment

- The locked optional generation environment contains highdicom 0.28.1 and
  pydicom 3.0.2. Runtime inspection found no highdicom Surface Segmentation
  constructor in either its top-level or segmentation API.
- Pydicom and DICOM-rs can both write the generic elements needed by either
  candidate, but generic element writing is not a domain constructor. Choosing
  Surface Segmentation now would require project-owned construction and
  validation of the complete segmentation/surface graph.
- Encapsulated STL needs no new generation dependency. DICOM-rs can own the
  Part 10 object while a small deterministic builder owns the binary STL
  payload. The generator and payload validator must remain separate
  implementations.
- The current planned registry row names Surface Segmentation and the
  `highdicom_pydicom` provider. Implementing this decision will require a
  deliberate later registry change to Encapsulated STL and a native provider;
  this evidence slice does not modify the registry.

## Feasibility Artifact

A disposable prototype, `/tmp/dts-mesh-feasibility.dcm`, was generated with
the locked pydicom environment solely to qualify the artifact convention. It
contained a four-triangle tetrahedron as a 284-byte binary STL stream:

- 80-byte deterministic header;
- little-endian `u32` triangle count of 4;
- four 50-byte triangle records, each containing a normal, three vertices, and
  a zero attribute-byte count;
- MIME type `model/stl`, modality `M3D`, millimetre UCUM units, a Frame of
  Reference UID, and Encapsulated Document Length 284.

The prototype is not a committed fixture and is not promotion evidence. Its
local qualification results were:

- locked dicom3tools `dciodvfy -new`: recognized `EncapsulatedSTL` and emitted
  no finding;
- locked DCMTK 3.7.0 `dcmdump`: recovered the SOP Class, `M3D`, `model/stl`,
  the 284-byte OB value, and the millimetre code sequence;
- an independent read-only Python `struct` parser: required exact length
  `84 + 50 * triangle_count`, finite binary32 values, zero per-triangle
  attribute-byte counts, four records, and four distinct vertex triples;
- prototype Part 10 SHA-256:
  `2d6425cc3d7eaef84e7c02e7ae5441320d1f053c7dc752e03b728723dd1bac26`;
- extracted STL SHA-256:
  `4acae561f53fa674d8220b4d7ad3655dd0629fba870af689b5fc85fddf87c0e5`.

## Required Vertical-Slice Contract

The later implementation should use a closed tetrahedron with explicit
non-zero normals and deterministic winding. Promotion should require all of
the following:

1. exact Part 10 and extracted-payload reproducibility across two runs;
2. strict DICOM checks for SOP identity, `M3D`, `model/stl`, millimetre units,
   Frame of Reference, document length, and any declared source references;
3. locked `dciodvfy` IOD validation with no unreviewed finding;
4. DCMTK parsing of the exact encapsulated OB value;
5. an independent STL parser that validates the header/count/length layout,
   finite coordinates and normals, triangle record attributes, winding,
   non-degenerate faces, closed-manifold edge incidence, bounds, and payload
   hash without reusing generator code; and
6. a report expectation of `recognized_unsupported` unless a consumer has an
   explicit 3D manufacturing-model capability. It must not be reported as an
   image, segmentation overlay, or ordinarily renderable pixel object.

Source-derived linkage may be added after the standalone payload is proven. If
the first case references source images, the Source Instance Sequence and all
corpus graph edges become mandatory in the same vertical slice.

## Why Surface Segmentation Is Deferred

Surface Segmentation remains the higher-value follow-on for viewers that
render segmented anatomy, but it has a substantially larger semantic surface:
segment classification, Segment-to-Surface counts and references, coordinate
payload cardinality, one-based primitive indices, mesh topology, presentation
attributes, Frame of Reference semantics, and source/reference closure. The
current locked generator ecosystem provides no domain constructor to reduce
that risk. A generic pydicom or DICOM-rs writer would also be the same project
implementation whose output needs independent semantic validation.

Reconsider Surface Segmentation when either a locked backend exposes a domain
constructor or the project has independently implemented validators for every
surface/segment/topology invariant above. Until then, Encapsulated STL is the
smallest conformant slice that provides real mesh payload coverage with clean
independent IOD and payload-validation paths.
