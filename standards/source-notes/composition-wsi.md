# Whole-slide microscopy composition source note

Checked against the locked DICOM 2026b base edition on 2026-08-29.

- PS3.3 VL Whole Slide Microscopy Image IOD and tiled-pixel modules establish
  total-pixel-matrix dimensions, tile positions, optical-path identity,
  specimen context, image flavors, and full-versus-sparse frame organization.
- PS3.3 Multi-frame Functional Groups, Dimension Organization, and Dimension
  Index modules establish explicit sparse placement and implicit TILED_FULL
  ordering, including optical path as a dimension axis.
- PS3.3 Pyramid UID and WSI image flavor semantics establish which volume and
  thumbnail instances share pyramid membership and distinguish label images.
- PS3.5 native Pixel Data encoding establishes exact RGB frame byte length,
  sample layout, value padding, and transfer-syntax byte order.

Sparse WSI retains the repository's pinned independent evidence boundary:
`pydicom-dicom-validator-wsi-sparse` is the primary IOD authority, while the
exact known dicom3tools full-grid-cardinality finding remains required
characterization and is not treated as a pass or allowlisted.

Composition defaults reuse the suite's qualified reduced WSI objects, resolve
them through the shared typed plan, and expose pyramid companions as a closed
deterministic default bundle. Caller content is accepted only when its exact
frame shape matches the selected structural template. These reduced objects do
not qualify full-scale WSI resource behavior.
