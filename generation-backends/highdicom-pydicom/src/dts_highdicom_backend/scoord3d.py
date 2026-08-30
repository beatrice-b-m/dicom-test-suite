"""Deterministic Comprehensive 3D SR distance with SCOORD3D coordinates."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import highdicom as hd
import numpy as np
from pydicom.dataset import Dataset
from pydicom.sr.codedict import codes
from pydicom.sr.coding import Code

from .protocol import ProtocolError
from .tid1500 import _load_source, _normalize_metadata, _primary_sop_instance_uid

CASE_ID = "derived/sr/comprehensive3d_scoord3d"
RECIPE_ID = "derived_sr_comprehensive3d_scoord3d"
COMPREHENSIVE_3D_SR_STORAGE = "1.2.840.10008.5.1.4.1.1.88.34"
ENHANCED_CT_STORAGE = "1.2.840.10008.5.1.4.1.1.2.1"
EXPLICIT_VR_LITTLE_ENDIAN = "1.2.840.10008.1.2.1"
OUTPUT_RELATIVE_PATH = "scoord3d-report.dcm"
SOURCE_IMAGE_ROLE = "source_image"
SOURCE_FRAMES = [1, 2]
GRAPHIC_TYPE = "POLYLINE"
GRAPHIC_DATA_PATIENT_MM = [[0.0, 0.0, 0.0], [0.0, 0.0, 2.5]]
MEASUREMENT_VALUE_MM = 2.5
TRACKING_IDENTIFIER = "DTS-SCOORD3D-ROI-1"


def _float_values(value: Any) -> list[float]:
    return [float(item) for item in value]


def _validate_source(request: dict[str, Any]) -> tuple[dict[str, Any], Dataset]:
    sources = request["sources"]
    if len(sources) != 1 or sources[0]["role"] != SOURCE_IMAGE_ROLE:
        raise ProtocolError("SCOORD3D recipe requires exactly one source_image")
    source = sources[0]
    if source["sop_class_uid"] != ENHANCED_CT_STORAGE:
        raise ProtocolError("SCOORD3D source image must be Enhanced CT")
    if source["frame_numbers"] != SOURCE_FRAMES:
        raise ProtocolError("SCOORD3D source must declare frames 1 and 2")

    ct = _load_source(request, source)
    identities = request["identities"]
    controlled = request["controlled_metadata"]
    for attribute, expected in (
        ("StudyInstanceUID", identities["study_instance_uid"]),
        ("FrameOfReferenceUID", identities["frame_of_reference_uid"]),
        ("PatientID", controlled["patient_id"]),
        ("PatientName", controlled["patient_name"]),
    ):
        if str(getattr(ct, attribute)) != expected:
            raise ProtocolError(f"source {attribute} mismatch")
    if int(ct.NumberOfFrames) != 2:
        raise ProtocolError("SCOORD3D Enhanced CT must contain exactly two frames")

    shared = ct.SharedFunctionalGroupsSequence[0]
    pixel_measures = shared.PixelMeasuresSequence[0]
    if _float_values(pixel_measures.PixelSpacing) != [0.75, 0.75]:
        raise ProtocolError("SCOORD3D Pixel Spacing must be 0.75 by 0.75")
    if float(pixel_measures.SliceThickness) != 2.5:
        raise ProtocolError("SCOORD3D Slice Thickness must be 2.5")
    if float(pixel_measures.SpacingBetweenSlices) != 2.5:
        raise ProtocolError("SCOORD3D Spacing Between Slices must be 2.5")
    orientation = shared.PlaneOrientationSequence[0].ImageOrientationPatient
    if _float_values(orientation) != [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]:
        raise ProtocolError("SCOORD3D source orientation must be canonical axial")
    positions = [
        _float_values(group.PlanePositionSequence[0].ImagePositionPatient)
        for group in ct.PerFrameFunctionalGroupsSequence
    ]
    if positions != GRAPHIC_DATA_PATIENT_MM:
        raise ProtocolError("SCOORD3D points must equal source frame positions")
    return source, ct


def _require_parameters(
    request: dict[str, Any],
) -> tuple[str, str, str, str, list[list[float]], float]:
    parameters = request["parameters"]
    expected = {
        "tracking_identifier": TRACKING_IDENTIFIER,
        "graphic_type": GRAPHIC_TYPE,
    }
    for field, value in expected.items():
        if parameters.get(field) != value:
            raise ProtocolError(f"SCOORD3D parameter {field} differs from recipe")
    values = tuple(
        parameters.get(field)
        for field in ("tracking_identifier", "tracking_uid", "observer_uid", "fiducial_uid")
    )
    if not all(isinstance(value, str) and value for value in values):
        raise ProtocolError("SCOORD3D tracking, observer, and fiducial identities are required")
    graphic_data = parameters.get("graphic_data_patient_mm")
    if (
        not isinstance(graphic_data, list)
        or len(graphic_data) != 2
        or any(not isinstance(point, list) or len(point) != 3 for point in graphic_data)
    ):
        raise ProtocolError("SCOORD3D graphic data must contain exactly two 3D points")
    graphic_data = [[float(coordinate) for coordinate in point] for point in graphic_data]
    measurement_value = float(parameters.get("measurement_value_mm"))
    return (*values, graphic_data, measurement_value)  # type: ignore[return-value]


def _preserve_numeric_value_lexical_form(dataset: Dataset, value: float) -> None:
    matches = []
    pending = list(dataset.ContentSequence)
    while pending:
        item = pending.pop(0)
        if hasattr(item, "MeasuredValueSequence"):
            matches.append(item.MeasuredValueSequence[0])
        pending.extend(getattr(item, "ContentSequence", []))
    if len(matches) != 1:
        raise ProtocolError("SCOORD3D report must contain exactly one numeric measurement")
    matches[0].NumericValue = format(value, ".15g")


def generate(request: dict[str, Any], output_root: Path) -> dict[str, Any]:
    case = request["case"]
    if (case["case_id"], case["recipe_id"]) != (CASE_ID, RECIPE_ID):
        raise ProtocolError("unsupported Comprehensive 3D SCOORD3D case or recipe")
    if case["expected_sop_class_uid"] != COMPREHENSIVE_3D_SR_STORAGE:
        raise ProtocolError("unexpected Comprehensive 3D SR SOP Class UID")
    if case["expected_transfer_syntax_uid"] != EXPLICIT_VR_LITTLE_ENDIAN:
        raise ProtocolError("SCOORD3D recipe requires Explicit VR Little Endian")

    source, ct = _validate_source(request)
    (
        tracking_identifier,
        tracking_uid,
        observer_uid,
        fiducial_uid,
        graphic_data,
        measurement_value,
    ) = _require_parameters(request)
    identities = request["identities"]
    controlled = request["controlled_metadata"]

    coordinates = hd.sr.CoordinatesForMeasurement3D(
        graphic_type=GRAPHIC_TYPE,
        graphic_data=np.asarray(graphic_data, dtype=np.float64),
        frame_of_reference_uid=str(ct.FrameOfReferenceUID),
        fiducial_uid=fiducial_uid,
    )
    measurement = hd.sr.Measurement(
        name=Code("121206", "DCM", "Distance"),
        value=measurement_value,
        unit=Code("mm", "UCUM", "millimeter"),
        referenced_coordinates=[coordinates],
    )
    source_image = hd.sr.SourceImageForMeasurementGroup(
        referenced_sop_class_uid=str(ct.SOPClassUID),
        referenced_sop_instance_uid=str(ct.SOPInstanceUID),
        referenced_frame_numbers=SOURCE_FRAMES,
    )
    group = hd.sr.MeasurementsAndQualitativeEvaluations(
        tracking_identifier=hd.sr.TrackingIdentifier(
            uid=tracking_uid,
            identifier=tracking_identifier,
        ),
        finding_type=Code("123037004", "SCT", "Body structure"),
        measurements=[measurement],
        source_images=[source_image],
    )
    observer = hd.sr.ObserverContext(
        observer_type=codes.DCM.Device,
        observer_identifying_attributes=hd.sr.DeviceObserverIdentifyingAttributes(
            uid=observer_uid,
            name=controlled["manufacturer"],
            manufacturer_name=controlled["manufacturer"],
            model_name=controlled["model_name"],
        ),
    )
    report = hd.sr.MeasurementReport(
        observation_context=hd.sr.ObservationContext(observer_device_context=observer),
        procedure_reported=Code("25045-6", "LN", "CT unspecified body region"),
        imaging_measurements=[group],
    )
    document = hd.sr.Comprehensive3DSR(
        evidence=[ct],
        content=report,
        series_instance_uid=identities["series_instance_uid"],
        series_number=8002,
        sop_instance_uid=_primary_sop_instance_uid(request),
        instance_number=1,
        manufacturer=controlled["manufacturer"],
        is_complete=True,
        is_final=True,
        is_verified=False,
        content_date=controlled["content_date"],
        content_time=controlled["content_time"],
        transfer_syntax_uid=EXPLICIT_VR_LITTLE_ENDIAN,
        manufacturer_model_name=controlled["model_name"],
        software_versions=controlled["software_versions"],
    )
    _preserve_numeric_value_lexical_form(document, measurement_value)
    _normalize_metadata(document, request)

    output_root.mkdir(parents=True, exist_ok=True)
    output_path = output_root / OUTPUT_RELATIVE_PATH
    document.save_as(output_path, enforce_file_format=True)

    return {
        "relative_path": OUTPUT_RELATIVE_PATH,
        "sop_class_uid": COMPREHENSIVE_3D_SR_STORAGE,
        "sop_instance_uid": str(document.SOPInstanceUID),
        "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN,
        "references": [{
            "role": source["role"],
            "relationship": "source_of_measurement",
            "sop_class_uid": source["sop_class_uid"],
            "sop_instance_uid": source["sop_instance_uid"],
            "series_instance_uid": source["series_instance_uid"],
            "frame_numbers": SOURCE_FRAMES,
        }],
        "expected_semantics": {
            "completion_flag": "COMPLETE",
            "preliminary_flag": "FINAL",
            "verification_flag": "UNVERIFIED",
            "root_template_identifier": "1500",
            "measurement_group_template_identifier": "1501",
            "tracking_identifier": tracking_identifier,
            "tracking_uid": tracking_uid,
            "observer_uid": observer_uid,
            "fiducial_uid": fiducial_uid,
            "graphic_type": GRAPHIC_TYPE,
            "graphic_data_patient_mm": graphic_data,
            "frame_of_reference_uid": identities["frame_of_reference_uid"],
            "source_frame_numbers": SOURCE_FRAMES,
            "measurement": {
                "name": {"value": "121206", "scheme": "DCM", "meaning": "Distance"},
                "value": measurement_value,
                "unit": {"value": "mm", "scheme": "UCUM", "meaning": "millimeter"},
            },
            "procedure_reported": {
                "value": "25045-6", "scheme": "LN", "meaning": "CT unspecified body region"
            },
            "finding": {"value": "123037004", "scheme": "SCT", "meaning": "Body structure"},
            "evidence_sop_instance_uids": [source["sop_instance_uid"]],
        },
        "payload_expectations": {
            "content_tree": "tid1500_tid1501_distance_scoord3d_polyline",
            "pixel_data": "absent",
        },
    }
