"""Deterministic floating-point Parametric Map proof recipes."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import highdicom as hd
import numpy as np
import pydicom
from pydicom.dataset import Dataset
from pydicom.sr.coding import Code

from .protocol import ProtocolError

FLOAT32_CASE_ID = "derived/parametric-map/float32_ct_derived_explicit_le"
FLOAT32_RECIPE_ID = "derived_parametric_map_float32_ct_derived_explicit_le"
FLOAT64_CASE_ID = "derived/parametric-map/float64_ct_derived_explicit_le"
FLOAT64_RECIPE_ID = "derived_parametric_map_float64_ct_derived_explicit_le"
# Preserve the original public constants for callers that target the first recipe.
CASE_ID = FLOAT32_CASE_ID
RECIPE_ID = FLOAT32_RECIPE_ID
PARAMETRIC_MAP_STORAGE = "1.2.840.10008.5.1.4.1.1.30"
EXPLICIT_VR_LITTLE_ENDIAN = "1.2.840.10008.1.2.1"
OUTPUT_RELATIVE_PATH = "parametric-map.dcm"
FLOAT64_SPATIAL_RANK_INCREMENT = 2.0**-30


@dataclass(frozen=True)
class _Recipe:
    case_id: str
    recipe_id: str
    sample_type: str
    output_relative_path: str
    dtype: str
    pixel_data_keyword: str
    payload_vr: str
    payload_bits_key: str
    lut_label: str
    lut_explanation: str
    window_explanation: str
    series_number: int
    device_serial_number: str
    content_description: str
    content_label: str


_FLOAT32_RECIPE = _Recipe(
    case_id=FLOAT32_CASE_ID,
    recipe_id=FLOAT32_RECIPE_ID,
    sample_type="float32",
    output_relative_path=OUTPUT_RELATIVE_PATH,
    dtype="<f4",
    pixel_data_keyword="FloatPixelData",
    payload_vr="OF",
    payload_bits_key="little_endian_float32_bits",
    lut_label="DTS_FLOAT32",
    lut_explanation="Synthetic CT-derived float32 test values",
    window_explanation="DTS float32 range",
    series_number=7001,
    device_serial_number="DTS-PM-001",
    content_description="Synthetic CT-derived float32 Parametric Map",
    content_label="DTSFLOAT32",
)
_FLOAT64_RECIPE = _Recipe(
    case_id=FLOAT64_CASE_ID,
    recipe_id=FLOAT64_RECIPE_ID,
    sample_type="float64",
    output_relative_path="parametric-map-float64.dcm",
    dtype="<f8",
    pixel_data_keyword="DoubleFloatPixelData",
    payload_vr="OD",
    payload_bits_key="little_endian_float64_bits",
    lut_label="DTS_FLOAT64",
    lut_explanation="Synthetic CT-derived float64 test values",
    window_explanation="DTS float64 range",
    series_number=7002,
    device_serial_number="DTS-PM-002",
    content_description="Synthetic CT-derived float64 Parametric Map",
    content_label="DTSFLOAT64",
)
_RECIPES = {
    (recipe.case_id, recipe.recipe_id): recipe
    for recipe in (_FLOAT32_RECIPE, _FLOAT64_RECIPE)
}


def _recipe_for_request(request: dict[str, Any]) -> _Recipe:
    case = request["case"]
    recipe = _RECIPES.get((case["case_id"], case["recipe_id"]))
    if recipe is None:
        raise ProtocolError("unsupported case or recipe")
    sample_type = request["parameters"].get("sample_type", recipe.sample_type)
    if sample_type != recipe.sample_type:
        raise ProtocolError(
            f"{recipe.case_id} requires sample_type {recipe.sample_type}"
        )
    return recipe


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
        raise ProtocolError("Parametric Map proof requires exactly three sources")
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


def _float64_pixel_array(
    sources: list[tuple[dict[str, Any], Dataset]],
    parameters: dict[str, Any],
) -> np.ndarray:
    scale = np.float64(parameters["stored_value_scale"])
    rank_increment = np.float64(parameters["spatial_rank_increment"])
    if rank_increment != np.float64(FLOAT64_SPATIAL_RANK_INCREMENT):
        raise ProtocolError(
            "float64 Parametric Map spatial rank increment must be 2^-30"
        )
    frames = []
    for index, (_, source) in enumerate(sources):
        stored = np.asarray(source.pixel_array, dtype=np.float64)
        frames.append(stored * scale + np.float64(index) * rank_increment)
    pixels = np.stack(frames).astype("<f8", copy=False)
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
    contribution_datetime = (
        controlled["content_date"]
        + controlled["content_time"]
        + controlled["timezone_offset_from_utc"]
    )
    for item in dataset.ContributingEquipmentSequence:
        item.ContributionDateTime = contribution_datetime


def generate(request: dict[str, Any], output_root: Path) -> dict[str, Any]:
    case = request["case"]
    recipe = _recipe_for_request(request)
    if case["expected_sop_class_uid"] != PARAMETRIC_MAP_STORAGE:
        raise ProtocolError("unexpected Parametric Map SOP Class UID")
    if case["expected_transfer_syntax_uid"] != EXPLICIT_VR_LITTLE_ENDIAN:
        raise ProtocolError("Parametric Map proof requires Explicit VR Little Endian")

    sources = _load_sources(request)
    source_datasets = [dataset for _, dataset in sources]
    if recipe.sample_type == "float32":
        pixels = _float_pixel_array(sources, request["parameters"])
    else:
        pixels = _float64_pixel_array(sources, request["parameters"])
    identities = request["identities"]
    sop_slots = [
        slot for slot in identities["sop_instances"] if slot["role"] == "primary"
    ]
    if len(sop_slots) != 1:
        raise ProtocolError("request must provide exactly one primary SOP Instance UID")

    minimum = float(pixels.min())
    maximum = float(pixels.max())
    mapping = hd.pm.RealWorldValueMapping(
        lut_label=recipe.lut_label,
        lut_explanation=recipe.lut_explanation,
        unit=Code("1", "UCUM", "no units"),
        value_range=(minimum, maximum),
        slope=1.0,
        intercept=0.0,
        quantity_definition=Code("110850", "DCM", "X-Ray Attenuation"),
    )
    voi = hd.VOILUTTransformation(
        window_center=(minimum + maximum) / 2.0,
        window_width=maximum - minimum,
        window_explanation=recipe.window_explanation,
    )
    controlled = request["controlled_metadata"]
    parametric_map = hd.pm.ParametricMap(
        source_images=source_datasets,
        pixel_array=pixels,
        series_instance_uid=identities["series_instance_uid"],
        series_number=recipe.series_number,
        sop_instance_uid=sop_slots[0]["uid"],
        instance_number=1,
        manufacturer=controlled["manufacturer"],
        manufacturer_model_name=controlled["model_name"],
        software_versions=controlled["software_versions"],
        device_serial_number=recipe.device_serial_number,
        contains_recognizable_visual_features=False,
        real_world_value_mappings=[mapping],
        voi_lut_transformations=[voi],
        transfer_syntax_uid=EXPLICIT_VR_LITTLE_ENDIAN,
        content_description=recipe.content_description,
        content_creator_name="DTS^Generator",
        content_label=recipe.content_label,
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
        getattr(parametric_map, recipe.pixel_data_keyword),
        dtype=recipe.dtype,
    ).reshape(pixels.shape)

    output_root.mkdir(parents=True, exist_ok=True)
    output_path = output_root / recipe.output_relative_path
    parametric_map.save_as(output_path, enforce_file_format=True)

    frame_hashes: list[str] = []
    frame_bits: list[list[int]] = []
    for frame in serialized_pixels:
        frame_bytes = frame.astype(recipe.dtype, copy=False).tobytes(order="C")
        frame_hashes.append(_sha256(frame_bytes))
        unsigned_dtype = "<u4" if recipe.sample_type == "float32" else "<u8"
        frame_bits.append(
            frame.view(unsigned_dtype).reshape(-1).astype(np.uint64).tolist()
        )

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
        "relative_path": recipe.output_relative_path,
        "sop_class_uid": PARAMETRIC_MAP_STORAGE,
        "sop_instance_uid": sop_slots[0]["uid"],
        "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN,
        "references": references,
        "expected_semantics": {
            "sample_type": recipe.sample_type,
            "rows": int(pixels.shape[1]),
            "columns": int(pixels.shape[2]),
            "frames": int(pixels.shape[0]),
            "minimum": minimum,
            "maximum": maximum,
            "real_world_value_mapping": {
                "lut_label": recipe.lut_label,
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
            "vr": recipe.payload_vr,
            recipe.payload_bits_key: frame_bits,
            "frame_sha256": frame_hashes,
            "value_length": int(pixels.size * pixels.dtype.itemsize),
        },
    }
