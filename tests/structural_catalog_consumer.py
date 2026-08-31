#!/usr/bin/env python3
"""Exercise every installed structural content kind through the public CLI."""

import json
import pathlib
import subprocess
import sys
import tempfile


BINARY = pathlib.Path(sys.argv[1]).resolve()


def invoke(arguments, cwd):
    result = subprocess.run(
        [str(BINARY), *arguments], cwd=cwd, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    assert result.returncode == 0, (arguments, result.returncode, result.stderr)
    assert result.stderr == "", (arguments, result.stderr)
    return json.loads(result.stdout)


bulk = [
    {"kind":"integer_pixel_data","source":{"kind":"inline_base64","base64":"AAECAw=="},"rows":2,"columns":2,"bits_allocated":8,"bits_stored":8},
    {"kind":"float_pixel_data","source":{"kind":"inline_base64","base64":"AACAPw=="},"rows":1,"columns":1},
    {"kind":"double_float_pixel_data","source":{"kind":"inline_base64","base64":"AAAAAAAA8D8="},"rows":1,"columns":1},
    {"kind":"waveform_data","source":{"kind":"inline_base64","base64":"AAAAAAAAAAA="},"channels":2,"samples":2,"bits_allocated":16},
    {"kind":"encapsulated_document","source":{"kind":"inline_base64","base64":"JVBERi0xLjQ="},"media_type":"application/pdf"},
    {"kind":"mesh","source":{"kind":"inline_base64","base64":"AAAAAAAAAAAAAAAA"}},
    {"kind":"general","tag":"7776,1000","vr":"OB","source":{"kind":"inline_base64","base64":"AQIDBA=="}},
]
instances = [
    {
        "instance_id":"structural", "sop_class_uid":"1.2.840.10008.5.1.4.1.1.7",
        "elements":[
            {"address":{"keyword":"PatientName"},"value":{"kind":"string","value":"SYNTHETIC^STRUCTURAL"}},
            {"address":{"keyword":"ImageType"},"value":{"kind":"strings","values":["ORIGINAL","PRIMARY"]}},
            {"address":{"keyword":"PatientID"},"value":{"kind":"empty"}},
            {"address":{"tag":"7776,0010"},"vr":"UL","value":{"kind":"integers","values":[1,4294967295]}},
            {"address":{"private_group":"0011","private_creator":"DTS_RELEASE","private_offset":"10"},"vr":"OB","value":{"kind":"bytes","base64":"AAECAw=="}},
            {"address":{"keyword":"ReferencedImageSequence"},"value":{"kind":"sequence","items":[{"elements":[{"address":{"keyword":"ReferencedSOPClassUID"},"value":{"kind":"string","value":"1.2.840.10008.5.1.4.1.1.7"}}]}]}},
        ],
        "references":[{"relationship":"derived_from","target_instance_id":"target","target_role":"sop"}],
    },
    {"instance_id":"target","sop_class_uid":"1.2.840.10008.5.1.4.1.1.7","identity":{"study_instance_uid":"1.2.3.10","series_instance_uid":"1.2.3.11","sop_instance_uid":"1.2.3.12","frame_of_reference_uid":"1.2.3.13"},"elements":[]},
]
for index, item in enumerate(bulk):
    instances.append({"instance_id":f"bulk-{index}","sop_class_uid":f"1.2.3.{index + 20}","elements":[],"bulk":[item]})

with tempfile.TemporaryDirectory(prefix="dts-structural-catalog-") as temporary:
    work = pathlib.Path(temporary)
    capabilities = invoke(["capabilities","--format","json"], work)["result"]["structural_assembly"]
    assert capabilities["availability"] == "available"
    covered = {"standard_elements","unknown_explicit_vr_elements","managed_private_elements","recursive_sequences"} | {"general_bulk" if item["kind"] == "general" else item["kind"] for item in bulk}
    assert covered == set(capabilities["supported_content_kinds"])
    request = work / "request.json"
    request.write_text(json.dumps({"assembly_request_schema_version":"1.0.0","instances":instances}, sort_keys=True))
    manifests = []
    roots = []
    outcomes = []
    for label, parallelism in [("serial",1),("parallel",8)]:
        cwd = work / f"cwd-{label}" / "nested"
        cwd.mkdir(parents=True)
        root = work / f"output-{label}"
        outcome = invoke(["assemble","--request",str(request),"--out",str(root),"--seed","93","--parallelism",str(parallelism),"--format","json"], cwd)
        invoke(["validate",str(root),"--format","json"], cwd)
        report = invoke(["report",str(root),"--format","json","--cli-api","1.0.0"], cwd)
        assert report["result"]["report_kind"] == "structural_assembly"
        assert report["result"]["report"]["iod_conformance"] == "not_assessed"
        outcome["result"].pop("requested_output_root")
        outcome["result"].pop("manifest_path")
        outcomes.append(outcome)
        roots.append(root)
        manifests.append(json.loads((root / "manifest.json").read_text()))
    assert outcomes[0] == outcomes[1]
    assert manifests[0]["run"]["corpus_plan_sha256"] == manifests[1]["run"]["corpus_plan_sha256"]
    assert manifests[0]["instances"] == manifests[1]["instances"]
    assert manifests[0]["run"]["iod_conformance"] == "not_assessed"
    serialized = json.dumps(manifests[0])
    for forbidden in ["template_id","profile_membership","case_id"]:
        assert forbidden not in serialized
    for entry in manifests[0]["instances"]:
        relative = entry["output_path"]
        assert (roots[0] / relative).read_bytes() == (roots[1] / relative).read_bytes()

print("installed structural catalog consumer passed")
