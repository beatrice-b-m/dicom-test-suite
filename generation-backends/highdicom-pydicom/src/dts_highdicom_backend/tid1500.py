"""Deterministic TID 1500 CT volume measurement report recipe."""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

import highdicom as hd
import pydicom
from pydicom.dataset import Dataset
from pydicom.sr.codedict import codes
from pydicom.sr.coding import Code

from .protocol import ProtocolError

CASE_ID = "derived/sr/tid1500_ct_measurement_report"
RECIPE_ID = "derived_sr_tid1500_ct_measurement_report"
COMPREHENSIVE_3D_SR_STORAGE = "1.2.840.10008.5.1.4.1.1.88.34"
ENHANCED_CT_STORAGE = "1.2.840.10008.5.1.4.1.1.2.1"
SEGMENTATION_STORAGE = "1.2.840.10008.5.1.4.1.1.66.4"
EXPLICIT_VR_LITTLE_ENDIAN = "1.2.840.10008.1.2.1"
OUTPUT_RELATIVE_PATH = "measurement-report.dcm"
SOURCE_IMAGE_ROLE = "source_image"
SEGMENTATION_ROLE = "segmentation"
SOURCE_FRAMES = [1, 2]
SEGMENT_NUMBER = 1
MEASUREMENT_VALUE = 5.625
TRACKING_IDENTIFIER = "DTS-TID1500-ROI-1"
HIGHDICOM_IMPLEMENTATION_CLASS_UID = "1.2.826.0.1.3680043.9.7433.1.1"
HIGHDICOM_IMPLEMENTATION_VERSION_NAME = "highdicom0.28.1"


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _load_source(
    request: dict[str, Any],
    source: dict[str, Any],
) -> Dataset:
    path = Path(request["staging"]["inputs_directory"]) / source["relative_path"]
    raw = path.read_bytes()
    if _sha256(raw) != source["sha256"]:
        raise ProtocolError(f"source hash mismatch for {source['relative_path']}")
    dataset = pydicom.dcmread(path)
    if str(dataset.SOPClassUID) != source["sop_class_uid"]:
        raise ProtocolError(f"source SOP Class UID mismatch for {source['role']}")
    if str(dataset.SOPInstanceUID) != source["sop_instance_uid"]:
        raise ProtocolError(f"source SOP Instance UID mismatch for {source['role']}")
    if str(dataset.SeriesInstanceUID) != source["series_instance_uid"]:
        raise ProtocolError(f"source Series Instance UID mismatch for {source['role']}")
    return dataset


def _validate_sources(
    request: dict[str, Any],
) -> tuple[dict[str, Any], Dataset, dict[str, Any], Dataset]:
    sources = request["sources"]
    if len(sources) != 2:
        raise ProtocolError("TID 1500 proof requires exactly one CT and one SEG")
    if [source["role"] for source in sources] != [
        SOURCE_IMAGE_ROLE,
        SEGMENTATION_ROLE,
    ]:
        raise ProtocolError("TID 1500 sources must be ordered as CT then SEG")
    ct_source, seg_source = sources
    if ct_source["sop_class_uid"] != ENHANCED_CT_STORAGE:
        raise ProtocolError("TID 1500 source image must be Enhanced CT")
    if ct_source["frame_numbers"] != SOURCE_FRAMES:
        raise ProtocolError("TID 1500 Enhanced CT must declare frames 1 and 2")
    if seg_source["sop_class_uid"] != SEGMENTATION_STORAGE:
        raise ProtocolError("TID 1500 ROI must be binary Segmentation Storage")
    if seg_source["frame_numbers"] is not None:
        raise ProtocolError("TID 1500 segment reference applies to all segment frames")

    ct = _load_source(request, ct_source)
    seg = _load_source(request, seg_source)
    identities = request["identities"]
    controlled = request["controlled_metadata"]
    for role, dataset in (("source_image", ct), ("segmentation", seg)):
        if str(dataset.StudyInstanceUID) != identities["study_instance_uid"]:
            raise ProtocolError(f"{role} Study Instance UID mismatch")
        if str(dataset.FrameOfReferenceUID) != identities["frame_of_reference_uid"]:
            raise ProtocolError(f"{role} Frame of Reference UID mismatch")
        if str(dataset.PatientID) != controlled["patient_id"]:
            raise ProtocolError(f"{role} Patient ID mismatch")
        if str(dataset.PatientName) != controlled["patient_name"]:
            raise ProtocolError(f"{role} Patient Name mismatch")

    if int(ct.NumberOfFrames) != 2:
        raise ProtocolError("TID 1500 Enhanced CT must contain exactly two frames")
    pixel_measures = ct.SharedFunctionalGroupsSequence[0].PixelMeasuresSequence[0]
    if [float(value) for value in pixel_measures.PixelSpacing] != [0.75, 0.75]:
        raise ProtocolError("TID 1500 Enhanced CT Pixel Spacing must be 0.75 by 0.75")
    if float(pixel_measures.SliceThickness) != 2.5:
        raise ProtocolError("TID 1500 Enhanced CT Slice Thickness must be 2.5")

    if str(seg.SegmentationType) != "BINARY" or int(seg.NumberOfFrames) != 2:
        raise ProtocolError("TID 1500 SEG must be a two-frame binary segmentation")
    segment_numbers = {
        int(item.SegmentNumber) for item in seg.SegmentSequence
    }
    if segment_numbers != {SEGMENT_NUMBER}:
        raise ProtocolError("TID 1500 SEG must contain only segment 1")
    referenced_frames: list[int] = []
    for functional_group in seg.PerFrameFunctionalGroupsSequence:
        identified = int(
            functional_group.SegmentIdentificationSequence[0]
            .ReferencedSegmentNumber
        )
        if identified != SEGMENT_NUMBER:
            raise ProtocolError("TID 1500 SEG frame does not identify segment 1")
        source_item = (
            functional_group.DerivationImageSequence[0]
            .SourceImageSequence[0]
        )
        if str(source_item.ReferencedSOPClassUID) != str(ct.SOPClassUID):
            raise ProtocolError("SEG source SOP Class UID does not match CT")
        if str(source_item.ReferencedSOPInstanceUID) != str(ct.SOPInstanceUID):
            raise ProtocolError("SEG source SOP Instance UID does not match CT")
        referenced_frames.append(int(source_item.ReferencedFrameNumber))
    if referenced_frames != SOURCE_FRAMES:
        raise ProtocolError("SEG must reference Enhanced CT frames 1 and 2")
    return ct_source, ct, seg_source, seg


def _primary_sop_instance_uid(request: dict[str, Any]) -> str:
    slots = [
        slot
        for slot in request["identities"]["sop_instances"]
        if slot["role"] == "primary"
    ]
    if len(slots) != 1:
        raise ProtocolError("request must provide exactly one primary SOP Instance UID")
    return str(slots[0]["uid"])


def _require_parameters(request: dict[str, Any]) -> tuple[str, str, str]:
    parameters = request["parameters"]
    if parameters.get("segment_number") != SEGMENT_NUMBER:
        raise ProtocolError("TID 1500 recipe requires segment_number 1")
    if parameters.get("measurement_value") != MEASUREMENT_VALUE:
        raise ProtocolError("TID 1500 recipe requires measurement_value 5.625")
    tracking_identifier = parameters.get("tracking_identifier")
    tracking_uid = parameters.get("tracking_uid")
    observer_uid = parameters.get("observer_uid")
    for name, value in (
        ("tracking_identifier", tracking_identifier),
        ("tracking_uid", tracking_uid),
        ("observer_uid", observer_uid),
    ):
        if not isinstance(value, str) or not value:
            raise ProtocolError(f"TID 1500 parameter {name} must be a string")
    if tracking_identifier != TRACKING_IDENTIFIER:
        raise ProtocolError(
            f"TID 1500 tracking_identifier must be {TRACKING_IDENTIFIER}"
        )
    return tracking_identifier, tracking_uid, observer_uid


def _normalize_metadata(dataset: Dataset, request: dict[str, Any]) -> None:
    controlled = request["controlled_metadata"]
    dataset.PatientName = controlled["patient_name"]
    dataset.PatientID = controlled["patient_id"]
    dataset.Manufacturer = controlled["manufacturer"]
    dataset.ManufacturerModelName = controlled["model_name"]
    dataset.SoftwareVersions = controlled["software_versions"]
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
    dataset.file_meta.ImplementationClassUID = HIGHDICOM_IMPLEMENTATION_CLASS_UID
    dataset.file_meta.ImplementationVersionName = HIGHDICOM_IMPLEMENTATION_VERSION_NAME


def _normalize_source_image_meaning(dataset: Dataset) -> None:
    """Apply the PS3.16 spelling expected by the independent SR validator."""
    pending = list(dataset.ContentSequence)
    while pending:
        item = pending.pop(0)
        if hasattr(item, "ConceptNameCodeSequence"):
            concept = item.ConceptNameCodeSequence[0]
            if str(concept.CodeValue) == "121233" and str(
                concept.CodingSchemeDesignator
            ) == "DCM":
                concept.CodeMeaning = "Source image for segmentation"
        pending.extend(getattr(item, "ContentSequence", []))


def generate(request: dict[str, Any], output_root: Path) -> dict[str, Any]:
    case = request["case"]
    if (case["case_id"], case["recipe_id"]) != (CASE_ID, RECIPE_ID):
        raise ProtocolError("unsupported TID 1500 case or recipe")
    if case["expected_sop_class_uid"] != COMPREHENSIVE_3D_SR_STORAGE:
        raise ProtocolError("unexpected TID 1500 SOP Class UID")
    if case["expected_transfer_syntax_uid"] != EXPLICIT_VR_LITTLE_ENDIAN:
        raise ProtocolError("TID 1500 proof requires Explicit VR Little Endian")

    ct_source, ct, seg_source, seg = _validate_sources(request)
    tracking_identifier, tracking_uid, observer_uid = _require_parameters(request)
    identities = request["identities"]
    controlled = request["controlled_metadata"]

    source_image = hd.sr.SourceImageForSegmentation(
        referenced_sop_class_uid=str(ct.SOPClassUID),
        referenced_sop_instance_uid=str(ct.SOPInstanceUID),
        referenced_frame_numbers=SOURCE_FRAMES,
    )
    referenced_segment = hd.sr.ReferencedSegment(
        sop_class_uid=str(seg.SOPClassUID),
        sop_instance_uid=str(seg.SOPInstanceUID),
        segment_number=SEGMENT_NUMBER,
        frame_numbers=None,
        source_images=[source_image],
    )
    measurement = hd.sr.Measurement(
        name=Code("118565006", "SCT", "Volume"),
        value=MEASUREMENT_VALUE,
        unit=Code("mm3", "UCUM", "cubic millimeter"),
    )
    measurement_group = hd.sr.VolumetricROIMeasurementsAndQualitativeEvaluations(
        tracking_identifier=hd.sr.TrackingIdentifier(
            uid=tracking_uid,
            identifier=tracking_identifier,
        ),
        referenced_segment=referenced_segment,
        finding_type=Code("123037004", "SCT", "Body structure"),
        measurements=[measurement],
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
        observation_context=hd.sr.ObservationContext(
            observer_device_context=observer,
        ),
        procedure_reported=Code(
            "25045-6",
            "LN",
            "CT unspecified body region",
        ),
        imaging_measurements=[measurement_group],
    )
    document = hd.sr.Comprehensive3DSR(
        evidence=[ct, seg],
        content=report,
        series_instance_uid=identities["series_instance_uid"],
        series_number=8001,
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
    _normalize_source_image_meaning(document)
    _normalize_metadata(document, request)

    output_root.mkdir(parents=True, exist_ok=True)
    output_path = output_root / OUTPUT_RELATIVE_PATH
    document.save_as(output_path, enforce_file_format=True)

    return {
        "relative_path": OUTPUT_RELATIVE_PATH,
        "sop_class_uid": COMPREHENSIVE_3D_SR_STORAGE,
        "sop_instance_uid": str(document.SOPInstanceUID),
        "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN,
        "references": [
            {
                "role": ct_source["role"],
                "relationship": "source_image_for_segmentation",
                "sop_class_uid": ct_source["sop_class_uid"],
                "sop_instance_uid": ct_source["sop_instance_uid"],
                "series_instance_uid": ct_source["series_instance_uid"],
                "frame_numbers": SOURCE_FRAMES,
            },
            {
                "role": seg_source["role"],
                "relationship": "referenced_segment",
                "sop_class_uid": seg_source["sop_class_uid"],
                "sop_instance_uid": seg_source["sop_instance_uid"],
                "series_instance_uid": seg_source["series_instance_uid"],
                "frame_numbers": None,
            },
        ],
        "expected_semantics": {
            "completion_flag": "COMPLETE",
            "preliminary_flag": "FINAL",
            "verification_flag": "UNVERIFIED",
            "root_template_identifier": "1500",
            "measurement_group_template_identifier": "1411",
            "tracking_identifier": tracking_identifier,
            "tracking_uid": tracking_uid,
            "observer_uid": observer_uid,
            "segment_number": SEGMENT_NUMBER,
            "source_frame_numbers": SOURCE_FRAMES,
            "measurement": {
                "name": {"value": "118565006", "scheme": "SCT", "meaning": "Volume"},
                "value": MEASUREMENT_VALUE,
                "unit": {"value": "mm3", "scheme": "UCUM", "meaning": "cubic millimeter"},
            },
            "procedure_reported": {
                "value": "25045-6",
                "scheme": "LN",
                "meaning": "CT unspecified body region",
            },
            "finding": {"value": "123037004", "scheme": "SCT", "meaning": "Body structure"},
            "evidence_sop_instance_uids": [
                ct_source["sop_instance_uid"],
                seg_source["sop_instance_uid"],
            ],
        },
        "payload_expectations": {
            "content_tree": "tid1500_tid1411_volume_referenced_segment",
            "pixel_data": "absent",
        },
    }
