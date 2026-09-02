#!/usr/bin/env python3
"""Select and optionally execute bounded Fast/subsystem test bundles."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CONFIG = ROOT / "product/change-test-routing.json"
ZERO_REVISION = "0" * 40
FORBIDDEN_ARG_FRAGMENTS = {
    "--all-features", "--all-targets", "--features", "--ignored", "--release",
}
ALLOWED_IGNORED_PREFIXES = {".agents/", ".codex/", "docs/"}
ALLOWED_IGNORED_EXACT = {
    ".gitignore", "AGENTS.md", "CHANGELOG.md", "CURRENT_PLAN.md", "CURRENT_PROGRESS.md",
    "LICENSE-APACHE", "LICENSE-MIT", "README.md", "SYSTEM_SPEC.md",
    "docs/synth-dicom-gen-dcmview-corpus-migration-status-2026-09-01.md",
    "product/migration-file-ownership-2026-09-01.json",
    "scripts/run_codex_current_slices.sh", "scripts/run_codex_slices.sh",
}


class RoutingError(RuntimeError):
    pass


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RoutingError(f"cannot load {path}: {error}") from error
    if not isinstance(value, dict):
        raise RoutingError(f"JSON root must be an object: {path}")
    return value


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def normal_path(value: str) -> str:
    if not value or "\0" in value or "\n" in value or "\r" in value:
        raise RoutingError(f"invalid changed path: {value!r}")
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or str(path) != value:
        raise RoutingError(f"changed path must be canonical repository-relative POSIX: {value!r}")
    return value


def command_key(command: dict[str, Any]) -> tuple[str, str, str]:
    return command.get("kind", "test"), command.get("target", ""), command.get("module", "")


def argv_for(command: dict[str, Any]) -> list[str]:
    argv = ["cargo", "test", "--locked", "--no-default-features"]
    if command.get("kind") == "lib":
        argv.append("--lib")
    else:
        argv.extend(["--test", command["target"]])
    if command.get("module"):
        argv.append(f"{command['module']}::")
    return argv


def ownership_index(ownership: dict[str, Any]) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]]]:
    targets = {item["name"]: item for item in ownership.get("targets", [])}
    groups = {item["source"]: item for item in ownership.get("entry_groups", [])}
    if len(targets) != len(ownership.get("targets", [])) or len(groups) != len(ownership.get("entry_groups", [])):
        raise RoutingError("ownership manifest contains duplicate target or source records")
    return targets, groups


def validate_command(command: dict[str, Any], targets: dict[str, dict[str, Any]], groups: dict[str, dict[str, Any]]) -> None:
    allowed = {"kind", "target", "module", "source", "list_count", "ignored_heavy_skipped"}
    if set(command) - allowed:
        raise RoutingError(f"route command must contain only structured target/module fields: {command}")
    if command.get("kind") == "lib":
        if (
            "target" in command
            or not isinstance(command.get("module"), str)
            or not command["module"]
            or not all(
                component and component.replace("_", "a").isalnum()
                for component in command["module"].split("::")
            )
            or not isinstance(command.get("source"), str)
            or not isinstance(command.get("list_count"), int)
            or command["list_count"] <= 0
        ):
            raise RoutingError(f"lib route requires an explicit filter/source and no integration target: {command}")
        group = groups.get(command["source"])
        if not group or group.get("target") != "synth_dicom_gen":
            raise RoutingError(f"lib route source is not owned by the library target: {command['source']}")
        ignored_exception = command.get("ignored_heavy_skipped") is True
        if ignored_exception and command["source"] != "src/generation_backends/process.rs":
            raise RoutingError("ignored-heavy lib exception is restricted to the R1.4 provider process inventory")
        if (
            group.get("verification_class") not in {"fast", "subsystem"}
            or (group.get("cost_tier") != "ordinary" and not ignored_exception)
            or group.get("heavy_entries")
        ):
            raise RoutingError(f"lib route source is not immediate ordinary ownership: {command['source']}")
        return
    if command.get("kind") not in {None, "test"} or not isinstance(command.get("target"), str):
        raise RoutingError(f"integration route requires a structured target: {command}")
    if set(command) & {"source", "list_count", "ignored_heavy_skipped"}:
        raise RoutingError(f"integration route cannot carry lib-only metadata: {command}")
    target_name = command["target"]
    target = targets.get(target_name)
    if not target or target.get("kind") != "integration":
        raise RoutingError(f"route command references unknown integration target: {target_name}")
    module = command.get("module")
    owned = [group for group in groups.values() if group.get("target") == target_name]
    if module is not None:
        if not isinstance(module, str) or not module or not module.replace("_", "a").isalnum():
            raise RoutingError(f"invalid route module: {module!r}")
        matches = [group for group in owned if Path(group["source"]).stem == module]
        if len(matches) != 1:
            raise RoutingError(f"route module must resolve once in ownership: {target_name}/{module}")
        group = matches[0]
        if group.get("verification_class") not in {"fast", "subsystem"}:
            raise RoutingError(f"immediate route is not Fast/subsystem owned: {target_name}/{module}")
        if group.get("cost_tier") != "ordinary" or group.get("heavy_entries"):
            raise RoutingError(f"immediate route contains heavy ownership: {target_name}/{module}")
    else:
        classes = target.get("verification_classes")
        if not isinstance(classes, list) or not classes or not set(classes) <= {"fast", "subsystem"}:
            raise RoutingError(f"mixed or nonordinary target requires a module filter: {target_name}")
        if any(group.get("cost_tier") != "ordinary" or group.get("heavy_entries") for group in owned):
            raise RoutingError(f"whole-target route contains heavy ownership: {target_name}")
    argv = argv_for(command)
    if any(fragment in argv for fragment in FORBIDDEN_ARG_FRAGMENTS):
        raise RoutingError(f"forbidden immediate route argument: {argv}")


def validate_config(config: dict[str, Any], ownership: dict[str, Any]) -> None:
    if config.get("schema_version") != 1:
        raise RoutingError("unsupported routing schema version")
    targets, groups = ownership_index(ownership)
    deferred = config.get("deferred_evidence")
    bundles = config.get("bundles")
    rules = config.get("rules")
    if not isinstance(deferred, dict) or not isinstance(bundles, dict) or not isinstance(rules, list):
        raise RoutingError("routing config lacks structured deferred evidence, bundles, or rules")
    unconditional = config.get("unconditional_fast_targets")
    if not isinstance(unconditional, list) or len(unconditional) != len(set(unconditional)):
        raise RoutingError("unconditional Fast target inventory is invalid")
    for target_name in unconditional:
        target = targets.get(target_name)
        if not target or target.get("verification_classes") != ["fast"]:
            raise RoutingError(f"unconditional target is not Fast-owned: {target_name}")
    owned_fast = {
        name for name, target in targets.items()
        if target.get("kind") == "integration" and target.get("verification_classes") == ["fast"]
    }
    if set(unconditional) != owned_fast:
        raise RoutingError(f"unconditional Fast target inventory drift: {sorted(set(unconditional) ^ owned_fast)}")
    for bundle_id, bundle in bundles.items():
        if not isinstance(bundle_id, str) or not isinstance(bundle, dict):
            raise RoutingError("invalid routing bundle")
        commands = bundle.get("commands")
        if not isinstance(commands, list):
            raise RoutingError(f"bundle lacks command list: {bundle_id}")
        expanded = []
        for command in commands:
            if not isinstance(command, dict):
                raise RoutingError(f"invalid command in bundle {bundle_id}")
            if command.get("scope") == "ordinary" and isinstance(command.get("target"), str):
                if set(command) != {"target", "scope"}:
                    raise RoutingError(f"ordinary scope command has extra fields: {command}")
                target_name = command["target"]
                expanded.extend(
                    {"target": target_name, "module": Path(group["source"]).stem}
                    for group in groups.values()
                    if group.get("target") == target_name
                    and group.get("verification_class") in {"fast", "subsystem"}
                    and group.get("cost_tier") == "ordinary"
                    and not group.get("heavy_entries")
                )
            else:
                expanded.append(command)
        if not expanded and commands:
            raise RoutingError(f"bundle expands to no ordinary commands: {bundle_id}")
        for command in expanded:
            validate_command(command, targets, groups)
        for target_name in bundle.get("covered_by_fast", []):
            if target_name not in unconditional:
                raise RoutingError(f"bundle references non-unconditional Fast coverage: {bundle_id}/{target_name}")
        for evidence in bundle.get("deferred", []):
            if evidence not in deferred:
                raise RoutingError(f"bundle references unknown deferred evidence: {bundle_id}/{evidence}")
    seen_rules: set[str] = set()
    for rule in rules:
        if not isinstance(rule, dict) or not isinstance(rule.get("id"), str) or rule["id"] in seen_rules:
            raise RoutingError("route IDs must be unique strings")
        seen_rules.add(rule["id"])
        if not rule.get("all_ordinary") and not rule.get("bundles"):
            raise RoutingError(f"route selects no ordinary bundle: {rule['id']}")
        for bundle_id in rule.get("bundles", []):
            if bundle_id not in bundles:
                raise RoutingError(f"route references unknown bundle: {rule['id']}/{bundle_id}")
        for evidence in rule.get("deferred", []):
            if evidence not in deferred:
                raise RoutingError(f"route references unknown deferred evidence: {rule['id']}/{evidence}")
        for value in rule.get("exact", []):
            normal_path(value)
        for value in rule.get("prefixes", []):
            if not isinstance(value, str) or not value.endswith("/"):
                raise RoutingError(f"route prefix must end with '/': {value!r}")
            normal_path(value[:-1])
        for value in rule.get("file_prefixes", []):
            normal_path(value)
    ignored = config.get("ignored")
    if not isinstance(ignored, dict):
        raise RoutingError("routing config lacks explicit ignored paths")
    ignored_exact = set(ignored.get("exact", []))
    ignored_prefixes = set(ignored.get("prefixes", []))
    rule_exact = {value for rule in rules for value in rule.get("exact", [])}
    if ignored_exact & rule_exact:
        raise RoutingError(f"paths cannot be both routed and ignored: {sorted(ignored_exact & rule_exact)}")
    if not ignored_exact <= ALLOWED_IGNORED_EXACT or not ignored_prefixes <= ALLOWED_IGNORED_PREFIXES:
        raise RoutingError("ignored routing entries exceed the non-executable governance allowlist")
    for value in ignored_exact:
        normal_path(value)
    for value in ignored_prefixes:
        if not isinstance(value, str) or not value.endswith("/"):
            raise RoutingError(f"ignored prefix must end with '/': {value!r}")
        normal_path(value[:-1])


def all_ordinary_commands(ownership: dict[str, Any], config: dict[str, Any]) -> list[dict[str, Any]]:
    targets, groups = ownership_index(ownership)
    commands: list[dict[str, str]] = []
    for target_name, target in targets.items():
        if target.get("kind") != "integration":
            continue
        classes = target.get("verification_classes", [])
        owned = [group for group in groups.values() if group.get("target") == target_name]
        if set(classes) <= {"fast", "subsystem"} and not any(group.get("cost_tier") != "ordinary" or group.get("heavy_entries") for group in owned):
            commands.append({"target": target_name})
            continue
        for group in owned:
            if group.get("verification_class") in {"fast", "subsystem"} and group.get("cost_tier") == "ordinary" and not group.get("heavy_entries"):
                commands.append({"target": target_name, "module": Path(group["source"]).stem})
    for bundle in config["bundles"].values():
        for command in bundle["commands"]:
            if command.get("kind") == "lib":
                commands.append(command)
    return commands


def matches(rule: dict[str, Any], path: str) -> bool:
    return (
        path in rule.get("exact", [])
        or any(path.startswith(prefix) for prefix in rule.get("prefixes", []))
        or any(path.startswith(prefix) for prefix in rule.get("file_prefixes", []))
    )


def ignored(config: dict[str, Any], path: str) -> bool:
    spec = config["ignored"]
    return path in spec.get("exact", []) or any(path.startswith(prefix) for prefix in spec.get("prefixes", []))


def select(paths: list[str], config: dict[str, Any], ownership: dict[str, Any], *, force_all: bool = False) -> dict[str, Any]:
    validate_config(config, ownership)
    _, groups = ownership_index(ownership)
    bundle_ids: set[str] = set()
    deferred_ids: set[str] = {"release_candidate"} if paths or force_all else set()
    matched_rules: dict[str, list[str]] = {}
    ignored_paths: list[str] = []
    all_ordinary = force_all
    for raw_path in sorted(set(paths)):
        path = normal_path(raw_path)
        path_rules = [rule for rule in config["rules"] if matches(rule, path)]
        if PurePosixPath(path).parent == PurePosixPath("tests") and path.endswith(".rs"):
            group = groups.get(path)
            if group is None:
                raise RoutingError(f"unowned Rust test source: {path}")
            matched_rules[path] = ["test-ownership"]
            if (
                group.get("verification_class") in {"fast", "subsystem"}
                and group.get("cost_tier") == "ordinary"
                and not group.get("heavy_entries")
            ):
                bundle_id = (
                    "corpus_definition_internal"
                    if group.get("target") == "synth_dicom_gen"
                    else f"test:{path}"
                )
                bundle_ids.add(bundle_id)
            else:
                if group.get("heavy_entries"):
                    deferred_ids.add("explicit_heavy")
                if path in {
                    "tests/composition_curated_migration.rs",
                    "tests/composition_quantitative.rs",
                }:
                    deferred_ids.add("native_provider_contract")
                elif group.get("verification_class") == "nightly":
                    deferred_ids.add("nightly")
                elif group.get("verification_class") != "release_candidate":
                    raise RoutingError(f"nonordinary test source lacks a deferred owner: {path}")
            continue
        if path_rules:
            matched_rules[path] = sorted(rule["id"] for rule in path_rules)
            for rule in path_rules:
                bundle_ids.update(rule.get("bundles", []))
                deferred_ids.update(rule.get("deferred", []))
                all_ordinary = all_ordinary or bool(rule.get("all_ordinary"))
            continue
        if ignored(config, path):
            ignored_paths.append(path)
            continue
        raise RoutingError(f"unmapped executable/code/data path: {path}")

    commands: list[dict[str, str]] = []
    covered: set[str] = set()
    if all_ordinary:
        commands.extend(all_ordinary_commands(ownership, config))
        bundle_ids.add("all-ordinary")
        deferred_ids.update(config["deferred_evidence"])
    for bundle_id in sorted(bundle_ids):
        if bundle_id.startswith("test:"):
            source = bundle_id.removeprefix("test:")
            group = groups[source]
            commands.append({"target": group["target"], "module": Path(source).stem})
            continue
        if bundle_id == "all-ordinary":
            continue
        bundle = config["bundles"][bundle_id]
        for command in bundle["commands"]:
            if command.get("scope") == "ordinary":
                commands.extend(
                    {"target": command["target"], "module": Path(group["source"]).stem}
                    for group in groups.values()
                    if group.get("target") == command["target"]
                    and group.get("verification_class") in {"fast", "subsystem"}
                    and group.get("cost_tier") == "ordinary"
                    and not group.get("heavy_entries")
                )
            else:
                commands.append(command)
        covered.update(bundle.get("covered_by_fast", []))
        deferred_ids.update(bundle.get("deferred", []))
    unconditional = set(config["unconditional_fast_targets"])
    deduped: dict[tuple[str, str, tuple[str, ...]], dict[str, Any]] = {}
    for command in commands:
        validate_command(command, *ownership_index(ownership))
        if command.get("target") in unconditional:
            covered.add(command["target"])
            continue
        deduped[command_key(command)] = command
    whole_targets = {
        command["target"] for command in deduped.values()
        if command.get("kind") != "lib" and not command.get("module")
    }
    deduped = {
        key: command for key, command in deduped.items()
        if command.get("kind") == "lib" or not command.get("module") or command.get("target") not in whole_targets
    }
    ordered = [deduped[key] for key in sorted(deduped)]
    return {
        "schema_version": 1,
        "changed_paths": sorted(set(paths)),
        "matched_rules": matched_rules,
        "ignored_paths": sorted(ignored_paths),
        "bundle_ids": sorted(bundle_ids),
        "commands": [{"argv": argv_for(command), **command} for command in ordered],
        "covered_by_unconditional_fast": sorted(covered | unconditional),
        "deferred_evidence": [
            {"id": evidence, "reason": config["deferred_evidence"][evidence]}
            for evidence in sorted(deferred_ids)
        ],
    }


def diff_paths(base: str, head: str) -> list[str]:
    for revision in [head]:
        if len(revision) != 40 or any(character not in "0123456789abcdef" for character in revision):
            raise RoutingError(f"git diff revision must be immutable lowercase 40-hex: {revision!r}")
    if not base or base == ZERO_REVISION:
        return []
    if len(base) != 40 or any(character not in "0123456789abcdef" for character in base):
        raise RoutingError(f"git diff revision must be immutable lowercase 40-hex: {base!r}")
    result = subprocess.run(
        ["git", "diff", "--name-status", "-z", f"{base}...{head}", "--"],
        cwd=ROOT,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode:
        raise RoutingError(result.stderr.decode("utf-8", "replace").strip() or "git diff failed")
    fields = result.stdout.split(b"\0")
    paths: list[str] = []
    index = 0
    while index < len(fields) and fields[index]:
        status = fields[index].decode("ascii", "strict")
        index += 1
        if not (
            status in {"A", "D", "M", "T"}
            or (
                status[:1] in {"R", "C"}
                and status[1:].isdigit()
                and 0 <= int(status[1:]) <= 100
            )
        ):
            raise RoutingError(f"unsupported git diff status: {status!r}")
        count = 2 if status.startswith(("R", "C")) else 1
        if index + count > len(fields):
            raise RoutingError("truncated git diff --name-status -z output")
        for field in fields[index:index + count]:
            if not field:
                raise RoutingError("truncated git diff --name-status -z output")
            paths.append(field.decode("utf-8", "strict"))
        index += count
    return paths


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--path", action="append", default=[])
    parser.add_argument("--diff", nargs=2, metavar=("BASE", "HEAD"))
    parser.add_argument("--all-ordinary", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not args.path and not args.diff and not args.all_ordinary:
            raise RoutingError("one of --path, --diff, or --all-ordinary is required")
        config_path = args.config.resolve()
        config = load_json(config_path)
        ownership_path = ROOT / config.get("ownership_manifest", "")
        ownership = load_json(ownership_path)
        force_all = args.all_ordinary
        paths = list(args.path)
        if args.diff:
            base, head = args.diff
            paths.extend(diff_paths(base, head))
            if not base or base == ZERO_REVISION:
                force_all = True
        result = select(paths, config, ownership, force_all=force_all)
        result["config_sha256"] = digest(config_path)
        result["ownership_sha256"] = digest(ownership_path)
        print(json.dumps(result, indent=2, sort_keys=True))
        if not args.dry_run:
            for command in result["commands"]:
                subprocess.run(command["argv"], cwd=ROOT, check=True)
    except (RoutingError, OSError, subprocess.CalledProcessError, UnicodeError) as error:
        print(f"change-test routing failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
