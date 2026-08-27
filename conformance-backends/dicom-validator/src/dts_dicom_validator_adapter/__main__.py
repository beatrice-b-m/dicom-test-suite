from __future__ import annotations

import argparse
import base64
import copy
import csv
import hashlib
import importlib.metadata
import json
import os
import struct
import sys
from pathlib import Path

import pydicom
from dicom_validator.spec_reader.edition_reader import EditionReader
from dicom_validator.validator.dicom_file_validator import DicomFileValidator
from dicom_validator.validator.error_handler import ValidationResultHandlerBase
from dicom_validator.validator.validation_result import Status


ADAPTER_VERSION = "0.7.0"
EDITION = "2026b"
DEFINITION_CORRECTION_ID = "rt-2026b-condition-compat-v2"
RT_CONTROL_POINT_MODULE = "C.36.2.2.5-1"
RT_DELIVERY_CONTROL_POINT_MODULE = "C.36.2.2.6-1"
C_ARM_PHOTON_ELECTRON_BEAM_MODULE = "C.36.15"
C_ARM_CONTROL_POINT_SEQUENCE = "(300A,062F)"
RECORDED_RT_CONTROL_POINT_DATETIME = "(300A,073A)"
LOCKED_MALFORMED_RECORDED_DATETIME = {
    "cond": {
        "index": 0,
        "op": "=",
        "tag": "(300A,0639)",
        "type": "MC",
        "values": ["YES"],
    },
    "name": "Recorded RT Control Point DateTime",
    "type": "1C",
}
RT_RECORD_FLAG_YES_ALTERNATIVE = {
    "index": 0,
    "op": "=",
    "tag": "(300A,0639)",
    "type": "MU",
    "values": ["YES"],
}
RT_DELIVERY_DEVICE_COMMON_MODULE = "C.36.12"
RT_TREATMENT_DEVICE_IDENTIFICATION_MODULE = "C.36.2.2.1-1"
DEVICE_COMPONENT_IDENTIFICATION_MODULE = "10.36-1"
TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE = "(300A,063A)"
DEVICE_ALTERNATE_IDENTIFIER_TYPE = "(3010,001C)"
DEVICE_ALTERNATE_IDENTIFIER_FORMAT = "(3010,001D)"
LOCKED_DEVICE_ALTERNATE_IDENTIFIER_CONDITION = {
    "index": 0,
    "op": "++",
    "tag": "(3010,001B)",
    "type": "MU",
}
CORRECTED_DEVICE_ALTERNATE_IDENTIFIER_CONDITION = {
    "index": 0,
    "op": "!=",
    "tag": "(3010,001B)",
    "type": "MU",
    "values": [""],
}
LOCKED_DEVICE_CONDITIONAL_ATTRIBUTES = {
    DEVICE_ALTERNATE_IDENTIFIER_TYPE: {
        "cond": LOCKED_DEVICE_ALTERNATE_IDENTIFIER_CONDITION,
        "name": "Device Alternate Identifier Type",
        "type": "1C",
    },
    DEVICE_ALTERNATE_IDENTIFIER_FORMAT: {
        "cond": LOCKED_DEVICE_ALTERNATE_IDENTIFIER_CONDITION,
        "name": "Device Alternate Identifier Format",
        "type": "1C",
    },
}
TWELVE_LEAD_ECG_STORAGE = "1.2.840.10008.5.1.4.1.1.9.1.1"
GENERAL_ECG_STORAGE = "1.2.840.10008.5.1.4.1.1.9.1.2"
WAVEFORM_PAYLOAD_SHA256 = (
    "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713"
)
WAVEFORM_CHANNELS = (
    ("I", "2:1", "Lead I", "7b4aee068e05c2bdff3896937c78a4c7a32f9ed2bde64d91b1d925913bf29476"),
    ("II", "2:2", "Lead II", "bd775dc70f76ea153a25832ad622b0cc26fbe6a37cf3ec6548a30965c4d17fba"),
    ("III", "2:61", "Lead III", "19d26b694df281209aa1296abbfa8f7d360e24a03a091422aba6f67663e2f3b1"),
    (
        "aVR",
        "2:62",
        "aVR, augmented voltage, right",
        "bb4c99d7857dbfcee5ee620bcff09b7060b61c5f2432427affc6139cb8d3cf9b",
    ),
    (
        "aVL",
        "2:63",
        "aVL, augmented voltage, left",
        "230f52ed2ac57624a9a35214d7867711008dd56014f4176ce258623e5b596d3a",
    ),
    (
        "aVF",
        "2:64",
        "aVF, augmented voltage, foot",
        "60e167db3c081ba5bca957aba820afb519b790d048b660634d49566df88105f2",
    ),
    ("V1", "2:3", "Lead V1", "cf8c73bebf746b799b1fe8aa2c908ca69bc7acc72311c64cbf4131fc8976609f"),
    ("V2", "2:4", "Lead V2", "0f11e5fb5105dac699fa4bcfc01c79fbe696a81db04606f39a719de57b4c7c30"),
    ("V3", "2:5", "Lead V3", "a41d5962abceb6dbe25f8421091ce3df6a69202c45b24ab6b0736159d15e253b"),
    ("V4", "2:6", "Lead V4", "d655e2cbb23d70e229ed52fedba9c45573e22729fed0a794ab690df8d7f33804"),
    ("V5", "2:7", "Lead V5", "005c539f9f4256a86d9e0a212b3bfe73741f99942b0677fb483c0c48db9583cd"),
    ("V6", "2:8", "Lead V6", "f448df95acb226c5c992363e27707a42efc3ffb974ebeff38e2a81522b57d82c"),
)
GENERAL_STANDARD_CHANNELS = tuple(
    (*channel[:3], channel_hash)
    for channel, channel_hash in zip(
        WAVEFORM_CHANNELS,
        (
            "3211bada5580e8bd9c5a2934deb231122706b00aa92f8cdc78480c03b2352197",
            "8f66471e35940851acdd9ea55b422c738bf50ea7971822deed0edca1980e1ea2",
            "9652eb91f4f73f2654c922048a1a8c8731a08062eecd6f5b373256831d0e82b0",
            "97fb26e75907437a705e4e28eb6492d51020570a23265bdf765aca3c4e7b2708",
            "c9776b85b3bda6adef798d33d3c7c95d64a1a7d5bf525866ccf7b0cf5fc3209e",
            "95871f48d729a001eeb1543b36a27059916df360e04838fd322d006661bafb44",
            "04513ee1f1d5803f3f53093f016a606a7fa874c5af8d2651749b909b93392366",
            "c12790f5b1f233662a0a1c3f266cd2abb15af5a75b39258ff961e9b4afaf7913",
            "750913ccad5eb7ec8d8199451e6eb9aa41357eb21d2a0dac6ba75dce4e5708bd",
            "218d5f967ef253722359fee1846485331c63de9330af1f9fad183d779a196cca",
            "9027ec7a0fc7fea3d8236a16a5aa6f265ff20e18a2575f99e61807e102fb3d81",
            "9280ad35672b82a7847d3ccabadd4d85a94be3d39d0a836191384571f0a23ab6",
        ),
        strict=True,
    )
)
GENERAL_AUXILIARY_CHANNELS = (
    (
        "A1",
        "2:75",
        "Auxiliary unipolar lead 1",
        "5da46776ad84a78eb0c16066cb8ac7d5e05ca6ad87170264b227c71261def284",
    ),
    (
        "A2",
        "2:76",
        "Auxiliary unipolar lead 2",
        "7bd73425422f4e79504b55932040e481ccdfafecabe1dba613ee36074a51b9e3",
    ),
    (
        "A3",
        "2:77",
        "Auxiliary unipolar lead 3",
        "e56dad9647dfa50a10b40d244e29eaedbf23d97a558901f46fbccc07ad1a1766",
    ),
    (
        "A4",
        "2:78",
        "Auxiliary unipolar lead 4",
        "e1b68207c92fe2cc4c6765fc097668f2600eeda152eb5a1d6f0444f4c9e36fbc",
    ),
)
GENERAL_GROUP_PAYLOAD_SHA256 = (
    "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb",
    "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe",
)
GENERAL_AGGREGATE_PAYLOAD_SHA256 = (
    "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea"
)
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


def correct_locked_definition(dicom_info: object) -> None:
    """Repair verified 2026b condition incompatibilities, failing closed on drift."""
    modules = getattr(dicom_info, "modules", None)
    if not isinstance(modules, dict):
        raise RuntimeError(
            f"definition correction {DEFINITION_CORRECTION_ID} module shape mismatch"
        )
    beam_module = modules.get(C_ARM_PHOTON_ELECTRON_BEAM_MODULE)
    control_point = (
        beam_module.get(C_ARM_CONTROL_POINT_SEQUENCE)
        if isinstance(beam_module, dict)
        else None
    )
    control_point_items = (
        control_point.get("items") if isinstance(control_point, dict) else None
    )
    if (
        not isinstance(control_point_items, dict)
        or control_point.get("name")
        != "C-Arm Photon-Electron Control Point Sequence"
        or control_point.get("type") != "1"
        or control_point_items.get("include")
        != [
            {"ref": RT_DELIVERY_CONTROL_POINT_MODULE},
            {"ref": "C.36.2.2.9-1"},
            {"ref": "C.36.2.2.11-1"},
        ]
    ):
        raise RuntimeError(
            f"definition correction {DEFINITION_CORRECTION_ID} path shape mismatch"
        )
    delivery_module = modules.get(RT_DELIVERY_CONTROL_POINT_MODULE)
    if not isinstance(delivery_module, dict) or delivery_module.get("include") != [
        {"ref": RT_CONTROL_POINT_MODULE}
    ]:
        raise RuntimeError(
            f"definition correction {DEFINITION_CORRECTION_ID} path shape mismatch"
        )
    module = modules.get(RT_CONTROL_POINT_MODULE)
    if not isinstance(module, dict):
        raise RuntimeError(
            f"definition correction {DEFINITION_CORRECTION_ID} module shape mismatch"
        )
    attribute = module.get(RECORDED_RT_CONTROL_POINT_DATETIME)
    if attribute != LOCKED_MALFORMED_RECORDED_DATETIME:
        raise RuntimeError(
            f"definition correction {DEFINITION_CORRECTION_ID} attribute shape mismatch"
        )
    corrected = copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
    corrected["cond"]["other_cond"] = copy.deepcopy(RT_RECORD_FLAG_YES_ALTERNATIVE)
    module[RECORDED_RT_CONTROL_POINT_DATETIME] = corrected

    delivery_device_module = modules.get(RT_DELIVERY_DEVICE_COMMON_MODULE)
    treatment_device_sequence = (
        delivery_device_module.get(TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE)
        if isinstance(delivery_device_module, dict)
        else None
    )
    if treatment_device_sequence != {
        "items": {"include": [{"ref": RT_TREATMENT_DEVICE_IDENTIFICATION_MODULE}]},
        "name": "Treatment Device Identification Sequence",
        "type": "1",
    }:
        raise RuntimeError(
            f"definition correction {DEFINITION_CORRECTION_ID} device path shape mismatch"
        )
    treatment_device_module = modules.get(RT_TREATMENT_DEVICE_IDENTIFICATION_MODULE)
    if not isinstance(treatment_device_module, dict) or treatment_device_module.get(
        "include"
    ) != [
        {"ref": "10.35-1"},
        {"ref": DEVICE_COMPONENT_IDENTIFICATION_MODULE},
    ]:
        raise RuntimeError(
            f"definition correction {DEFINITION_CORRECTION_ID} device path shape mismatch"
        )
    device_component_module = modules.get(DEVICE_COMPONENT_IDENTIFICATION_MODULE)
    if not isinstance(device_component_module, dict):
        raise RuntimeError(
            f"definition correction {DEFINITION_CORRECTION_ID} device module shape mismatch"
        )
    for tag, locked_attribute in LOCKED_DEVICE_CONDITIONAL_ATTRIBUTES.items():
        if device_component_module.get(tag) != locked_attribute:
            raise RuntimeError(
                f"definition correction {DEFINITION_CORRECTION_ID} "
                "device attribute shape mismatch"
            )
    for tag, locked_attribute in LOCKED_DEVICE_CONDITIONAL_ATTRIBUTES.items():
        corrected_attribute = copy.deepcopy(locked_attribute)
        corrected_attribute["cond"] = copy.deepcopy(
            CORRECTED_DEVICE_ALTERNATE_IDENTIFIER_CONDITION
        )
        device_component_module[tag] = corrected_attribute


def stable_context(context: dict | None) -> str:
    return json.dumps(context or {}, sort_keys=True, separators=(",", ":"), default=str)


def enum_value(value: object) -> str:
    return str(getattr(value, "value", value))


def validate(input_path: Path, standard_root: Path, lock_path: Path) -> int:
    for name, version in EXPECTED_DISTRIBUTIONS.items():
        verify_distribution(name, version)
    verify_standard(standard_root, lock_path)
    dicom_info = EditionReader(standard_root).load_dicom_info(EDITION)
    correct_locked_definition(dicom_info)
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


def extract_u32_pixels(input_path: Path, standard_root: Path, lock_path: Path) -> int:
    for name, version in EXPECTED_DISTRIBUTIONS.items():
        verify_distribution(name, version)
    verify_standard(standard_root, lock_path)
    dataset = pydicom.dcmread(input_path)
    transfer_syntax = str(dataset.file_meta.TransferSyntaxUID)
    if transfer_syntax != "1.2.840.10008.1.2.1":
        raise RuntimeError(f"unsupported transfer syntax for u32 extraction: {transfer_syntax}")
    rows = int(dataset.Rows)
    columns = int(dataset.Columns)
    frames = int(getattr(dataset, "NumberOfFrames", 1))
    samples = int(dataset.SamplesPerPixel)
    bits_allocated = int(dataset.BitsAllocated)
    bits_stored = int(dataset.BitsStored)
    high_bit = int(dataset.HighBit)
    pixel_representation = int(dataset.PixelRepresentation)
    pixel_element = dataset[0x7FE00010]
    pixel_bytes = bytes(pixel_element.value)
    frame_size = rows * columns * samples * 4
    if rows <= 0 or columns <= 0 or frames <= 0:
        raise RuntimeError("u32 extraction requires positive image dimensions")
    if (
        bits_allocated != 32
        or bits_stored != 32
        or high_bit != 31
        or pixel_representation != 0
        or samples != 1
        or str(dataset.PhotometricInterpretation) != "MONOCHROME2"
        or pixel_element.VR != "OW"
    ):
        raise RuntimeError("dataset does not satisfy the locked unsigned u32 pixel shape")
    if len(pixel_bytes) != frame_size * frames:
        raise RuntimeError(
            "u32 Pixel Data length does not match rows, columns, samples, and frames"
        )
    values = list(struct.unpack(f"<{len(pixel_bytes) // 4}I", pixel_bytes))
    result = {
        "adapter_id": "pydicom-dicom-validator-u32",
        "bits_allocated": bits_allocated,
        "bits_stored": bits_stored,
        "byte_order": "little_endian",
        "columns": columns,
        "frame_hashes": [
            hashlib.sha256(frame).hexdigest()
            for frame in (
                pixel_bytes[offset : offset + frame_size]
                for offset in range(0, len(pixel_bytes), frame_size)
            )
        ],
        "frames": frames,
        "high_bit": high_bit,
        "photometric_interpretation": str(dataset.PhotometricInterpretation),
        "pixel_data_sha256": hashlib.sha256(pixel_bytes).hexdigest(),
        "pixel_data_vr": pixel_element.VR,
        "pixel_representation": pixel_representation,
        "rows": rows,
        "samples_per_pixel": samples,
        "stored_values": values,
        "transfer_syntax_uid": transfer_syntax,
    }
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


def extract_nonsquare_spacing(
    input_path: Path, standard_root: Path, lock_path: Path
) -> int:
    for name, version in EXPECTED_DISTRIBUTIONS.items():
        verify_distribution(name, version)
    verify_standard(standard_root, lock_path)
    dataset = pydicom.dcmread(input_path)
    transfer_syntax = str(dataset.file_meta.TransferSyntaxUID)
    if transfer_syntax != "1.2.840.10008.1.2.1":
        raise RuntimeError(
            f"unsupported transfer syntax for non-square extraction: {transfer_syntax}"
        )

    rows = int(dataset.Rows)
    columns = int(dataset.Columns)
    frames = int(getattr(dataset, "NumberOfFrames", 1))
    samples = int(dataset.SamplesPerPixel)
    bits_allocated = int(dataset.BitsAllocated)
    bits_stored = int(dataset.BitsStored)
    high_bit = int(dataset.HighBit)
    pixel_representation = int(dataset.PixelRepresentation)
    pixel_element = dataset[0x7FE00010]
    pixel_bytes = bytes(pixel_element.value)
    if (
        rows != 4
        or columns != 6
        or frames != 1
        or samples != 1
        or bits_allocated != 8
        or bits_stored != 8
        or high_bit != 7
        or pixel_representation != 0
        or str(dataset.PhotometricInterpretation) != "MONOCHROME2"
        or pixel_element.VR != "OB"
        or len(pixel_bytes) != 24
    ):
        raise RuntimeError("dataset does not satisfy the locked non-square pixel shape")

    def semantic_element(tag: int, expected_vr: str) -> dict | None:
        if tag not in dataset:
            return None
        element = dataset[tag]
        values = list(element.value) if isinstance(element.value, pydicom.multival.MultiValue) else [element.value]
        lexical_values = [str(value) for value in values]
        if element.VR != expected_vr or element.VM != 2:
            raise RuntimeError(
                f"tag {element.tag} has VR/VM {element.VR}/{element.VM}, expected {expected_vr}/2"
            )
        return {
            "tag": f"{element.tag.group:04X},{element.tag.element:04X}",
            "vr": element.VR,
            "vm": element.VM,
            "lexical_value": "\\".join(lexical_values),
            "values": lexical_values,
        }

    pixel_spacing = semantic_element(0x00280030, "DS")
    nominal_spacing = semantic_element(0x00182010, "DS")
    pixel_aspect_ratio = semantic_element(0x00280034, "IS")
    spacing_variant = (
        pixel_spacing is not None
        and nominal_spacing is not None
        and pixel_aspect_ratio is None
        and pixel_spacing["values"] == ["0.6", "0.3"]
        and nominal_spacing["values"] == ["0.6", "0.3"]
    )
    aspect_variant = (
        pixel_spacing is None
        and nominal_spacing is None
        and pixel_aspect_ratio is not None
        and pixel_aspect_ratio["values"] == ["2", "1"]
    )
    if spacing_variant == aspect_variant:
        raise RuntimeError(
            "dataset does not contain exactly one locked non-square spatial variant"
        )

    forbidden = {
        "imager_pixel_spacing": 0x00181164,
        "pixel_spacing_calibration_type": 0x00280A02,
        "pixel_spacing_calibration_description": 0x00280A04,
        "image_position_patient": 0x00200032,
        "image_orientation_patient": 0x00200037,
        "frame_of_reference_uid": 0x00200052,
    }
    present_forbidden = sorted(name for name, tag in forbidden.items() if tag in dataset)
    if present_forbidden:
        raise RuntimeError(
            "forbidden non-square spatial attributes are present: "
            + ", ".join(present_forbidden)
        )

    frame_hash = hashlib.sha256(pixel_bytes).hexdigest()
    result = {
        "adapter_id": "pydicom-dicom-validator-u32",
        "bits_allocated": bits_allocated,
        "bits_stored": bits_stored,
        "columns": columns,
        "frame_hashes": [frame_hash],
        "frames": frames,
        "high_bit": high_bit,
        "nominal_scanned_pixel_spacing": nominal_spacing,
        "patient_space_geometry_present": False,
        "photometric_interpretation": str(dataset.PhotometricInterpretation),
        "pixel_aspect_ratio": pixel_aspect_ratio,
        "pixel_data_sha256": frame_hash,
        "pixel_data_vr": pixel_element.VR,
        "pixel_representation": pixel_representation,
        "pixel_spacing": pixel_spacing,
        "rows": rows,
        "samples_per_pixel": samples,
        "transfer_syntax_uid": transfer_syntax,
        "uncalibrated": True,
        "variant_id": "pixel_spacing" if spacing_variant else "pixel_aspect_ratio",
    }
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


def extract_waveform(input_path: Path, standard_root: Path, lock_path: Path) -> int:
    """Extract and independently verify an ordered locked ECG waveform."""
    for name, version in EXPECTED_DISTRIBUTIONS.items():
        verify_distribution(name, version)
    verify_standard(standard_root, lock_path)
    dataset = pydicom.dcmread(input_path)
    transfer_syntax = str(dataset.file_meta.TransferSyntaxUID)
    if transfer_syntax != "1.2.840.10008.1.2.1":
        raise RuntimeError(
            f"unsupported transfer syntax for waveform extraction: {transfer_syntax}"
        )
    sop_class_uid = str(dataset.SOPClassUID)
    if sop_class_uid == TWELVE_LEAD_ECG_STORAGE:
        group_specs = (
            {
                "label": "RESTING_12_LEAD",
                "channel_count": 12,
                "sample_count": 500,
                "sampling_frequency_hz": 500,
                "duration_seconds": 1,
                "channels": WAVEFORM_CHANNELS,
                "payload_sha256": WAVEFORM_PAYLOAD_SHA256,
                "sample_value_formula": (
                    "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000"
                ),
            },
        )
    elif sop_class_uid == GENERAL_ECG_STORAGE:
        group_specs = (
            {
                "label": "STD12_250HZ",
                "channel_count": 12,
                "sample_count": 1000,
                "sampling_frequency_hz": 250,
                "duration_seconds": 4,
                "channels": GENERAL_STANDARD_CHANNELS,
                "payload_sha256": GENERAL_GROUP_PAYLOAD_SHA256[0],
                "sample_value_formula": (
                    "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) "
                    "mod 2001) - 1000"
                ),
            },
            {
                "label": "AUX4_1000HZ",
                "channel_count": 4,
                "sample_count": 4000,
                "sampling_frequency_hz": 1000,
                "duration_seconds": 4,
                "channels": GENERAL_AUXILIARY_CHANNELS,
                "payload_sha256": GENERAL_GROUP_PAYLOAD_SHA256[1],
                "sample_value_formula": (
                    "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) "
                    "mod 2001) - 1000"
                ),
            },
        )
    else:
        raise RuntimeError(f"unsupported waveform SOP Class UID: {sop_class_uid}")
    if str(dataset.Modality) != "ECG":
        raise RuntimeError("ECG waveform Modality must be ECG")
    if 0x00400555 not in dataset or len(dataset.AcquisitionContextSequence) != 0:
        raise RuntimeError("Acquisition Context Sequence must be present and empty")
    groups = list(dataset.WaveformSequence)
    if len(groups) != len(group_specs):
        raise RuntimeError(
            f"locked ECG waveform requires exactly {len(group_specs)} multiplex groups"
        )
    if 0x7FE00010 in dataset:
        raise RuntimeError("Pixel Data must be absent from a waveform object")
    absent_content_tags = {
        "annotation_module": (0x0040B020,),
        "synchronization_module": (
            0x0018106A,
            0x00181800,
            0x00181801,
            0x00181802,
            0x00181803,
            0x00200200,
        ),
        "references": (0x00081140, 0x0008114A, 0x00082112),
        "image": (0x00280010, 0x00280011, 0x00280004),
        "pixel_data": (0x7FE00010,),
    }
    for name, tags in absent_content_tags.items():
        if any(tag in dataset for tag in tags):
            raise RuntimeError(f"forbidden waveform {name} content is present")

    group_results = []
    ordered_payloads = []
    for group_index, (group, spec) in enumerate(
        zip(groups, group_specs, strict=True)
    ):
        ordinal = group_index + 1
        channel_count = int(group.NumberOfWaveformChannels)
        sample_count = int(group.NumberOfWaveformSamples)
        sampling_frequency = float(group.SamplingFrequency)
        bits_allocated = int(group.WaveformBitsAllocated)
        sample_interpretation = str(group.WaveformSampleInterpretation)
        waveform_element = group[0x54001010]
        payload = bytes(waveform_element.value)
        if (
            channel_count != spec["channel_count"]
            or sample_count != spec["sample_count"]
            or sampling_frequency != float(spec["sampling_frequency_hz"])
            or str(group.WaveformOriginality) != "ORIGINAL"
            or str(group.MultiplexGroupLabel) != spec["label"]
            or bits_allocated != 16
            or sample_interpretation != "SS"
            or waveform_element.VR != "OW"
        ):
            raise RuntimeError(
                f"multiplex group {ordinal} does not satisfy the locked waveform shape"
            )
        if any(tag in group for tag in (0x00181068, 0x00181069, 0x0018106E)):
            raise RuntimeError(
                f"multiplex group {ordinal} contains forbidden timing attributes"
            )
        if 0x5400100A in group:
            raise RuntimeError(
                f"multiplex group {ordinal} Waveform Padding Value must be absent"
            )
        if len(payload) != channel_count * sample_count * 2:
            raise RuntimeError(
                f"multiplex group {ordinal} Waveform Data length does not match channels and samples"
            )

        values = struct.unpack(f"<{channel_count * sample_count}h", payload)
        if sop_class_uid == TWELVE_LEAD_ECG_STORAGE:
            expected_values = tuple(
                ((sample * (channel + 1) * 37 + channel * 101) % 2001) - 1000
                for sample in range(sample_count)
                for channel in range(channel_count)
            )
        else:
            expected_values = tuple(
                (
                    (
                        sample
                        * (channel + 1)
                        * (group_index + 1)
                        * 37
                        + channel * 101
                        + group_index * 307
                    )
                    % 2001
                )
                - 1000
                for sample in range(sample_count)
                for channel in range(channel_count)
            )
        if values != expected_values:
            raise RuntimeError(
                f"multiplex group {ordinal} Waveform Data does not match the locked sample formula"
            )
        payload_hash = hashlib.sha256(payload).hexdigest()
        if payload_hash != spec["payload_sha256"]:
            raise RuntimeError(
                f"multiplex group {ordinal} Waveform Data hash does not match the lock"
            )

        definitions = list(group.ChannelDefinitionSequence)
        if len(definitions) != channel_count:
            raise RuntimeError(
                f"multiplex group {ordinal} Channel Definition Sequence length does not match channels"
            )
        channel_results = []
        channel_hashes = []
        for channel, (definition, locked) in enumerate(
            zip(definitions, spec["channels"], strict=True)
        ):
            label, code_value, code_meaning, expected_hash = locked
            sources = list(definition.ChannelSourceSequence)
            units = list(definition.ChannelSensitivityUnitsSequence)
            if len(sources) != 1 or len(units) != 1:
                raise RuntimeError(
                    f"multiplex group {ordinal} channel {channel + 1} coding sequences must have one item"
                )
            source = sources[0]
            unit = units[0]
            if (
                int(definition.WaveformChannelNumber) != channel + 1
                or str(definition.ChannelLabel) != label
                or str(source.CodingSchemeDesignator) != "MDC"
                or str(source.CodeValue) != code_value
                or str(source.CodeMeaning) != code_meaning
                or float(definition.ChannelSensitivity) != 1.0
                or str(unit.CodingSchemeDesignator) != "UCUM"
                or str(unit.CodeValue) != "uV"
                or str(unit.CodeMeaning) != "microvolt"
                or float(definition.ChannelSensitivityCorrectionFactor) != 1.0
                or float(definition.ChannelBaseline) != 0.0
                or int(definition.WaveformBitsStored) != 16
                or float(definition.ChannelTimeSkew) != 0.0
                or 0x003A0215 in definition
            ):
                raise RuntimeError(
                    f"multiplex group {ordinal} channel {channel + 1} metadata does not match the lock"
                )
            channel_values = values[channel::channel_count]
            channel_bytes = struct.pack(f"<{sample_count}h", *channel_values)
            channel_hash = hashlib.sha256(channel_bytes).hexdigest()
            if channel_hash != expected_hash:
                raise RuntimeError(
                    f"multiplex group {ordinal} channel {channel + 1} hash does not match the lock"
                )
            channel_hashes.append(channel_hash)
            channel_results.append(
                {
                    "baseline": 0,
                    "bits_stored": 16,
                    "channel_number": channel + 1,
                    "channel_sha256": channel_hash,
                    "correction_factor": 1,
                    "label": label,
                    "sample_skew_present": False,
                    "sensitivity": 1,
                    "sensitivity_unit": {
                        "code_meaning": "microvolt",
                        "code_value": "uV",
                        "coding_scheme_designator": "UCUM",
                    },
                    "source": {
                        "code_meaning": code_meaning,
                        "code_value": code_value,
                        "coding_scheme_designator": "MDC",
                    },
                    "time_skew": 0,
                }
            )

        ordered_payloads.append(payload)
        group_results.append(
            {
                "ordinal": ordinal,
                "originality": "ORIGINAL",
                "label": spec["label"],
                "channel_count": channel_count,
                "samples_per_channel": sample_count,
                "sampling_frequency_hz": spec["sampling_frequency_hz"],
                "duration_seconds": spec["duration_seconds"],
                "simultaneous_sampling": True,
                "channels": channel_results,
                "storage": {
                    "bits_allocated": bits_allocated,
                    "sample_interpretation": sample_interpretation,
                    "data_vr": waveform_element.VR,
                    "byte_order": "little_endian",
                    "interleave_order": "channel_then_sample",
                    "payload_length_bytes": len(payload),
                    "payload_sha256": payload_hash,
                    "channel_sha256": channel_hashes,
                    "sample_value_formula": spec["sample_value_formula"],
                    "sample_min": min(values),
                    "sample_max": max(values),
                    "waveform_padding_value_absent": True,
                    "value_field_padding_bytes": 0,
                    "formula_match": True,
                },
            }
        )

    aggregate_payload = b"".join(ordered_payloads)
    aggregate_hash = hashlib.sha256(aggregate_payload).hexdigest()
    if (
        sop_class_uid == GENERAL_ECG_STORAGE
        and aggregate_hash != GENERAL_AGGREGATE_PAYLOAD_SHA256
    ):
        raise RuntimeError("ordered General ECG aggregate payload hash does not match the lock")
    result = {
        "adapter_id": "pydicom-dicom-validator-waveform",
        "acquisition_context_items": 0,
        "absent_content": {name: True for name in absent_content_tags},
        "modality": str(dataset.Modality),
        "multiplex_groups": group_results,
        "pixel_data_present": False,
        "sop_class_uid": sop_class_uid,
        "transfer_syntax_uid": transfer_syntax,
        "aggregate": {
            "group_count": len(group_results),
            "total_channel_count": sum(
                group["channel_count"] for group in group_results
            ),
            "common_duration_seconds": group_results[0]["duration_seconds"],
            "total_payload_length_bytes": len(aggregate_payload),
            "group_payload_sha256": [
                group["storage"]["payload_sha256"] for group in group_results
            ],
            "aggregate_payload_sha256": aggregate_hash,
        },
    }
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    result.add_argument("--version", action="store_true")
    result.add_argument("--standard-path", type=Path)
    result.add_argument("--lock-path", type=Path)
    result.add_argument("--pixel-u32", action="store_true")
    result.add_argument("--nonsquare-spacing", action="store_true")
    result.add_argument("--waveform", action="store_true")
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
            f"definition_correction={DEFINITION_CORRECTION_ID} "
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
        if args.pixel_u32:
            raise SystemExit(extract_u32_pixels(args.input, standard_path, args.lock_path))
        if args.nonsquare_spacing:
            raise SystemExit(
                extract_nonsquare_spacing(args.input, standard_path, args.lock_path)
            )
        if args.waveform:
            raise SystemExit(extract_waveform(args.input, standard_path, args.lock_path))
        raise SystemExit(validate(args.input, standard_path, args.lock_path))
    except Exception as error:
        print(f"Error: dicom-validator adapter failure: {error}", file=sys.stderr)
        raise SystemExit(126) from error


if __name__ == "__main__":
    main()
