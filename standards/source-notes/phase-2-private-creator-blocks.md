# Phase 2 Private Creator Block Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `metadata/sc/private_creator_blocks`
- Recipe ID: `metadata_sc_private_creator_blocks`
- Manifest, generation, raw validation, and reporting rules for private creator
  reservation and private data elements

## Required Decision

Generate one Secondary Capture instance with three independently scoped private
creator reservations:

- `(0011,0010)` LO `DTS_PRIVATE_ALPHA` reserves block `10`; private elements
  `(0011,1001)` LO `ALPHA-GROUP-0011` and `(0011,10F0)` US `4660` occupy it.
- `(0011,0012)` LO `DTS_PRIVATE_BETA` reserves block `12`; private element
  `(0011,1201)` LO `BETA-BLOCK-12` occupies it.
- `(0013,0011)` LO `DTS_PRIVATE_ALPHA` independently reserves block `11` in a
  second odd group; private element `(0013,1101)` LO `ALPHA-GROUP-0013`
  occupies it.

The recipe proves that block identifiers come from the creator element's low
byte, that two creators can coexist in one group without collision, and that a
creator string reused in another group receives that group's independent block.
The US value is emitted as typed little-endian binary, not a string surrogate.

## KB Query

- Tool: `dicom_search_standard_text`
- Input: `Private Creator Data Element private block 0010 00FF`, PS3.5 filter.
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the query surface identifies private creator terminology but does not
  expose the full reusable block-allocation algorithm needed by the recipe.
- Why insufficient: exact creator-to-block mapping requires the official PS3.5
  private data element section.

## Official Source Evidence

- PS3.5 Section 7.8.1 reserves `(gggg,0010-00FF)` in odd groups for Private
  Creator Data Elements and maps the creator element's low byte to the high
  byte of its reserved `(gggg,xx00-xxFF)` block.
- PS3.5 Section 6.2.2 requires private creator values to use LO and limits them
  to the default character repertoire.
- PS3.5 Section 7.8 scopes private creator reservations independently by group.
- Source artifact identity: the locked DICOM 2026b KB source manifest above.

## Project Action

- Registry status: implemented after the typed manifest contract, native
  recipe, exact raw validation, reports, and independent conformance gates
  passed.
- Manifest decision: record each group, creator tag/value, reserved block, and
  typed private payload with exact raw Value Length and SHA-256.
- Should become KB patch: no; the required allocation algorithm is a narrow
  PS3.5 recipe decision rather than a missing dictionary row.
- Expected cleanup after KB coverage exists: none.

## Promotion Evidence

- Two seed-1 `core` generations produced 42 byte-identical files and zero
  strict validation failures. The private fixture SHA-256 is
  `cd7e529698c8716890da44045faaef6b218d35e18e91543103877971fe82a56c`.
- `dciodvfy` accepted the Secondary Capture IOD and emitted only its expected
  informational warnings that the four private data tags are unrecognized;
  `dcentvfy` was silent.
- DCMTK `dcmdump` independently recovered all three LO creator reservations,
  the three private LO values, and private US value `4660` at their exact
  tags and VRs.
- Pydicom 3.0.2 from the repository's locked offline `uv` environment
  recovered all seven typed values. Its rewrite was byte-identical to the
  native fixture and passed the same dicom3tools and DCMTK checks.
