# Composition Content Security and Resource Policy

**Status:** required composition contract

**Applies to:** local assets, inline fixtures, staging, external content
providers, bulk writers, validation tools, and transactional publication

## Trust model

Composition specifications, referenced paths, caller assets, provider
responses, encoded frames, and bulk payloads are untrusted. A provider
executable is explicitly authorized and fingerprinted, but this protocol does
not make malicious native code safe to execute. Template descriptors, committed schemas, the standards lock, and
the transfer-syntax capability matrix are trusted only after their committed
schema and integrity checks pass.

Caller content is opaque. The project packages and validates its declared
structure and provenance; it does not classify PHI, infer anatomy, fetch
dependencies, or interpret domain-specific content.

## Path boundary

Every local input path is relative to the directory containing the composition
specification. Accepted paths:

- are non-empty UTF-8 relative paths;
- contain no absolute prefix, drive prefix, backslash, NUL, `.` segment, or
  `..` segment;
- resolve beneath the canonical specification directory;
- identify one existing regular file; and
- do not traverse a symbolic link at any component.

The resolver walks path components with no-follow semantics and compares the
opened file identity with the inspected identity before and after hashing.
Directories, FIFOs, sockets, devices, hard-linked files whose platform identity
changes during staging, and files modified during hashing are rejected. A
platform that cannot establish the required no-follow and stable-file checks
reports `safe_path_semantics_unavailable`; it does not silently downgrade.

The manifest records only the specification-relative normalized path. Canonical
host paths never enter deterministic identity, resolved-plan, manifest-entry,
or report hashes.

## Resource envelopes

The executable supplies conservative defaults. A caller may lower limits but
cannot raise them above build policy without an explicit API or CLI opt-in
whose effective values are recorded. Checked arithmetic is mandatory before
allocation, seek, copy, frame multiplication, DICOM even-length padding, or
manifest size aggregation.

The envelope independently bounds:

- logical instances and bundle-expanded instances;
- input files and provider-declared files;
- bytes per input file and provider output file;
- total input, staged, generated-output, and manifest bytes;
- rows, columns, frames, fragments, Sequence depth/items, attribute count, and
  value multiplicity;
- decoded sample count and native/encoded frame length;
- provider wall time, response and diagnostic bytes, declared output count and
  bytes, and the owned process group; and
- file-level parallelism and queued work.

Limits are checked at schema/resolution time when values are known, again while
streaming bytes, and before publication against observed totals. Crossing a
limit cancels the run, closes writers, terminates owned provider processes,
and removes the exact private staging root.

Inline fixtures are for small neutral test inputs only. Their encoded JSON
string is limited to 87,384 characters, corresponding to at most 65,536
decoded bytes including base64 rounding. Larger values must use a bounded local
file or provider slot.

## Private staging

Composition never writes directly into the requested output root. It creates a
new private sibling staging directory with owner-only permissions, a unique
non-deterministic staging name, and an exclusive create operation. The staging
name is operational state and is excluded from deterministic hashes.

Inputs are copied or streamed into an input subdirectory whose filenames are
allocated by the resolver, not supplied by the caller. Outputs are written to
predeclared resolver-owned relative paths. Providers receive dedicated input
and output subdirectories and cannot choose paths outside them.

Before promotion the runner verifies:

1. every staged input still matches its recorded size and SHA-256;
2. every output is a regular file at an exact declared path;
3. no symlink, hard-link escape, directory replacement, device, or undeclared
   file exists in staging;
4. all expected entries and only those entries are present;
5. observed counts and byte totals fit the envelope;
6. generation-time, strict generic, template, and content validation passed;
7. the manifest was assembled from the same resolved plans; and
8. the requested destination still does not exist.

Promotion is one same-filesystem atomic no-replace rename from the complete private
staging root to the requested output root. Cross-filesystem copy promotion and
merge/overwrite behavior are forbidden. On any failure the destination remains
absent. Cleanup targets only the exact staging directory created by this run.

## Network prohibition

Generation performs no network fetch for content, templates, standards data,
dependencies, codecs, validators, or providers. A provider receives a cleared
environment, no credentials or network source, and
`DTS_COMPOSITION_PROVIDER_NETWORK=disabled`. This is a protocol prohibition,
not a portable OS socket sandbox. A prepared provider must operate offline; an
operator running code that is not trusted to honor the contract must add an
external platform sandbox. The project does not describe protocol-level
disablement as enforced network isolation.

## Provider execution contract

Provider support begins only after the versioned request/response schemas land.
The runner:

- resolves and fingerprints an explicitly configured executable before use;
- invokes it directly without a shell;
- passes the exact schema-bounded fixed argument vector and binds its canonical
  hash into the request, response, and manifest;
- clears the inherited environment, then supplies only documented protocol and
  staging variables;
- sets private working and output directories;
- preallocates deterministic identities and declared content slots;
- transmits a canonical request with sizes, hashes, limits, and output paths;
- enforces wall-clock timeout and protocol byte/count limits;
- captures bounded stdout and stderr separately;
- requires one schema-valid canonical response;
- rejects path traversal, symlinks, undeclared files, identity changes,
  duplicate slots, missing slots, and hash/size disagreement; and
- records executable SHA-256, version, protocol, request/response hashes,
  resource outcome, and termination status.

Provider child processes are placed in an owned process group or equivalent so
timeout and cancellation terminate descendants. A crash, signal, malformed
response, hang, output flood, resource violation, or non-zero exit fails the
transaction with a typed provider error.

Providers cannot emit DICOM Part 10 files for publication. They emit only the
declared opaque slot payloads; the shared resolved-plan materializer remains the
sole Part 10 writer.

## Streaming and memory

Local and provider bulk content is hashed and copied with fixed-size buffers.
Native Pixel Data and other large value fields use a bounded-memory writer or a
spill file within private staging. No operation may require holding the full
corpus in memory. Codec adapters that require a full frame allocate at most one
file-envelope-checked native frame per codec worker.

Parallel workers receive immutable resolved plans and preassigned output paths.
Completion order cannot affect UIDs, paths, references, manifest ordering, or
hash inputs. The manifest is sorted only after every worker joins successfully.

## Validation tools

Optional independent validators and decoders receive only already-materialized
private staged files. Their executable identity, timeout, environment policy,
output bound, and findings are recorded. Tools are invoked without a shell and
cannot cause an unavailable route to be reported as passed. Required
independent routes block qualification when unavailable.

## Failure and cleanup invariants

Every error is typed and identifies its failing contract boundary. Diagnostics
may include normalized relative
paths but must not print inline content, provider payloads, private values, or
unnecessary absolute caller paths.

Cancellation is cooperative inside the resolver and writers and forceful at
the owned provider-process boundary. Cleanup is idempotent. A cleanup failure
is reported separately and preserves the exact staging path for deliberate
operator recovery; the code does not broaden deletion to a parent directory.

Successful publication leaves no staging directory, spill file, provider work
directory, or undeclared output. Failed publication leaves no requested output
root.

## Required adversarial qualification

Tests cover absolute and parent paths, mixed separators, symlink components,
file replacement races, special files where supported, oversized inline data,
per-file and aggregate overflow, checked pixel arithmetic, Sequence depth,
undeclared provider files, malformed provider responses, hash disagreement,
crash, timeout, descendant cleanup, output flooding, cancellation, validation
failure, destination races, and partial-promotion prevention.
