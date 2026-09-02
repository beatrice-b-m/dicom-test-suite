#!/usr/bin/env python3
"""Assemble the current embedded corpus bytes into a strict R4.2 bundle."""

import argparse
import hashlib
import json
import shutil
from pathlib import Path


def descriptor(path: Path, logical: str) -> dict:
    data = path.read_bytes()
    return {"path": logical, "size_bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    source = args.source.resolve()
    output = args.output
    if output.exists():
        raise SystemExit(f"refusing existing output: {output}")
    output.mkdir(mode=0o700, parents=True)

    registry_path = source / "cases/registry.json"
    registry = json.loads(registry_path.read_bytes())
    recipe_files = sorted((source / "cases/recipes").rglob("*.json"))
    recipes = {json.loads(path.read_bytes())["binding"]["case_id"]: path for path in recipe_files}
    recipe_to_case = {
        (row["recipe_id"], row["recipe_version"]): row["case_id"]
        for row in registry["cases"] if row["status"] == "implemented"
    }

    cases = []
    evidence_by_path = {}
    for row in registry["cases"]:
        for item in row.get("standards_evidence", []):
            if item.get("source") == "local-source-note":
                path = item["query"]
                evidence_by_path[path] = "source-note." + path.removeprefix("standards/source-notes/").removesuffix(".md").replace("/", ".")
    for row in registry["cases"]:
        if row["status"] != "implemented":
            continue
        source_recipe = recipes[row["case_id"]]
        recipe = json.loads(source_recipe.read_bytes())
        logical = source_recipe.relative_to(source).as_posix()
        dependencies = [recipe_to_case[(item["recipe"]["recipe_id"], item["recipe"]["recipe_version"])] for item in recipe["dependencies"]]
        evidence_ids = []
        for item in row.get("standards_evidence", []):
            if item.get("source") == "local-source-note":
                evidence_path = item["query"]
                evidence_id = "source-note." + evidence_path.removeprefix("standards/source-notes/").removesuffix(".md").replace("/", ".")
                evidence_ids.append(evidence_id)
        cases.append({
            "case_id": row["case_id"],
            "recipe_id": row["recipe_id"],
            "recipe_version": row["recipe_version"],
            "recipe": descriptor(source_recipe, logical),
            "dependencies": dependencies,
            "evidence_ids": sorted(set(evidence_ids)),
        })

    direct = ["smoke", "core", "extended", "legacy", "stress", "negative", "fuzz"]
    scopes = {"smoke":"valid", "core":"valid", "extended":"valid", "legacy":"legacy", "stress":"stress", "negative":"expected_invalid", "fuzz":"fuzz"}
    profiles = [{"profile_id": profile, "scope": scopes[profile], "members": sorted(row["case_id"] for row in registry["cases"] if profile in row["profiles"])} for profile in direct]
    profiles.append({"profile_id":"all", "scope":"valid", "union_of":["smoke","core","extended"], "optional_profile":"stress"})
    evidence = [{"evidence_id": evidence_id, "media_type":"text/markdown", **descriptor(source / path, path)} for path, evidence_id in sorted(evidence_by_path.items())]
    manifest = {
        "corpus_definition_bundle_schema_version": "1.0.0",
        "definition_id": "dcmview.current-source",
        "definition_version": "1.0.0",
        "profiles": profiles,
        "registry": descriptor(registry_path, "cases/registry.json"),
        "cases": sorted(cases, key=lambda item: item["case_id"]),
        "evidence": evidence,
        "assets": [],
    }
    files = [registry_path, *recipe_files, *(source / path for path in evidence_by_path)]
    for path in files:
        destination = output / path.relative_to(source)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
    (output / "corpus-definition.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()
