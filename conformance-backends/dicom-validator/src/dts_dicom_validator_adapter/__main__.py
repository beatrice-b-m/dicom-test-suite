from __future__ import annotations

import argparse
import base64
import csv
import hashlib
import importlib.metadata
import json
import os
import sys
from pathlib import Path

from dicom_validator.spec_reader.edition_reader import EditionReader
from dicom_validator.validator.dicom_file_validator import DicomFileValidator
from dicom_validator.validator.error_handler import ValidationResultHandlerBase
from dicom_validator.validator.validation_result import Status


ADAPTER_VERSION = "0.1.0"
EDITION = "2026b"
EXPECTED_DISTRIBUTIONS = {
    "dicom-validator": "0.8.2",
    "lxml": "6.1.2",
    "pydicom": "3.0.2",
    "pyparsing": "3.3.2",
}


class QuietResultHandler(ValidationResultHandlerBase):
    """Retain structured results without dicom-validator's console renderer."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_distribution(name: str, expected_version: str) -> None:
    distribution = importlib.metadata.distribution(name)
    if distribution.version != expected_version:
        raise RuntimeError(
            f"distribution {name} version {distribution.version} != {expected_version}"
        )
    record_text = distribution.read_text("RECORD")
    if record_text is None:
        raise RuntimeError(f"distribution {name} has no wheel RECORD")
    for relative, encoded_hash, encoded_size in csv.reader(record_text.splitlines()):
        if not encoded_hash:
            continue
        algorithm, expected_hash = encoded_hash.split("=", 1)
        if algorithm != "sha256":
            raise RuntimeError(f"distribution {name} uses unsupported RECORD hash {algorithm}")
        path = Path(distribution.locate_file(relative))
        if not path.is_file():
            raise RuntimeError(f"distribution {name} file is missing: {relative}")
        if encoded_size and path.stat().st_size != int(encoded_size):
            raise RuntimeError(f"distribution {name} file size mismatch: {relative}")
        actual = base64.urlsafe_b64encode(hashlib.sha256(path.read_bytes()).digest())
        actual_hash = actual.rstrip(b"=").decode("ascii")
        if actual_hash != expected_hash:
            raise RuntimeError(f"distribution {name} file hash mismatch: {relative}")


def verify_standard(standard_root: Path, lock_path: Path) -> None:
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    if lock.get("edition") != EDITION:
        raise RuntimeError("standard lock edition mismatch")
    for artifact in lock.get("artifacts", []):
        path = standard_root / artifact["path"]
        if not path.is_file():
            raise RuntimeError(f"locked standard artifact is missing: {artifact['path']}")
        if path.stat().st_size != artifact["size_bytes"]:
            raise RuntimeError(f"locked standard artifact size mismatch: {artifact['path']}")
        if sha256_file(path) != artifact["sha256"]:
            raise RuntimeError(f"locked standard artifact hash mismatch: {artifact['path']}")


def stable_context(context: dict | None) -> str:
    return json.dumps(context or {}, sort_keys=True, separators=(",", ":"), default=str)


def enum_value(value: object) -> str:
    return str(getattr(value, "value", value))


def validate(input_path: Path, standard_root: Path, lock_path: Path) -> int:
    for name, version in EXPECTED_DISTRIBUTIONS.items():
        verify_distribution(name, version)
    verify_standard(standard_root, lock_path)
    dicom_info = EditionReader(standard_root).load_dicom_info(EDITION)
    validator = DicomFileValidator(
        dicom_info,
        suppress_vr_warnings=False,
        error_handler=QuietResultHandler(),
    )
    results = validator.validate(input_path)
    exit_code = 0
    for file_path in sorted(results):
        result = results[file_path]
        if result.status not in (Status.Passed, Status.Failed):
            print(
                "Error: dicom-validator "
                f"status={result.status.name} file={Path(file_path).name}"
            )
            exit_code = max(exit_code, 2)
            continue
        for module_name in sorted(result.module_errors or {}):
            errors = result.module_errors[module_name]
            for dicom_tag, error in sorted(errors.items(), key=lambda item: item[0]):
                parents = "/".join(str(parent) for parent in dicom_tag.parents or [])
                print(
                    "Error: dicom-validator "
                    f"code={error.code.name} module={json.dumps(module_name)} "
                    f"tag={dicom_tag.tag} parents={parents or '-'} "
                    f"type={enum_value(error.type)} scope={error.scope.name} "
                    f"context={stable_context(error.context)}"
                )
        print(
            "Info - dicom-validator "
            f"edition={EDITION} sop_class_uid={result.sop_class_uid} "
            f"status={result.status.name} errors={result.errors}"
        )
        exit_code = max(exit_code, min(result.errors, 125))
    return exit_code


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    result.add_argument("--version", action="store_true")
    result.add_argument("--standard-path", type=Path)
    result.add_argument("--lock-path", type=Path)
    result.add_argument("input", nargs="?", type=Path)
    return result


def main() -> None:
    args = parser().parse_args()
    if args.version:
        versions = " ".join(
            f"{name}={importlib.metadata.version(name)}"
            for name in sorted(EXPECTED_DISTRIBUTIONS)
        )
        print(
            f"dts-dicom-validator {ADAPTER_VERSION} edition={EDITION} "
            f"python={sys.version.split()[0]} {versions}"
        )
        return
    if args.input is None:
        raise SystemExit("input DICOM path is required")
    if args.lock_path is None:
        raise SystemExit("--lock-path is required")
    standard_path = args.standard_path
    if standard_path is None:
        configured = os.environ.get("DTS_DICOM_VALIDATOR_STANDARD_HOME")
        if not configured:
            raise SystemExit("DTS_DICOM_VALIDATOR_STANDARD_HOME is required")
        standard_path = Path(configured)
    try:
        raise SystemExit(validate(args.input, standard_path, args.lock_path))
    except Exception as error:
        print(f"Error: dicom-validator adapter failure: {error}", file=sys.stderr)
        raise SystemExit(126) from error


if __name__ == "__main__":
    main()
