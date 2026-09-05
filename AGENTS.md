# Repository Guidelines

## Project Overview

`synth-dicom-gen` is a Rust project for generating deterministic, synthetic,
non-PHI DICOM corpora for viewer, parser, codec, and interoperability testing.
It emits DICOM Part 10 files, versioned manifests, coverage reports, and
profile-specific qualification evidence. Generation is standards-led and must
remain independent of any one viewer's current behavior.

Before changing generation or documentation behavior, read:

1. `README.md` for the current public capability overview;
2. `docs/generation-guide.md` for supported workflows and output semantics;
3. `cases/registry.json` and `cases/taxonomy.md` for authoritative selection;
4. `SYSTEM_SPEC.md` for architecture and invariants; and
5. the relevant dated status/source-note document for the subsystem being
   changed.

## Source Of Truth

Use sources in this order when implementation, docs, and historical plans
disagree:

1. The executable command behavior and schemas define what the current binary
   accepts and emits.
2. `cases/registry.json` defines case identity, status, profiles, requirements,
   provider, standards evidence, and roadmap blockers.
3. A generated `manifest.json` defines what a particular run actually emitted
   or skipped.
4. `transfer-syntax/capability-matrix.json` defines codec availability claims.
5. Dated phase/status documents explain qualification evidence and remaining
   boundaries; implementation plans are historical unless explicitly marked
   current.

Do not manually summarize case counts as an invariant. Derive them with
`list-cases`, `report gaps`, or registry queries because the inventory changes.

## Profiles And Evidence Boundaries

- `all` is the union of `smoke`, `core`, and `extended`. It does not include
  `legacy`, `negative`, or `fuzz`; stress joins only with `--include-stress`.
- `negative` contains expected-invalid files and must remain isolated from valid
  conformance inputs.
- `fuzz` is a payload-free bounded qualification; do not retain generated
  sources or candidates.
- `stress` is reduced-scale, opt-in valid coverage. Do not imply that reduced
  qualifications prove full-scale resource behavior.
- Built-in generation and validation are same-project evidence. Independent
  conformance and interoperability claims require the pinned external adapters
  documented under `conformance/` and `docs/phase-8-interoperability-status.md`.
- Planned, feature-gated, backend-unavailable, and peer-unavailable coverage
  must remain explicit. Never convert missing capability into an implied pass.

## Case Change Workflow

For every new or materially changed generated case:

1. Establish DICOM Standard evidence through the configured knowledge base or
   an official-source note, and keep `standards.lock.json` policy intact.
2. Update the registry entry and any provider/transfer-syntax capability
   artifact.
3. Implement deterministic generation with explicit recipe versioning.
4. Record manifest expectations, hashes, references, semantics, stressors, and
   determinism classification.
5. Add generation-time and strict validation before promoting the case to
   `implemented`.
6. Add report fields/grouping when the compatibility axis would otherwise be
   invisible.
7. Test CLI selection, schema validity, generation, validation, report output,
   and reproducibility in proportion to the change.
8. Update the public generation guide and relevant status/source-note document
   when capability or usage changes.

Prefer small, orthogonal cases. A logical case may emit multiple instances;
keep reference closure and source dependencies explicit in the manifest. Never
commit generated DICOM files, ordinary run manifests/reports, standards source
artifacts, caches, virtual environments, or private keys.

## Local Verification

The ordinary default regression baseline is:

```sh
cargo test --locked --all-targets --no-default-features
```

For a focused current-repository change, inspect the fail-closed Fast/subsystem
selection before executing it:

```sh
python3 scripts/route-changed-tests.py --path src/sdk.rs --dry-run
python3 scripts/route-changed-tests.py --path src/sdk.rs
```

Repeat `--path` for overlaps. The JSON report names unconditional Fast
coverage, selected ordinary commands, and deferred Nightly, codec, provider,
heavy, package/release, or future external-corpus evidence. An unmapped
code/data path is an error; `--all-ordinary` is the conservative manual
fallback and still does not run deferred evidence.

That command deliberately skips the six explicitly ignored R2.3 heavyweight
entries. Run the applicable slice through
`scripts/run-heavy-qualification.sh byte-parity`, `all-profile`, `wsi`, or
`stress`; use `scripts/run-heavy-qualification.sh all` only at the scheduled
Nightly or exact release-candidate boundary. The dispatcher preserves the
secondary scopes: byte parity includes stress and legacy, all-profile includes
opt-in stress, and WSI includes ordinary and reduced-stress evidence.

For documentation or CLI changes, also exercise the exact documented commands
against a fresh output path and run `git diff --check`. Feature-gated codec work
must run its feature-specific tests and validate a generated corpus with the
same feature set. External-command work must record tool version and executable
fingerprint according to `docs/external-codec-verification.md`.

Generated output paths must be new; the generator intentionally refuses to
overwrite or merge an existing root. Use ignored repository paths or a private
temporary directory and remove temporary artifacts only when their exact target
is known.

## Documentation Maintenance

User-facing docs must distinguish:

- registry `implemented` from runtime-available and actually generated;
- valid corpora from negative, fuzz, stress, media, and protocol evidence;
- same-project validation from independent conformance evidence; and
- byte-stable output from semantic-stable codec output.

When a capability changes, search all Markdown files for stale future-tense,
profile membership, command syntax, runtime dependencies, and scope claims.
Historical status records may remain dated, but the README, generation guide,
corpus consumption guide, taxonomy, and this file must describe current
behavior.

## Git Commit Policy

Every completed task **MUST** be tracked in a descriptive, granular git commit. This requirement is **absolutely critical** and must be followed under all circumstances - no exceptions.

**Rules:**

- Commit after every distinct logical unit of work, not at the end of a session.
- Each commit covers one coherent behavior or migration slice, including related tests, routing and necessary documentation. Do not batch unrelated changes. A helper, review or status entry is not automatically a separate task.
- Commit messages must be informative: use `type(scope): subject` format, include a blank line, then a body describing *what* changed and *why*.
- Types: `feat`, `fix`, `test`, `docs`, `refactor`, `chore`
- Scope: the module, file, or subsystem affected, such as `backend`, `frontend`, `pixels`, `server`, `types`, or `tests`
- Subject: imperative mood, 72 characters or fewer
- Body: explain the design decision, the invariant being established, or the behavior being changed, not a restatement of the diff
- Stage files selectively (`git add <file>`) rather than `git add -A`. Only commit files that belong to the current logical unit.
- Never amend or force-push commits that have been logged here.

**Verification:** After each task, run `git log --oneline -3` to confirm the commit was recorded before moving to the next task.

## Migration Execution

For the current separation migration, follow sections 5 and 7 of
`docs/synth-dicom-gen-dcmview-corpus-separation-plan.md`, amended 2026-09-05.
Deliver vertical slices; target 2–4 coherent commits across affected repositories
without forcing unrelated changes together. Record agent ownership in messages,
not assignment commits. Keep one concise entry per completed slice or genuine
blocker in `docs/migration-continuation-status-2026-09-05.md`, preferably in the
implementation commit. Other repositories link to it; do not duplicate history.
Run focused checks during implementation and mapped ordinary coverage once at
integration, repeating only invalidated checks. Preserve all acceptance gates.
Reuse evidence mechanisms and immutable historical source/definition fixtures.
Do not encode corpus-specific values as engine whitelists. Authorization for the
plan covers necessary local implementation and bounded verification; historical
per-helper approval steps do not require renewed permission within that scope.
