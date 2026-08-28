# Phase 6 Stress Corpus Status

Status date: 2026-08-28

## Promoted reduced-scale cases

The opt-in `stress` profile implements the seven approved reduced boundary
recipes:

- a 128-instance, 64 by 64 classic CT study;
- one 256-frame, 64 by 64 Enhanced CT instance;
- one Secondary Capture instance with 64 MiB of native bulk Pixel Data;
- one Secondary Capture instance with a depth-32 private Sequence and 16 MiB
  nested bulk value;
- one Secondary Capture instance with 1,024 private UT Values totaling 1 MiB;
- one 256-frame RLE Lossless Secondary Capture instance with a 64 MiB native
  decoded payload and multi-fragment encapsulation; and
- a three-level, 1,024 by 1,024 VL Whole Slide Microscopy pyramid with 256 by
  256 tiles.

Ordinary `all` generation does not select these cases. They require the
`stress` profile or the existing explicit stress inclusion option. The reduced
resource envelope is 256 MiB output, 512 MiB peak RSS, two minutes per case,
and ten minutes for the job.

## Qualification evidence

Each promoted logical case emits one `stress_case_run` qualification. The
qualification records its canonical requested and actual scale parameters,
resource envelope, output bytes, elapsed milliseconds, bounded outcome, and
status. It also records peak RSS when the platform exposes that measurement.

The final seed-7 reduced root
`/tmp/dts-stress-reduced-20260828-d` contained:

- 7 passing stress qualifications;
- 139 generated DICOM files;
- 160,213,322 total qualified stress-case output bytes;
- 19,376 ms summed case elapsed time; and
- no available peak-RSS measurement on the qualification platform.

The seven qualifications bind 136 of those files. The remaining three files
are the already implemented, bounded `vl/wsi/pyramid_multiresolution` case and
total 8,694 bytes; they are not included in the 160,213,322 qualified-byte
total.

Strict validation checked all 139 files with zero failures. The DICOM files in
`/tmp/dts-stress-reduced-20260828-c` and
`/tmp/dts-stress-reduced-20260828-d` were byte-for-byte identical. Elapsed
milliseconds are observed telemetry and are not part of the DICOM byte-
stability claim.

The encapsulated boundary uses 64 Fragments **per Frame** across 256 Frames,
for 16,384 Fragments total. Its 64 MiB payload measurement is the native
decoded Pixel Data size, not the compressed Fragment-stream or whole-file
size. The generated RLE Lossless file was 67,786,772 bytes. This terminology
keeps the qualification consistent with the executable recipe and avoids
misstating 64 as the total Fragment count.

The built-in generator checks and strict validator are same-project evidence.
They establish the suite's scale, resource, manifest, and DICOM contracts, but
they are not independent conformance or interoperability evidence. No report
or status claim should describe them as an independent DICOM implementation.

## Explicit unavailable full scale

Every reduced qualification records the `full` scale as unavailable with
reason code `full_scale_runner_unimplemented`. The scheduled full-scale
streaming runner and its independent resource qualification are not
implemented. This includes the genuine greater-than-4-GiB Extended Offset
Table case needed to place a later Item beyond `0xFFFF_FFFF`.

Reduced Phase 6 coverage is therefore promoted and reportable, while full
scale remains explicit unavailable coverage. Full jobs remain outside ordinary
CI and cannot be inferred from the reduced qualification results.
