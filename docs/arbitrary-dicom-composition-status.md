# Arbitrary DICOM composition status

## 2026-08-29 — Phase P7 completion gate

Phase P7 is complete. External callers can use the file-backed CLI, the
same-pipeline Rust byte API, or the versioned `1.0.0` single-output content
provider protocol without adding a repository recipe. Providers receive
preallocated identities and an exact slot/size/hash envelope, execute with a
cleared environment and bounded diagnostics in an owned process group, and
cannot publish DICOM or an undeclared file. The manifest binds provider,
executable, fixed-argument, request, response, resource, termination, and
payload identity. Protocol-level network disablement is documented honestly as
distinct from an OS socket sandbox.

Large top-level native Pixel Data and typed bulk values use 64 KiB staged copy
and hash buffers through Part 10 materialization. Native per-frame hashing is
also streamed; single-bit frames are incrementally canonicalized. The RLE path
reads one native frame at a time, records backend availability/version/feature
identity, and decodes every encoded frame for exact native-hash qualification.
RLE remains byte-stable; unavailable feature/runtime codecs are not added to
the qualified catalog.

File-level parallelism is bounded from 1 through 64 and cannot affect paths,
UIDs, references, plan hashes, manifest ordering, or byte-stable outputs. The
128-instance qualification produces identical sequential and eight-worker
identity/hash projections. The 16 MiB single-value qualification uses the
stream-copy writer. Resource qualifications cover expanded instances, file and
aggregate input bytes, total DICOM-plus-manifest publication bytes,
cancellation, provider crash/hang/output flood/undeclared output, private
cleanup, and destination races. Cancellation terminates provider descendants
and leaves no requested output. Final publication uses owner-only staging and
Linux/macOS atomic no-replace rename semantics.

Focused P7 evidence is in `tests/composition_public_api.rs`,
`tests/composition_provider.rs`, `tests/composition_streaming.rs`,
`tests/composition_parallel.rs`, `tests/composition_codec.rs`, and
`tests/composition_resources.rs`. The external workflow and evidence boundary
are documented in `docs/composition-integration-guide.md`. P7 does not broaden
the qualified template inventory or convert same-project codec/provider checks
into independent conformance.

## 2026-08-29 — Phase P6 completion gate

Phase P6 is complete. The catalog and executable inventory audit now cover
every currently implemented valid DICOM SOP Class with a qualified template or
deterministic bundle. Newly qualified families include binary, fractional,
label-map, and WSI-tile SEG; float and double-float Parametric Map; Real World
Value Mapping; five structured-report roles; the six-object RT graph; Twelve-
lead and General ECG; Encapsulated PDF; and Encapsulated STL.

All pixel, float, waveform, document, and mesh content is represented by typed
bulk plans. Composition manifests record the exact byte length, SHA-256,
source provenance, placement, bounds, and family semantic validator for every
materialized value. Caller content round-trips only within the qualified fixed
shape or bounded format, and malformed lengths, non-finite values, invalid PDF
or STL structure, unknown semantic parameters, and protected structural
collisions abort before publication.

Derived, SR, and RT templates use named reference roles. Their default bundles
close source dependencies deterministically, rewrite embedded DICOM references
from the logical graph, and reject unknown targets, invalid frames, cycles, or
identity-cardinality conflicts. SR and RT expose only schema-bounded semantic
controls; additional valid data uses the typed override policy rather than an
untyped tree API.

Every corresponding P5/P6 curated artifact now rematerializes through the
shared resolved plan while retaining its specialized validation oracle.
Byte-stable artifacts are required to remain identical. Semantic-stable
backend artifacts retain their decoded/semantic contracts and refresh the
published file hash and size after canonical rematerialization. The optional
Deflated Image Frame SEG path is covered when the `deflate` feature is active
and remains explicitly unavailable otherwise.

The catalog binds each bulk-bearing template to required content rules and an
independent semantic route in addition to the IOD route. Same-project focused
tests prove provenance, graph closure, caller round trips, transactional
failure, and two-run determinism; independent evidence remains scoped to the
pinned adapters and qualified content domains named by each descriptor.

## 2026-08-29 — Phase P5 completion gate

Phase P5 is complete. Qualified templates now cover Enhanced CT, Enhanced MR,
Enhanced PET, the two-part Enhanced CT concatenation, tiled-full and sparse
WSI, multiple optical paths, the three-member WSI pyramid, spatial and
deformable registration, and grayscale, color, blending, and advanced blending
presentation states. Enhanced and WSI templates accept either their bounded
qualified defaults or caller-owned native frames with an exact structural
shape.

The shared plan validates functional-group and dimension cardinality, implicit
and explicit WSI tiling, concatenation UID/number/total/offset continuity, and
one-based referenced-frame ranges before publication. Derived-object bundles
support explicit sources or deterministic defaults. Their embedded referenced
SOP, series, study, and Frame of Reference UIDs are rewritten onto the resolved
logical graph; paired blending sources share the intended two-series identity
topology. Manifest and report projections distinguish requested members from
default dependencies and expose source provenance, closure, and frame roles.

Default, caller-content, malformed-structure, explicit-source substitution,
two-run reproducibility, root validation, report, and DICOM-reference closure
tests pass. The pinned independent route reports the expected Enhanced, WSI,
Registration, and Presentation State IODs without findings, except sparse WSI:
its locked `pydicom-dicom-validator-wsi-sparse` primary route remains clean and
the exact known dicom3tools full-grid finding remains non-accepted
characterization.

All corresponding curated cases now rematerialize through the shared plan with
a mandatory byte-equality check. Their prior specialized validation and
independent evidence remain in place, and each manifest row records the
`curated_composition_plan` check. Default bundle members remain composition
artifacts and are never projected as curated registry cases.

## 2026-08-29 — Phase P4 completion gate

Phase P4 is complete. Every curated Secondary Capture, CT, MR, CR, DX,
mammography, ultrasound, Nuclear Medicine, PET, visible-light, XA, and XRF
recipe now crosses the shared resolved-plan and Part 10 materialization path.
Generated manifests retain the existing case identities, profiles, semantics,
validation evidence, reports, and independent-conformance meaning while adding
an internal `curated_composition_plan` check that makes the shared production
path auditable.

The pre-migration seed-1 smoke, core, and extended corpora were retained as
private qualification oracles. All byte-stable DICOM and auxiliary files match
those oracles exactly; semantic-stable outputs retain their decoded hashes and
contracts. Core output, including manifest ordering, also matches exactly
before and after the dispatch refactor. Reproducibility, profile selection,
generation, root validation, and reporting regression suites pass.

Central curated dispatch is now a typed registry of recipe implementations.
Its stages preserve the established source-dependency and manifest-order
boundaries, and a completeness/uniqueness test binds every migrated recipe
table entry to exactly one implementation. The duplicate family-specific file
meta/write paths are gone; the remaining element constructors are inputs to the
curated plan bridge and remain shared with object families scheduled for P5
and P6 migration.

## 2026-08-29 — Phase P3 completion gate

Phase P3 is complete. Every implemented classic-image and single/multi-frame
visible-light SOP Class in `templates/inventory.json` resolves to a qualified,
standards-evidenced descriptor. The catalog-rendered
`docs/composition-template-reference.md` exposes every qualified version, SOP
Class, transfer syntax, determinism class, and independent route; a gate test
prevents that reference or the inventory mappings from drifting.

All family defaults pass same-project composition validation and the pinned
`dicom3tools-dciodvfy` executable without warning or error findings. Caller
native pixels are checked against family-specific frame, photometric, sample,
and bit-depth contracts. XA/XRF additionally qualify deterministic built-in RLE
Lossless encoding with fragment-integrity validation, lossless decode evidence,
and byte-stable two-run output. Descriptor audits require protected, derived,
conditional, and caller-settable policy coverage for every P3 inventory entry.

## 2026-08-29 — Multi-frame Secondary Capture qualification

Multi-frame Single Bit and Multi-frame Grayscale Byte Secondary Capture are
qualified composition templates. Caller single-bit content remains
continuously packed across frame boundaries; per-frame hashes use canonical
frame-local packing even when a frame boundary is not byte aligned. Both
profiles pass caller round trips, byte reproducibility, root validation and
reporting, and the pinned independent route without findings.

Every currently implemented classic and single/multi-frame VL SOP Class has a
native qualified template. The cross-family and codec closeout is recorded in
the Phase P3 completion entry above.

## 2026-08-29 — Phase P3.7 native XA/XRF checkpoint

Native Explicit VR Little Endian XA and XRF composition templates are
qualified. Their defaults model single-plane image type, X-ray acquisition,
geometry, XA positioner angles, XRF column angulation, and intensity
relationship semantics. Both pass caller native-pixel round trips,
wrong-signedness rejection, root validation/reporting, byte comparison, and
the pinned independent route without findings.

This entry records the native checkpoint. Built-in RLE caller-frame integration
and the cross-family closeout were subsequently qualified by the Phase P3
completion entry above.

## 2026-08-29 — Phase P3.6 visible-light lane

VL Endoscopic, VL Microscopic, and VL Photographic Image Storage are qualified
composition templates. Each has a bounded synthetic RGB default, documented
acquisition-context behavior, and an exact single-frame interleaved RGB 8-bit
caller slot. Planar input contradicting that contract is rejected before
publication.

All three profiles pass root validation and reporting, caller-pixel round
trips, two-run byte comparison, and the pinned independent route without
warning or error findings. This closes P3.6 only; XA/XRF, multi-frame SC, and
the cross-family P3 gate evidence remain open.

## 2026-08-29 — Phase P3.5 US, NM, and PET lane

Single-frame and multi-frame Ultrasound, Nuclear Medicine, and PET Image
Storage templates are qualified. The profiles cover ultrasound timing and
color-presence semantics, NM energy-window/detector vectors and isotope and
orientation sequences, and PET series, correction, geometry, isotope, and
rescale requirements. NM vectors derive from the resolved caller frame count
rather than assuming the bounded two-frame default.

All four defaults pass root validation, reporting, caller native-pixel round
trips, multi-frame derivation checks, wrong-model rejection, and two-run byte
comparison. The pinned independent route identifies `USImage`,
`USMultiFrameImage`, `NMImage`, and `PETImage` without warning or error
findings. This closes P3.5 only; the remaining P3 lanes and gate stay open.

## 2026-08-28 — Phase P3.4 DX and mammography lane

Digital X-Ray For Presentation and Digital Mammography For Presentation and
For Processing are qualified composition templates. Their profiles model coded
anatomy and view, detector and positioning state, acquisition context, native
intensity/rescale fields, and presentation LUT behavior. Mammography
presentation uses MONOCHROME1 with inverse LUT semantics; processing uses
MONOCHROME2, processing intent, and no presentation window default.

All three defaults pass same-project root validation and reporting, exact
caller native-pixel round trips, wrong-photometric rejection before
publication, and two-run byte comparison. The pinned independent route reports
`DXImageForPresentation`, `MammographyImageForPresentation`, and
`MammographyImageForProcessing` with no warning or error finding.

This closes P3.4 only. The remaining P3 family lanes and the P3 breadth gate
remain open.

## 2026-08-28 — Phase P3.3 CT, MR, and CR lane

The first modality-specific classic-image lane is qualified. The catalog now
exposes CT Image Storage, MR Image Storage, and Computed Radiography Image
Storage templates with bounded deterministic defaults, explicit protected,
derived, conditional, and caller-settable policies, and exact native pixel
contracts. Caller-owned pixels round-trip for each permitted model; a wrong
signedness or other structural mismatch fails before output publication.

All three defaults pass composition plan/materialization validation, root
validation, composition reporting, and two-run byte comparison. The pinned
`dicom3tools-dciodvfy` executable recorded below identifies them as `CTImage`,
`MRImage`, and `CRImage` respectively with no warning or error finding. This is
template-specific independent IOD evidence and does not imply qualification
for compressed transfer syntaxes or a broader pixel domain.

P3.3 is a completed family lane, not the P3 breadth gate. DX/mammography,
US/NM/PET, VL, XA/XRF, and the multi-frame Secondary Capture families remain
to be promoted before P3 closes.

## 2026-08-28 — Phase P2 Secondary Capture gate

The shared plan engine is publicly exercised through two qualified Secondary
Capture templates: native unsigned monochrome and native 8-bit RGB. Template-
only specifications resolve deterministic non-PHI modules and pixels. Local raw
pixel inputs are staged under the documented resource policy, bound to an exact
pixel shape, and recorded with whole-value and per-frame SHA-256 evidence.

The P2 qualification uses `dicom3tools-dciodvfy` from
`conformance/validator-lock.json`: snapshot `1.00.snapshot.20260803085716`,
executable SHA-256
`1aeb75d6ccd3f193e3b322b6da77742cdce2e0604868eaf2a2669c786cbc27e5`.
Both default monochrome and RGB outputs were identified as `SCImage`, returned
success, and produced no warning or error finding. This independent IOD opinion
is additive to the project-owned typed, Part 10, content, plan-hash, and
manifest validation; it does not broaden the two descriptors beyond their
documented native pixel contracts.

P2 remains distinct from completion of the composition program. Classic image
breadth begins at P3, curated recipe migration at P4, enhanced and concatenated
objects at P5, non-image graphs at P6, extension/performance/API hardening at
P7, and full migration and qualification closeout at P8.
