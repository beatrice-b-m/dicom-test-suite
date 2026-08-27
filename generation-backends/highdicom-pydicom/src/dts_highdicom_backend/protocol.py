"""Protocol 0.1.0 request and response handling."""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

import highdicom
import pydicom

from . import __version__

PROTOCOL_VERSION = "0.1.0"
BACKEND_ID = "highdicom_pydicom"
BACKEND_NAME = "dicom-test-suite-highdicom-backend"


class ProtocolError(ValueError):
    """A controlled request or protocol failure."""


def require_environment_path(name: str) -> Path:
    value = os.environ.get(name)
    if not value:
        raise ProtocolError(f"required environment variable {name} is missing")
    return Path(value)


def read_request() -> tuple[dict[str, Any], Path, Path]:
    request_path = require_environment_path("DTS_BACKEND_REQUEST")
    response_path = require_environment_path("DTS_BACKEND_RESPONSE")
    output_root = require_environment_path("DTS_BACKEND_OUTPUTS")
    with request_path.open("r", encoding="utf-8") as stream:
        request = json.load(stream)
    if request.get("protocol_version") != PROTOCOL_VERSION:
        raise ProtocolError("unsupported protocol version")
    if request.get("backend_id") != BACKEND_ID:
        raise ProtocolError("request backend_id does not match this backend")
    return request, response_path, output_root


def backend_provenance() -> dict[str, str]:
    required = {
        "dependency_lock_sha256": "DTS_BACKEND_DEPENDENCY_LOCK_SHA256",
        "executable_fingerprint": "DTS_BACKEND_EXECUTABLE_FINGERPRINT",
        "environment_fingerprint": "DTS_BACKEND_ENVIRONMENT_FINGERPRINT",
    }
    values: dict[str, str] = {
        "name": BACKEND_NAME,
        "version": (
            f"{__version__}+highdicom.{highdicom.__version__}"
            f".pydicom.{pydicom.__version__}"
        ),
    }
    for field, environment_name in required.items():
        value = os.environ.get(environment_name)
        if not value:
            raise ProtocolError(
                f"required environment variable {environment_name} is missing"
            )
        values[field] = value
    return values


def base_response(request: dict[str, Any]) -> dict[str, Any]:
    return {
        "response_schema_version": "0.1.0",
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request["request_id"],
        "backend_id": BACKEND_ID,
        "backend": backend_provenance(),
        "warnings": [],
    }


def write_response(path: Path, response: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(response, stream, indent=2, sort_keys=True, ensure_ascii=True)
        stream.write("\n")
