# Independent Conformance Validation Framework: Agent Brief

## Assignment

Build a reproducible framework in this repository that collects and evaluates
independent conformance evidence for every generated DICOM instance. Complete
the work through implementation, tests, documentation, representative real-tool
runs, and granular commits.

The framework must establish confidence in the corpus. It must not prescribe
viewer behavior, launch a viewer, or turn the generator into a viewer-specific
test harness.

Read `AGENTS.md`, `SYSTEM_SPEC.md`, `docs/corpus-consumption.md`, and this brief
before changing files. Follow the repository's granular commit policy.

## Objective

The completed framework shall answer, with machine-readable evidence:

1. Which exact corpus and validator builds were used?
2. Did an independent IOD validator report errors or warnings for each file?
3. Is patient/study/series/instance identity consistent across the corpus?
4. Can an implementation independent of the generator parse every file?
5. Which lossless pixel cases were independently decoded to the manifest's
   expected native frame hashes?
6. Which findings remain unresolved, which are accepted validator limitations,
   and why?

Do not describe the result as official DICOM certification. The evidence is a
reproducible engineering assessment against named tools and versions.

## Required Initial Tool Strategy

Use these roles. Do not silently substitute a parser check for IOD validation.

- **Primary instance validator:** `dciodvfy -new`, from `dicom3tools`. It checks
  individual instances against the applicable SOP Class IOD.
- **Corpus entity validator:** `dcentvfy`, from the same package. It checks
  patient, study, series, and instance consistency across files.
- **Independent parser:** DCMTK `dcmdump` initially. Add an adapter boundary so
  another parser such as GDCM or PixelMed can be added without changing the
  evidence model.
- **Independent pixel decoders:** adapter-based and capability-reported. Begin
  with installed DCMTK/GDCM commands where they support a transfer syntax. Do
  not claim independence when the decoder shares the generator's codec
  implementation.
- **SR-specific second validator:** evaluate PixelMed `DicomSRValidator` for the
  generated SR cases after the primary framework works. Record unsupported or
  unavailable status rather than weakening the main acceptance rules.

Relevant upstream documentation:

- <https://www.dclunie.com/dicom3tools/dciodvfy.html>
- <https://manpages.debian.org/testing/dicom3tools/dciodvfy.1.en.html>
- <https://manpages.debian.org/testing/dicom3tools/dcentvfy.1.en.html>
- <https://www.dclunie.com/pixelmed/software/javadoc/com/pixelmed/validate/DicomSRValidator.html>

Pin every real tool by exact source revision, package version, executable
SHA-256, or immutable container digest. Record the standard definitions or
templates used by the tool when discoverable. The repository targets the
standards baseline in `standards.lock.json`; any validator-definition mismatch
must be visible in evidence and finding dispositions.

## Fixed Command Surface

Extend the existing binary with these commands unless a documented technical
constraint requires a narrowly different spelling:

```text
dicom-test-suite conformance check-tools [--config PATH]
dicom-test-suite conformance run GENERATED_ROOT --out EVIDENCE_ROOT [--config PATH]
dicom-test-suite conformance verify EVIDENCE_ROOT [--allowlist PATH]
```

Required behavior:

- `check-tools` resolves every configured command, fingerprints the executable,
  captures version output where available, and reports available, unsupported,
  or misconfigured without generating a corpus.
- `run` reads `GENERATED_ROOT/manifest.json`; it must never discover files and
  infer case identity independently of the manifest. It executes configured
  adapters with timeouts, captures raw output, parses normalized findings, runs
  corpus-level checks, and writes a versioned evidence bundle.
- `verify` validates evidence structure, applies the committed finding
  allowlist, rejects stale or overly broad dispositions, and exits nonzero when
  unresolved errors, undispositioned warnings, tool failures, evidence
  corruption, or required-tool gaps remain.

Default generation and validation must continue to work without any external
validator installed. External tools are runtime capabilities, not default Cargo
build dependencies.

## Committed Configuration And Schemas

Add this committed structure:

```text
conformance/
  README.md
  validators.json
  validator-lock.json
  accepted-findings.json
schemas/
  conformance-run.schema.json
  conformance-accepted-findings.schema.json
```

Generated evidence belongs under ignored `reports/conformance/` or another
ignored output root. Do not commit generated DICOM instances, raw validator
logs, or ordinary run reports.

### `validators.json`

Define adapter IDs, roles, executable names or configured paths, arguments,
timeout seconds, availability requirements, supported platforms, and declared
capabilities. Command arguments must be arrays; never evaluate a shell command
string.

### `validator-lock.json`

Record for each accepted real-tool baseline:

- adapter ID and role;
- tool name and version;
- source repository and revision or package identity;
- executable SHA-256 or container digest;
- definition/template version when available;
- platforms on which the fingerprint was verified;
- verification date; and
- notes describing known edition or capability limits.

Runtime fingerprints must be compared with the lock. A mismatch is visible and
fails strict verification unless explicitly allowed for a development run.

### `accepted-findings.json`

Every accepted finding must be narrow and reviewable. Require:

- validator adapter ID;
- validator fingerprint or tightly bounded version identity;
- exact `case_id` and optional manifest-relative path;
- stable rule ID when the validator supplies one;
- normalized message fingerprint;
- original severity;
- disposition category;
- rationale;
- DICOM part and section/table citation or validator issue URL;
- reviewer and review date; and
- recheck condition or expiry.

Allowed disposition categories should initially be:

- `validator_limitation`;
- `validator_standard_version_gap`;
- `standard_ambiguity`; and
- `generator_intent_confirmed`.

Do not allow global message suppression, case wildcards, severity downgrades
without rationale, or acceptance based only on current viewer behavior.

## Evidence Bundle Contract

Each `conformance run` writes:

```text
EVIDENCE_ROOT/
  conformance-run.json
  raw/
    <adapter-id>/
      <stable-instance-key>.stdout
      <stable-instance-key>.stderr
  entity/
    dcentvfy.stdout
    dcentvfy.stderr
  pixels/
    <adapter-id>/
      <stable-instance-key>.json
```

`conformance-run.json` must contain at least:

- evidence schema version and deterministic run ID;
- creation time;
- repository commit and dirty-state flag;
- source manifest path and SHA-256;
- generator identity, seed, profile, features, and standards lock identity;
- host OS, architecture, and tool fingerprints;
- one result per manifest file and validator role;
- raw-output relative paths and hashes;
- exit status, duration, timeout state, and invocation arguments;
- normalized findings with severity, rule/message fingerprint, and DICOM tag or
  path where available;
- allowlist disposition, or unresolved state;
- corpus-level entity-validation result;
- independent parser result;
- independent pixel-decode result or an explicit unsupported reason; and
- summary counts by tool, severity, disposition, SOP Class, and transfer
  syntax.

Use manifest-relative paths throughout. A `case_id` is not a unique file key
because some recipes emit multiple SOP Instances. Derive stable instance keys
from the manifest path, not enumeration order.

Raw logs must be retained byte-for-byte. Normalization may remove unstable
absolute path prefixes and byte offsets from the comparison fingerprint, but it
must not alter the preserved raw output.

## Finding And Exit Policy

Normalize tool output into these severities:

- `error`;
- `warning`;
- `info`;
- `tool_failure`;
- `timeout`;
- `unsupported`; and
- `unparsed_output`.

The first implementation must characterize actual `dciodvfy` and `dcentvfy`
exit behavior with controlled fixtures. Do not assume exit code zero means no
findings, or that all relevant messages are written to stdout.

Strict `conformance verify` succeeds only when:

- the evidence JSON matches its schema;
- the source manifest hash matches the recorded hash;
- required tool fingerprints match the lock;
- every manifest file has a completed primary-validator result;
- no unresolved `error`, `tool_failure`, `timeout`, or `unparsed_output`
  remains;
- every `warning` has an exact committed disposition;
- corpus entity validation has no unresolved finding;
- the independent parser opened every file, unless a narrowly documented parser
  capability gap is accepted; and
- pixel-decode requirements selected for the milestone are satisfied or
  reported as explicit blockers.

`unsupported` is never equivalent to passed. It must appear in summaries and
may block completion depending on the required role and milestone.

## Independent Pixel Evidence

Use `pixel_data.frame_hashes` in the manifest as the lossless comparison oracle.
An external decoder adapter must normalize its decoded output to the same native
frame byte convention before hashing. Document byte order, signed sample
representation, planar organization, bit packing, frame boundaries, and any
PNM/TIFF container parsing.

For each transfer syntax, record:

- generator encoder implementation;
- current internal decoder implementation;
- candidate external decoder implementation;
- whether it is genuinely independent;
- supported image shapes and photometric interpretations; and
- evidence status.

Do not use OpenJPH decoding as independent evidence for a file encoded by the
same OpenJPH implementation, or OpenJPEG decoding as independent evidence for a
file encoded through the same OpenJPEG implementation. Such a decode may still
be a useful integrity check, but label it `same_implementation`.

Lossy comparison is out of the first strict milestone unless the manifest
already provides an unambiguous tolerance contract. Continue to validate JPEG
Baseline structurally and record decode metrics, but do not invent a new visual
acceptance policy in this task.

## Implementation Phases And Commits

Complete these phases in order. Each bullet headed **Commit** is one coherent
commit.

### Phase 0: Baseline And Decisions

- Re-run the locked default suite and an all-features corpus generation.
- Inventory installed validators and decoders without changing the system.
- Research acquisition options for missing tools and record licensing,
  platform, version, and definition-data constraints.
- Confirm which generated SOP Classes the selected `dciodvfy` build recognizes.

**Commit:** `docs(conformance): record validator architecture and baseline`

Exit: `conformance/README.md`, `validators.json`, and a documented tool decision
matrix exist; unresolved acquisition choices are explicit blockers.

### Phase 1: Evidence Data Contracts

- Add both JSON Schemas and example minimal fixtures under tests.
- Add `validator-lock.json` and an empty, valid accepted-findings file.
- Add structural tests that validate real examples against the full schemas,
  not only required-property names.

**Commit:** `feat(conformance): define evidence and disposition schemas`

Exit: malformed evidence, unknown fields, invalid severities, broad allowlist
entries, and missing fingerprints are rejected by tests.

### Phase 2: Tool Discovery And Fingerprinting

- Implement `conformance check-tools`.
- Resolve explicit configured paths before `PATH` lookup.
- Hash executable bytes and capture version output without shell evaluation.
- Represent absent, mismatched, and versionless tools distinctly.

**Commit:** `feat(conformance): add validator discovery and fingerprinting`

Exit: fake-tool tests cover available, absent, timeout, nonzero-version command,
fingerprint mismatch, and OpenJPH-style versionless behavior.

### Phase 3: Per-Instance Validation

- Implement the adapter execution engine and `dciodvfy -new` adapter.
- Drive work exclusively from manifest file entries.
- Capture raw output and normalized findings.
- Make concurrency bounded and output ordering deterministic.

**Commit:** `feat(conformance): collect per-instance IOD evidence`

Exit: controlled fake-validator tests cover errors, warnings, clean output,
stderr-only output, malformed output, timeout, and multiple files sharing one
case ID.

### Phase 4: Corpus Entity Validation

- Implement `dcentvfy` with a generated file list to avoid command-line length
  limits.
- Capture and normalize corpus-level findings separately from file findings.

**Commit:** `feat(conformance): collect corpus entity evidence`

Exit: tests detect inconsistent entity values, reused child identifiers, clean
corpora, and paths containing spaces.

### Phase 5: Verification And Allowlist

- Implement `conformance verify`.
- Validate schemas, hashes, fingerprints, result completeness, raw-log hashes,
  and exact finding dispositions.
- Produce a concise terminal summary and nonzero exit on unresolved evidence.

**Commit:** `feat(conformance): enforce evidence acceptance policy`

Exit: tests prove that unknown warnings, expired dispositions, stale manifest
hashes, altered logs, tool gaps, and wildcard-like allowlist entries fail.

### Phase 6: Independent Parser

- Add the DCMTK `dcmdump` adapter without treating it as IOD validation.
- Record parse success, tool failure, timeout, and unsupported transfer syntax.

**Commit:** `feat(conformance): add independent parser evidence`

Exit: every file in a representative complete corpus has a parser result and
strict verification treats unexplained gaps correctly.

### Phase 7: Independent Pixel Decode Matrix

- Commit a transfer-syntax/backend independence matrix.
- Implement external decode adapters in small commits, one decoder family per
  commit.
- Compare normalized lossless frame hashes with manifest expectations.
- Leave genuinely unavailable independent decoders as explicit blockers.

**Commits:** `feat(conformance): validate <codec> pixels independently`

Exit: every lossless transfer syntax either has passing independent decoded
frame evidence or one precise blocker naming the missing independent stack.

### Phase 8: Real-Tool Acceptance And Automation

- Acquire or build pinned tools using approved, documented methods.
- Generate all-features `all` and `legacy` corpora with seed 1.
- Run internal validation, conformance collection, and strict verification.
- Review every finding; fix generator defects before adding dispositions.
- Add a manual or scheduled CI job using immutable tool identities and upload
  evidence as an artifact. Keep ordinary default CI independent of these tools.

**Commit:** `test(conformance): automate pinned external validation`

**Commit:** `docs(conformance): record acceptance evidence and limitations`

Exit: the complete acceptance criteria below are met, or the plan names the
remaining blocker without calling the corpus ready.

## Testing Requirements

- Keep default tests hermetic and network-free.
- Use fake executables for command, timeout, output, and fingerprint tests.
- Do not skip parser tests merely because a real tool is absent.
- Add conditional real-tool integration tests behind an explicit environment
  switch or ignored test target.
- Validate generated JSON with a complete Draft 2020-12 JSON Schema engine.
- Run `cargo fmt -- --check` and
  `cargo test --locked --all-targets --no-default-features` after each phase.
- At final acceptance, run `cargo test --locked --all-targets --all-features`
  and the complete generation workflow from `docs/corpus-consumption.md`.

## Complete Acceptance Criteria

The framework is complete only when:

1. `list-cases` and generation agree on both `all` and `legacy` selection.
2. All-features `all` and `legacy` generation and internal validation pass.
3. Every generated file has primary `dciodvfy` evidence tied to its path and
   manifest hash.
4. `dcentvfy` has evaluated the complete combined corpus.
5. Every generated file has independent-parser evidence.
6. No validator error remains unresolved.
7. Every warning is fixed or narrowly dispositioned with standards evidence.
8. Every lossless transfer syntax has independent pixel evidence or one
   explicit, technically justified blocker.
9. Tool executables, definitions, commands, raw logs, and evidence JSON are
   fingerprinted and mutually hash-linked.
10. Strict verification returns success on the acceptance evidence bundle.
11. CI or a documented repeatable release procedure can reproduce the run.
12. Documentation states the remaining scope limits and avoids claims of
    official certification.

## Stop And Escalate Conditions

Stop and report rather than guessing when:

- obtaining a validator requires approval, a non-redistributable definition
  package, or a license decision;
- the validator does not recognize an implemented SOP Class or transfer syntax;
- a finding conflicts with the pinned DICOM source evidence;
- independent decoding would reuse the encoder implementation;
- satisfying a validator appears to require weakening or changing a valid
  recipe; or
- a proposed allowlist entry cannot cite a precise standard basis or validator
  limitation.

When blocked, preserve the raw evidence, identify the exact affected case IDs,
and propose the smallest decision needed to resume.
