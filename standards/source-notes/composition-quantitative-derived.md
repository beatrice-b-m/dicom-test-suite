# Composition quantitative-derived source note

Checked: 2026-08-29 against the repository-pinned DICOM 2026b edition.

The public quantitative templates preserve the qualified curated structures and
only expose typed payload and reference surfaces. PS3.3 A.51 and the
Segmentation Image, Segment Identification, Derivation Image, and Common
Instance Reference modules bind each SEG frame to its segment and source
frame. Binary frames remain continuously bit packed; fractional probability
frames retain Maximum Fractional Value; label-map values remain in the declared
segment-number domain. The WSI specialization retains slide-coordinate and tile
frame references.

PS3.3 A.75 and C.8.32 bind Parametric Map Float Pixel Data or Double Float
Pixel Data to multi-frame functional groups, derivation references, and the
Real World Value Mapping Macro. Caller payloads are accepted only at the exact
qualified shape, byte width, and length, and every floating value must be
finite. The source geometry is a closed three-instance CT series.

PS3.3 A.46 defines the standalone Real World Value Mapping IOD. Its public
template exposes source-frame references while retaining the qualified mapped
range, slope, intercept, and UCUM unit code. No missing source or unavailable
backend is represented as a pass; the default bundle must generate every
referenced source deterministically before publication.
