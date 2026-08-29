# Composition radiotherapy source note

- Checked: 2026-08-29
- Baseline: the official DICOM Standard edition locked by `standards.lock.json`
- Scope: RT Structure Set, RT Dose, RT Plan, RT Image, C-Arm
  Photon-Electron Radiation, and RT Radiation Set composition templates

## Standards anchors

PS3.3 defines the six IODs and their reference relationships. The qualified
composition graph preserves the curated suite's closed RT chain: image evidence
feeds a Structure Set and Dose, those objects feed a Plan, the Plan feeds RT
Image and Radiation, and Plan plus Radiation feed Radiation Set. PS3.5 defines
the native Pixel Data encoding used by the fixed RT Dose and RT Image content
slots. SOP Class identities are taken from PS3.4 and PS3.6.

## Safe composition boundary

The templates expose typed references rather than arbitrary UID-valued tags.
The serialized DICOM references are rewritten from the resolved graph, so a
caller cannot create a mismatch between the logical graph and the Part 10
objects. RT Dose accepts exactly two 2 by 2 unsigned 16-bit frames and RT Image
accepts exactly one 4 by 4 unsigned 8-bit frame. Content length and hashes are
checked before publication.

Only three bounded semantic controls are exposed: Structure Set ROI Name, Dose
Grid Scaling, and RT Plan Label. They preserve the qualified contour, dose-grid,
beam, fraction, control-point, treatment-position, and reference topology.
Other changes must use typed attribute overrides and remain subject to the
composition override policy.

## Evidence boundary

Default objects are produced by the existing deterministic curated writers and
then materialized through the shared composition plan. Same-project validation
checks identity, content integrity, and graph closure. Independent IOD and graph
qualification remains a distinct route and is never inferred from successful
same-project generation alone.
