# Arbitrary DICOM composition status

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
