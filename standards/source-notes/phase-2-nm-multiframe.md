# Phase 2 Nuclear Medicine Multi-frame Evidence

Checked: 2026-08-27
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case ID: `classic/nm/multiframe_explicit_le`
- Recipe ID: `classic_nm_multiframe_explicit_le`
- NM Image Type, Frame Increment Pointer, indexing vectors, information
  sequence cardinalities, and native multi-frame pixel validation

## Required Decision

Generate one four-frame Nuclear Medicine Image Storage instance with Image Type
`ORIGINAL\\PRIMARY\\STATIC\\EMISSION`. The frames form a two-dimensional
array with two energy windows and two detectors. Frame Increment Pointer
`(0028,0009)` contains Energy Window Vector `(0054,0010)` followed by Detector
Vector `(0054,0020)`, making the detector index the most rapidly changing:

- Energy Window Vector: `1, 1, 2, 2`
- Detector Vector: `1, 2, 1, 2`
- Frame tuples: `(1,1), (1,2), (2,1), (2,2)`

Number of Energy Windows `(0054,0011)` and Number of Detectors `(0054,0021)`
are both two. Energy Window Information Sequence `(0054,0012)` and Detector
Information Sequence `(0054,0022)` must each contain exactly two Items in the
same one-based index order. Each 2 by 2 frame contains a distinct unsigned
16-bit MONOCHROME2 pattern; the manifest and validators bind the ordered frame
tuple to its independently checkable native frame hash.

## KB Query

- Tool: `dicom_list_modules_for_iod`, `dicom_list_attributes_for_module`,
  `dicom_lookup_defined_terms`, `dicom_lookup_enumerated_values`, and
  `dicom_retrieve_standard_text`
- Input: Nuclear Medicine Image IOD; NM Image Pixel, Multi-frame, NM
  Multi-frame, NM Image, NM Isotope, and NM Detector modules; Image Type in
  Nuclear Medicine Image context; Photometric Interpretation in NM Image Pixel
  context; PS3.3 Sections C.8.4.7, C.8.4.7.1.1, C.8.4.8, C.8.4.8.1.1,
  C.8.4.9, and C.8.4.9.1.1
- Edition returned: 2026b
- Source manifest SHA-256:
  `1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728`
- Result: the IOD and module tables are parsed and identify all mandatory NM
  modules and attribute types. The parsed defined-term and enumerated-value
  lookups do not resolve the context-specific Image Type or Photometric
  Interpretation values, while persisted official text does.
- Why insufficient: the generic UID evidence already in the registry does not
  establish the NM dimension ordering or context-specific values, and the
  parsed value-term APIs omit those two specialization sections.

## Official Source Evidence

- PS3.3 Table A.5-1 makes NM Image Pixel, Multi-frame, NM Multi-frame, NM
  Image, NM Isotope, and NM Detector mandatory modules of the Nuclear Medicine
  Image IOD.
- PS3.3 Section C.8.4.8 and Table C.8-7 define each indexing vector as a
  one-dimensional array with one element per frame, use one-based indices, and
  require corresponding information sequences to contain one Item per index.
- PS3.3 Section C.8.4.8.1.1 and Table C.8-8 require STATIC frames to be ordered
  by Energy Window Vector followed by Detector Vector. The last pointer is the
  most rapidly changing index.
- PS3.3 Section C.8.4.9.1.1 permits `ORIGINAL`, requires `PRIMARY`, defines
  STATIC for Image Type Value 3, and EMISSION for Value 4. Section C.8.4.9
  makes Actual Frame Duration Type 1C for STATIC images.
- PS3.3 Section C.8.4.7 and Section C.8.4.7.1.1 permit MONOCHROME2, require one
  sample per pixel, permit 8 or 16 Bits Allocated, require Bits Stored to equal
  Bits Allocated, and require High Bit to be one less than Bits Stored.
- PS3.3 Tables C.8-10 and C.8-11 require Energy Window Information Sequence
  and Detector Information Sequence cardinalities to equal their declared
  counts.
- Source artifact identity: the locked DICOM 2026b KB source manifest above.

## Project Action

- Registry status: implemented after generation, typed manifest and report
  contracts, internal and manifest-driven validation, determinism, exact
  independent native pixel extraction, and independent IOD gates passed.
- Registry reason or linked issue: none; the `recipe_unimplemented` blocker was
  removed by the completed vertical slice.
- Should become KB patch: yes; context-specific Image Type and Photometric
  Interpretation terms are systematic parsed-value lookup gaps.
- Expected cleanup after KB coverage exists: replace the two local value-term
  fallbacks with normal KB evidence while retaining the case-specific dimension
  and cardinality contract.

## Promotion Evidence

- Two seed-1 `core` generations each produced 43 files. Recursive comparison
  found no differences, and strict corpus validation reported zero failures.
- The promoted instance SHA-256 is
  `facb70cd576c5d4b0ffbed58450d11a73c9bdd2c4bbc04960a342c41dc6a2d21`.
- Locked `dciodvfy -new` identifies only `NMImage`; isolated `dcentvfy` is
  silent.
- DCMTK `dcmdump +W` independently extracted exactly 32 native pixel bytes.
  Splitting them into four ordered 8-byte frames produced SHA-256 values
  `245bbd9d484dcf27c714e2690cd6544973de5d54aa9cd82eab23d6046a65faa8`,
  `a58214fbfec2da6f1e9fc6a2641c8a0af73fb383860180a73d4439fe31b44189`,
  `4908c41ec85a7552278ed886fa3c43819f44d4df5b73138a9c5855926c750a58`,
  and `a12837f26e181e5420b019bae0940e221d2927e13fea963ad899945c34c697fe`.
- Offline pydicom 3.0.2 in the locked `uv` environment independently decoded
  the `(4, 2, 2)` unsigned pixel array, frame pointers, vectors, window names,
  and detector angles. Its enforced Part 10 rewrite was byte-identical and
  retained the same clean dicom3tools and DCMTK results.
