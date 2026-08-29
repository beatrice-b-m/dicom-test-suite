# Composition Secondary Capture source note

- Checked: 2026-08-28
- DICOM edition: 2026b
- Scope: Secondary Capture Image Storage (`1.2.840.10008.5.1.4.1.1.7`)
- Qualification owner: `secondary_capture`

The initial composition templates use the Secondary Capture Image IOD module
table in PS3.3 and the native Pixel Data encoding rules in PS3.5. The shared
resolver supplies the mandatory Patient, General Study, General Series,
General Equipment, SC Equipment, General Image, Image Pixel, and SOP Common
attributes. SOP Class, SOP Instance, Study, Series, transfer syntax, native
pixel shape, and Pixel Data are derived or protected so caller operations
cannot contradict the resolved object identity or content contract.

The monochrome template permits unsigned 8- and 16-bit native samples with
`MONOCHROME1` or `MONOCHROME2`. The RGB template permits unsigned 8-bit native
RGB samples and supports both planar configurations. Both descriptors default
to Explicit VR Little Endian and may emit multiple frames through Number of
Frames when the supplied content contains more than one frame.

Qualification is intentionally limited to deterministic synthetic inputs and
the pinned independent IOD route. It does not imply qualification for
compressed transfer syntaxes, palette color, YBR, signed samples, or other SC
SOP Classes.

## Official anchors

- PS3.3, Secondary Capture Image IOD and its module table.
- PS3.3, SC Equipment, General Image, Image Pixel, and SOP Common modules.
- PS3.5, native Pixel Data encoding, value representation, byte ordering, and
  even-length value padding.
