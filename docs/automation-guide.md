# Automation and agent integration

This guide is the black-box contract for scripts, CI jobs, side projects, and
agents. Use the installed `synth-dicom-gen` executable (`$GENERATOR` below). Never
discover artifacts by repository layout, assumed counts, or filename guesses.

## Choose the workflow

Start every integration session with:

```sh
"$GENERATOR" version --format json
"$GENERATOR" capabilities --format json
```

Use `generate` for curated registry cases and profiles, `compose` for qualified
templates with caller values/content, and `assemble` for expert structural
Part 10 construction without an IOD-conformance claim. These evidence classes
are disjoint. Discovery availability, registry `implemented` status, and files
actually emitted by a run are different facts.

## Machine protocol

Select `--format json` in durable automation. Commands whose only JSON contract
is the current machine envelope report its CLI API directly. `report` retains a
historical raw-JSON mode, so select CLI API `1.0.0` explicitly there:

```sh
"$GENERATOR" list-cases --profile smoke --format json
"$GENERATOR" generate --profile smoke \
  --out "$RUN_ROOT" --seed 1 --format json
"$GENERATOR" validate "$RUN_ROOT" --format json
"$GENERATOR" report "$RUN_ROOT" --format json --cli-api 1.0.0
```

Success writes one JSON envelope to stdout and nothing to stderr. Failure writes
one JSON error envelope to stderr and no partial JSON to stdout. Exit classes
are stable: `0` success, `2` syntax, `3` input, `4` unavailable capability,
`5` execution, and `6` validation. Branch on `error.code`, not human detail.
The embedded `product/cli-error-codes.json` identity is reported by discovery.

Historical raw report output is retained only at its documented compatibility
boundary. New integrations explicitly select CLI API `1.0.0`.

## Output and evidence

Every output root must be absent before a file-producing command starts. The
product refuses overwrite/merge, stages privately, publishes once, and removes
only its exact private staging root on failure or cancellation.

Read the returned manifest path, then enumerate `manifest.json`; do not assume
case counts or paths. Preserve the request/spec, seed, version result,
capability result, manifest, report, release-manifest identity, feature set,
external executable fingerprints, and unavailable rows. Run `validate` before
downstream consumption. Negative inputs never join valid conformance corpora;
fuzz evidence is payload-free; stress is opt-in reduced-scale evidence.

For reproducibility, compare exact bytes only where `determinism` is
`byte_stable`. For `semantic_stable`, use the declared decoded/semantic oracle
and ignore only fields explicitly documented as nondeterministic, such as
elapsed time and explicit output-root paths.

## Optional capability trust

Feature-gated codecs require a binary built with the named feature. External
providers and validators require explicit prepared runtimes and locked
fingerprints. Same-project generation plus validation is not independent
conformance. Missing, feature-gated, backend-unavailable, and peer-unavailable
capabilities remain typed unavailable outcomes; never convert them to passes.

## Minimal agent recipe

An agent must perform these steps in order:

1. Verify the release checksum and record `version --format json`.
2. Call `capabilities --format json`; stop or choose a supported alternative
   when a required capability is unavailable.
3. Choose `generate`, `compose`, or `assemble` from the evidence needed.
4. Validate the versioned request locally and use `--dry-run` where supported.
5. Allocate a fresh output root and execute with an explicit seed/API version.
6. Discover every artifact from the returned manifest.
7. Run strict validation and produce the appropriate report.
8. Preserve unavailable outcomes and all artifact/tool/resource identities.
9. Compare reruns according to their declared determinism class.

See the [generation guide](generation-guide.md),
[composition guide](composition-guide.md), [assembly guide](assembly-guide.md),
[SDK guide](sdk-guide.md), and [compatibility policy](compatibility-policy.md)
for workflow-specific request and upgrade contracts.
