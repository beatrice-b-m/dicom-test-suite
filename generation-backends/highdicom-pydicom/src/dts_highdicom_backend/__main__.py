"""Command-line entrypoint for the protocol backend."""

from __future__ import annotations

import sys
import json
from typing import Any

from . import __version__
from .parametric_map import generate
from .protocol import base_response, read_request, runtime_identity, write_response


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
        output = generate(request, output_root)
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
