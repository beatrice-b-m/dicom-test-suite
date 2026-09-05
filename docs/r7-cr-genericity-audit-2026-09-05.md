# R7 CR genericity preparation audit — 2026-09-05

Read-only source review after SC acceptance at `e21958c`; no generic CR
implementation, native execution or new qualification is claimed. Corpus
source provenance is accepted at `3e5256b`; baseline/import/parity precede this
future compatibility change.

## Bounded capability and shared-provider boundary

Add a CR-only inspection seam in `src/recipes/classic_mr_cr.rs` before its
historical recipe matcher. Require the complete native Explicit VR Little
Endian CR template@1, native content, classic MR/CR algorithm and projection
tuple. The shared algorithm/projection alone cannot distinguish MR from CR:
CR template or CR-specific overlay/modality-LUT/VOI-LUT parameters establish
intent; partial or crossed declarations must reject while pure historical MR
remains outside this inspector. Reject mixed artifacts.

Keep one logical instance/order0, explicit caller path and caller-owned unique
planning/projection order. Preserve empty untyped maps, shared validation and
curated projection at both levels; exclude unrelated algorithm/encoding,
metadata, attribute and nonsquare contracts. The accepted CR form uses U8/OB
MONOCHROME2, one frame and checked dimensions/count/range/hash validation.

The existing fragment checks do not fully qualify arbitrary overlay packing
or LUT shapes. Initially retain the evidenced overlay/LUT form: matching
geometry, G overlay, origin[1,1], bits1/position0 and padded data, four-entry
16-bit LUT descriptors/data, modality type US and no VOI type. Broader forms
need separate evidence. Check dimension products, overlay byte rounding and
LUT byte lengths before narrowing or allocation.

## Required integration and evidence discrepancy

`src/curated_execution.rs` still chooses MR validation by `classic/mr/` prefix
for the shared algorithm. Qualified CR must take precedence so a misleading
MR-named CR reaches CR validation. Shared planning already calls the MR/CR
planner; projection review found no additional CR name dispatch.

The `classic/cr` template limitation text says unsigned12-in16 pixels, while
the actual accepted recipe/planner emits U8/OB. Root confirmed both source
facts. Preserve the accepted payload bytes and record the limitation-text
discrepancy explicitly; it does not authorize changing pixels or inventing a
conformance result. A later documentation correction must respect template
identity/version policy and the source-pinned baseline.

## Sequential ownership and acceptance

1. MR/CR planner and pure planner tests, preserving four historical recipes.
2. Loader, shared execution dispatch, bundle/shared tests and exact routing.
3. Separate caller fixture and CLI/SDK proof after accepted CR migration parity.
4. Root-owned current guides and dated evidence/status.

Tests need arbitrary/misleading identities, paths and orders, crossed tuples,
maximum dimensions, pixel/hash/LUT/overlay corruption and historical regressions.
The original4013-byte recipe remains SHA-256
`7ee5bd86e7a83db9b484ce4cdb2f12243616749fdf392ec155ff3d7604b8c8d1`.

Verification: read-only source/recipe/template inspection by the assigned
agent and root, no Cargo, generated output or build storage. Diff check passed.
