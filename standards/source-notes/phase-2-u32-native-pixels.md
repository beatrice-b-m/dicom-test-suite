# Phase 2 Unsigned 32-bit Native Pixel Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/sc/mono2_u32_explicit_le`
- Recipe ID: `classic_sc_mono2_u32_explicit_le`
- Unsigned 32-bit Image Pixel attributes, native OW encoding, exact sample
  values, manifest semantics, validation, and report coverage

## Required Decision

Generate one 2 by 2 Secondary Capture Image Storage instance with unsigned
MONOCHROME2 samples `0`, `65535`, `2147483648`, and `4294967295`. Bits
Allocated and Bits Stored are 32, High Bit is 31, Pixel Representation is zero,
and Planar Configuration is absent. Pixel Data is native OW in Explicit VR
Little Endian, so the four samples are encoded as exact little-endian 32-bit
words and span both sides of the signed 32-bit boundary.

## KB Query

- Tool: `dicom_lookup_uid`, `dicom_list_modules_for_iod`,
  `dicom_list_attributes_for_module`, and `dicom_retrieve_standard_text`
- Input: Secondary Capture Image Storage; Secondary Capture Image IOD; Image
  Pixel Module; PS3.3 Sections A.8.1 and C.7.6.3; PS3.5 Sections 8.1.1 and A.2
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the parsed UID and IOD/module surfaces identify Secondary Capture
  Image Storage and its mandatory Image Pixel Module, but do not by themselves
  establish the permitted 32-bit native Pixel Data encoding and OW choice.
- Why insufficient: the existing registry evidence resolves only the SOP Class
  UID. The recipe needs the official IOD description and PS3.5 native encoding
  rules to prove that unsigned 32-bit integer samples are permitted and that
  their Pixel Data VR remains OW rather than OL.

## Official Source Evidence

- PS3.3 Section A.8.1.1 describes the legacy single-frame Secondary Capture
  Image IOD as having no constraint on pixel data format. Section A.8.1.3 and
  Table A.8-1 make the Image Pixel Module mandatory.
- PS3.3 Section C.7.6.3 and Table C.7-11a require the Image Pixel Module and
  its Pixel Data; Table C.7-11c supplies the Image Pixel Description Macro
  attributes used to describe the integer samples.
- PS3.5 Section 8.1.1 permits Bits Allocated to be one or a multiple of eight,
  requires Bits Stored not to exceed Bits Allocated, requires High Bit to equal
  Bits Stored minus one, and requires the complete Pixel Data Value Field to
  have even length. The selected 32/32/31 values satisfy those invariants.
- PS3.5 Section A.2 requires native Pixel Data with Bits Allocated greater than
  eight in Explicit VR Little Endian to use OW and little-endian encoding. Its
  note explicitly states that OL and OV are not used for Pixel Data even when
  Bits Allocated is 32 or 64.
- PS3.6 Table A-1 identifies Secondary Capture Image Storage as
  `1.2.840.10008.5.1.4.1.1.7` and Explicit VR Little Endian as
  `1.2.840.10008.1.2.1`.
- Source artifact identity: the locked DICOM 2026b KB source manifest above;
  official source artifacts remain uncommitted under repository policy.

## Project Action

- Registry status: implemented. At qualification it remained planned until the
  user authorized selection and locking of another independent IOD validator
  at the explicit checkpoint and every report/test/conformance gate completed.
- Resolved blocker: `independent_iod_validator_unavailable`. The selected
  pydicom `dicom-validator` 0.8.2 engine recognizes Secondary Capture, evaluates
  its 2026b IOD/module definitions, and passes the candidate with zero errors.
  It runs through repository adapter 0.2.0 in an exact CPython 3.12.12 `uv`
  environment and does not become a generation-profile dependency.
- Should become KB patch: yes; the cross-part IOD-permission and native OW
  encoding decision should become a repeatable evidence query.
- Expected cleanup after KB coverage exists: replace the local cross-part
  fallback with linked KB evidence while retaining the exact case recipe and
  promotion record.

## Preliminary Implementation Evidence

- Two clean extended generations with seed 1 each emitted 85 DICOM files and
  were recursively byte-identical. Internal strict validation checked all 85
  manifest entries with zero failures.
- The candidate instance SHA-256 is
  `bec7dfedcb7cec08426f38f46f6d5deead6294c2a4a6e4464ba972bb97592630`.
  Its native Pixel Data SHA-256 is
  `56bca1a85c2838126b1d1a5fbedfe731839496d972df2c6ab33e1a1183392b41`.
- DCMTK 3.7.0 `dcmdump` independently reports Rows 2, Columns 2, Bits
  Allocated 32, Bits Stored 32, High Bit 31, Pixel Representation 0, Pixel
  Data VR OW, and the exact 16-byte word sequence.
- The repository's frozen offline `uv` environment ran pydicom 3.0.2 with
  NumPy 2.5.2. It independently decoded a `(2, 2)` `uint32` array containing
  `[[0, 65535], [2147483648, 4294967295]]` and reproduced the exact Pixel Data
  hash above.
- Locked `dciodvfy` SHA-256:
  `1aeb75d6ccd3f193e3b322b6da77742cdce2e0604868eaf2a2669c786cbc27e5`.
  Locked `dcentvfy` SHA-256:
  `1b96e598f28f66deee1bfc1cb52ff460c316ab6b0625dae575d701f20c836e2c`.
  Both abort while evaluating the candidate with the dicom3tools assertion
  `bitsallocated <= bytesinword*8u` in `Proposal14EncodingRules`; neither
  produces an IOD conformance result. This is an unavailable required gate,
  not an accepted warning and not grounds to reduce the planned coverage.
- The already-locked PixelMed 20260608 alternative was evaluated without
  changing any profile runtime requirement. The official binary and dependency
  archives matched their committed SHA-256 identities, as did `pixelmed.jar`,
  Commons Codec, Saxon HE, and XML Resolver. With the repository-locked Oracle
  Java 25.0.3 and Saxon transformer configuration,
  `com.pixelmed.validate.DicomInstanceValidator` completed but reported
  `IOD (SOP Class) unrecognized`. The same result for an existing legacy
  Secondary Capture comparator confirms that this PixelMed release does not
  provide the missing SC IOD coverage; it is not evidence against the u32
  fixture.

## Independent Validator Qualification

- The selected runtime is locked in
  `conformance-backends/dicom-validator/uv.lock`. The accepted arm64 macOS
  composite fingerprint is
  `e0b445d4f4bc5b338f248251c7eff9416ffef9da079b4e1cd4c46924dc17539d`.
  It binds the CPython executable, `.python-version`, `pyproject.toml`, exact
  `uv.lock`, adapter sources, the standards lock, three official 2026b DocBook
  parts, and all four derived validator-definition artifacts.
- The clean candidate reports Secondary Capture SOP Class
  `1.2.840.10008.5.1.4.1.1.7`, status `Passed`, and zero errors. A control with
  required Conversion Type removed reports `TagMissing` Type 1. A second
  control with Pixel Representation set to two reports `EnumValueNotAllowed`
  with allowed values zero and one. These controls establish that a zero exit
  is an IOD-validation result rather than a parse-only result.
- The same locked adapter independently reads raw OW Pixel Data through pydicom
  and little-endian unsigned unpacking without NumPy. It returns exact stored
  values `[0, 65535, 2147483648, 4294967295]` and frame/Pixel Data SHA-256
  `56bca1a85c2838126b1d1a5fbedfe731839496d972df2c6ab33e1a1183392b41`.
  Strict verification cross-links every extracted attribute and value to the
  manifest and rejects a sidecar whose hash is recomputed after semantic
  tampering.
- The original file still triggers the locked `dcentvfy` pixel-word assertion.
  Entity validation therefore receives an evidence-bearing projection of this
  case only: bytes zero through 929 are copied exactly and the terminal 28-byte
  Pixel Data element at offset 930 is omitted. The source copy retains SHA-256
  `bec7dfedcb7cec08426f38f46f6d5deead6294c2a4a6e4464ba972bb97592630`;
  the projection has SHA-256
  `b078217dad7f87238cfa3042ace25ec4fcc974dc2ced472b9974623b0caa19a4`.
  `dcentvfy` is silent with exit zero on the isolated projection. The untouched
  original remains the sole input to IOD and pixel validation.
- A real isolated conformance run records the matched case-specific primary,
  independent DCMTK parse, passed U32 payload comparison, completed entity
  validation, and zero strict-verification failures. All run artifacts remain
  under `/tmp` and are not committed.
