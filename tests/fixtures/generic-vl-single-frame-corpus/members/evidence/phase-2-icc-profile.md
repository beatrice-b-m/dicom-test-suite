# Phase 2 ICC Profile Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `vl/photo/rgb_icc_profile_explicit_le`
- Recipe ID: `vl_photo_rgb_icc_profile_explicit_le`
- VL Photographic Image Storage identity, native RGB Pixel Data, embedded ICC
  Profile, Color Space declaration, manifests, validation, reports, and
  independent color-management evidence

## Required Decision

Clone the existing 2 by 2 native RGB, planar-0 VL Photographic recipe so the
only new compatibility axis is color management. Add the optional ICC Profile
Module. Once selected, its ICC Profile `(0028,2000)` is Type 1, OB, VM 1. Add
the Type 3 Color Space `(0028,2002)` with the defined term `SRGB` so the
well-known color-space claim is explicit and consistent with the profile.

Use the 736-byte `DCMTK_SRGB_ICC_SAMPLE` from DCMTK 3.7.0
`dcmiod/iccexample.h`. It is the purpose-built DICOM example derived from the
CC0 `sRGB-v2-magic.icc` 182-point-curve profile. The embedded profile itself
declares copyright `CC0`; source provenance and the applicable DCMTK notice
remain visible in the repository. Store the source profile as reviewable hex
recipe data rather than discovering a platform profile or committing a
generated DICOM artifact.

Lock these exact profile properties:

- byte length and declared header size: `736` (`0x000002E0`);
- SHA-256:
  `8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef`;
- ICC version: `2.1.0` (`0x02100000`);
- Device/Class Signature at header bytes 12 through 15: `scnr`;
- input Color Space Signature at bytes 16 through 19: `RGB `;
- Profile Connection Space at bytes 20 through 23: `XYZ `;
- profile signature at bytes 36 through 39: `acsp`;
- rendering intent: perceptual (`0`);
- tag count: `9`; and
- profile description/copyright: `sRGB` / `CC0`.

Ordinary operating-system sRGB profiles are commonly display-class `mntr`
profiles and therefore do not satisfy the DICOM input-profile constraint. They
must not be substituted at generation time.

## KB Query

- Tool: `dicom_lookup_data_element`, `dicom_lookup_sop_class`,
  `dicom_list_modules_for_iod`, `dicom_list_attributes_for_module`,
  `dicom_resolve_attribute_context`, `dicom_search_standard_text`, and
  `dicom_retrieve_standard_text`
- Input: ICC Profile, Color Space, VL Photographic Image Storage and IOD, ICC
  Profile Module, PS3.3 Sections C.11.15, C.11.15.1.1, and C.11.15.1.2
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the KB resolves the optional IOD module, Type 1 ICC payload, Type 3
  color-space label, OB/VM 1 registry identity, and mandatory ICC header
  constraints.
- Why insufficient: the parsed KB does not expose the Color Space defined
  terms or a reusable typed profile-header contract. This note freezes the
  `SRGB` term, exact profile bytes, provenance, and cross-field recipe decision.

## Official Source Evidence

- PS3.3 Table A.32.4-1 permits the ICC Profile Module in the VL Photographic
  Image IOD. Selecting that module makes its requirements applicable.
- PS3.3 Table C.11.15-1 defines ICC Profile `(0028,2000)` as Type 1 and Color
  Space `(0028,2002)` as Type 3; the latter shall agree with an ICC Profile
  when both are present.
- PS3.3 Section C.11.15.1.1 permits only Input Device profiles: header bytes
  12 through 15 are `scnr`; the input color space is `RGB `; and the PCS is
  either `Lab ` or `XYZ `. This recipe selects `XYZ ` and perceptual rendering
  intent.
- PS3.3 Section C.11.15.1.2 defines `SRGB` as the IEC 61966-2-1 well-known
  RGB color space term.
- PS3.3 Table C.8-77 retains the existing VL Photographic RGB, planar-0,
  unsigned 8-bit pixel constraints. PS3.4 Table B.5-1 and PS3.6 Table A-1
  identify VL Photographic Image Storage UID
  `1.2.840.10008.5.1.4.1.1.77.1.4`; PS3.6 Table 6-1 defines the ICC and Color
  Space element registry properties.
- Source artifact identity: the locked DICOM 2026b KB source manifest above;
  official DICOM source artifacts remain uncommitted under repository policy.

## Qualification Plan

- Internal validation will reopen `(0028,2000)` and bind its OB bytes, hash,
  declared size, header signatures, rendering intent, tag table, and `SRGB`
  declaration to the manifest contract.
- Locked dicom3tools `dciodvfy` remains the independent IOD validator and
  `dcentvfy` the entity checker. A separate fingerprinted LittleCMS path will
  parse and transform through the extracted profile; locked DCMTK extraction
  will bind the exact DICOM Value Field.
- Negative controls will change `scnr` to `mntr`, corrupt `acsp` or the
  declared profile size, and mismatch Color Space. Acceptance is based on
  normalized findings and exact semantic evidence, not process exit alone.

## Independent Validator Qualification

The selected case-scoped validator is LittleCMS 2.19 `transicc`, calculator
version 5.1. The executable SHA-256 is
`44a0fe12b05c82c80ce04001a2a0abea737cf8cd3efc0c4f9fe8aa483913331f`;
the linked `liblcms2.2.dylib` SHA-256 is
`c74076bc75654249cd88fee91aa4413c9cf00d3708710cf652bef04eec1a9ad1`;
their committed composite adapter fingerprint is
`498f65088efa9f32a013a26232336348a3c195eb9cb8f487411f2fe51e085328`.
The separately locked DCMTK `dcmdump` fingerprint remains
`d2261944ea1ceb6743df9866f2237014b284fa39119c8a5eee226ae922ead45f`.

Capability controls showed that `dciodvfy` rejects an empty selected ICC
Profile but accepts corrupt `acsp`, monitor-class `mntr`, and a mismatched
`ADOBERGB` label. The evaluated `uv`-locked pydicom `dicom-validator` likewise
did not enforce those semantics. LittleCMS rejects the corrupt signature and
an invalid required transform-tag offset, but accepts both input- and
monitor-class profiles; the strict adapter must therefore enforce the DICOM
`scnr` class and every other locked header/label invariant itself.

On 2026-08-27 a fresh isolated generated instance passed `dciodvfy -new` and
`dcentvfy` without findings. DCMTK reconstructed all 736 ICC bytes and
LittleCMS produced XYZ vectors `43.6035 22.2443 1.3901`,
`38.5101 71.6934 9.7076`, `14.3066 6.0623 71.3928`, and
`96.4203 100.0000 82.4905` for red, green, blue, and white. The evidence tools
were lock-matched and strict conformance verification reported zero failures.
Automated controls reject unavailable tools and hash-corrected but
semantically relinked evidence.

## Project Action

- Registry status: implemented after the deterministic recipe, strict
  manifest contract, and independent conformance path passed.
- Registry reason: no remaining implementation blocker.
- Should become KB patch: yes; expose Color Space defined terms and the ICC
  header constraints as typed query results.
- Expected cleanup after KB coverage exists: replace the local term/header
  interpretation with direct typed KB evidence while retaining the exact
  source-profile provenance and hash.
