#!/usr/bin/env python3
"""Fail-closed audit of the R3.4 product spelling transition."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INVENTORY = ROOT / "product/spelling-transition-2026-09-02.json"
EXCLUDED = {
    "product/spelling-transition-2026-09-02.json",
    "scripts/check-spelling-transition.py",
    "tests/test_spelling_transition.py",
}
LEGACY = re.compile(
    r"\b(?:DICOM_TEST_SUITE|DTS)_[A-Z0-9_]+\b"
    r"|\b(?:dicom_test_suite|dts_[a-z][a-z0-9_]*)\b"
    r"|(?<![A-Za-z0-9])(?:dicom-test-suite|dts-[a-z0-9][a-z0-9-]*)(?![A-Za-z0-9])"
)
ENVIRONMENT = re.compile(r"\b(?:DICOM_TEST_SUITE|DTS)_[A-Z0-9_]+\b")
PATH_MARKERS = (
    "temp_dir",
    "TemporaryDirectory",
    "mktemp",
    ".join(",
    "with_extension",
    "conformance_work_dir",
)


def tracked_text() -> dict[str, str]:
    result = subprocess.run(
        ["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True
    )
    texts: dict[str, str] = {}
    for raw in result.stdout.split(b"\0"):
        if not raw:
            continue
        path = raw.decode()
        if path in EXCLUDED or path.endswith(".md"):
            continue
        try:
            texts[path] = (ROOT / path).read_text()
        except (UnicodeDecodeError, OSError):
            continue
    return texts


def retained_environment_names(inventory: dict[str, object]) -> set[str]:
    return {item["name"] for item in inventory["retained_adapter_environment"]}


def propose_class(token: str, inventory: dict[str, object]) -> str:
    if token in retained_environment_names(inventory):
        return "qualified_adapter_environment"
    if ENVIRONMENT.fullmatch(token):
        return "dicom_payload_identifier"
    if token == "dicom_test_suite" or token.startswith("dts_"):
        return "locked_python_module_or_backend"
    if token.startswith("dts-"):
        return "qualified_adapter_or_test_fixture"
    if token == "dicom-test-suite":
        return "legacy_payload_schema_fixture_or_evidence"
    raise AssertionError(f"unclassified legacy token {token!r}")


def legacy_occurrences(texts: dict[str, str]) -> Counter[tuple[str, str]]:
    findings: Counter[tuple[str, str]] = Counter()
    for path, text in texts.items():
        for match in LEGACY.finditer(text):
            findings[(path, match.group(0))] += 1
    return findings


def proposed_records(
    texts: dict[str, str], inventory: dict[str, object]
) -> list[dict[str, object]]:
    records = []
    classes = inventory["retained_classes"]
    for (path, token), count in sorted(legacy_occurrences(texts).items()):
        category = propose_class(token, inventory)
        metadata = classes[category]
        records.append(
            {
                "path": path,
                "token": token,
                "class": category,
                "count": count,
                "owner": metadata["owner"],
                "reason": metadata["reason"],
            }
        )
    return records


def snapshot(records: list[dict[str, object]]) -> dict[str, object]:
    encoded = json.dumps(records, sort_keys=True, separators=(",", ":")).encode()
    by_class: Counter[str] = Counter()
    for record in records:
        by_class[record["class"]] += record["count"]
    return {
        "match_count": sum(by_class.values()),
        "sha256": hashlib.sha256(encoded).hexdigest(),
        "by_class": dict(sorted(by_class.items())),
    }


def environment_accesses(texts: dict[str, str]) -> set[tuple[str, str]]:
    accesses: set[tuple[str, str]] = set()
    for path, text in texts.items():
        for line in text.splitlines():
            for match in ENVIRONMENT.finditer(line):
                token = match.group(0)
                quoted = re.escape(token)
                if (
                    re.search(
                        rf"(?:env::(?:var|var_os|set_var|remove_var)|"
                        rf"\.env(?:_remove)?|require_environment_path)\s*\(\s*['\"]{quoted}['\"]",
                        line,
                    )
                    or re.search(
                        rf"os\.(?:getenv|environ\.get)\s*\(\s*['\"]{quoted}['\"]",
                        line,
                    )
                    or re.search(rf"\$(?:\{{)?{quoted}\b", line)
                    or re.search(
                        rf"['\"](?:executable_env|root_env|artifact_root_env|environment_override)['\"]"
                        rf"\s*:\s*['\"]{quoted}['\"]",
                        line,
                    )
                    or ("GITHUB_ENV" in line and token in line)
                ):
                    accesses.add((path, token))
    return accesses


def rust_test_lines(text: str) -> set[int]:
    test_lines: set[int] = set()
    pending = False
    depth = 0
    for line_number, line in enumerate(text.splitlines(), 1):
        if line.strip() == "#[cfg(test)]":
            pending = True
            test_lines.add(line_number)
            continue
        if pending:
            test_lines.add(line_number)
            if "{" in line:
                depth = line.count("{") - line.count("}")
                pending = False
            continue
        if depth > 0:
            test_lines.add(line_number)
            depth += line.count("{") - line.count("}")
    return test_lines


def production_path_building(texts: dict[str, str]) -> list[tuple[str, int, str]]:
    findings: list[tuple[str, int, str]] = []
    for path, text in texts.items():
        if path.startswith("tests/") or "/tests/" in path:
            continue
        if path.startswith("src/") and path.endswith("_tests.rs"):
            continue
        test_lines = rust_test_lines(text) if path.endswith(".rs") else set()
        lines = text.splitlines()
        for line_number, line in enumerate(lines, 1):
            if line_number in test_lines:
                continue
            tokens = [match.group(0) for match in LEGACY.finditer(line)]
            if not tokens:
                continue
            nearby = "\n".join(lines[max(0, line_number - 3) : line_number + 1])
            if any(marker in nearby for marker in PATH_MARKERS):
                findings.extend((path, line_number, token) for token in tokens)
    return findings


def validate(texts: dict[str, str], inventory: dict[str, object]) -> list[str]:
    errors: list[str] = []
    exceptions = {
        (item["token"], item["path"])
        for item in inventory["allowed_removed_occurrences"]
    }
    for old, new in inventory["removed_environment"]:
        old_paths = [path for path, text in texts.items() if old in text]
        unexpected = [path for path in old_paths if (old, path) not in exceptions]
        if unexpected:
            errors.append(f"removed environment {old} remains in {unexpected}")
        if not any(new in text for text in texts.values()):
            errors.append(f"replacement environment {new} is undiscoverable")
    for old, new in inventory["removed_path_prefixes"]:
        old_paths = [path for path, text in texts.items() if old in text]
        if old_paths:
            errors.append(f"removed path prefix {old!r} remains in {old_paths}")
        if not any(new in text for text in texts.values()):
            errors.append(f"replacement path prefix {new!r} is undiscoverable")
    observed_env = {
        match.group(0) for text in texts.values() for match in ENVIRONMENT.finditer(text)
    }
    retained_adapter = retained_environment_names(inventory)
    missing_adapter = sorted(retained_adapter - observed_env)
    if missing_adapter:
        errors.append(f"inventoried adapter environments are absent: {missing_adapter}")
    for item in inventory["retained_adapter_environment"]:
        for evidence_path in item["evidence_paths"]:
            if evidence_path not in texts:
                errors.append(
                    f"adapter environment {item['name']} evidence is absent: {evidence_path}"
                )
            elif item["name"] not in texts[evidence_path]:
                errors.append(
                    f"adapter environment {item['name']} is not bound by {evidence_path}"
                )
    unexpected_access = sorted(
        (path, token)
        for path, token in environment_accesses(texts)
        if token not in retained_adapter
    )
    if unexpected_access:
        errors.append(f"unapproved legacy environment access remains: {unexpected_access}")
    old_paths = production_path_building(texts)
    if old_paths:
        errors.append(f"legacy production path-building callsites remain: {old_paths}")

    approved: dict[tuple[str, str], dict[str, object]] = {}
    for record in inventory["retained_occurrences"]:
        key = (record["path"], record["token"])
        if key in approved:
            errors.append(f"duplicate retained occurrence record: {key}")
        approved[key] = record
        if record["class"] not in inventory["retained_classes"]:
            errors.append(f"unknown retained class for {key}: {record['class']}")
        if not record["owner"] or not record["reason"]:
            errors.append(f"retained occurrence lacks owner/reason: {key}")
    observed = legacy_occurrences(texts)
    unlisted = sorted(set(observed) - set(approved))
    missing = sorted(set(approved) - set(observed))
    if unlisted:
        errors.append(f"unlisted legacy occurrences require review: {unlisted}")
    if missing:
        errors.append(f"approved legacy occurrences are absent: {missing}")
    actual_records = []
    for key in sorted(set(observed) & set(approved)):
        record = dict(approved[key])
        record["count"] = observed[key]
        actual_records.append(record)
        if observed[key] != approved[key]["count"]:
            errors.append(
                f"retained occurrence count drift for {key}: "
                f"expected={approved[key]['count']} actual={observed[key]}"
            )
    actual = snapshot(actual_records)
    if actual != inventory["retained_snapshot"]:
        errors.append(
            "retained spelling snapshot drift: "
            f"expected={inventory['retained_snapshot']} actual={actual}"
        )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--inventory", type=Path, default=DEFAULT_INVENTORY)
    parser.add_argument("--bootstrap", action="store_true")
    args = parser.parse_args()
    inventory = json.loads(args.inventory.read_text())
    texts = tracked_text()
    if args.bootstrap:
        records = proposed_records(texts, inventory)
        print(
            json.dumps(
                {
                    "proposal_only": True,
                    "retained_occurrences": records,
                    "retained_snapshot": snapshot(records),
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 0
    errors = validate(texts, inventory)
    if errors:
        print("spelling transition check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print(
        "spelling_transition=passed "
        f"matches={inventory['retained_snapshot']['match_count']} "
        f"sha256={inventory['retained_snapshot']['sha256']} "
        f"classes={json.dumps(inventory['retained_snapshot']['by_class'], sort_keys=True)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
