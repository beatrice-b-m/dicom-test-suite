# Composition structured-report source note

Checked: 2026-08-29 against the repository-pinned DICOM 2026b edition.

The Basic Text, Comprehensive, Comprehensive 3D, TID 1500, and Key Object
templates retain the fully typed, qualified SR content trees produced by their
curated recipes. PS3.3 C.17.3 establishes the recursive Content Sequence model
and value-type-specific attributes. The public parameter schemas expose only
the existing TEXT value, bounded NUM values with fixed UCUM units, and the
six-coordinate two-point SCOORD3D POLYLINE. Code sequences, relationship types,
tracking identities, container structure, and template identifiers are not
arbitrary caller trees.

The TID 1500 surface follows PS3.16 TID 1500 and its single qualified
measurement group. The Key Object surface follows TID 2010 and exposes the
selected Enhanced CT and SEG objects through typed reference slots. Every
reference is one-based, SOP-class constrained, cycle forbidden, and projected
both into the logical manifest graph and the embedded DICOM evidence/content
tree. Defaults generate their Enhanced CT and SEG closure deterministically.

These templates are synthetic interoperability fixtures. They do not claim
clinical measurement correctness, observer verification, or support for
arbitrary SR template construction.
