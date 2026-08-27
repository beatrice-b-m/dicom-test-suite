# Phase 3 Color Softcopy Presentation State Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`
Source manifest SHA-256:
`1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`

## Affected Project Surface

- Case ID: `derived/presentation-state/color_softcopy`
- Recipe ID: `derived_presentation_state_color_softcopy`
- Selected provider: `rust_native`
- Source image: `classic/sc/rgb_planar0_explicit_le`
- Output: Color Softcopy Presentation State Storage
  (`1.2.840.10008.5.1.4.1.1.11.2`), Explicit VR Little Endian
- Recommended manifest field: `expected_color_softcopy_presentation_state`

The existing smoke-profile source is a single-frame 2 by 2 Secondary Capture
Image with `RGB` Photometric Interpretation, Samples per Pixel 3,
Planar Configuration 0, 8 allocated and stored bits, and the deterministic
red, green, blue, white test pattern. Reusing it makes the color dependency
available in every extended generation without adding a hidden source recipe.

## Locked IOD And Module Contract

PS3.3 Table A.33.2-1 makes the Patient, General Study, General Series,
Presentation Series, General Equipment, Presentation State Identification,
Presentation State Relationship, Presentation State Shutter, Displayed Area,
ICC Profile, and SOP Common Modules mandatory. The recipe shall include those
mandatory Modules and omit the optional clinical-trial and specimen Modules.
Display Shutter, Bitmap Display Shutter, Overlay Plane, Overlay Activation,
Graphic Annotation, Spatial Transformation, Graphic Layer, and Graphic Group
remain absent because this minimal recipe applies none of those features.
Presentation State Shutter is a mandatory Module whose Attributes are
conditional; both shutter presentation values are absent when no shutter is
present. The output contains no Pixel Data.

The normal Patient and Study Type 1 and Type 2 Attributes are copied from the
source identity. The Presentation State shares the source Study Instance UID
but has its own Series Instance UID. PS3.3 A.1.2.3 requires Grayscale, Color,
and Pseudo-Color Softcopy Presentation States and their referenced Images to
be in the same Study and requires Presentation States to be grouped in a
Series without Images. Modality is `PR`. Series Number is present as Type 2.
To make the General Series Laterality condition unambiguous to both locked IOD
validators, Body Part Examined is `HAND` and Laterality is `R`. General
Equipment includes Manufacturer and deterministic model, serial, and software
identities.

The Presentation State Identification Module contains deterministic
Presentation Creation Date `20260101` and Presentation Creation Time `000000`.
The included Content Identification Macro contains Instance Number `1`,
Content Label `DTSCOLORPR`, a non-empty deterministic Content Description, and
Content Creator Name `DTS^Generator`. Content Label is CS VM 1 and is suitable
for identifying the recipe; the description and creator do not depend on host
locale or clock state.

## Reference Topology And Complete-Instance Selection

Referenced Series Sequence `(0008,1115)` contains exactly one Item. Its Series
Instance UID equals the source Series Instance UID, and its Referenced Image
Sequence `(0008,1140)` contains exactly one Item with the source Secondary
Capture SOP Class UID and exact source SOP Instance UID. Referenced Frame Number is absent
because the single-frame source is selected as a complete Instance. No other
SOP reference is present.

Generation shall reopen and hash the source before writing the Presentation
State. Strict validation shall prove that the source manifest and DICOM object
agree on path, SHA-256, Study, Series, SOP Class, SOP Instance, Rows, Columns,
Samples per Pixel, Photometric Interpretation, Planar Configuration, bit
depth, frame count, and transfer syntax. It shall also prove that the
Presentation State Study equals the source Study, that the two Series differ,
and that the nested reference identifies that exact source Series and
Instance.

## Displayed Area Contract

Displayed Area Selection Sequence `(0070,005A)` contains exactly one Item and
has no nested Referenced Image Sequence, so it applies to every Image and Frame
in the Presentation State Relationship Module. The locked Item is:

- Displayed Area Top Left Hand Corner `(0070,0052)`: SL VM 2, `[1,1]`
  in column, row order
- Displayed Area Bottom Right Hand Corner `(0070,0053)`: SL VM 2, `[2,2]`
  in column, row order
- Presentation Size Mode `(0070,0100)`: `SCALE TO FIT`
- Presentation Pixel Aspect Ratio `(0070,0102)`: IS VM 2, `[1,1]`

Presentation Pixel Spacing and Presentation Pixel Magnification Ratio are
absent. The corners select the entire 2 by 2 source after any spatial
transformation; no spatial transformation is present. Strict validation owns
positive one-based coordinates, top-left/bottom-right ordering, bounds against
the referenced source dimensions, one global selection, and the exact size
and aspect declarations.

## ICC Profile Contract

PS3.3 C.11.15 makes ICC Profile `(0028,2000)` Type 1 in this IOD. The recipe
reuses the project's deterministic synthetic sRGB profile, not an operating
system or network profile. Its locked contract is:

- VR and VM: OB VM 1
- Value length: 736 bytes
- SHA-256:
  `8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef`
- profile device class at bytes 12 through 15: `scnr`
- input color space at bytes 16 through 19: `RGB `
- profile connection space at bytes 20 through 23: `XYZ `
- profile signature at bytes 36 through 39: `acsp`
- DICOM Color Space `(0028,2002)`: `SRGB`

Color Space is Type 3, but it is deliberately present and shall remain
consistent with the profile. Strict validation shall check the complete
profile byte identity and length as well as its header fields; merely finding
an ICC Profile element is insufficient.

## KB Query And Locked Official Evidence

- Query: `dicom-kb lookup uid ColorSoftcopyPresentationStateStorage --edition 2026b`
- Edition: 2026b
- Result: `1.2.840.10008.5.1.4.1.1.11.2`
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Limitation: the KB registry evidence proves the SOP Class UID but does not
  expose the IOD module table, same-Study relationship, displayed-area
  semantics, nested cardinalities, or ICC payload rules required by this
  recipe.

The exact rules are anchored in PS3.3 A.33.2 and Table A.33.2-1 (IOD and
mandatory Modules), A.1.2.3 (same Study and separate Presentation Series),
C.7.1.1, C.7.2.1, C.7.3.1, C.7.5.1, and C.12.1 (mandatory generic Modules),
C.11.9 and Table C.11.9-1 (Presentation Series and `PR` Modality), C.11.10 and
Table C.11.10-1 plus Table 10-12 (Presentation State Identification and
Content Identification), C.11.11 and Table C.11.11-1b plus Tables 10-3 and
10-11 (relationship and image SOP reference), C.11.12 and Table C.11.12-1
(conditional shutter presentation values), C.10.4 and Table C.10-4 (Displayed
Area), and C.11.15 and Table C.11.15-1 (mandatory ICC Profile). PS3.4 Table
B.5-1 identifies the Storage SOP Class. PS3.6 Tables A-1 and 6-1 lock the SOP
Class UID and data element VR/VM definitions.

The repository `standards.lock.json` records official PS3.3, PS3.4, and PS3.6
artifacts as `unavailable_not_downloaded`; none is committed. The independently
locked 2026b validator cache used for this check pins official DocBook PS3.3
SHA-256
`4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05`,
PS3.4 SHA-256
`8445baf9a360e423b76671bae6b2de158cb545b688d7a2b085ea91c46147230b`,
and PS3.6 SHA-256
`512977071f31403dba5f00ea437157ee02bdf5b148375a826b2662085edd6a70`
in `conformance-backends/dicom-validator/standard-lock.json`.

## Prototype And Independent Validator Evidence

The temporary candidate used the exact source and contract above. The final
1,956-byte Color Softcopy Presentation State prototype had SHA-256
`a3044e2dd64dcd2fa1e37620172db176495e68c598d3620986aaa194c436e982`.
It completed locked `dciodvfy -new` with no findings, DCMTK 3.7.0 `dcmdump`
with exit code 0, isolated `dcentvfy` over the source and Presentation State
silently, and the independently implemented, `uv`-locked `dicom-validator`
0.8.2 adapter against the exact 2026b definitions with `Passed` and zero
errors. The prototype is qualification evidence only and is not committed.

Both IOD validators rejected a missing mandatory ICC Profile and a missing
Referenced Series Sequence. Empirical negative mutations also establish their
limits: both accepted a wrong enclosing referenced Series Instance UID,
Displayed Area Top Left Hand Corner `[0,0]`, out-of-bounds Displayed Area
Bottom Right Hand Corner `[3,3]`, and corruption of the first ICC profile byte.
Isolated `dcentvfy` detected a dangling referenced SOP Instance UID but was
silent for the wrong enclosing Series, invalid displayed corners, and corrupt
ICC payload. These are validator limitations, not accepted findings.

## Manifest, Validation, Report, And Acceptance Contract

`expected_color_softcopy_presentation_state` shall bind:

- source case, path, SHA-256, Study, Series, SOP Class, SOP Instance, image
  dimensions, color shape, bit depth, frame count, and complete-instance
  selection;
- same-Study and different-Series invariants, PR Modality, body part,
  Laterality, deterministic content identity, and creation date/time;
- exact relationship sequence cardinalities and absence of frame selection;
- the single global displayed-area Item, exact corners, size mode, aspect
  ratio, and absent spacing and magnification;
- ICC VR, byte length, payload SHA-256, header fields, and `SRGB` DICOM Color
  Space; and
- absence of shutters, overlays, annotations, graphic layers/groups, spatial
  transformations, and Pixel Data.

Strict Rust validation owns every exact semantic invariant above, including
source closure, same-Study/different-Series topology, nested cardinalities,
complete-instance selection, displayed-area mathematics and bounds, full ICC
payload and header validation, and optional-module absence. External IOD
validation remains additive and cannot replace these checks. Negative tests
shall cover redirected and dangling references, wrong Study or Series,
duplicate or missing relationship and displayed-area Items, partial frame
selection, zero or out-of-bounds corners, reversed corners, wrong size mode or
aspect ratio, absent/truncated/corrupt/substituted ICC profiles, inconsistent
Color Space, forbidden optional modules, and added Pixel Data.

JSON and Markdown reports should expose source identity, reference topology,
complete-instance selection, displayed-area item count and global scope,
corners, size mode, pixel aspect ratio, ICC byte count and hash, ICC input and
connection color spaces, DICOM Color Space, optional-module presence, and
pixel absence. Expected capabilities should include opening metadata,
resolving the source, applying the color Presentation State and displayed
area, and color-managing the ICC profile, while allowing recognized-
unsupported reporting for viewers that cannot apply it.

Promotion requires clean locked `dciodvfy -new`, clean DCMTK `dcmdump`, silent
isolated `dcentvfy` reference closure, and the locked `dicom-validator` 0.8.2
presentation-state adapter as additive secondary IOD evidence.
No new finding may be silently allowlisted; validation must not be weakened and
unavailable coverage must not be reclassified to make this case pass.

## Project Action

- Registry status: planned; retain `semantic_stable` until two generated runs
  prove byte identity.
- Registry provider: `rust_native`. DICOM-rs can write the mandatory nested
  Sequences and exact ICC OB payload directly and deterministically while
  preserving pydicom/dicom-validator as an independent implementation.
- Registry blocker: exactly `recipe_unimplemented`; the standards contract,
  source dependency, and independent IOD route are selected and locked.
- Should become KB patch: yes; expose the Color Softcopy Presentation State
  module table, same-Study relationship, reference topology, displayed-area
  semantics, and ICC requirements as structured 2026b queries.
- Do not commit generated DICOM files, validator outputs, Python caches, or
  official standards artifacts.
