# `synth-dicom-gen` / `dcmview-test-corpus` file ownership inventory

**Recorded:** 2026-09-01

**Plan item:** R0.3

**Baseline revision:**
`f640748b412151b4410dfb104685519cef2bde75`

**Machine-readable authority:**
`product/migration-file-ownership-2026-09-01.json`

## Scope and interpretation

The JSON inventory explicitly enumerates every one of the 799 paths tracked at
the baseline revision plus the two new R0.3 ownership artifacts. The migration
status document is the third file owned by this task, but it was already one of
the 799 baseline paths. It is therefore classified once, not added a second
time. The resulting inventory has 801 unique path entries.

Each entry has one disposition, one primary destination, one ownership domain,
one rationale, one invalidated verification class, and one migration phase or
slice. A `split` entry still has exactly one primary destination and additionally
names concrete synth and corpus outputs. Disposition is about the intended
terminal ownership, not permission to move a file before its phase gate passes.

The inventory freezes decisions at R0.3. It does not claim that a move, split,
rename, API extraction, corpus parity check, or repository creation has happened.
It also does not pass the R0 gate: R0.4 remains required.

## Summary

| Disposition | Paths |
| --- | ---: |
| `retain_synth` | 283 |
| `move_corpus` | 310 |
| `split` | 175 |
| `retire` | 0 |
| `archive_history` | 33 |
| **Total** | **801** |

| Ownership domain | Paths |
| --- | ---: |
| `engine` | 105 |
| `generic_capability` | 136 |
| `corpus_definition` | 313 |
| `viewer_expectations` | 20 |
| `historical_evidence` | 33 |
| `build_release` | 20 |
| `documentation` | 26 |
| `governance_legal` | 4 |
| `test_infrastructure` | 144 |
| **Total** | **801** |

The zero `retire` count is deliberate. At freeze time every tracked artifact has
continuing value either as an implementation/output to retain or move, a file to
split, or immutable history. Later phases may retire extracted embedded-corpus
code only after supported replacements and parity evidence exist; R0.3 does not
pre-authorize deletion.

## Ownership rules applied

- `cases/registry.json`, `cases/taxonomy.md`, and every explicitly enumerated
  `cases/recipes/...` document are corpus definitions and move to
  `dcmview-test-corpus` by the named R7 slice.
- Qualified reusable templates, generic provider/backend adapters, transfer
  syntax capability records, engine primitives, and generic validation remain
  in `synth-dicom-gen`.
- Source notes for composition, structural assembly, and UID generation remain
  generic capability evidence. Notes whose purpose is selection of a dcmview
  case move with that case. The lossy-codec and negative-mutation notes split
  because they currently mix reusable capability evidence with case-selection
  justification.
- Dated plans, ADRs, audits, status records, candidate hashes, and historical
  qualification claims remain `archive_history` under synth. Their old product
  and artifact names must remain unchanged wherever changing them would falsify
  the recorded event.
- Schemas for case registry/recipe/selection, coverage, media,
  interoperability, and viewer results move downstream. Generic CLI, SDK,
  assembly, composition, provider, validation, and release schemas remain with
  the generator. Shared manifest, report, discovery, standards, and independent
  evidence envelopes split into a neutral engine contract and downstream
  corpus/viewer projections.
- Tests follow the contract they prove. Engine, template, provider, CLI/SDK,
  resource, package, and security tests remain in synth. Assertions bound to
  dcmview case IDs, profiles, expected bytes, relationships, or availability
  move with the corpus. Mixed tests split and are assigned to R2 harness work or
  the later API/projection phase rather than being copied unchanged.

## Shared-file split map

The JSON `split_outputs` object is the exhaustive path-by-path split map. These
are its recurring meanings:

| Current surface | Primary synth output | Downstream output/meaning | Phase |
| --- | --- | --- | --- |
| Root policy, license, README, system spec, Cargo/toolchain, and workflow files | Renamed generator product, release, and CI contract | Independent repository policy, dependency pin, Corpus PR, artifact, and consumer contract | R1, R3, R6, R9 |
| `build.rs` and `src/product_resources.rs` | Immutable `EngineResources` and independent engine/template/schema identities | Versioned caller-owned corpus bundle and digest | R4 |
| Family recipe modules | Generic capability/provider implementation with stable IDs | Declarative dcmview defaults, case IDs, selectors, and combinations | R7 |
| Curated planning/manifest/validation modules | Neutral caller-owned corpus planning and projection primitives | Embedded dcmview selection and expected evidence | R5, R7 |
| Manifest/report/capability/conformance schemas | Neutral engine envelope | Corpus definition, viewer result, and independent-evidence projection | R5, R7 |
| Media/protocol code and tests | Generic runner/result mechanism | dcmview peer selection, known result, and compatibility baseline | R7, R8 |
| Conformance backends and configuration | Generic adapter/discovery capability | Pinned corpus invocation and accepted findings | R7 |
| Mixed integration and release tests | Generator mechanism/package qualification | Corpus definition, pin, smoke, viewer, and artifact-consumer qualification | R2, R6-R9 |

Splitting does not permit the downstream repository to import internal modules.
The corpus-side output must consume only the supported CLI or `synth_dicom_gen::sdk`
surface when its phase is implemented.

## Ambiguous-looking decisions resolved

- `src/corpus_plan.rs` is retained as `engine`, despite its name. The separation
  plan explicitly assigns neutral `CorpusPlan` and bounded execution to synth;
  dcmview registry/profile ownership is separate.
- `templates/` is retained. Templates and generic typed providers are qualified
  reusable capabilities; `cases/recipes/` moves because it selects and
  parameterizes the dcmview corpus.
- `src/recipes/model.rs`, loader/encoding/provider/typed-bulk helpers, and codec
  registry remain generic. Family recipe modules split so reusable algorithms
  can be promoted without preserving embedded dcmview policy.
- Conformance adapters split rather than move wholesale. Adapter execution and
  fingerprinting can be reusable generator capability, while pins, required
  peers, accepted findings, and viewer-facing interpretation belong downstream.
- `schemas/manifest.schema.json` splits. Generic execution/provenance fields are
  generator-owned; corpus selection, skipped/unavailable case outcomes, and
  viewer projections are downstream extensions with independent identities.
- `Cargo.lock` is split as repository build metadata, but R4 removes it from the
  monolithic runtime-resource identity. Each repository ultimately owns its own
  lockfile; neither may use the sibling lockfile as runtime input.
- Historical status and source-of-truth evidence is not moved merely because it
  mentions cases. Dated facts, hashes, and old artifact identities remain in
  synth history; current corpus selection notes move or split.
- No current file is marked `retire`. Deletion becomes safe only after a later
  phase proves the supported replacement and parity at an exact revision.

These are decisions, not ownership blockers. The only external authority
boundary remains creation/location of the new repository, which R0.3 does not
perform.

## Deterministic integrity verification

At the R0.3 commit, the following Python check proves that the inventory paths
equal `git ls-files` exactly, with no duplicates, missing paths, or extras. It
also independently reconstructs the baseline-plus-task scope and validates the
required keys, enums, destination prefix, split-output shape, and non-empty
text fields.

```sh
python3 - <<'PY'
import json
import subprocess

inventory = "product/migration-file-ownership-2026-09-01.json"
data = json.load(open(inventory, encoding="utf-8"))
entries = data["entries"]
paths = [entry["path"] for entry in entries]
tracked = subprocess.check_output(["git", "ls-files"], text=True).splitlines()
baseline = subprocess.check_output(
    ["git", "ls-tree", "-r", "--name-only", data["baseline_revision"]],
    text=True,
).splitlines()
expected = sorted(set(baseline) | set(data["scope"]["task_created_paths"]))

assert len(paths) == len(set(paths)), "duplicate inventory path"
assert sorted(paths) == sorted(tracked), "inventory differs from git ls-files"
assert sorted(paths) == expected, "inventory differs from fixed baseline + R0.3 files"
assert data["scope"]["baseline_git_tracked_count"] == len(baseline) == 799
assert data["scope"]["inventory_path_count"] == len(paths) == 801

required = {
    "path", "disposition", "primary_destination", "ownership_domain",
    "rationale", "verification_class_invalidated", "migration_phase_slice",
}
for entry in entries:
    assert required <= entry.keys(), entry["path"]
    assert entry["disposition"] in data["enums"]["disposition"], entry["path"]
    assert entry["ownership_domain"] in data["enums"]["ownership_domain"], entry["path"]
    assert entry["verification_class_invalidated"] in data["enums"]["verification_class_invalidated"], entry["path"]
    assert entry["primary_destination"].startswith(("synth-dicom-gen:", "dcmview-test-corpus:")), entry["path"]
    assert all(isinstance(entry[key], str) and entry[key] for key in required), entry["path"]
    if entry["disposition"] == "split":
        assert set(entry["split_outputs"]) == {
            "synth-dicom-gen", "dcmview-test-corpus", "meaning"
        }, entry["path"]
    else:
        assert "split_outputs" not in entry, entry["path"]
print(f"verified {len(paths)} unique tracked inventory paths")
PY

jq empty product/migration-file-ownership-2026-09-01.json
```

The focused documentation test and `git diff --check` remain proportional R0.3
verification. They do not qualify generation, codecs, providers, packages,
releases, a downstream repository, or any target platform.
