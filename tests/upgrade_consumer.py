#!/usr/bin/env python3
"""Exercise every advertised current/prior CLI input version on an installed binary."""

import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

BINARY = pathlib.Path(sys.argv[1]).resolve()

def run(arguments, cwd, expected=0):
    result = subprocess.run([str(BINARY), *arguments], cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    assert result.returncode == expected, (arguments, result.returncode, result.stderr)
    assert (result.stderr if expected == 0 else result.stdout) == ""
    return json.loads(result.stdout if expected == 0 else result.stderr)

with tempfile.TemporaryDirectory(prefix="dts-upgrade-consumer-") as temporary:
    work = pathlib.Path(temporary)
    supported = run(["capabilities","--format","json"], work)["result"]["supported_versions"]
    assert supported["composition_request"] == ["0.1.0"]
    assert supported["assembly_request"] == ["1.0.0"]
    assert supported["curated_manifest"] == ["0.2.0","0.3.0"]
    assert supported["composition_manifest"] == ["0.4.0","0.5.0"]
    assert supported["cli_api"] == ["1.0.0"]

    composition_spec = work / "composition.json"
    composition_spec.write_text(json.dumps({"composition_spec_schema_version":"0.1.0","instances":[{"instance_id":"primary","template":{"id":"classic/secondary-capture/monochrome"}}]}))
    composition = work / "composition-current"
    run(["compose","--spec",str(composition_spec),"--out",str(composition),"--format","json"], work)
    prior_composition = work / "composition-prior"
    shutil.copytree(composition, prior_composition)
    manifest_path = prior_composition / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["manifest_schema_version"] = "0.4.0"
    manifest.pop("product_resources")
    manifest_path.write_text(json.dumps(manifest))
    run(["validate",str(prior_composition),"--format","json"], work)
    run(["report",str(prior_composition),"--format","json","--cli-api","1.0.0"], work)

    curated = work / "curated-current"
    run(["generate","--profile","smoke","--out",str(curated),"--seed","1","--format","json"], work)
    prior_curated = work / "curated-prior"
    shutil.copytree(curated, prior_curated)
    manifest_path = prior_curated / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["manifest_schema_version"] = "0.2.0"
    manifest.pop("product_resources")
    manifest_path.write_text(json.dumps(manifest))
    run(["validate",str(prior_curated),"--format","json"], work)
    run(["report",str(prior_curated),"--format","json","--cli-api","1.0.0"], work)

    invalid_composition = work / "invalid-composition.json"
    invalid_composition.write_text(json.dumps({"composition_spec_schema_version":"9.0.0","instances":[]}))
    invalid_assembly = work / "invalid-assembly.json"
    invalid_assembly.write_text(json.dumps({"assembly_request_schema_version":"9.0.0","instances":[]}))
    failures = [
        run(["compose","--spec",str(invalid_composition),"--out",str(work / "never-compose"),"--format","json"], work, 2),
        run(["assemble","--request",str(invalid_assembly),"--out",str(work / "never-assemble"),"--format","json"], work, 2),
        run(["report",str(composition),"--format","json","--cli-api","9.0.0"], work, 2),
    ]
    for failure in failures:
        assert failure["error"]["code"] == "request.version.unsupported"
        assert failure["error"]["context"]["migration_action"] == "select a version advertised by capabilities.result.supported_versions"

print("installed upgrade consumer passed")
