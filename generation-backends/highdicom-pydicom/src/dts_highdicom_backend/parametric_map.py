"""Deterministic float32 Parametric Map proof recipe."""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

import highdicom as hd
import numpy as np
import pydicom
from pydicom.dataset import Dataset
from pydicom.sr.coding import Code

from .protocol import ProtocolError

CASE_ID = "derived/parametric-map/float32_ct_derived_explicit_le"
RECIPE_ID = "derived_parametric_map_float32_ct_derived_explicit_le"
PARAMETRIC_MAP_STORAGE = "1.2.840.10008.5.1.4.1.1.30"
EXPLICIT_VR_LITTLE_ENDIAN = "1.2.840.10008.1.2.1"
OUTPUT_RELATIVE_PATH = "parametric-map.dcm"


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _load_sources(request: dict[str, Any]) -> list[tuple[dict[str, Any], Dataset]]:
    input_root = Path(request["staging"]["inputs_directory"])
    loaded: list[tuple[dict[str, Any], Dataset]] = []
    for source in request["sources"]:
        path = input_root / source["relative_path"]
        raw = path.read_bytes()
        if _sha256(raw) != source["sha256"]:
            raise ProtocolError(f"source hash mismatch for {source['relative_path']}")
        dataset = pydicom.dcmread(path)
        if str(dataset.SOPClassUID) != source["sop_class_uid"]:
            raise ProtocolError("source SOP Class UID mismatch")
        if str(dataset.SOPInstanceUID) != source["sop_instance_uid"]:
            raise ProtocolError("source SOP Instance UID mismatch")
        loaded.append((source, dataset))
    if len(loaded) != 3:
        raise ProtocolError("float32 Parametric Map proof requires exactly three sources")
    return loaded


def _float_pixel_array(
    sources: list[tuple[dict[str, Any], Dataset]],
    parameters: dict[str, Any],
) -> np.ndarray:
    scale = np.float32(parameters["stored_value_scale"])
    rank_increment = np.float32(parameters["spatial_rank_increment"])
    frames = []
    for index, (_, source) in enumerate(sources):
        stored = np.asarray(source.pixel_array, dtype=np.float32)
        frames.append(stored * scale + np.float32(index) * rank_increment)
    pixels = np.stack(frames).astype("<f4", copy=False)
    if not np.isfinite(pixels).all():
        raise ProtocolError("Parametric Map proof values must all be finite")
    return pixels


def _replace_dimension_uid(dataset: Dataset, uid: str) -> None:
    for item in dataset.DimensionOrganizationSequence:
        item.DimensionOrganizationUID = uid
    for item in dataset.DimensionIndexSequence:
        item.DimensionOrganizationUID = uid


def _normalize_metadata(dataset: Dataset, request: dict[str, Any]) -> None:
    controlled = request["controlled_metadata"]
    dataset.PatientName = controlled["patient_name"]
    dataset.PatientID = controlled["patient_id"]
    dataset.Manufacturer = controlled["manufacturer"]
    dataset.ManufacturerModelName = controlled["model_name"]
    dataset.SoftwareVersions = controlled["software_versions"]
    dataset.Laterality = "R"
    dataset.SyntheticData = "YES"
    dataset.TimezoneOffsetFromUTC = controlled["timezone_offset_from_utc"]
    for keyword, field in (
        ("StudyDate", "study_date"),
        ("SeriesDate", "content_date"),
        ("ContentDate", "content_date"),
        ("InstanceCreationDate", "content_date"),
        ("StudyTime", "study_time"),
        ("SeriesTime", "content_time"),
        ("ContentTime", "content_time"),
        ("InstanceCreationTime", "content_time"),
    ):
        setattr(dataset, keyword, controlled[field])


def generate(request: dict[str, Any], output_root: Path) -> dict[str, Any]:
    case = request["case"]
    if case["case_id"] != CASE_ID or case["recipe_id"] != RECIPE_ID:
        raise ProtocolError("unsupported case or recipe")
    if case["expected_sop_class_uid"] != PARAMETRIC_MAP_STORAGE:
        raise ProtocolError("unexpected Parametric Map SOP Class UID")
    if case["expected_transfer_syntax_uid"] != EXPLICIT_VR_LITTLE_ENDIAN:
        raise ProtocolError("float32 proof requires Explicit VR Little Endian")

    sources = _load_sources(request)
    source_datasets = [dataset for _, dataset in sources]
    pixels = _float_pixel_array(sources, request["parameters"])
    identities = request["identities"]
    sop_slots = [
        slot for slot in identities["sop_instances"] if slot["role"] == "primary"
    ]
    if len(sop_slots) != 1:
        raise ProtocolError("request must provide exactly one primary SOP Instance UID")

    minimum = float(pixels.min())
    maximum = float(pixels.max())
    mapping = hd.pm.RealWorldValueMapping(
        lut_label="DTS_FLOAT32",
        lut_explanation="Synthetic CT-derived float32 test values",
        unit=Code("1", "UCUM", "no units"),
        value_range=(minimum, maximum),
        slope=1.0,
        intercept=0.0,
        quantity_definition=Code("110850", "DCM", "X-Ray Attenuation"),
    )
    voi = hd.VOILUTTransformation(
        window_center=(minimum + maximum) / 2.0,
        window_width=maximum - minimum,
        window_explanation="DTS float32 range",
    )
    controlled = request["controlled_metadata"]
    parametric_map = hd.pm.ParametricMap(
        source_images=source_datasets,
        pixel_array=pixels,
        series_instance_uid=identities["series_instance_uid"],
        series_number=7001,
        sop_instance_uid=sop_slots[0]["uid"],
        instance_number=1,
        manufacturer=controlled["manufacturer"],
        manufacturer_model_name=controlled["model_name"],
        software_versions=controlled["software_versions"],
        device_serial_number="DTS-PM-001",
        contains_recognizable_visual_features=False,
        real_world_value_mappings=[mapping],
        voi_lut_transformations=[voi],
        transfer_syntax_uid=EXPLICIT_VR_LITTLE_ENDIAN,
        content_description="Synthetic CT-derived float32 Parametric Map",
        content_creator_name="DTS^Generator",
        content_label="DTSFLOAT32",
        content_qualification="RESEARCH",
        dimension_organization_type="3D",
        derivation=Code("112187", "DCM", "Unspecified method of calculation"),
    )
    if str(parametric_map.StudyInstanceUID) != identities["study_instance_uid"]:
        raise ProtocolError("derived Study Instance UID does not match request")
    if str(parametric_map.FrameOfReferenceUID) != identities["frame_of_reference_uid"]:
        raise ProtocolError("derived Frame of Reference UID does not match request")
    _replace_dimension_uid(
        parametric_map,
        request["parameters"]["dimension_organization_uid"],
    )
    _normalize_metadata(parametric_map, request)

    # highdicom normalizes source images into DICOM spatial dimension order.
    # Report the serialized frame order, not the caller's input-array order.
    serialized_pixels = np.frombuffer(
        parametric_map.FloatPixelData,
        dtype="<f4",
    ).reshape(pixels.shape)

    output_root.mkdir(parents=True, exist_ok=True)
    output_path = output_root / OUTPUT_RELATIVE_PATH
    parametric_map.save_as(output_path, enforce_file_format=True)

    frame_hashes: list[str] = []
    frame_bits: list[list[int]] = []
    for frame in serialized_pixels:
        frame_bytes = frame.astype("<f4", copy=False).tobytes(order="C")
        frame_hashes.append(_sha256(frame_bytes))
        frame_bits.append(frame.view("<u4").reshape(-1).astype(np.uint64).tolist())

    references = [
        {
            "role": source["role"],
            "relationship": "source_image",
            "sop_class_uid": source["sop_class_uid"],
            "sop_instance_uid": source["sop_instance_uid"],
            "series_instance_uid": source["series_instance_uid"],
            "frame_numbers": source["frame_numbers"],
        }
        for source, _ in sources
    ]
    return {
        "relative_path": OUTPUT_RELATIVE_PATH,
        "sop_class_uid": PARAMETRIC_MAP_STORAGE,
        "sop_instance_uid": sop_slots[0]["uid"],
        "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN,
        "references": references,
        "expected_semantics": {
            "sample_type": "float32",
            "rows": int(pixels.shape[1]),
            "columns": int(pixels.shape[2]),
            "frames": int(pixels.shape[0]),
            "minimum": minimum,
            "maximum": maximum,
            "real_world_value_mapping": {
                "lut_label": "DTS_FLOAT32",
                "slope": 1.0,
                "intercept": 0.0,
                "unit": {
                    "value": "1",
                    "scheme": "UCUM",
                    "meaning": "no units",
                },
                "quantity": {
                    "value": "110850",
                    "scheme": "DCM",
                    "meaning": "X-Ray Attenuation",
                },
            },
            "dimension_organization_uid": request["parameters"][
                "dimension_organization_uid"
            ],
        },
        "payload_expectations": {
            "vr": "OF",
            "little_endian_float32_bits": frame_bits,
            "frame_sha256": frame_hashes,
            "value_length": int(pixels.size * 4),
        },
    }
