#!/usr/bin/env python3
"""Validate the R2.1 Rust test ownership inventory without compiling tests."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

SCHEMA_VERSION = "1.0.0"
DOMAINS = {
    "assembly",
    "cli_sdk",
    "codec",
    "composition",
    "conformance_interoperability",
    "corpus_generation",
    "engine",
    "provider",
    "release_ci",
    "schema_resources",
    "standards_validation",
}
CLASSES = {"fast", "subsystem", "nightly", "release_candidate"}
COST_TIERS = {"ordinary", "heavy"}
FAST_TARGETS = {
    "ci_release_gates",
    "compatibility_ownership",
    "schema_artifacts",
    "standalone_docs",
}
FAST_HEAVY_MARKERS = re.compile(
    r"(?:^|_)(?:all_profile|archive|codec|external|full_profile|heavy|package|"
    r"parity|provider|release|reproducibility|stress|wsi)(?:_|$)"
)
FAST_SOURCE_MARKERS = re.compile(
    r"(?:Command::new\s*\(|thread::sleep|cargo (?:build|package|run|test)|"
    r"--all-features|--all-targets|--include-stress|"
    r"--profile (?:all|core|extended)|build-release-archive)"
)
TEST_ATTRIBUTE = re.compile(r"(?m)^[ \t]*#\s*\[\s*test\s*\]")
UNSUPPORTED_TEST_ATTRIBUTE = re.compile(
    r"#\s*\[\s*(?:async_std::test|parameterized|rstest|test_case|tokio::test)\b"
)
FUNCTION = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>{}]*>)?\s*\(")
KNOWN_HEAVY_ENTRIES = {
    "tests/case_recipe_catalog.rs": {
        "data_first_sc_and_metadata_values_and_hashes_match_current_generator_bytes"
    },
    "tests/curated_stress_manifest.rs": {
        "typed_stress_projection_matches_frozen_file_values_and_resources"
    },
    "tests/curated_stress_sc_integration.rs": {
        "all_stress_sc_cases_execute_through_private_streaming_services"
    },
    "tests/generate_cli.rs": {
        "generate_command_writes_all_profile_union_and_skips_planned_cases"
    },
    "tests/wsi_direct_plan.rs": {
        "ordinary_wsi_direct_plans_match_fresh_seed_one_bytes_and_manifest_facts"
    },
    "tests/wsi_pyramid.rs": {"stress_profile_emits_complete_three_instance_wsi_pyramid"},
}


class OwnershipError(Exception):
    pass


def relative(root: Path, path: str) -> str:
    return Path(path).resolve().relative_to(root.resolve()).as_posix()


def cargo_targets(root: Path) -> list[dict[str, object]]:
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    package = json.loads(result.stdout)["packages"][0]
    targets = []
    for target in package["targets"]:
        kinds = target["kind"]
        if "test" not in kinds and "lib" not in kinds and "bin" not in kinds:
            continue
        kind = "integration" if "test" in kinds else kinds[0]
        targets.append(
            {
                "name": target["name"],
                "path": relative(root, target["src_path"]),
                "kind": kind,
            }
        )
    return sorted(targets, key=lambda item: (str(item["kind"]), str(item["name"])))


def attribute_block(source: str, offset: int) -> str:
    lines = source[:offset].splitlines()
    selected = []
    for line in reversed(lines[-6:]):
        stripped = line.strip()
        if stripped.startswith("#[") or not stripped:
            selected.append(stripped)
            continue
        break
    return "\n".join(reversed(selected))


def test_entries(path: Path) -> list[dict[str, object]]:
    source = path.read_text(encoding="utf-8")
    unsupported = UNSUPPORTED_TEST_ATTRIBUTE.search(source)
    if unsupported is not None:
        raise OwnershipError(
            f"{path}: unsupported generated/async test attribute requires checker support"
        )
    matches = list(TEST_ATTRIBUTE.finditer(source))
    entries = []
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(source)
        segment = source[match.start() : end]
        function = FUNCTION.search(segment)
        if function is None:
            raise OwnershipError(f"{path}: #[test] is not followed by a function")
        name = function.group(1)
        attributes = attribute_block(source, match.start()) + "\n" + segment[: function.start()]
        entries.append(
            {
                "name": name,
                "ignored": "#[ignore" in attributes,
                "heavy_source_marker": bool(FAST_SOURCE_MARKERS.search(segment)),
                "segment_sha256": hashlib.sha256(segment.encode()).hexdigest(),
            }
        )
    names = [str(entry["name"]) for entry in entries]
    if len(names) != len(set(names)):
        raise OwnershipError(f"{path}: duplicate #[test] function names")
    return entries


def group_digest(entries: list[dict[str, object]]) -> str:
    projection = [
        {
            "name": entry["name"],
            "ignored": entry["ignored"],
            "segment_sha256": entry["segment_sha256"],
        }
        for entry in entries
    ]
    encoded = json.dumps(projection, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def domain_for(path: str) -> str:
    name = Path(path).stem
    if any(token in name for token in ("assembly", "assemble")):
        return "assembly"
    if any(token in name for token in ("codec", "jpeg", "pixel", "eot_")):
        return "codec"
    if name.startswith("composition") or "compose" in name or "template" in name:
        return "composition"
    if any(token in name for token in ("conformance", "interoperate", "interoperability", "media", "protocol")):
        return "conformance_interoperability"
    if any(token in name for token in ("backend", "provider")):
        return "provider"
    if any(token in name for token in ("schema", "resource", "artifact", "inventory")):
        return "schema_resources"
    if any(token in name for token in ("release", "consumer", "standalone", "compatibility", "project_artifacts", "ci_")):
        return "release_ci"
    if any(token in name for token in ("cli", "sdk", "capabilities", "version", "list_cases")):
        return "cli_sdk"
    if any(token in name for token in ("validate", "validation", "standards", "conformance")):
        return "standards_validation"
    if any(token in name for token in ("plan", "executor", "planning")):
        return "engine"
    return "corpus_generation"


def class_for(path: str, target: str) -> str:
    name = Path(path).stem
    if target in FAST_TARGETS:
        return "fast"
    if path in KNOWN_HEAVY_ENTRIES:
        return "nightly"
    if any(token in name for token in ("release", "installed_artifact", "consumer", "upgrade")):
        return "release_candidate"
    if any(
        token in name
        for token in (
            "full_file",
            "parity",
            "qualification",
            "reproducibility",
            "stress",
            "wsi",
        )
    ):
        return "nightly"
    return "subsystem"


def cost_for(path: str, entries: list[dict[str, object]]) -> str:
    name = Path(path).stem
    if any(token in name for token in ("full_file", "stress", "wsi")):
        return "heavy"
    if any(bool(entry["ignored"]) for entry in entries):
        return "heavy"
    return "ordinary"


def discovered_groups(root: Path, targets: list[dict[str, object]]) -> list[dict[str, object]]:
    groups = []
    target_by_path = {str(item["path"]): str(item["name"]) for item in targets}
    sources = sorted(root.glob("tests/*.rs"))
    sources.extend(sorted(root.glob("src/**/*.rs")))
    for path in sources:
        entries = test_entries(path)
        rel = path.relative_to(root).as_posix()
        if not entries and not rel.startswith("tests/"):
            continue
        target = target_by_path.get(rel, "dicom_test_suite")
        groups.append(
            {
                "source": rel,
                "target": target,
                "entries": [str(entry["name"]) for entry in entries],
                "entry_count": len(entries),
                "entry_digest": group_digest(entries),
                "has_ignored": any(bool(entry["ignored"]) for entry in entries),
                "has_heavy_source_marker": any(
                    bool(entry["heavy_source_marker"]) for entry in entries
                ),
            }
        )
    return groups


def bootstrap(root: Path) -> dict[str, object]:
    targets = cargo_targets(root)
    groups = discovered_groups(root, targets)
    manifest_targets = []
    target_classes = {}
    for target in targets:
        path = str(target["path"])
        name = str(target["name"])
        verification_class = class_for(path, name)
        target_classes[name] = verification_class
        manifest_targets.append(
            {
                **target,
                "domain": domain_for(path),
                "verification_class": verification_class,
            }
        )
    manifest_groups = []
    for group in groups:
        path = str(group["source"])
        target = str(group["target"])
        entries = test_entries(root / path)
        verification_class = class_for(path, target)
        record = {
            "source": path,
            "target": target,
            "domain": domain_for(path),
            "verification_class": verification_class,
            "cost_tier": cost_for(path, entries),
            "entry_count": group["entry_count"],
            "entries": group["entries"],
            "entry_digest": group["entry_digest"],
        }
        known_heavy = sorted(KNOWN_HEAVY_ENTRIES.get(path, set()))
        if known_heavy:
            record["heavy_entries"] = known_heavy
        exemptions = {
            "ci_release_gates": "Static workflow and script inspection; no heavyweight command is executed.",
            "schema_artifacts": "Static JSON Schema validation; WSI names do not generate a corpus.",
            "standalone_docs": "Static documentation inspection; external-consumer names launch no process.",
        }
        if target in exemptions:
            record["fast_cost_exemption"] = exemptions[target]
        manifest_groups.append(record)
    return {
        "schema_version": SCHEMA_VERSION,
        "scope": {
            "rust_test_targets": len(targets),
            "rust_test_entry_groups": len(groups),
            "rust_test_entries": sum(int(group["entry_count"]) for group in groups),
            "inventory_rule": "Cargo lib/bin/integration targets plus every Rust #[test] entry",
        },
        "enums": {
            "domain": sorted(DOMAINS),
            "verification_class": sorted(CLASSES),
            "cost_tier": sorted(COST_TIERS),
        },
        "targets": manifest_targets,
        "entry_groups": manifest_groups,
    }


def verify(root: Path, manifest: dict[str, object]) -> dict[str, object]:
    errors = []
    if manifest.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"schema_version must be {SCHEMA_VERSION}")
    expected_enums = {
        "domain": sorted(DOMAINS),
        "verification_class": sorted(CLASSES),
        "cost_tier": sorted(COST_TIERS),
    }
    if manifest.get("enums") != expected_enums:
        errors.append("manifest enums differ from checker authority")

    actual_targets = cargo_targets(root)
    actual_groups = discovered_groups(root, actual_targets)
    owned_targets = manifest.get("targets", [])
    owned_groups = manifest.get("entry_groups", [])
    if not isinstance(owned_targets, list) or not isinstance(owned_groups, list):
        raise OwnershipError("targets and entry_groups must be arrays")

    target_keys = [(item.get("kind"), item.get("name"), item.get("path")) for item in owned_targets]
    if len(target_keys) != len(set(target_keys)):
        errors.append("multiply owned Cargo test target")
    actual_target_keys = {(item["kind"], item["name"], item["path"]) for item in actual_targets}
    owned_target_keys = set(target_keys)
    for key in sorted(actual_target_keys - owned_target_keys):
        errors.append(f"unowned Cargo test target: {key}")
    for key in sorted(owned_target_keys - actual_target_keys):
        errors.append(f"stale Cargo test target ownership: {key}")

    for item in owned_targets:
        if item.get("domain") not in DOMAINS:
            errors.append(f"invalid target domain: {item.get('name')}")
        if item.get("verification_class") not in CLASSES:
            errors.append(f"invalid target verification class: {item.get('name')}")

    sources = [item.get("source") for item in owned_groups]
    if len(sources) != len(set(sources)):
        errors.append("multiply owned Rust test-entry source group")
    actual_by_source = {str(item["source"]): item for item in actual_groups}
    owned_by_source = {str(item.get("source")): item for item in owned_groups}
    for source in sorted(set(actual_by_source) - set(owned_by_source)):
        errors.append(f"unowned Rust test-entry source group: {source}")
    for source in sorted(set(owned_by_source) - set(actual_by_source)):
        errors.append(f"stale Rust test-entry source ownership: {source}")

    target_names = {str(item["name"]) for item in actual_targets}
    target_by_name = {str(item["name"]): item for item in owned_targets}
    for source in sorted(set(actual_by_source) & set(owned_by_source)):
        actual = actual_by_source[source]
        owned = owned_by_source[source]
        for field in ("target", "entry_count", "entries", "entry_digest"):
            if owned.get(field) != actual.get(field):
                errors.append(f"test-entry metadata drift in {source}: {field}")
        if owned.get("target") not in target_names:
            errors.append(f"test-entry group references unknown target: {source}")
        if owned.get("domain") not in DOMAINS:
            errors.append(f"invalid test-entry domain: {source}")
        verification_class = owned.get("verification_class")
        if verification_class not in CLASSES:
            errors.append(f"invalid test-entry verification class: {source}")
        cost_tier = owned.get("cost_tier")
        if cost_tier not in COST_TIERS:
            errors.append(f"invalid test-entry cost tier: {source}")
        heavy_entries = owned.get("heavy_entries", [])
        if not isinstance(heavy_entries, list) or len(heavy_entries) != len(set(heavy_entries)):
            errors.append(f"invalid heavy-entry inventory: {source}")
            heavy_entries = []
        for name in heavy_entries:
            if name not in actual["entries"]:
                errors.append(f"unknown heavy test entry in {source}: {name}")
        required_heavy = KNOWN_HEAVY_ENTRIES.get(source, set())
        if set(heavy_entries) != required_heavy:
            errors.append(f"R0 heavyweight entry ownership drift: {source}")

        target_record = target_by_name.get(str(owned.get("target")))
        if source.startswith("tests/") and target_record is not None:
            if owned.get("domain") != target_record.get("domain"):
                errors.append(f"integration target/test domain disagreement: {source}")
            if verification_class != target_record.get("verification_class"):
                errors.append(f"integration target/test class disagreement: {source}")

        if verification_class == "fast":
            marked = [name for name in actual["entries"] if FAST_HEAVY_MARKERS.search(str(name))]
            needs_exemption = bool(
                actual["has_ignored"]
                or actual["has_heavy_source_marker"]
                or cost_tier == "heavy"
                or heavy_entries
                or marked
            )
            if actual["has_ignored"] or cost_tier == "heavy" or heavy_entries:
                errors.append(f"heavy or ignored test assigned to Fast: {source}")
            if needs_exemption and not owned.get("fast_cost_exemption"):
                errors.append(f"Fast heavy marker lacks cost exemption: {source}")

    scope = manifest.get("scope", {})
    expected_counts = {
        "rust_test_targets": len(actual_targets),
        "rust_test_entry_groups": len(actual_groups),
        "rust_test_entries": sum(int(item["entry_count"]) for item in actual_groups),
    }
    for key, value in expected_counts.items():
        if not isinstance(scope, dict) or scope.get(key) != value:
            errors.append(f"scope count drift: {key}")

    if errors:
        raise OwnershipError("\n".join(errors))
    return {
        **expected_counts,
        "targets_by_class": dict(Counter(str(item["verification_class"]) for item in owned_targets)),
    }


def entry_class_counts(groups: list[dict[str, object]]) -> dict[str, int]:
    counts: Counter[str] = Counter()
    for item in groups:
        counts[str(item["verification_class"])] += int(item["entry_count"])
    return dict(sorted(counts.items()))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", default="product/test-ownership.json")
    parser.add_argument("--bootstrap", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    manifest_path = root / args.manifest
    if args.bootstrap:
        manifest = bootstrap(root)
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {manifest_path.relative_to(root)}")
        return 0
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        report = verify(root, manifest)
    except (OwnershipError, OSError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        print(f"test ownership check failed:\n{error}", file=sys.stderr)
        return 1
    groups = manifest["entry_groups"]
    print(
        "test_ownership=passed "
        f"targets={report['rust_test_targets']} "
        f"entry_groups={report['rust_test_entry_groups']} "
        f"entries={report['rust_test_entries']} "
        f"targets_by_class={json.dumps(report['targets_by_class'], sort_keys=True)} "
        f"entries_by_class={json.dumps(entry_class_counts(groups), sort_keys=True)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
