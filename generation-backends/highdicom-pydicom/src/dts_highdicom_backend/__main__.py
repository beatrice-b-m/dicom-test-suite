"""Command-line entrypoint for the protocol backend."""

from __future__ import annotations

import sys
import json
from pathlib import Path
from typing import Any

from . import __version__
from .protocol import base_response, read_request, runtime_identity, write_response


FLOAT32_CASE_ID = "derived/parametric-map/float32_ct_derived_explicit_le"
FLOAT64_CASE_ID = "derived/parametric-map/float64_ct_derived_explicit_le"
SCOORD3D_CASE_ID = "derived/sr/comprehensive3d_scoord3d"
TID1500_CASE_ID = "derived/sr/tid1500_ct_measurement_report"
WSI_TILE_SEGMENTATION_CASE_ID = "derived/seg/wsi_tile_reference"


def generate_parametric_map(request: dict[str, Any], output_root: Path) -> dict[str, Any]:
    from .parametric_map import generate

    return generate(request, output_root)


def generate_scoord3d(request: dict[str, Any], output_root: Path) -> dict[str, Any]:
    from .scoord3d import generate

    return generate(request, output_root)


def generate_tid1500(request: dict[str, Any], output_root: Path) -> dict[str, Any]:
    from .tid1500 import generate

    return generate(request, output_root)


def generate_wsi_tile_segmentation(
    request: dict[str, Any], output_root: Path
) -> dict[str, Any]:
    from .wsi_tile_segmentation import generate

    return generate(request, output_root)


def _generate(request: dict[str, Any], output_root: Path) -> dict[str, Any]:
    case_id = request["case"]["case_id"]
    if case_id in {FLOAT32_CASE_ID, FLOAT64_CASE_ID}:
        return generate_parametric_map(request, output_root)
    if case_id == TID1500_CASE_ID:
        return generate_tid1500(request, output_root)
    if case_id == SCOORD3D_CASE_ID:
        return generate_scoord3d(request, output_root)
    if case_id == WSI_TILE_SEGMENTATION_CASE_ID:
        return generate_wsi_tile_segmentation(request, output_root)
    raise ValueError(f"unsupported case {case_id}")


def _failure(request: dict[str, Any], message: str) -> dict[str, Any]:
    response = base_response(request)
    response.update(
        {
            "status": "failed",
            "outputs": [],
            "failure": {
                "code": "generation_failed",
                "message": message,
                "retryable": False,
            },
        }
    )
    return response


def main() -> int:
    if sys.argv[1:] == ["--version"]:
        print(f"dts-highdicom-backend {__version__}")
        return 0
    if sys.argv[1:] == ["--runtime-identity"]:
        print(json.dumps(runtime_identity(), sort_keys=True, separators=(",", ":")))
        return 0
    if sys.argv[1:]:
        print("backend accepts only --version", file=sys.stderr)
        return 2

    request, response_path, output_root = read_request()
    try:
        output = _generate(request, output_root)
        response = base_response(request)
        response.update(
            {
                "status": "generated",
                "outputs": [output],
                "failure": None,
            }
        )
    except Exception as error:  # convert controlled generation failures to protocol output
        response = _failure(request, f"{type(error).__name__}: {error}")
    write_response(response_path, response)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
