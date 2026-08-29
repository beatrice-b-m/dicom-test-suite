# Registration and presentation-state composition source note

Checked against the locked DICOM 2026b base edition on 2026-08-29.

- PS3.3 Spatial Registration and Deformable Spatial Registration IODs establish
  registered Frame of Reference identity, referenced-instance closure, rigid
  matrices, deformation grids, and pre/post matrix semantics.
- PS3.3 Grayscale, Color, Blending, and Advanced Blending Presentation State
  IODs establish source-series closure, displayed areas, ordered blending
  inputs, transforms, palettes, opacity, and ICC color management.
- PS3.3 Common Instance Reference establishes study, series, SOP Class, SOP
  Instance, and optional frame consistency across derived-object graphs.

Composition rewrites every embedded referenced SOP, series, study, and Frame of
Reference UID from the qualified default artifact onto the resolved logical
graph before publication. Default bundles are deterministic and their source
members are not counted as caller-requested objects. Explicit sources suppress
the corresponding defaults and are subject to the same closure checks.
