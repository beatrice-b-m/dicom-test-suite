# Structural assembly standards foundation

**Recorded:** 2026-08-31

**DICOM baseline:** the `2026b` edition and source identities locked by
`standards.lock.json`

## Scope

This note establishes the standards boundary for the generic structural
assembly design. It does not qualify any caller-selected SOP Class or assert
that an arbitrary data-element set is an IOD-conformant instance.

## Official-standard anchors

- PS3.10 Section 7 and Table 7.1-1 define the DICOM File Format and File Meta
  Information. The product owns the preamble/prefix, Media Storage SOP Class
  and Instance UIDs, Transfer Syntax UID, implementation identity, and the
  required separation between group `0002` File Meta Information and the
  dataset. `SYSTEM_SPEC.md` Sections 3.4 and 4.3 retain the project policy for
  the primary knowledge-base gap around Table 7.1-1.
- PS3.5 Sections 6 and 7 define value representation, value multiplicity,
  explicit/implicit VR encoding, data-element structure, Sequence/Item
  encoding, even-length values, and byte ordering. Structural assembly checks
  those encoding rules but cannot derive an IOD's Type 1/2/3 or conditional
  requirements from them.
- PS3.5 Sections 6.2.2 and 7.8 define private creator values and private data
  element reservation. The existing locked project evidence is in
  `standards/source-notes/phase-2-private-creator-blocks.md`; the assembler uses
  managed reservations rather than permitting unscoped private elements.
- PS3.5 Annex A defines transfer-syntax encoding, including native Pixel Data
  value-field rules. `SYSTEM_SPEC.md` Sections 9.1, 9.3, and 9.4 and the current
  transfer-syntax capability matrix remain the executable project's scoped
  encoding policy. Availability in another workflow is not automatically an
  assembler qualification.
- PS3.3 Image Pixel and modality/IOD modules define when Rows, Columns, sample
  description, Pixel Data, Float Pixel Data, or Double Float Pixel Data are
  required and how they are interpreted for particular IODs. The structural
  assembler uses typed shape fields to keep payload encoding internally
  consistent, but does not claim to satisfy those module conditions.
- PS3.6 supplies standard element tags, keywords, and VRs. The bundled
  dictionary is the offline lookup authority for known elements. An unknown
  tag requires an explicit VR and receives a manifest warning rather than an
  invented dictionary or IOD meaning.

Existing object-specific source notes for native pixels, float/double-float
Parametric Maps, waveform, encapsulated document, mesh, private blocks, and
composition bulk content remain evidence for their qualified recipes/templates.
They inform shared encoding primitives but do not transfer their IOD claim to
structural assembly.

## Derived product rules

1. File Meta Information and dataset SOP identities must be generated from one
   protected identity declaration and cannot be raw caller overrides.
2. Known tags use a bundled dictionary-permitted VR; unknown tags require an
   explicit VR whose value encoding the product can validate.
3. Recursive Sequences and private blocks are structurally valid only when
   their item/creator encodings and resource bounds hold.
4. Typed bulk owns the payload tag and its declared shape fields so the
   manifest cannot claim a shape contradicted by raw attributes.
5. Generic Part 10/data-element success is not IOD conformance. The permanent
   structural projection is `iod_conformance = "not_assessed"`.

## Promotion evidence still required

Before S4 implementation is promoted, the exact assembly schema and writer
behavior must be checked against the locked DICOM edition and bundled
dictionary; positive and adversarial byte-level fixtures must cover the rules
above; and any content-kind-specific standards detail not already supported by
an official source note must receive one. `standards.lock.json` policy remains
unchanged, and no standards artifact is redistributed.
