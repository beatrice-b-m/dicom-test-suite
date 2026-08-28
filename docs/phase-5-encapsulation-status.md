# Phase 5 Encapsulation Status

**Qualified:** 2026-08-28

Phase 5 milestone 1 is complete for the small real object and the simulated
32-bit range crossing. The large on-disk stress object remains behind the
Phase 6 resource-budget checkpoint.

## Real Extended Offset Table object

`encapsulation/sc/eot_single_fragment_multiframe` is a byte-stable,
extended-profile Multi-frame Grayscale Byte Secondary Capture instance using
RLE Lossless. Its three 2 by 2 Frames have one Fragment each, an empty-present
Basic Offset Table, Extended Offset Table Values `[0, 78, 152]`, and Extended
Offset Table Lengths `[69, 66, 69]`. The Lengths are the unpadded compressed
bitstream lengths; the Fragment Item Value lengths are `[70, 66, 70]`.

The generator reopens the object and checks both `OV` Attributes, the Pixel
Data Item layout, frame increment metadata, required grayscale rescale/display
Attributes, and exact decoded pixels. Strict manifest-driven validation
recomputes every Item-Tag-relative offset from reopened bytes and rejects
missing, mismatched, padded-length, or multi-fragment representations.

A fresh extended seed-1 corpus contains 115 files and validates with zero
strict failures. Locked conformance run
`329321c97edeb1f624d9aba7399c70c317036a06ab1f88393b1ef9a2176cd649`
records clean `dciodvfy`, clean DCMTK parsing, and independent DCMTK RLE decode
with all three native frame hashes matching. The qualified instance SHA-256 is
`a5ca7e3750bf5363d17989eb7d72ea398eafbaa9a86254c719636ee51c80b79a`.

## Simulated overflow qualification

`qualification/encapsulation/eot_u64_overflow` is an implemented non-file
qualification fixture. Checked arithmetic computes the second virtual offset
as `0x1_0000_0006`, proves it is not representable in a populated 32-bit Basic
Offset Table, and separately rejects `u64` overflow. The registry schema
requires this fixture to have empty profiles and null DICOM identity, so it
cannot be mistaken for a generated or independently validated large object.

## Remaining Phase 5 boundary

The next lossy case is JPEG-LS Near-Lossless. Promotion requires a project
decision on the shared numeric acceptance policy: per-channel maximum error,
aggregate metric, metadata assertions, and independent-decoder requirements.
No lossy or video row is promoted by the EOT qualification.
