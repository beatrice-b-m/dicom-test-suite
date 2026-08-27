# Phase 2 UTF-8 Person Name Evidence

Checked: 2026-08-26  
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `metadata/sc/utf8_person_name`
- Recipe ID: `metadata_sc_utf8_person_name`
- Manifest and validation rules for Specific Character Set and decoded PN
  component groups

## Required Decision

The instance declares `ISO_IR 192`, encodes Patient Name as UTF-8, and records
the exact decoded alphabetic and ideographic component groups. Validation must
prove both the raw character-set declaration and the decoded Unicode value;
successful parsing alone is not sufficient.

## KB Query

- Tools: `dicom_search_standard_text`, `dicom_retrieve_standard_text`
- Inputs: `ISO_IR 192 UTF-8`, PS3.5 `sect_6.1`, `sect_6.1.2.2`,
  `sect_6.2.1.2`, and `sect_J.1`
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the persisted text directly covers the required UTF-8 repertoire,
  Specific Character Set declaration, PN group ordering, delimiters, and a
  Unicode PN byte example.
- Why insufficient: the individual KB results are authoritative enough for the
  decisions, but this note binds those separate anchors into one reviewable
  recipe contract.

## Official Source Evidence

- PS3.5 Section 6.1 identifies ISO IR 192 as UTF-8.
- PS3.5 Section 6.1.2.2 requires replacement repertoires used by PN values to
  be declared in Specific Character Set `(0008,0005)`.
- PS3.5 Section 6.2.1.2 defines up to three PN component groups, ordered
  alphabetic, ideographic, then phonetic, separated by `=`.
- PS3.5 Section J.1 demonstrates `ISO_IR 192` with an alphabetic and
  ideographic Chinese PN encoded as UTF-8.
- Source artifact identity: the locked DICOM 2026b KB source manifest above.

## Project Action

- Registry status: planned until the complete vertical slice passes
  deterministic and independent conformance gates.
- Registry reason: deterministic native recipe is not yet implemented.
- Should become KB patch: no; the persisted standard text covers the rules.
- Expected cleanup after KB coverage exists: none.
