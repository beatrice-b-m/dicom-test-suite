# Maintainer release and migration procedure

This procedure creates one target-specific release candidate from a clean
clone, verifies its package and native archive, and supplies the exact facts
required for release notes. Repeat target qualification independently; a
successful build on one host is not evidence for another target.

## Prerequisites

Install the pinned Rust toolchain implied by `Cargo.lock`/`rust-version` plus
`git`, `jq`, `tar`, and either `shasum` or `sha256sum`. Fetch dependencies before
entering an offline build environment. Optional codec/provider tools are
qualified separately and must never be inferred from their presence on the
maintainer machine.

## Prepare a clean source identity

Use a new clone and an explicit signed/tagged revision. The example target is
macOS arm64; substitute only a target that will run the complete target gate:

```sh
git clone https://github.com/beatrice-b-m/dicom-test-suite.git dts-release
cd dts-release
git checkout --detach RELEASE_REVISION
test -z "$(git status --porcelain)"
rustc --version
cargo --version
TARGET=aarch64-apple-darwin
DIST="$PWD/dist"
```

Before building, move the `[Unreleased]` changelog entries under the candidate
version/date, add a fresh `[Unreleased]` section, and describe migration for
every changed compatibility domain. Commit that change; never build a public
artifact from a dirty tree.

## Verify source and package once

Use focused tests while developing. Run the heavyweight source/package gates
once for the exact candidate revision, not after every small edit:

```sh
cargo fmt --all -- --check
git diff --check
RUSTFLAGS='-D warnings' cargo check --locked --all-targets --no-default-features
cargo test --locked --all-targets --no-default-features
scripts/run-heavy-qualification.sh all
cargo package --locked --offline
```

The broad Cargo command is ordinary evidence and skips the six explicitly
ignored heavy bodies. The dispatcher then selects byte parity (including
stress and legacy), all-profile (including opt-in stress), ordinary and stress
WSI, and stress projection/execution exactly once. The release-candidate
workflow inherits this completed Nightly/default gate and must not rerun the
dispatcher in its packaging job.

Use these verification tiers in order, stopping at the narrowest tier that
matches the change until a phase or release boundary is reached:

1. For each commit, run formatting/diff checks plus the named affected test or
   smallest target that owns the changed contract.
2. At a numbered plan item or subsystem boundary, run the owning target bundle
   and any public CLI, schema, resource, or packaged-consumer boundary it can
   affect.
3. At a phase gate, run that phase's black-box or packaged-artifact contract.
4. For an exact release candidate, run the complete default command, applicable
   feature/backend matrices, package/archive qualification, and every terminal
   acceptance row exactly once per claimed target.

Invalidate previously passed evidence only when a later change intersects its
dependency surface. Changes to generation bytes, execution, manifest/report
projection, embedded resources, stress, or WSI invalidate their corresponding
heavyweight slices. Documentation-only, CI-only, or isolated SDK/CLI changes do
not. The exact release-candidate matrix remains mandatory even when every
development tier passed.

Wall-clock, cancellation, and resource-ceiling tests are product contracts.
Run a failing timing test alone before diagnosing contention, but never skip,
relax, or represent it as unavailable because a broad concurrent run failed.
Feature jobs should execute feature-sensitive unit/integration and corpus
surfaces; feature-independent library tests belong to the default gate and are
not multiplied across the feature matrix.

If a failure requires a localized repair, rerun the named affected test and
its subsystem bundle first. Then resume the failed candidate gate without
repeating already-passed heavyweight slices; the final artifact itself still
must complete every S6 and terminal-matrix row required for its target.

## Build and independently verify the archive

The builder refuses a dirty source tree, builds the locked target with no
default features, confirms the binary's reported target/features, and writes a
`.tar.gz` plus `.sha256` without overwriting an existing artifact:

```sh
scripts/build-release-archive.sh "$TARGET" "$DIST"
ARCHIVE="$DIST/dicom-test-suite-$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')-$TARGET.tar.gz"
scripts/verify-release-archive.sh "$ARCHIVE"
```

Do not set `DTS_RELEASE_ALLOW_DIRTY` or `DTS_RELEASE_BINARY` for a public
candidate; those hooks exist only for isolated qualification tests. Select
`DTS_RELEASE_FEATURES` only when the target's exact feature matrix will be
qualified and published.

The verifier checks the adjacent checksum, safe single-root extraction,
release-manifest inventory hashes/sizes, executable discovery identity,
embedded resource identity, and required license/changelog/example payloads.
It does not replace S6 black-box, determinism, template/assembly, regression,
upgrade, or security qualification.

## Describe and record the release

Copy facts; do not infer them. Release notes and the dated standalone status
must record:

- product version, source revision, source-dirty flag, target, feature set, and
  archive SHA-256 from `release-manifest.json` and the checksum file;
- embedded resource-set SHA-256 and supported CLI/request/result/manifest/
  report/catalog/provider versions from discovery;
- target-filtered third-party notices and both project licenses;
- exact verification commands/results, unavailable optional capabilities, and
  every unqualified target;
- changelog additions and a migration action for each changed compatibility
  domain; and
- terminal-matrix outcomes for the exact immutable archive.

Publish the archive, checksum, changelog/migration notes, and dated matrix
together. Never promote a general standalone release until Linux x86_64 and
macOS arm64 each pass the plan's complete external-consumer contract. Never
represent a missing optional runtime, peer, codec, or target as a pass.

## Consumer upgrade rehearsal

Install the candidate beside the prior version, verify both discovery results,
and execute every still-supported request/manifest/report/CLI fixture. Compare
byte-stable or semantic-stable outputs according to the manifest. Unsupported
versions must return the documented stable version error and migration action.
Keep rollback possible until the target's release-candidate evidence is
complete.
