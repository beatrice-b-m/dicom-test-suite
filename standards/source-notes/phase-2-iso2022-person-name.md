# Phase 2 ISO 2022 Person Name Evidence

Checked: 2026-08-26  
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `metadata/sc/iso2022_person_name_component_groups`
- Recipe ID: `metadata_sc_iso2022_person_name_component_groups`
- Manifest and raw validation rules for multi-valued Specific Character Set,
  ISO 2022 escape transitions, and three PN component groups

## Required Decision

Use the PS3.5 Japanese Example 1 contract exactly: an empty first Specific
Character Set value selects default ISO-IR 6, Value 2 is `ISO 2022 IR 87`, and
Patient Name decodes as
`Yamada^Tarou=山田^太郎=やまだ^たろう`. Raw validation must prove every
IR 87 invocation and IR 6 reset before the `^` and `=` delimiters.

## KB Query

- Tool: `dicom_search_standard_text`
- Inputs: `ISO 2022 IR 6 ISO 2022 IR 87 Person Name`, `PN equals delimiter
  reset ISO 2022 IR 87 escape`, and `Japanese Person Name ISO 2022 IR 87`, all
  filtered to PS3.5
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: `not_found` for all three compound text queries.
- Why insufficient: compound search did not resolve the normative example or
  reset rules, so the known official anchors were retrieved directly.

## Official Source Evidence

- PS3.5 Section 6.1.2.5 permits ISO/IEC 2022 code extension when Specific
  Character Set is multi-valued.
- PS3.5 Section 6.1.2.5.3 requires the Value 1/default repertoire before PN
  `^` and `=` delimiters and an explicit escape before the first use of another
  repertoire in each component and component group.
- PS3.5 Section H.3 defines alphabetic, ideographic, and phonetic group order.
- PS3.5 Section H.3.1 and Table H.3-1 provide the exact Example 1 declaration,
  decoded value, encoded octets, IR 87 escape `ESC 02/04 04/02`, and IR 6 reset
  `ESC 02/08 04/02` used by this recipe.
- Source artifact identity: the locked DICOM 2026b KB source manifest above.

## Project Action

- Registry status: planned until exact raw bytes, two independent reader
  round-trips, deterministic generation, and IOD/entity gates pass.
- Registry reason: the deterministic ISO 2022 recipe is not yet implemented.
- Should become KB patch: no; direct retrieval covers the required anchors.
- Expected cleanup after KB coverage exists: none.
