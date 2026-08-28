# Phase 7 Deterministic Negative Mutation Evidence

Checked: 2026-08-28
Standards baseline: 2026b, `standards.lock.json`
Mutation contract: `0.1.0`
Negative recipe contract: `0.1.0`

## Scope And Profile Boundary

The `negative` profile contains intentionally invalid DICOM Part 10 byte
streams derived from deterministic known-good project cases. It is never a
member of `all`, `smoke`, `core`, `extended`, `legacy`, or `stress`. A negative
artifact is successful only when its exact mutation evidence is intact and a
bounded consumer outcome matches its case contract. Timeout, crash, signal
termination, or unbounded resource use is always a suite failure.

Generated negative bytes and valid mutation sources remain build artifacts.
The registry, recipe, exact source and output hashes, byte ranges, expected
failure layer, and bounded acceptable outcomes are committed contracts.

## Standards Boundaries Intentionally Violated

The cases exercise these locked encoding rules:

- PS3.10 Section 7 requires the Part 10 preamble/prefix and File Meta
  Information, fixes File Meta Information to Explicit VR Little Endian, and
  identifies the Transfer Syntax that encodes the following Data Set.
- PS3.5 Sections 7 and 7.1 require legal two-byte Explicit VR fields and Value
  Length fields that match the encoded Value Field.
- PS3.5 Sections 7.5 and 7.5.2 define Sequence and Item nesting. Undefined-
  length Sequences and Items require their corresponding Delimitation Items.
- PS3.5 Section 8.1 binds native Pixel Data length to its encoded samples and
  the Image Pixel description.
- PS3.5 Section A.4 requires encapsulated Pixel Data to contain a Basic Offset
  Table Item, explicit-length Fragment Items, correct Frame offsets, and a
  Sequence Delimitation Item. Extended Offset Tables use the same Fragment
  Item-Tag-relative layout locked by
  `standards/source-notes/phase-5-extended-offset-table.md`.
- PS3.3 IOD tables establish Type 1 presence requirements; deleting a Type 1
  Attribute is a semantic IOD failure even when the byte stream still parses.
- PS3.5 character-set and VR rules require declared repertoires and Values to
  agree. A malformed declaration or encoded Value may fail at text decoding
  or may be accepted only with the case's bounded warning outcome.

The current official PS3.5 and PS3.10 publications were checked as a
cross-edition confirmation. Their relevant structural rules are unchanged
from the repository's pinned 2026b baseline; the registry continues to cite
2026b rather than silently advancing the project lock.

## Valid Sources And Mutation Families

| Source case | Mutation families |
| --- | --- |
| `classic/sc/mono2_u8_explicit_le` | file-meta/dataset truncation, Explicit VR and Value Length corruption, Transfer Syntax and UID disagreement, missing Type 1, invalid pixel description/length, native Pixel Value truncation |
| `metadata/sc/utf8_person_name` | invalid Specific Character Set declaration and malformed encoded PN bytes |
| `metadata/sc/defined_undefined_sequence_lengths` | invalid nested Item length, truncated Sequence Item, and removal of an existing undefined-length Sequence Delimitation Item |
| `classic/sc/mono1_u8_rle_lossless` | truncated encapsulated Fragment |
| `encapsulation/sc/eot_single_fragment_multiframe` | broken Extended Offset Table entry |

The source selector is part of each recipe. It is not legal to substitute an
arbitrary file merely because it contains a similarly named Attribute.
Locators accept only known-good Explicit VR Little Endian or RLE Part 10 input,
enforce element/depth/item/fragment limits, and never allocate from an
untrusted declared Value Length.

## Evidence Contract

Every negative file records:

1. case ID, recipe version, source case ID, source Transfer Syntax, source
   shape, source SHA-256, output SHA-256, output size, and deterministic path;
2. each ordered mutation step, including stable mutation ID and typed
   parameters;
3. exact half-open source and output byte ranges for every edit;
4. the input and output SHA-256 for every step, with adjacent steps chained;
5. expected failure layer and a non-empty controlled set of acceptable
   outcomes; and
6. a bounded probe result that distinguishes clean rejection, parse failure,
   validation/decode failure, accepted-with-bounded-warning, timeout, and
   crash/signal termination.

Validation branches on `validity: expected_invalid` before ordinary valid-file
reopen and conformance logic. It verifies the evidence and independently probes
the artifact; an expected rejection is not counted as a valid-profile failure.
Conversely, negative evidence never suppresses a failure for a valid artifact.
Reports place negative outcomes in their own robustness section and exclude
them from valid IOD and pixel-conformance totals.

## Determinism And Promotion Gate

Promotion requires two clean same-seed runs with byte-identical negative files
and canonical manifests, exact source/output hash closure, strict profile
isolation, and focused tamper tests for mutation ranges and step chaining. All
fifteen named negative registry rows must produce bounded outcomes. The later
`fuzz` profile uses the same evidence vocabulary but remains separate and may
not promote an unbounded or unreproducible input into the named corpus.

Official anchors:

- PS3.10 Section 7, DICOM File Format;
- PS3.5 Sections 7, 7.1, 7.5, and 7.5.2, Data Set and nesting encoding;
- PS3.5 Section 8.1, native Pixel Data encoding; and
- PS3.5 Section A.4, encapsulated Pixel Data encoding.

Should become KB patch: no. These cases deliberately violate existing rules;
they do not expose missing standard knowledge.
