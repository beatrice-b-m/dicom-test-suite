#!/usr/bin/env python3
"""Exercise the live installed qualified-template catalog without fixed counts."""

import json
import pathlib
import subprocess
import sys
import tempfile


BINARY = pathlib.Path(sys.argv[1]).resolve()


def invoke(arguments, cwd):
    completed = subprocess.run(
        [str(BINARY), *arguments],
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert completed.returncode == 0, (arguments, completed.returncode, completed.stderr)
    assert completed.stderr == "", (arguments, completed.stderr)
    return json.loads(completed.stdout)


def projection(manifest):
    return {
        entry["instance_id"]: {
            key: entry[key]
            for key in [
                "template_id",
                "template_version",
                "uids",
                "sha256",
                "resolved_plan_sha256",
                "content",
                "references",
                "path",
            ]
        }
        for entry in manifest["composition"]["entries"]
    }


with tempfile.TemporaryDirectory(prefix="dts-qualified-catalog-") as temporary:
    work = pathlib.Path(temporary)
    catalog = invoke(["templates", "list", "--format", "json"], work)["result"]
    templates = catalog["templates"]
    assert templates
    assert all(template["status"] == "qualified" for template in templates)
    dependencies = {
        dependency["template_id"]
        for template in templates
        for dependency in template["default_bundle"]["dependencies"]
    }
    roots = [template for template in templates if template["template_id"] not in dependencies]
    assert roots
    instances = [
        {
            "instance_id": f"template-{index:03}",
            "template": {
                "id": template["template_id"],
                "version": template["template_version"],
            },
        }
        for index, template in enumerate(roots)
    ]

    manifests = []
    output_roots = []
    outcomes = []
    for label, parallelism in [("serial", 1), ("parallel", 8)]:
        cwd = work / f"cwd-{label}" / "nested"
        cwd.mkdir(parents=True)
        spec = cwd / "catalog.json"
        spec.write_text(
            json.dumps(
                {
                    "composition_spec_schema_version": "0.1.0",
                    "parallelism": parallelism,
                    "instances": instances,
                },
                sort_keys=True,
            )
        )
        output_root = work / f"output-{label}"
        outcome = invoke(
            [
                "compose",
                "--spec",
                str(spec),
                "--out",
                str(output_root),
                "--seed",
                "80",
                "--format",
                "json",
            ],
            cwd,
        )
        invoke(["validate", str(output_root), "--format", "json"], cwd)
        report = invoke(
            ["report", str(output_root), "--format", "json", "--cli-api", "1.0.0"],
            cwd,
        )
        assert report["result"]["report_kind"] == "composition"
        output_roots.append(output_root)
        manifests.append(json.loads((output_root / "manifest.json").read_text()))
        result = outcome["result"]
        result.pop("requested_output_root")
        result.pop("manifest_path")
        outcomes.append(outcome)

    assert outcomes[0] == outcomes[1]
    assert manifests[0]["run"]["corpus_plan_sha256"] == manifests[1]["run"]["corpus_plan_sha256"]
    assert projection(manifests[0]) == projection(manifests[1])
    assert {entry["template_id"] for entry in manifests[0]["composition"]["entries"]} == {
        template["template_id"] for template in templates
    }
    for entry in manifests[0]["composition"]["entries"]:
        relative = entry["path"]
        assert (output_roots[0] / relative).read_bytes() == (output_roots[1] / relative).read_bytes()

print("installed qualified catalog consumer passed")
