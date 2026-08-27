from __future__ import annotations

import argparse
import base64
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


ADAPTER_VERSION = "0.4.0"
EDITION = "2026b"
TWELVE_LEAD_ECG_STORAGE = "1.2.840.10008.5.1.4.1.1.9.1.1"
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
    """Extract and independently verify the locked Twelve-lead ECG payload."""
    for name, version in EXPECTED_DISTRIBUTIONS.items():
        verify_distribution(name, version)
    verify_standard(standard_root, lock_path)
    dataset = pydicom.dcmread(input_path)
    transfer_syntax = str(dataset.file_meta.TransferSyntaxUID)
    if transfer_syntax != "1.2.840.10008.1.2.1":
        raise RuntimeError(
            f"unsupported transfer syntax for waveform extraction: {transfer_syntax}"
        )
    if str(dataset.SOPClassUID) != TWELVE_LEAD_ECG_STORAGE:
        raise RuntimeError("dataset is not Twelve-lead ECG Waveform Storage")
    if str(dataset.Modality) != "ECG":
        raise RuntimeError("Twelve-lead ECG Modality must be ECG")
    if len(dataset.WaveformSequence) != 1:
        raise RuntimeError("Twelve-lead ECG requires exactly one multiplex group")

    group = dataset.WaveformSequence[0]
    channel_count = int(group.NumberOfWaveformChannels)
    sample_count = int(group.NumberOfWaveformSamples)
    sampling_frequency = float(group.SamplingFrequency)
    bits_allocated = int(group.WaveformBitsAllocated)
    sample_interpretation = str(group.WaveformSampleInterpretation)
    waveform_element = group[0x54001010]
    payload = bytes(waveform_element.value)
    if (
        channel_count != 12
        or sample_count != 500
        or sampling_frequency != 500.0
        or str(group.WaveformOriginality) != "ORIGINAL"
        or str(group.MultiplexGroupLabel) != "RESTING_12_LEAD"
        or bits_allocated != 16
        or sample_interpretation != "SS"
        or waveform_element.VR != "OW"
    ):
        raise RuntimeError("dataset does not satisfy the locked Twelve-lead waveform shape")
    if len(payload) != channel_count * sample_count * 2:
        raise RuntimeError("Waveform Data length does not match channels and samples")
    if 0x5400100A in group:
        raise RuntimeError("Waveform Padding Value must be absent")
    if 0x7FE00010 in dataset:
        raise RuntimeError("Pixel Data must be absent from a waveform object")

    values = struct.unpack(f"<{channel_count * sample_count}h", payload)
    expected_values = tuple(
        ((sample * (channel + 1) * 37 + channel * 101) % 2001) - 1000
        for sample in range(sample_count)
        for channel in range(channel_count)
    )
    if values != expected_values:
        raise RuntimeError("Waveform Data does not match the locked sample formula")
    payload_hash = hashlib.sha256(payload).hexdigest()
    if payload_hash != WAVEFORM_PAYLOAD_SHA256:
        raise RuntimeError("Waveform Data hash does not match the locked payload")

    definitions = list(group.ChannelDefinitionSequence)
    if len(definitions) != channel_count:
        raise RuntimeError("Channel Definition Sequence length does not match channels")
    channel_results = []
    channel_hashes = []
    for channel, (definition, locked) in enumerate(
        zip(definitions, WAVEFORM_CHANNELS, strict=True)
    ):
        label, code_value, code_meaning, expected_hash = locked
        sources = list(definition.ChannelSourceSequence)
        units = list(definition.ChannelSensitivityUnitsSequence)
        if len(sources) != 1 or len(units) != 1:
            raise RuntimeError(f"channel {channel + 1} coding sequences must have one item")
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
            raise RuntimeError(f"channel {channel + 1} metadata does not match the lock")
        channel_values = values[channel::channel_count]
        channel_bytes = struct.pack(f"<{sample_count}h", *channel_values)
        channel_hash = hashlib.sha256(channel_bytes).hexdigest()
        if channel_hash != expected_hash:
            raise RuntimeError(f"channel {channel + 1} hash does not match the lock")
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

    result = {
        "adapter_id": "pydicom-dicom-validator-waveform",
        "bits_allocated": bits_allocated,
        "byte_order": "little_endian",
        "channel_count": channel_count,
        "channel_definitions": channel_results,
        "channel_hashes": channel_hashes,
        "duration_seconds": 1,
        "formula_match": True,
        "interleave_order": "channel_then_sample",
        "modality": str(dataset.Modality),
        "multiplex_group_count": 1,
        "multiplex_group_label": str(group.MultiplexGroupLabel),
        "originality": str(group.WaveformOriginality),
        "pixel_data_present": False,
        "sample_count": sample_count,
        "sample_interpretation": sample_interpretation,
        "sampling_frequency_hz": 500,
        "sop_class_uid": str(dataset.SOPClassUID),
        "stored_value_max": max(values),
        "stored_value_min": min(values),
        "transfer_syntax_uid": transfer_syntax,
        "waveform_data_length": len(payload),
        "waveform_data_sha256": payload_hash,
        "waveform_data_vr": waveform_element.VR,
        "waveform_padding_present": False,
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
