# Composition bulk-content source note

Checked: 2026-08-29 against the repository-pinned DICOM 2026b edition.

This note records the standards boundary for the public waveform, encapsulated
PDF, and encapsulated STL composition templates. PS3.3 A.34.3 and A.34.4 place
Waveform Data (5400,1010) in each Waveform Sequence (5400,0100) item; the
multiplex group channel count, sample count, bits allocated, and sample
interpretation determine the exact payload length and decoding. The existing
case-specific waveform source notes retain the detailed channel and coded-term
evidence.

PS3.3 A.45.1 and the Encapsulated Document Module require the PDF byte stream
in Encapsulated Document (0042,0011), its MIME type, and the unpadded
Encapsulated Document Length (0042,0015). PS3.3 A.85 applies the same bulk
element contract to Encapsulated STL and adds the Manufacturing 3D Model,
Frame of Reference, measurement-unit, and model-property requirements. The
qualified STL slot is binary STL: its 80-byte header and four-byte little-endian
triangle count are followed by exactly 50 bytes per triangle.

Composition therefore treats these values as typed, hash-addressed bulk data.
Waveform replacement is exact-length because the template keeps group topology
fixed. PDF and STL replacement may vary within the resource envelope; the
composer derives (0042,0015) from the unpadded bytes and performs an independent
format-level check in addition to the pinned IOD route. This evidence does not
claim clinical ECG interpretation, PDF active-content safety, or anatomical
correctness of a supplied mesh.
