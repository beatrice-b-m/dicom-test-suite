#!/usr/bin/env python3
"""Explicit R5 boundary proof; never package, installed-release, or RC qualification."""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import time


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def inventory(root):
    files = sorted(p for p in root.rglob("*") if p.is_file())
    return {"file_count": len(files), "logical_bytes": sum(p.stat().st_size for p in files),
            "allocated_bytes": sum(p.stat().st_blocks * 512 for p in files)}


def bundle(source, destination, planned=False):
    registry = json.loads((source / "cases/registry.json").read_bytes())
    rows = [copy.deepcopy(row) for row in registry["cases"] if "smoke" in row["profiles"]]
    assert len(rows) == 3 and all(row["status"] == "implemented" for row in rows)
    for row in rows:
        # A valid caller-owned metadata difference, not a changed DICOM recipe.
        assert "metadata" not in row["compatibility_axes"]
        row["compatibility_axes"].append("metadata")
    if planned:
        row = next(copy.deepcopy(row) for row in registry["cases"] if row["case_id"] == "classic/dx/mono2_u12_jpeg_extended")
        assert row["status"] == "planned" and row["profiles"] == ["extended"]
        rows.append(row)
    destination.mkdir(mode=0o700)
    (destination / "cases").mkdir()
    write_json(destination / "cases/registry.json", {"case_registry_schema_version": registry["case_registry_schema_version"], "cases": rows})

    def descriptor(path):
        p = destination / path
        return {"path": path, "size_bytes": p.stat().st_size, "sha256": sha(p)}

    recipes = {}
    for path in sorted((source / "cases/recipes").rglob("*.json")):
        value = json.loads(path.read_bytes())
        recipes[value["binding"]["case_id"]] = (path, value)
    cases = []
    for row in rows:
        if row["status"] != "implemented":
            continue
        path, recipe = recipes[row["case_id"]]
        assert recipe["dependencies"] == []
        assert not any(e.get("source") == "local-source-note" for e in row["standards_evidence"])
        logical = path.relative_to(source).as_posix()
        target = destination / logical
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
        cases.append({"case_id": row["case_id"], "recipe_id": row["recipe_id"], "recipe_version": row["recipe_version"],
                      "recipe": descriptor(logical), "dependencies": [], "evidence_ids": []})
    scopes = {"smoke": "valid", "core": "valid", "extended": "valid", "legacy": "legacy", "stress": "stress", "negative": "expected_invalid", "fuzz": "fuzz"}
    profiles = [{"profile_id": name, "scope": scope, "members": sorted(r["case_id"] for r in rows if name in r["profiles"])} for name, scope in scopes.items()]
    profiles.append({"profile_id": "all", "scope": "valid", "union_of": ["smoke", "core", "extended"], "optional_profile": "stress"})
    definition = {"corpus_definition_bundle_schema_version": "1.0.0", "definition_id": "isolated-sdk.planned" if planned else "isolated-sdk.smoke", "definition_version": "1.0.0",
                  "profiles": profiles, "registry": descriptor("cases/registry.json"), "cases": sorted(cases, key=lambda c: c["case_id"]), "evidence": [], "assets": []}
    write_json(destination / "corpus-definition.json", definition)
    return {"descriptor_sha256": sha(destination / "corpus-definition.json"), **inventory(destination)}


def assert_sdk_imports(text):
    imports = re.findall(r"synth_dicom_gen::([A-Za-z_][A-Za-z_0-9]*|\{)", text)
    assert imports and set(imports) == {"sdk"}, imports
    assert "include!(" not in text and "CARGO_MANIFEST_DIR" not in text and "extern crate" not in text


def lock_packages(text):
    packages = set()
    for block in text.split("[[package]]")[1:]:
        fields = dict(re.findall(r'^([a-z_]+) = "([^"\n]+)"$', block, re.MULTILINE))
        if "source" in fields:
            packages.add(tuple(fields.get(key) for key in ("name", "version", "source", "checksum")))
    return packages


def remove_owned(root, name):
    assert name in {"source", "consumer", "target"}
    path = root / name
    assert path.parent == root and not path.is_symlink() and path.is_dir()
    shutil.rmtree(path)
    assert not path.exists()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--retain", required=True, type=Path)
    args = parser.parse_args()
    checkout = Path(__file__).resolve().parents[1]
    assert re.fullmatch(r"[0-9a-f]{40}", args.revision), "immutable full revision required"
    assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip() == args.revision
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=checkout), "candidate must be clean"
    root = args.artifacts
    assert root.is_absolute() and not root.exists() and root.parent.is_dir()
    assert root.parent.resolve() == root.parent, "use a non-symlinked artifact parent"
    retained = args.retain
    assert retained.is_absolute() and not retained.exists()
    assert retained.parent == checkout / "generated", "durable evidence must be ignored workspace generated output"
    retained.parent.mkdir(exist_ok=True)
    root.mkdir(mode=0o700)
    source, consumer, target = (root / name for name in ("source", "consumer", "target"))
    source.mkdir(); consumer.mkdir(); (consumer / "src").mkdir()
    logs = root / "logs"; logs.mkdir()
    receipt = {"proof_schema_version": "1.0.0", "classification": "isolated_committed_source_sdk_cli_proof_not_package_or_release_qualification",
               "source_revision": args.revision, "offline_cache_reliance": True, "commands": [], "features": [],
               "runtime_root": str(root), "retained_root": str(retained)}

    def run(label, command, cwd, env=None):
        started = time.monotonic()
        result = subprocess.run([str(v) for v in command], cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        (logs / (label + ".stdout")).write_bytes(result.stdout)
        (logs / (label + ".stderr")).write_bytes(result.stderr)
        receipt["commands"].append({"label": label, "argv": [str(v) for v in command], "cwd": str(cwd), "exit": result.returncode, "wall_seconds": time.monotonic() - started})
        write_json(root / "receipt.json", receipt)
        assert result.returncode == 0, "{} failed: {}".format(label, result.stderr.decode(errors="replace"))
        return result.stdout

    archive = root / "source.tar"
    run("archive", ["git", "archive", "--format=tar", "--output", archive, args.revision], checkout)
    receipt["source_archive"] = {"sha256": sha(archive), "size_bytes": archive.stat().st_size}
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            path = source / member.name
            assert not member.name.startswith("/") and ".." not in Path(member.name).parts
            assert member.isfile() or member.isdir(), "no symlink or special source members"
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(tar.extractfile(member).read())
                path.chmod(member.mode & 0o777)
    fixture = source / "tests/fixtures/isolated-corpus-consumer/main.rs"
    assert_sdk_imports(fixture.read_text())
    shutil.copyfile(fixture, consumer / "src/main.rs")
    (consumer / "Cargo.toml").write_text('[package]\nname="isolated-corpus-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nsynth-dicom-gen={path=' + json.dumps(str(source)) + ',default-features=false}\nserde_json="=1.0.150"\n[profile.dev]\ndebug=0\nincremental=false\n')
    shutil.copyfile(source / "Cargo.lock", consumer / "Cargo.lock")
    source_packages = lock_packages((source / "Cargo.lock").read_text())
    receipt["snapshot_cargo_lock_sha256"] = sha(source / "Cargo.lock")
    receipt["bundles"] = {"smoke": bundle(source, root / "caller-smoke"), "planned": bundle(source, root / "caller-planned", planned=True)}
    baseline = json.loads((source / "docs/baselines/dcmview-smoke-parity-seed-1-2026-09-01.json").read_bytes())
    for expected in baseline["cases"]:
        assert sha(source / expected["recipe"]["logical_path"]) == expected["recipe"]["sha256"]
    write_json(root / "r0-baseline.json", baseline)
    env = os.environ.copy(); env["CARGO_TARGET_DIR"] = str(target); env["CARGO_INCREMENTAL"] = "0"
    rustc = run("rustc", ["rustc", "-vV"], root).decode()
    host = re.search(r"^host: (\S+)$", rustc, re.MULTILINE).group(1)
    receipt["target"] = host
    run("cargo", ["cargo", "-V"], root)
    # Add the consumer root while retaining the seeded resolution, rather than
    # regenerate-lockfile's fresh resolver choices from the local cache.
    run("consumer-lock", ["cargo", "metadata", "--offline", "--filter-platform", host, "--format-version", "1"], consumer, env)
    consumer_packages = lock_packages((consumer / "Cargo.lock").read_text())
    assert consumer_packages.issubset(source_packages), "consumer dependency versions/checksums diverged from snapshot lock"
    receipt["consumer_dependency_alignment"] = {"registry_packages": len(consumer_packages), "all_versions_sources_checksums_match_snapshot": True}
    metadata = json.loads(run("consumer-metadata", ["cargo", "metadata", "--offline", "--locked", "--filter-platform", host, "--format-version", "1"], consumer, env))
    package = next(p for p in metadata["packages"] if p["name"] == "synth-dicom-gen")
    assert Path(package["manifest_path"]) == source / "Cargo.toml"
    node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == package["id"])
    assert node["features"] == [], node["features"]
    assert str(checkout) not in json.dumps(metadata), "original checkout dependency"
    receipt["dependency_manifest"] = package["manifest_path"]
    receipt["consumer_lock_sha256"] = sha(consumer / "Cargo.lock")
    run("build-consumer", ["cargo", "build", "--offline", "--locked", "--no-default-features", "--target", host, "--target-dir", target], consumer, env)
    run("build-cli", ["cargo", "build", "--offline", "--locked", "--no-default-features", "--target", host, "--bin", "synth-dicom-gen", "--target-dir", target], source, env)
    binaries = root / "bin"; binaries.mkdir()
    for name in ("isolated-corpus-consumer", "synth-dicom-gen"):
        shutil.copy2(target / host / "debug" / name, binaries / name)
    receipt["binaries"] = {p.name: {"sha256": sha(p), "size_bytes": p.stat().st_size} for p in binaries.iterdir()}
    receipt["target_before_cleanup"] = inventory(target)
    receipt["source_before_cleanup"] = inventory(source)
    receipt["consumer_before_cleanup"] = inventory(consumer)
    remove_owned(root, "source"); remove_owned(root, "consumer")
    receipt["source_roots_removed_before_runtime"] = [str(source), str(consumer)]
    unrelated = root / "unrelated"; unrelated.mkdir()
    runtime_env = os.environ.copy(); runtime_env["PATH"] = ""
    run("sdk-proof", [binaries / "isolated-corpus-consumer", root], unrelated, runtime_env)
    cli = binaries / "synth-dicom-gen"

    def command(label, argv):
        value = json.loads(run(label, [cli, *argv], unrelated, runtime_env))
        write_json(root / (label + ".json"), value)
        return value

    command("version", ["version", "--format", "json"])
    common = ["--corpus", root / "caller-smoke/corpus-definition.json", "--asset-root", root / "caller-smoke", "--profile", "smoke", "--seed", "1", "--parallelism", "2", "--format", "json"]
    capabilities = command("cli-capabilities", ["capabilities", *common])["result"]
    assert capabilities == json.loads((root / "sdk-capabilities.json").read_bytes())
    result = command("cli-generate", ["generate", *common, "--out", root / "cli-profile"])["result"]
    assert result["outcome"] == "published"
    id_options = [item for case in baseline["cases"] for item in ("--case-id", case["case_id"])]
    command("cli-generate-ids", ["generate", *common, *id_options, "--out", root / "cli-ids"])
    command("cli-validate-ids", ["validate", root / "cli-ids", "--format", "json"])
    command("cli-validate", ["validate", root / "cli-profile", "--format", "json"])
    report = command("cli-report", ["report", root / "cli-profile", "--format", "json", "--cli-api", "1.0.0"])["result"]["report"]
    sdk_manifest = json.loads((root / "sdk-profile/manifest.json").read_bytes())
    cli_manifest = json.loads((root / "cli-profile/manifest.json").read_bytes())
    assert sdk_manifest == cli_manifest, "SDK/CLI full manifest parity"
    assert sdk_manifest == json.loads((root / "sdk-repeat/manifest.json").read_bytes()), "SDK exact reproduction"
    assert json.loads((root / "sdk-ids/manifest.json").read_bytes()) == json.loads((root / "cli-ids/manifest.json").read_bytes())
    assert report == json.loads((root / "sdk-profile/sdk-report.json").read_bytes())
    preview = command("cli-dry", ["generate", *common, "--out", root / "cli-dry", "--dry-run"])["result"]
    assert preview["outcome"] == "planned" and not (root / "cli-dry").exists()
    noexec = command("cli-noexec", ["generate", "--corpus", root / "caller-planned/corpus-definition.json", "--asset-root", root / "caller-planned", "--profile", "extended", "--out", root / "cli-noexec", "--format", "json"])["result"]
    assert noexec["outcome"] == "no_executable_cases" and not (root / "cli-noexec").exists()
    identity = sdk_manifest["identity_projection"]["corpus_definition"]["identity"]
    assert identity["definition_id"] == "isolated-sdk.smoke"
    assert identity["corpus_definition_sha256"] != "571fa23fd392dd557ccdbe2db527698eaedc7078d86543efc68dfffc877411f7"
    receipt["verified_corpus_identity"] = identity
    receipt["installed_identity_domains"] = capabilities["identity_domains"]
    files = {f["case_id"]: f for f in sdk_manifest["files"]}
    assert len(files) == 3 and "skipped_cases" not in sdk_manifest
    for expected in baseline["cases"]:
        actual = files[expected["case_id"]]
        for name in ("path", "sha256", "size_bytes", "determinism"):
            assert actual[name] == expected["output"][name], (expected["case_id"], name)
        for name in ("dicom", "image", "pixel_data", "uids", "references", "expected_capabilities", "expected_semantics", "known_stressors", "profile_membership"):
            assert actual[name] == expected[name], (expected["case_id"], name)
        assert actual["validation"]["status"] == "passed"
        for key in ("recipe_id", "recipe_version"):
            assert actual["recipe"][key] == expected["recipe"][key]
        assert actual["expected_visual_checks"]["pattern"] == expected["expected_visual_pattern"]
        for key in ("internal", "standards", "external"):
            assert len(actual["validation"][key]) == expected["validation"][key + "_check_count"]
        assert len(actual["standards_evidence"]) == expected["validation"]["standards_evidence_count"]
        for output in ("sdk-profile", "sdk-ids", "sdk-repeat", "cli-profile", "cli-ids"):
            assert sha(root / output / actual["path"]) == expected["output"]["sha256"]
    for row in sdk_manifest["selection_ledger"]:
        assert row["outcome"] == "generated" and "metadata" in row["case_definition"]["compatibility_axes"]
    assert report["source_manifest"] == sdk_manifest
    receipt["r0_comparison"] = {"files": 3, "payload_bytes": 2790, "scope": "exact smoke file hashes and recorded per-case semantic fields; identity and selector contracts deliberately differ", "passed": True}
    receipt["generated"] = {name: inventory(root / name) for name in ("sdk-profile", "sdk-ids", "sdk-repeat", "cli-profile", "cli-ids")}
    receipt["status"] = "passed"
    remove_owned(root, "target")
    receipt["target_removed_after_measurement"] = True
    write_json(root / "receipt.json", receipt)
    shutil.copytree(root, retained)
    assert sha(root / "receipt.json") == sha(retained / "receipt.json")
    assert sha(archive) == sha(retained / "source.tar")
    print(json.dumps({"status": "passed", "artifacts": str(root), "receipt_sha256": sha(root / "receipt.json")}))


if __name__ == "__main__":
    main()
