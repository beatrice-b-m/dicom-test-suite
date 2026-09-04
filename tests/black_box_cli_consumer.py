#!/usr/bin/env python3
"""Black-box CLI API 1.0.0 consumer using only subprocess, JSON, and schemas."""

import json
import pathlib
import subprocess
import sys
import tempfile


BINARY = pathlib.Path(sys.argv[1]).resolve()
CONTRACT_ROOT = pathlib.Path(sys.argv[2]).resolve()
SCHEMAS = CONTRACT_ROOT / "schemas"


def load_schema(name):
    return json.loads((SCHEMAS / name).read_text())


def dereference_root(schema):
    reference = schema.get("$ref")
    if not reference:
        return schema
    assert reference.startswith("#/$defs/")
    return schema["$defs"][reference.rsplit("/", 1)[1]]


def validate_contract(value, schema_name):
    schema = dereference_root(load_schema(schema_name))
    assert isinstance(value, dict), (schema_name, value)
    required = set(schema.get("required", []))
    assert required <= set(value), (schema_name, required - set(value))
    if schema.get("additionalProperties") is False:
        assert set(value) <= set(schema.get("properties", {})), (
            schema_name,
            set(value) - set(schema.get("properties", {})),
        )
    for field, declaration in schema.get("properties", {}).items():
        if field not in value:
            continue
        if "const" in declaration:
            assert value[field] == declaration["const"], (schema_name, field)
        if "enum" in declaration:
            assert value[field] in declaration["enum"], (schema_name, field)


def invoke(arguments, expected=0):
    completed = subprocess.run(
        [str(BINARY), *arguments],
        cwd=WORK,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert completed.returncode == expected, (arguments, completed.returncode, completed.stderr)
    payload_text = completed.stdout if expected == 0 else completed.stderr
    other = completed.stderr if expected == 0 else completed.stdout
    assert other == "", (arguments, other)
    payload = json.loads(payload_text)
    validate_contract(
        payload,
        "cli-success-envelope.schema.json" if expected == 0 else "cli-error-envelope.schema.json",
    )
    return payload


def success(arguments, result_schema):
    envelope = invoke(arguments)
    validate_contract(envelope["result"], result_schema)
    return envelope


def write_composition_spec(path):
    path.write_text(
        json.dumps(
            {
                "composition_spec_schema_version": "0.1.0",
                "instances": [
                    {
                        "instance_id": "primary",
                        "template": {"id": "classic/secondary-capture/monochrome"},
                    }
                ],
            }
        )
    )


def write_assembly_request(path):
    path.write_text(
        json.dumps(
            {
                "assembly_request_schema_version": "1.0.0",
                "instances": [
                    {
                        "instance_id": "structural",
                        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7",
                        "elements": [
                            {
                                "address": {"keyword": "PatientName"},
                                "value": {
                                    "kind": "string",
                                    "value": "SYNTHETIC^CONSUMER",
                                },
                            }
                        ],
                        "bulk": [
                            {
                                "kind": "integer_pixel_data",
                                "source": {
                                    "kind": "inline_base64",
                                    "base64": "AAECAw==",
                                },
                                "rows": 2,
                                "columns": 2,
                                "bits_allocated": 8,
                                "bits_stored": 8,
                            }
                        ],
                    }
                ],
            }
        )
    )


with tempfile.TemporaryDirectory(prefix="dts-python-consumer-") as temporary:
    WORK = pathlib.Path(temporary)
    spec = WORK / "request.json"
    write_composition_spec(spec)

    version = success(["version", "--format", "json"], "version-result-v2.schema.json")
    capabilities = success(
        ["capabilities", "--format", "json"], "capabilities-result-v3.schema.json"
    )
    assert version["result"]["cli_api_version"] == "1.0.0"
    assert "composition" in capabilities["result"]["supported_versions"]["result_schemas"]

    success(
        ["templates", "list", "--format", "json"], "templates-result.schema.json"
    )
    success(
        ["list-cases", "--profile", "smoke", "--format", "json"],
        "case-list-result.schema.json",
    )
    success(
        ["standards", "check-lock", "--format", "json"],
        "standards-result.schema.json",
    )
    success(
        ["conformance", "check-tools", "--format", "json"],
        "conformance-result.schema.json",
    )

    preview_root = WORK / "preview"
    preview = success(
        [
            "compose",
            "--spec",
            str(spec),
            "--out",
            str(preview_root),
            "--dry-run",
            "--format",
            "json",
        ],
        "composition-result.schema.json",
    )
    assert preview["result"]["published"] is False
    assert not preview_root.exists()

    composition_root = WORK / "composition"
    published = success(
        [
            "compose",
            "--spec",
            str(spec),
            "--out",
            str(composition_root),
            "--format",
            "json",
        ],
        "composition-result.schema.json",
    )
    assert published["result"]["published"] is True
    assert set(preview["result"]) == set(published["result"])
    success(
        ["validate", str(composition_root), "--format", "json"],
        "validation-result.schema.json",
    )

    raw_report = subprocess.run(
        [str(BINARY), "report", str(composition_root), "--format", "json"],
        cwd=WORK,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    assert raw_report.stderr == ""
    raw_report = json.loads(raw_report.stdout)
    wrapped_report = success(
        [
            "report",
            str(composition_root),
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ],
        "report-result.schema.json",
    )
    assert wrapped_report["result"]["report"] == raw_report

    assembly_request = WORK / "assembly.json"
    write_assembly_request(assembly_request)
    assert capabilities["result"]["structural_assembly"]["availability"] == "available"
    assembly_preview_root = WORK / "assembly-preview"
    assembly_preview = success(
        [
            "assemble",
            "--request",
            str(assembly_request),
            "--out",
            str(assembly_preview_root),
            "--dry-run",
            "--format",
            "json",
        ],
        "assembly-result.schema.json",
    )
    assert assembly_preview["result"]["published"] is False
    assert not assembly_preview_root.exists()
    assembly_root = WORK / "assembly"
    assembly = success(
        [
            "assemble",
            "--request",
            str(assembly_request),
            "--out",
            str(assembly_root),
            "--format",
            "json",
        ],
        "assembly-result.schema.json",
    )
    assert assembly["result"]["published"] is True
    assert set(assembly_preview["result"]) == set(assembly["result"])
    structural_manifest = json.loads((assembly_root / "manifest.json").read_text())
    assert structural_manifest["run"]["iod_conformance"] == "not_assessed"
    assert "template_id" not in json.dumps(structural_manifest)
    success(
        ["validate", str(assembly_root), "--format", "json"],
        "validation-result.schema.json",
    )
    structural_report = success(
        [
            "report",
            str(assembly_root),
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ],
        "report-result.schema.json",
    )
    assert structural_report["result"]["report_kind"] == "structural_assembly"
    assert structural_report["result"]["report"]["iod_conformance"] == "not_assessed"

    generation_root = WORK / "generated"
    success(
        [
            "generate",
            "--profile",
            "smoke",
            "--out",
            str(generation_root),
            "--format",
            "json",
        ],
        "generation-result.schema.json",
    )

    syntax = invoke(["capabilities", "--format", "json", "--unknown"], 2)
    unavailable = invoke(
        ["standards", "verify-kb", "--edition", "2026b", "--format", "json"], 3
    )
    conflict = invoke(
        [
            "generate",
            "--profile",
            "smoke",
            "--out",
            str(generation_root),
            "--format",
            "json",
        ],
        4,
    )
    (composition_root / "instances" / "primary.dcm").write_bytes(b"tampered")
    evidence = invoke(["validate", str(composition_root), "--format", "json"], 5)
    directory_registry = WORK / "registry-directory"
    directory_registry.mkdir()
    internal_io = invoke(
        [
            "list-cases",
            "--registry",
            str(directory_registry),
            "--format",
            "json",
        ],
        6,
    )
    assert [
        syntax["error"]["code"],
        unavailable["error"]["code"],
        conflict["error"]["code"],
        evidence["error"]["code"],
        internal_io["error"]["code"],
    ] == [
        "command.syntax.invalid",
        "capability.runtime.unavailable",
        "output.destination.exists",
        "validation.artifact.failed",
        "io.read.failed",
    ]

print("black-box CLI API 1.0.0 consumer passed")
