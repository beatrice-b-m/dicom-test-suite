#!/usr/bin/env python3
"""Qualify installed composition with external raw files and typed attributes."""

import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile


BINARY = pathlib.Path(sys.argv[1]).resolve()


def invoke(arguments, cwd):
    result = subprocess.run([str(BINARY), *arguments], cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    assert result.returncode == 0, (arguments, result.returncode, result.stderr)
    assert result.stderr == "", (arguments, result.stderr)
    return json.loads(result.stdout)


with tempfile.TemporaryDirectory(prefix="dts-caller-content-") as temporary:
    work = pathlib.Path(temporary)
    assets = work / "assets"
    assets.mkdir()
    mono = bytes([0, 64, 128, 255])
    rgb = bytes(range(12))
    (assets / "mono.raw").write_bytes(mono)
    (assets / "rgb.raw").write_bytes(rgb)
    pixel_base = {"rows":2,"columns":2,"frames":1,"sample_type":"uint","bits_allocated":8,"bits_stored":8,"high_bit":7,"byte_order":"little"}
    spec = {
        "composition_spec_schema_version":"0.1.0",
        "instances":[
            {
                "instance_id":"mono", "template":{"id":"classic/secondary-capture/monochrome"},
                "attributes":[
                    {"address":{"keyword":"PatientName"},"operation":"set","vr":"PN","value":{"kind":"string","value":"SYNTHETIC^CALLER"}},
                    {"address":{"keyword":"ImageType"},"operation":"set","vr":"CS","value":{"kind":"multi","values":[{"kind":"string","value":"DERIVED"},{"kind":"string","value":"SECONDARY"}]}},
                    {"address":{"keyword":"PatientBirthDate"},"operation":"empty"},
                    {"address":{"tag":"0011,1010","private_creator":"DTS_CALLER"},"operation":"set","vr":"OB","value":{"kind":"binary","base64":"AAECAw=="}},
                    {"address":{"keyword":"ReferencedSeriesSequence"},"operation":"set","vr":"SQ","value":{"kind":"sequence","items":[{"attributes":[{"address":{"keyword":"SeriesInstanceUID"},"operation":"set","vr":"UI","value":{"kind":"string","value":"2.25.987654321"}}]}]}},
                ],
                "content":[{"slot":"pixels","source":{"kind":"local_file","path":"assets/mono.raw","sha256":hashlib.sha256(mono).hexdigest(),"pixel":pixel_base | {"samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2"}}}],
            },
            {
                "instance_id":"rgb", "template":{"id":"classic/secondary-capture/rgb"},
                "content":[{"slot":"pixels","source":{"kind":"local_file","path":"assets/rgb.raw","sha256":hashlib.sha256(rgb).hexdigest(),"pixel":pixel_base | {"samples_per_pixel":3,"photometric_interpretation":"RGB","planar_configuration":0}}}],
            },
        ],
    }
    spec_path = work / "request.json"
    spec_path.write_text(json.dumps(spec, sort_keys=True))
    output_root = work / "output"
    invoke(["compose","--spec",str(spec_path),"--out",str(output_root),"--seed","94","--format","json"], work)
    invoke(["validate",str(output_root),"--format","json"], work)
    invoke(["report",str(output_root),"--format","json","--cli-api","1.0.0"], work)
    manifest = json.loads((output_root / "manifest.json").read_text())
    entries = {entry["instance_id"]: entry for entry in manifest["composition"]["entries"]}
    assert entries["mono"]["content"][0]["sha256"] == hashlib.sha256(mono).hexdigest()
    assert entries["rgb"]["content"][0]["sha256"] == hashlib.sha256(rgb).hexdigest()
    assert entries["mono"]["content"][0]["properties"]["spec_relative_path"] == "assets/mono.raw"
    assert entries["rgb"]["content"][0]["properties"]["spec_relative_path"] == "assets/rgb.raw"
    origins = {item["tag"]: item["origin"] for item in entries["mono"]["value_provenance"]}
    for tag in ["0010,0010","0008,0008","0010,0030","0011,1010","0008,1115"]:
        assert origins[tag] == "instance_override", (tag, origins.get(tag))
    asset_hashes = {asset["sha256"] for asset in manifest["composition"]["assets"]}
    assert {hashlib.sha256(mono).hexdigest(), hashlib.sha256(rgb).hexdigest()} <= asset_hashes

print("installed caller content consumer passed")
