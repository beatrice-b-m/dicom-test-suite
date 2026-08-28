# Phase 5 Extended Offset Table Evidence

Checked: 2026-08-28
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case IDs: `encapsulation/sc/eot_single_fragment_multiframe` and
  `encapsulation/sc/eot_simulated_overflow`
- Recipe IDs: `encapsulation_sc_eot_single_fragment_multiframe` and
  `encapsulation_sc_eot_simulated_overflow`
- Schema and validation surface: encapsulated Pixel Data layout, Basic Offset
  Table state, Extended Offset Table state, frame-to-fragment mapping, and
  checked offset arithmetic

## Required Decision

The small case shall be an encapsulated multi-frame image with exactly one
Fragment per Frame. It shall carry non-empty Extended Offset Table
(`7FE0,0001`) and Extended Offset Table Lengths (`7FE0,0002`) Values and an
empty Basic Offset Table Item. The two Extended Offset Table Attributes are
top-level Image Pixel Module Attributes, not Items inside Pixel Data. Their
tags sort immediately before Pixel Data (`7FE0,0010`).

The simulated-overflow case is an arithmetic qualification fixture. It shall
exercise the same checked `u64` calculation past the Basic Offset Table's
32-bit range without allocating, writing, validating, or claiming to validate
a multi-gigabyte Pixel Data payload. A later opt-in stress object is required
to qualify actual large-file I/O.

## Attribute And Encapsulation Contract

PS3.6 Table 6-1 assigns both Attributes VR `OV` and VM `1`:

| Attribute | Tag | VR | Value contract |
| --- | --- | --- | --- |
| Extended Offset Table | `(7FE0,0001)` | `OV` | one unsigned 64-bit offset per Frame |
| Extended Offset Table Lengths | `(7FE0,0002)` | `OV` | one unsigned 64-bit compressed-bitstream length per Frame |

`OV` is a stream of 64-bit words whose byte order follows the negotiated
Transfer Syntax. For the project's little-endian encapsulated syntaxes, each
word is serialized little-endian. The Attribute Value Length is therefore
exactly `8 * NumberOfFrames` for each table.

PS3.3 Table C.7-11a and Section C.7.6.3.1.8 establish all of these invariants:

- Pixel Data is present and its Transfer Syntax uses Encapsulated Format.
- The Transfer Syntax separates Frames into Fragments.
- Every Frame is entirely contained in one and only one Fragment. An Extended
  Offset Table is not valid for native Pixel Data or a multi-fragment Frame.
- Pixel Data's mandatory first Item is the Basic Offset Table Item and has
  zero Item Length. "Empty Basic Offset Table" means the Item is present with
  no Value; it does not mean that the Item itself is omitted.
- Extended Offset Table is optional under those conditions, but, when present,
  it is not permitted to be empty. It has exactly one offset per Frame and its
  first Value is zero.
- Extended Offset Table Lengths is Type 1C and is required whenever Extended
  Offset Table is present. It has exactly one length per indexed Frame.
- In a Concatenation, each Instance indexes only the Frames in that Instance.
  Re-encoding Pixel Data requires recomputing or removing the tables.

PS3.5 Section A.4 additionally requires encapsulated Pixel Data VR `OB`, an
undefined Pixel Data Value Length, one explicit-length Item per Fragment, even
and non-zero Fragment Item Value Lengths, and a Sequence Delimitation Item.
The final Fragment of a Frame may be padded to make its Item Value even.

## Offset And Length Semantics

An Extended Offset Table Value points to the first byte of the Item Tag
`(FFFE,E000)` for the Frame's sole Fragment. Offsets use the first byte of the
first Fragment Item Tag after the empty Basic Offset Table Item as origin zero.
They do not point to the Fragment Value and do not include the eight-byte Basic
Offset Table Item in their origin.

Let `L[i]` be the compressed bitstream length for Frame `i`, before any trailing
Item padding, and let `P[i] = L[i] + (L[i] mod 2)` be the Fragment Item Value
Length written on the wire. With exactly one Fragment per Frame:

```text
offset[0] = 0
offset[i + 1] = checked_add(offset[i], checked_add(8, P[i]))
extended_length[i] = L[i]
```

The `8` is the four-byte Item Tag plus four-byte explicit Item Length. Thus an
offset includes every preceding Fragment Item header, padded Item Value, and
padding byte. In contrast, Extended Offset Table Lengths records only the
compressed bitstream length. It excludes the Item Tag, Item Length field, and
any trailing padding; consequently an Extended Offset Table Length may be odd.

The following three-Frame byte-layout oracle deliberately includes an odd
middle bitstream. It is a serialization-unit fixture; promoted DICOM content
must also contain bitstreams valid for its declared Transfer Syntax.

| Frame | Bitstream `L` | Item Value `P` | EOT offset | EOT Length |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 4 | 4 | 0 | 4 |
| 2 | 5 | 6 | 12 | 5 |
| 3 | 6 | 6 | 26 | 6 |

For a little-endian Transfer Syntax, the table Value bytes are:

```text
Extended Offset Table:
00 00 00 00 00 00 00 00  0C 00 00 00 00 00 00 00
1A 00 00 00 00 00 00 00

Extended Offset Table Lengths:
04 00 00 00 00 00 00 00  05 00 00 00 00 00 00 00
06 00 00 00 00 00 00 00
```

The Pixel Data Value begins with the empty Basic Offset Table Item bytes
`FE FF 00 E0 00 00 00 00`. The first Fragment Item follows it. Its offset is
zero even though its Item Tag is eight bytes after the start of the Pixel Data
Value. The second Fragment begins 12 bytes after that origin: eight bytes of
first-Item header plus four bytes of first-Item Value. The third begins after
another 14 bytes: eight bytes of second-Item header plus its six-byte padded
Value.

## Simulated 32-bit Overflow Contract

A populated Basic Offset Table stores concatenated unsigned 32-bit offsets.
The implementation shall compute the canonical offsets in checked `u64`
arithmetic first and decide Basic Offset Table representability only after the
full Values are known. It shall never truncate, wrap, saturate, cast before
checking, or infer representability from payload length alone.

Use virtual compressed lengths `[0xFFFF_FFFE, 2]`. The first length is the
largest even explicit Fragment Item Value Length. No bytes are allocated. The
expected calculation is:

```text
offset[0] = 0
offset[1] = 0 + 8 + 0xFFFF_FFFE = 0x1_0000_0006
```

`offset[1]` is greater than `u32::MAX` (`0xFFFF_FFFF`), so a populated Basic
Offset Table cannot represent this layout. The Extended Offset Table can
represent both offsets as `u64`. The arithmetic helper must also return a
controlled error when `8 + P[i]` or the cumulative offset exceeds `u64::MAX`;
that implementation-overflow path is separate from the expected 32-bit Basic
Offset Table range crossing.

The simulator records inputs, padded Item lengths, computed offsets, the first
non-representable Frame index, and the exact boundary comparison. It does not
emit a DICOM instance with fictional offsets or an absent corresponding
payload. Any manifest/report representation must label this as simulated
arithmetic evidence, not as a generated large-object validation result.

## Validation And Deterministic Test Requirements

Promotion of the small case requires reopened-file validation independent of
the generator's in-memory metadata. The validator shall:

1. require encapsulated `OB` Pixel Data with undefined Value Length;
2. find the present, zero-length Basic Offset Table Item;
3. decode both `OV` Values in Transfer Syntax byte order and require table
   counts equal to Number of Frames;
4. require one Fragment Item per Frame and reject empty EOT, missing EOT
   Lengths, or either table without the other;
5. derive Fragment Item Tag positions from the reopened bytes and compare them
   exactly with the EOT origin and offsets;
6. distinguish the compressed bitstream length from the padded Item Value
   Length and compare the unpadded bitstream length with EOT Lengths;
7. independently decode every Frame and compare exact native-frame SHA-256
   values; and
8. require strict IOD validation and parsing through the project's locked
   independent routes.

Unit tests shall cover the numerical three-Frame oracle, an odd compressed
length, empty and populated Basic Offset Table rejection/acceptance as
appropriate, zero and mismatched table counts, multi-fragment rejection,
native Pixel Data rejection, little-endian `OV` serialization, the exact
`0x1_0000_0006` simulated crossing, and checked `u64` failure. Mutation tests
shall prove that changing an Item header, padding-sensitive offset, table word,
length word, Frame count, or Fragment count is detected.

Generation uses fixed pixels, metadata, UIDs, codec configuration, and Frame
order. Two clean runs with the same seed must produce byte-identical small
instances, manifests, validation sidecars, and reports. Generated DICOM and
simulation outputs remain outside git. The genuine large-object case belongs
to the opt-in `stress` profile and is not established by this note.

## KB Query And Official Source Evidence

- `dicom_lookup_data_element ExtendedOffsetTable` returned
  `(7FE0,0001)`, VR `OV`, VM `1`, non-retired.
- `dicom_lookup_data_element ExtendedOffsetTableLengths` returned
  `(7FE0,0002)`, VR `OV`, VM `1`, non-retired.
- `dicom_retrieve_standard_text PS3.3 sect_C.7.6.3.1.8` supplied the Item Tag
  origin, one-Fragment-per-Frame, empty Basic Offset Table, and first-offset
  rules.
- `dicom_retrieve_standard_text PS3.3 table_C.7-11a` supplied the presence
  conditions, required Lengths Attribute, unpadded length semantics, and
  Concatenation/re-encoding rules.
- `dicom_retrieve_standard_text PS3.5 sect_A.4` supplied the encapsulated Pixel
  Data Item, explicit Item Length, even Fragment Value, padding, undefined
  Pixel Data length, and Basic Offset Table encoding rules.
- `dicom_search_standard_text PS3.5 "OV 64-bit words multiple of 8 bytes"`
  resolved PS3.5 Section 6.2 and Table 6.2-1 for the `OV` word contract.
- All successful queries returned edition 2026b and source manifest SHA-256
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`.

Official anchors:

- PS3.3 Table C.7-11a, Image Pixel Module Attributes;
- PS3.3 Section C.7.6.3.1.8, Extended Offset Table;
- PS3.5 Section 6.2 and Table 6.2-1, Value Representation;
- PS3.5 Section A.4, Transfer Syntaxes for Encapsulation of Encoded Pixel Data;
  and
- PS3.6 Table 6-1, Registry of DICOM Data Elements.

The repository lock records official PS3.3, PS3.5, and PS3.6 artifacts as
`unavailable_not_downloaded`; no official source artifact is added here. The
pinned KB fully covers the needed text and reports the locked manifest identity,
so this note needs no `standards.lock.json` change.

## Project Action

- Registry status remains `planned` until registry evidence, generation,
  manifest extraction, independent validation, reporting, tests, and two-run
  reproducibility are integrated.
- Recommended small-case profile: `extended`, provider `rust_native`,
  determinism `byte_stable`.
- Recommended overflow-fixture profile: `extended` arithmetic qualification;
  it must remain visibly distinct from a generated large DICOM object.
- The later genuine large case is `stress` only and requires explicit byte,
  memory, runtime, and CI-scheduling budgets.
- Should become KB patch: no; the pinned 2026b KB covers the required anchors.
- Expected cleanup after KB coverage exists: none.
