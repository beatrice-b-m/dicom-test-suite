"""Locked WSI tile-referencing Segmentation proof recipe."""

from __future__ import annotations

import hashlib
import time
from pathlib import Path
from typing import Any

import highdicom as hd
import numpy as np
import pydicom
from pydicom.dataset import Dataset
from pydicom.sequence import Sequence
from pydicom.sr.coding import Code

from .protocol import ProtocolError

CASE_ID = "derived/seg/wsi_tile_reference"
RECIPE_ID = "derived_seg_wsi_tile_reference"
SOURCE_CASE_ID = "vl/wsi/tiled_full_small"
SEGMENTATION_STORAGE = "1.2.840.10008.5.1.4.1.1.66.4"
WSI_STORAGE = "1.2.840.10008.5.1.4.1.1.77.1.6"
EXPLICIT_VR_LITTLE_ENDIAN = "1.2.840.10008.1.2.1"
OUTPUT_RELATIVE_PATH = "wsi-tile-segmentation.dcm"
MAXIMUM_OUTPUT_BYTES = 16 * 1024
MAXIMUM_GENERATION_SECONDS = 5.0

FRAME_VALUES = (
    (255, 0, 0, 255),
    (0, 255, 255, 0),
)
FRAME_SHA256 = (
    "34aaa746c25a0f105c4316bbb1f009aa359f49582656ee97d73c58132d563423",
    "10db5223d19bd1d58c2b8eb3c723b0ba104cf17564f9434e53e1b9e642fb3b37",
)
PAYLOAD_SHA256 = "74fa7cbb10160e0eb1f16f35fa9ad0e7f2712af56019996e88cf1034be92635e"
RECONSTRUCTED_MATRIX_SHA256 = (
    "a8ec6f910c0fb02685163a3251bed92517d1016c9173f1e4f021e6b4194f2467"
)
SOURCE_FRAME_NUMBERS = (1, 4)
DIMENSION_INDICES = (
    "ReferencedSegmentNumber",
    "RowPositionInTotalImagePixelMatrix",
    "ColumnPositionInTotalImagePixelMatrix",
)
DIMENSION_INDEX_VALUES = ((1, 1, 1), (1, 2, 2))
POSITIONS = (
    {
        "source_frame_number": 1,
        "row_position": 1,
        "column_position": 1,
        "x_offset": "0",
        "y_offset": "0",
        "z_offset": "0",
    },
    {
        "source_frame_number": 4,
        "row_position": 3,
        "column_position": 3,
        "x_offset": "1",
        "y_offset": "1",
        "z_offset": "0",
    },
)

_SOURCE_FRAME_SHA256 = (
    "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
    "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21",
)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _require_equal(actual: Any, expected: Any, name: str) -> None:
    if actual != expected:
        raise ProtocolError(f"{name} mismatch: expected {expected!r}, got {actual!r}")


def _load_source(request: dict[str, Any]) -> tuple[dict[str, Any], Dataset]:
    sources = request.get("sources", [])
    if len(sources) != 1:
        raise ProtocolError("WSI tile segmentation requires exactly one source")
    source = sources[0]
    _require_equal(source.get("role"), "source_image", "source role")
    _require_equal(source.get("source_case_id"), SOURCE_CASE_ID, "source case")
    _require_equal(source.get("frame_numbers"), list(SOURCE_FRAME_NUMBERS), "source frames")

    relative_path = Path(source["relative_path"])
    if relative_path.is_absolute() or ".." in relative_path.parts:
        raise ProtocolError("source relative path must remain inside staging")
    input_root = Path(request["staging"]["inputs_directory"])
    source_path = input_root / relative_path
    raw = source_path.read_bytes()
    _require_equal(_sha256(raw), source["sha256"], "source hash")
    dataset = pydicom.dcmread(source_path)

    _require_equal(str(dataset.SOPClassUID), WSI_STORAGE, "source SOP Class UID")
    _require_equal(str(dataset.SOPClassUID), source["sop_class_uid"], "declared source SOP Class UID")
    _require_equal(str(dataset.SOPInstanceUID), source["sop_instance_uid"], "source SOP Instance UID")
    _require_equal(str(dataset.SeriesInstanceUID), source["series_instance_uid"], "source Series Instance UID")
    _require_equal(str(dataset.file_meta.TransferSyntaxUID), EXPLICIT_VR_LITTLE_ENDIAN, "source transfer syntax")
    for keyword, expected in (
        ("Modality", "SM"),
        ("DimensionOrganizationType", "TILED_FULL"),
        ("PhotometricInterpretation", "RGB"),
    ):
        _require_equal(str(getattr(dataset, keyword)), expected, f"source {keyword}")
    for keyword, expected in (
        ("Rows", 2),
        ("Columns", 2),
        ("NumberOfFrames", 4),
        ("TotalPixelMatrixRows", 4),
        ("TotalPixelMatrixColumns", 4),
        ("TotalPixelMatrixFocalPlanes", 1),
        ("SamplesPerPixel", 3),
        ("PlanarConfiguration", 0),
        ("BitsAllocated", 8),
        ("BitsStored", 8),
        ("HighBit", 7),
        ("PixelRepresentation", 0),
    ):
        _require_equal(int(getattr(dataset, keyword)), expected, f"source {keyword}")
    _require_equal(
        [float(value) for value in dataset.ImageOrientationSlide],
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        "source Image Orientation Slide",
    )
    origin = dataset.TotalPixelMatrixOriginSequence[0]
    _require_equal(
        [float(origin.XOffsetInSlideCoordinateSystem), float(origin.YOffsetInSlideCoordinateSystem), float(origin.ZOffsetInSlideCoordinateSystem)],
        [0.0, 0.0, 0.0],
        "source total matrix origin",
    )
    measures = dataset.SharedFunctionalGroupsSequence[0].PixelMeasuresSequence[0]
    _require_equal([float(value) for value in measures.PixelSpacing], [0.5, 0.5], "source pixel spacing")

    pixels = np.asarray(dataset.pixel_array, dtype=np.uint8)
    selected_hashes = tuple(_sha256(pixels[index - 1].tobytes()) for index in SOURCE_FRAME_NUMBERS)
    _require_equal(selected_hashes, _SOURCE_FRAME_SHA256, "selected source frame hashes")
    return source, dataset


def _validate_request(request: dict[str, Any], source: Dataset) -> str:
    case = request["case"]
    _require_equal(case["case_id"], CASE_ID, "case ID")
    _require_equal(case["recipe_id"], RECIPE_ID, "recipe ID")
    _require_equal(case["expected_sop_class_uid"], SEGMENTATION_STORAGE, "expected SOP Class UID")
    _require_equal(case["expected_transfer_syntax_uid"], EXPLICIT_VR_LITTLE_ENDIAN, "expected transfer syntax")

    identities = request["identities"]
    _require_equal(identities["study_instance_uid"], str(source.StudyInstanceUID), "Study Instance UID")
    _require_equal(identities["frame_of_reference_uid"], str(source.FrameOfReferenceUID), "Frame of Reference UID")
    if identities["series_instance_uid"] == str(source.SeriesInstanceUID):
        raise ProtocolError("derived Series Instance UID must differ from source")
    primary = [item for item in identities["sop_instances"] if item["role"] == "primary"]
    if len(primary) != 1:
        raise ProtocolError("request must provide exactly one primary SOP Instance UID")

    parameters = request["parameters"]
    for field, expected in (
        ("segmentation_type", "FRACTIONAL"),
        ("fractional_type", "OCCUPANCY"),
        ("maximum_fractional_value", 255),
    ):
        _require_equal(parameters.get(field), expected, field)
    dimension_uid = parameters.get("dimension_organization_uid")
    if not isinstance(dimension_uid, str) or not dimension_uid:
        raise ProtocolError("dimension_organization_uid must be a non-empty string")
    return primary[0]["uid"]


def _mask() -> np.ndarray:
    return np.asarray(
        [
            [1, 0, 0, 0],
            [0, 1, 0, 0],
            [0, 0, 0, 1],
            [0, 0, 1, 0],
        ],
        dtype=np.uint8,
    )[np.newaxis, ...]


def reconstructed_total_pixel_matrix() -> np.ndarray:
    return _mask()[0] * np.uint8(255)


def _code_item(value: str, scheme: str, meaning: str) -> Dataset:
    item = Dataset()
    item.CodeValue = value
    item.CodingSchemeDesignator = scheme
    item.CodeMeaning = meaning
    return item


def _normalize_functional_groups(
    segmentation: Dataset,
    source: Dataset,
    dimension_organization_uid: str,
) -> None:
    for item in segmentation.DimensionOrganizationSequence:
        item.DimensionOrganizationUID = dimension_organization_uid
    for item in segmentation.DimensionIndexSequence:
        item.DimensionOrganizationUID = dimension_organization_uid

    actual_dimensions = tuple(
        pydicom.datadict.keyword_for_tag(item.DimensionIndexPointer)
        for item in segmentation.DimensionIndexSequence
    )
    _require_equal(actual_dimensions, DIMENSION_INDICES, "dimension index order")

    shared = segmentation.SharedFunctionalGroupsSequence[0]
    identification = Dataset()
    identification.ReferencedSegmentNumber = 1
    shared.SegmentIdentificationSequence = Sequence([identification])

    for frame, expected_values, position in zip(
        segmentation.PerFrameFunctionalGroupsSequence,
        DIMENSION_INDEX_VALUES,
        POSITIONS,
        strict=True,
    ):
        if "SegmentIdentificationSequence" in frame:
            del frame.SegmentIdentificationSequence
        _require_equal(
            tuple(int(value) for value in frame.FrameContentSequence[0].DimensionIndexValues),
            expected_values,
            "dimension index values",
        )
        plane = frame.PlanePositionSlideSequence[0]
        _require_equal(int(plane.RowPositionInTotalImagePixelMatrix), position["row_position"], "row position")
        _require_equal(int(plane.ColumnPositionInTotalImagePixelMatrix), position["column_position"], "column position")

        source_item = Dataset()
        source_item.ReferencedSOPClassUID = source.SOPClassUID
        source_item.ReferencedSOPInstanceUID = source.SOPInstanceUID
        source_item.ReferencedFrameNumber = position["source_frame_number"]
        source_item.SpatialLocationsPreserved = "YES"
        source_item.PurposeOfReferenceCodeSequence = Sequence(
            [_code_item("121322", "DCM", "Source Image for Image Processing Operation")]
        )
        derivation = Dataset()
        derivation.DerivationCodeSequence = Sequence(
            [_code_item("113076", "DCM", "Segmentation")]
        )
        derivation.SourceImageSequence = Sequence([source_item])
        frame.DerivationImageSequence = Sequence([derivation])


def _normalize_metadata(segmentation: Dataset, request: dict[str, Any]) -> None:
    controlled = request["controlled_metadata"]
    segmentation.PatientName = controlled["patient_name"]
    segmentation.PatientID = controlled["patient_id"]
    segmentation.Manufacturer = controlled["manufacturer"]
    segmentation.ManufacturerModelName = controlled["model_name"]
    segmentation.SoftwareVersions = controlled["software_versions"]
    segmentation.SyntheticData = "YES"
    segmentation.TimezoneOffsetFromUTC = controlled["timezone_offset_from_utc"]
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
        setattr(segmentation, keyword, controlled[field])
    contribution_datetime = controlled["content_date"] + controlled["content_time"] + controlled["timezone_offset_from_utc"]
    for item in segmentation.ContributingEquipmentSequence:
        item.ContributionDateTime = contribution_datetime


def generate(request: dict[str, Any], output_root: Path) -> dict[str, Any]:
    started = time.monotonic()
    source_request, source = _load_source(request)
    sop_instance_uid = _validate_request(request, source)
    identities = request["identities"]
    parameters = request["parameters"]
    controlled = request["controlled_metadata"]

    segment = hd.seg.SegmentDescription(
        segment_number=1,
        segment_label="DTS_SYNTHETIC_REGION",
        segmented_property_category=Code("T-D0050", "SRT", "Tissue"),
        segmented_property_type=Code("113343", "DCM", "Organ"),
        algorithm_type="MANUAL",
    )
    segmentation = hd.seg.Segmentation(
        source_images=[source],
        pixel_array=_mask(),
        segmentation_type="FRACTIONAL",
        fractional_type="OCCUPANCY",
        max_fractional_value=255,
        segment_descriptions=[segment],
        series_instance_uid=identities["series_instance_uid"],
        series_number=7101,
        sop_instance_uid=sop_instance_uid,
        instance_number=1,
        manufacturer=controlled["manufacturer"],
        manufacturer_model_name=controlled["model_name"],
        software_versions=controlled["software_versions"],
        device_serial_number="DTS-WSI-SEG-001",
        content_description="Synthetic WSI tile occupancy segmentation",
        content_creator_name="DTS^Generator",
        content_label="DTSWSISEG",
        transfer_syntax_uid=EXPLICIT_VR_LITTLE_ENDIAN,
        omit_empty_frames=True,
        dimension_organization_type="TILED_SPARSE",
        tile_pixel_array=True,
        tile_size=(2, 2),
    )
    _normalize_functional_groups(
        segmentation,
        source,
        parameters["dimension_organization_uid"],
    )
    _normalize_metadata(segmentation, request)

    payload = bytes(segmentation.PixelData)
    _require_equal(len(payload), 8, "Pixel Data length")
    _require_equal(_sha256(payload), PAYLOAD_SHA256, "Pixel Data hash")
    _require_equal(
        tuple(_sha256(payload[offset : offset + 4]) for offset in (0, 4)),
        FRAME_SHA256,
        "Pixel Data frame hashes",
    )

    output_root.mkdir(parents=True, exist_ok=True)
    output_path = output_root / OUTPUT_RELATIVE_PATH
    segmentation.save_as(output_path, enforce_file_format=True)
    size_bytes = output_path.stat().st_size
    if size_bytes > MAXIMUM_OUTPUT_BYTES:
        output_path.unlink()
        raise ProtocolError("WSI tile segmentation exceeds the 16 KiB ceiling")
    if time.monotonic() - started > MAXIMUM_GENERATION_SECONDS:
        output_path.unlink()
        raise ProtocolError("WSI tile segmentation exceeds the five-second ceiling")

    reference = {
        "role": "source_image",
        "relationship": "derivation",
        "sop_class_uid": source_request["sop_class_uid"],
        "sop_instance_uid": source_request["sop_instance_uid"],
        "series_instance_uid": source_request["series_instance_uid"],
        "frame_numbers": list(SOURCE_FRAME_NUMBERS),
    }
    return {
        "relative_path": OUTPUT_RELATIVE_PATH,
        "sop_class_uid": SEGMENTATION_STORAGE,
        "sop_instance_uid": sop_instance_uid,
        "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN,
        "references": [reference],
        "expected_semantics": {
            "rows": 2,
            "columns": 2,
            "frames": 2,
            "total_pixel_matrix_rows": 4,
            "total_pixel_matrix_columns": 4,
            "dimension_organization_type": "TILED_SPARSE",
            "segmentation_type": "FRACTIONAL",
            "fractional_type": "OCCUPANCY",
            "maximum_fractional_value": 255,
            "segment_number": 1,
            "dimension_organization_uid": parameters["dimension_organization_uid"],
            "dimension_indices": list(DIMENSION_INDICES),
            "dimension_index_values": [list(values) for values in DIMENSION_INDEX_VALUES],
            "positions": [dict(position) for position in POSITIONS],
        },
        "payload_expectations": {
            "vr": "OB",
            "frame_values": [list(values) for values in FRAME_VALUES],
            "frame_sha256": list(FRAME_SHA256),
            "payload_sha256": PAYLOAD_SHA256,
            "value_length": 8,
            "reconstructed_total_pixel_matrix_sha256": RECONSTRUCTED_MATRIX_SHA256,
            "reconstructed_shape": [4, 4],
        },
    }
