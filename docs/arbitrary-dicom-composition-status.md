# Arbitrary DICOM composition status

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
