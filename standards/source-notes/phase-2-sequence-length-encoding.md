# Phase 2 Sequence Length Encoding Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `metadata/sc/defined_undefined_sequence_lengths`
- Recipe ID: `metadata_sc_defined_undefined_sequence_lengths`
- Explicit VR Little Endian SQ Value Length, Sequence Delimitation Item, and
  equivalent decoded item-content validation

## Required Decision

Generate two Secondary Capture instances with the same one-item Anatomic
Region Sequence `(0008,2218)`. Each item encodes Code Value `69536005`, Coding
Scheme Designator `SCT`, and Code Meaning `Head`.

- The `defined` instance encodes an explicit SQ Value Length and omits the
  Sequence Delimitation Item.
- The `undefined` instance encodes SQ Value Length `FFFFFFFFH` and terminates
  the sequence with `(FFFE,E0DD)` length zero.
- Both instances retain an undefined-length item terminated by `(FFFE,E00D)`.
  The slice varies only the sequence length strategy, so decoded item content
  and item encoding remain directly comparable.

The manifest must record the exact raw sequence length field, delimiter state,
item length strategy, and decoded code triplet. Validation must inspect raw
Explicit VR Little Endian bytes independently of the object-model decode.

## KB Query

- Tool: `dicom_search_standard_text`
- Input: `Sequence of Items Explicit Length Undefined Length Sequence
  Delimitation Item`, PS3.5 filter.
- Edition returned: 2026b.
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the query surface identifies sequence and delimiter terminology but
  does not expose enough encoding detail to lock the byte-level comparison.
- Why insufficient: exact Value Length and delimiter invariants require the
  normative delimitation section and encoding tables.

## Official Source Evidence

- PS3.5 Section 7.5.2 permits an SQ Value to use either explicit length or
  undefined length and requires decoders to support both encodings.
- For explicit length, the 32-bit SQ Value Length is the byte count of all
  encoded Items and no Sequence Delimitation Item terminates the Value.
- For undefined length, the SQ Value Length is `FFFFFFFFH` and a Sequence
  Delimitation Item `(FFFE,E0DD)` with zero length follows the last Item.
- PS3.5 Section 7.5.1 permits an Item to use undefined length and requires an
  Item Delimitation Item `(FFFE,E00D)` with zero length in that form.
- Source artifact identity: the locked DICOM 2026b KB source manifest above.

## Writer-Control Evidence

- `dicom-core` 0.9.1 exposes `DataSetSequence::new(items, Length)` and
  `Length::UNDEFINED`.
- `dicom-parser` 0.9.1 exposes the `NoChange` explicit-length writer strategy.
  The default strategy deliberately converts explicit sequence lengths to
  undefined, so this case must opt into `NoChange` after computing and
  validating the exact defined length.
- The recipe will fail closed if the computed item bytes differ from the
  declared sequence Value Length.

## Project Action

- Registry status: planned until schema, generator, byte-level validation,
  reports, determinism, and independent conformance gates pass.
- Should become KB patch: no; this is a narrow byte-encoding decision already
  anchored by PS3.5 Sections 7.5.1 and 7.5.2.
- Expected cleanup after KB coverage exists: none.
