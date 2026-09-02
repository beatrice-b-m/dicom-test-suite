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
}
LEGACY = re.compile(
    r"\b(?:DICOM_TEST_SUITE|DTS)_[A-Z0-9_]+\b"
    r"|\b(?:dicom_test_suite|dts_[a-z][a-z0-9_]*)\b"
    r"|(?<![A-Za-z0-9])(?:dicom-test-suite|dts-[a-z0-9][a-z0-9-]*)(?![A-Za-z0-9])"
)
ENVIRONMENT = re.compile(r"\b(?:DICOM_TEST_SUITE|DTS)_[A-Z0-9_]+\b")


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


def classify(token: str, path: str, inventory: dict[str, object]) -> str:
    exceptions = {
        (item["token"], item["path"]): item["class"]
        for item in inventory["allowed_removed_occurrences"]
    }
    if (token, path) in exceptions:
        return exceptions[(token, path)]
    if token in inventory["retained_adapter_environment"]:
        return "qualified_adapter_environment"
    if ENVIRONMENT.fullmatch(token):
        return "dicom_payload_identifier"
    if token == "dicom_test_suite" or token.startswith("dts_"):
        return "locked_python_module_or_backend"
    if token.startswith("dts-"):
        return "qualified_adapter_or_test_fixture"
    if token == "dicom-test-suite":
        return "legacy_payload_schema_fixture_or_evidence"
    raise AssertionError(f"unclassified legacy token {token!r} in {path}")


def snapshot(texts: dict[str, str], inventory: dict[str, object]) -> dict[str, object]:
    findings: Counter[tuple[str, str, str]] = Counter()
    for path, text in texts.items():
        for match in LEGACY.finditer(text):
            token = match.group(0)
            findings[(path, token, classify(token, path, inventory))] += 1
    records = [
        {"path": path, "token": token, "class": category, "count": count}
        for (path, token, category), count in sorted(findings.items())
    ]
    encoded = json.dumps(records, sort_keys=True, separators=(",", ":")).encode()
    by_class: Counter[str] = Counter()
    for record in records:
        by_class[record["class"]] += record["count"]
    return {
        "match_count": sum(by_class.values()),
        "sha256": hashlib.sha256(encoded).hexdigest(),
        "by_class": dict(sorted(by_class.items())),
    }


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
        match.group(0)
        for text in texts.values()
        for match in ENVIRONMENT.finditer(text)
    }
    retained_adapter = set(inventory["retained_adapter_environment"])
    missing_adapter = sorted(retained_adapter - observed_env)
    if missing_adapter:
        errors.append(f"inventoried adapter environments are absent: {missing_adapter}")
    actual = snapshot(texts, inventory)
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
    actual = snapshot(texts, inventory)
    if args.bootstrap:
        print(json.dumps(actual, indent=2, sort_keys=True))
        return 0
    errors = validate(texts, inventory)
    if errors:
        print("spelling transition check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print(
        "spelling_transition=passed "
        f"matches={actual['match_count']} sha256={actual['sha256']} "
        f"classes={json.dumps(actual['by_class'], sort_keys=True)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
