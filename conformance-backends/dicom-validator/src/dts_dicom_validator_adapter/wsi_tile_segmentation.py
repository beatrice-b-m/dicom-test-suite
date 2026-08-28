from __future__ import annotations

import argparse
import importlib.metadata
import json
import os
import sys
from pathlib import Path

import pydicom
from dicom_validator.spec_reader.edition_reader import EditionReader
from dicom_validator.validator.dicom_file_validator import DicomFileValidator
from dicom_validator.validator.validation_result import Status

from .__main__ import (
    EDITION,
    EXPECTED_DISTRIBUTIONS,
    QuietResultHandler,
    correct_locked_definition,
    enum_value,
    stable_context,
    verify_distribution,
    verify_standard,
)


ADAPTER_VERSION = "0.2.0"
SEGMENTATION_STORAGE_UIDS = (
    "1.2.840.10008.5.1.4.1.1.66.4",
    "1.2.840.10008.5.1.4.1.1.66.7",
)
CORRECTION_ID = "segmentation-a51-2-functional-groups-v1"
EXACT_CASE_REQUIREMENT_ID = "derived-seg-wsi-tile-reference-functional-groups-v1"
LOCKED_MODULES = {
    "Multi-frame Functional Groups": {"ref": "C.7.6.16", "use": "M"},
    "Segmentation Image": {"ref": "C.8.20.2", "use": "M"},
    "Segmentation Series": {"ref": "C.8.20.1", "use": "M"},
}
SEGMENTATION_GROUP_MACROS = {
    "Pixel Measures": {"ref": "C.7.6.16.2.1", "use": "U"},
    "Plane Position (Patient)": {"ref": "C.7.6.16.2.3", "use": "U"},
    "Plane Orientation (Patient)": {"ref": "C.7.6.16.2.4", "use": "U"},
    "Plane Position (Slide)": {"ref": "C.8.12.6.1", "use": "U"},
    "Derivation Image": {"ref": "C.7.6.16.2.6", "use": "U"},
    "Frame Content": {
        "cond": {"type": "UF"},
        "ref": "C.7.6.16.2.2",
        "use": "U - Shall not be used as a Shared Functional Group.",
    },
    "Segmentation": {
        "cond": {
            "and": [
                {
                    "index": 0,
                    "op": "!=",
                    "tag": "(0020,9311)",
                    "values": ["TILED_FULL"],
                },
                {
                    "index": 0,
                    "op": "!=",
                    "tag": "(0062,0001)",
                    "values": ["LABELMAP"],
                },
            ],
            "type": "MU",
        },
        "ref": "C.8.20.3.1",
        "use": "C - Required if Dimension Organization Type is not TILED_FULL and Segmentation Type is not LABELMAP.",
    },
}
SEGMENTATION_MACRO_MODULE = {
    "(0062,000A)": {
        "items": {
            "(0062,000B)": {
                "name": "Referenced Segment Number",
                "type": "1",
            }
        },
        "name": "Segment Identification Sequence",
        "type": "1",
    }
}


def correct_segmentation_group_macros(dicom_info: object) -> None:
    """Restore PS3.3 Table A.51-2 after verifying the locked parser omission."""
    iods = getattr(dicom_info, "iods", None)
    modules = getattr(dicom_info, "modules", None)
    if not isinstance(iods, dict) or not isinstance(modules, dict):
        raise RuntimeError(f"definition correction {CORRECTION_ID} root shape mismatch")
    for name, macro in SEGMENTATION_GROUP_MACROS.items():
        reference = macro["ref"]
        if name == "Segmentation":
            if reference in modules:
                raise RuntimeError(
                    f"definition correction {CORRECTION_ID} no longer matches missing module"
                )
            continue
        if reference not in modules:
            raise RuntimeError(
                f"definition correction {CORRECTION_ID} missing module {reference}"
            )
    for sop_class_uid in SEGMENTATION_STORAGE_UIDS:
        iod = iods.get(sop_class_uid)
        if not isinstance(iod, dict) or iod.get("title") != "Segmentation IOD":
            raise RuntimeError(
                f"definition correction {CORRECTION_ID} IOD shape mismatch"
            )
        if iod.get("group_macros") != {}:
            raise RuntimeError(
                f"definition correction {CORRECTION_ID} no longer matches empty macros"
            )
        iod_modules = iod.get("modules")
        if not isinstance(iod_modules, dict) or any(
            iod_modules.get(name) != expected
            for name, expected in LOCKED_MODULES.items()
        ):
            raise RuntimeError(
                f"definition correction {CORRECTION_ID} module shape mismatch"
            )
        iod["group_macros"] = json.loads(json.dumps(SEGMENTATION_GROUP_MACROS))
    modules["C.8.20.3.1"] = json.loads(json.dumps(SEGMENTATION_MACRO_MODULE))


def require_single_item_sequence(container: object, keyword: str, location: str) -> None:
    sequence = getattr(container, keyword, None)
    if sequence is None or len(sequence) != 1:
        raise RuntimeError(
            f"exact-case requirement {EXACT_CASE_REQUIREMENT_ID} requires "
            f"one-item {keyword} at {location}"
        )


def verify_exact_case_functional_groups(input_path: Path) -> None:
    """Require the locked M6 macro placement before general IOD validation."""
    dataset = pydicom.dcmread(input_path)
    if str(getattr(dataset, "SOPClassUID", "")) != SEGMENTATION_STORAGE_UIDS[0]:
        raise RuntimeError(
            f"exact-case requirement {EXACT_CASE_REQUIREMENT_ID} requires Segmentation Storage"
        )
    if str(getattr(dataset, "SegmentationType", "")) != "FRACTIONAL":
        raise RuntimeError(
            f"exact-case requirement {EXACT_CASE_REQUIREMENT_ID} requires FRACTIONAL segmentation"
        )
    if str(getattr(dataset, "DimensionOrganizationType", "")) != "TILED_SPARSE":
        raise RuntimeError(
            f"exact-case requirement {EXACT_CASE_REQUIREMENT_ID} requires TILED_SPARSE organization"
        )
    require_single_item_sequence(dataset, "SharedFunctionalGroupsSequence", "dataset")
    shared = dataset.SharedFunctionalGroupsSequence[0]
    require_single_item_sequence(shared, "PixelMeasuresSequence", "shared functional groups")
    require_single_item_sequence(
        shared, "SegmentIdentificationSequence", "shared functional groups"
    )
    per_frame = getattr(dataset, "PerFrameFunctionalGroupsSequence", None)
    number_of_frames = int(getattr(dataset, "NumberOfFrames", 0))
    if per_frame is None or number_of_frames <= 0 or len(per_frame) != number_of_frames:
        raise RuntimeError(
            f"exact-case requirement {EXACT_CASE_REQUIREMENT_ID} requires one per-frame item per frame"
        )
    for index, frame in enumerate(per_frame, start=1):
        location = f"per-frame functional groups item {index}"
        require_single_item_sequence(frame, "FrameContentSequence", location)
        require_single_item_sequence(frame, "PlanePositionSlideSequence", location)
        require_single_item_sequence(frame, "DerivationImageSequence", location)


def validate(input_path: Path, standard_root: Path, lock_path: Path) -> int:
    for name, version in EXPECTED_DISTRIBUTIONS.items():
        verify_distribution(name, version)
    verify_standard(standard_root, lock_path)
    verify_exact_case_functional_groups(input_path)
    dicom_info = EditionReader(standard_root).load_dicom_info(EDITION)
    correct_locked_definition(dicom_info)
    correct_segmentation_group_macros(dicom_info)
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
            f"edition={EDITION} correction={CORRECTION_ID} "
            f"sop_class_uid={result.sop_class_uid} "
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
            f"dts-dicom-validator-wsi-tile-segmentation {ADAPTER_VERSION} "
            f"edition={EDITION} definition_correction={CORRECTION_ID} "
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
