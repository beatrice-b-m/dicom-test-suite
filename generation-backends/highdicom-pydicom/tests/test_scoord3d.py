from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

import pydicom
from pydicom.dataset import Dataset, FileDataset, FileMetaDataset
from pydicom.sequence import Sequence

from dts_highdicom_backend.__main__ import _generate
from dts_highdicom_backend.protocol import ProtocolError
from dts_highdicom_backend.scoord3d import (
    CASE_ID,
    COMPREHENSIVE_3D_SR_STORAGE,
    ENHANCED_CT_STORAGE,
    EXPLICIT_VR_LITTLE_ENDIAN,
    GRAPHIC_DATA_PATIENT_MM,
    OUTPUT_RELATIVE_PATH,
    RECIPE_ID,
    TRACKING_IDENTIFIER,
    generate,
)

STUDY_UID = "2.25.200"
FRAME_OF_REFERENCE_UID = "2.25.201"
CT_SERIES_UID = "2.25.202"
CT_SOP_UID = "2.25.203"
SR_SERIES_UID = "2.25.204"
SR_SOP_UID = "2.25.205"
TRACKING_UID = "2.25.206"
OBSERVER_UID = "2.25.207"
FIDUCIAL_UID = "2.25.208"


def _write_ct(path: Path) -> FileDataset:
    meta = FileMetaDataset()
    meta.MediaStorageSOPClassUID = ENHANCED_CT_STORAGE
    meta.MediaStorageSOPInstanceUID = CT_SOP_UID
    meta.TransferSyntaxUID = EXPLICIT_VR_LITTLE_ENDIAN
    meta.ImplementationClassUID = "2.25.999"
    ct = FileDataset(path, {}, file_meta=meta, preamble=b"\0" * 128)
    for keyword, value in (
        ("SOPClassUID", ENHANCED_CT_STORAGE),
        ("SOPInstanceUID", CT_SOP_UID),
        ("StudyInstanceUID", STUDY_UID),
        ("SeriesInstanceUID", CT_SERIES_UID),
        ("FrameOfReferenceUID", FRAME_OF_REFERENCE_UID),
        ("PatientName", "DTS^Synthetic^Patient001"),
        ("PatientID", "DTS-PATIENT-001"),
        ("PatientBirthDate", ""),
        ("PatientSex", ""),
        ("AccessionNumber", ""),
        ("StudyID", "DTS-STUDY"),
        ("StudyDate", "20260101"),
        ("StudyTime", "000000"),
        ("Modality", "CT"),
        ("NumberOfFrames", 2),
    ):
        setattr(ct, keyword, value)
    measures = Dataset()
    measures.PixelSpacing = [0.75, 0.75]
    measures.SliceThickness = 2.5
    measures.SpacingBetweenSlices = 2.5
    orientation = Dataset()
    orientation.ImageOrientationPatient = [1, 0, 0, 0, 1, 0]
    shared = Dataset()
    shared.PixelMeasuresSequence = Sequence([measures])
    shared.PlaneOrientationSequence = Sequence([orientation])
    ct.SharedFunctionalGroupsSequence = Sequence([shared])
    per_frame = []
    for position_value in GRAPHIC_DATA_PATIENT_MM:
        position = Dataset()
        position.ImagePositionPatient = position_value
        group = Dataset()
        group.PlanePositionSequence = Sequence([position])
        per_frame.append(group)
    ct.PerFrameFunctionalGroupsSequence = Sequence(per_frame)
    ct.save_as(path, enforce_file_format=True)
    return ct


def _request(root: Path) -> dict:
    path = root / "ct.dcm"
    ct = _write_ct(path)
    return {
        "case": {
            "case_id": CASE_ID,
            "recipe_id": RECIPE_ID,
            "expected_sop_class_uid": COMPREHENSIVE_3D_SR_STORAGE,
            "expected_transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN,
        },
        "staging": {"inputs_directory": str(root)},
        "identities": {
            "study_instance_uid": STUDY_UID,
            "series_instance_uid": SR_SERIES_UID,
            "frame_of_reference_uid": FRAME_OF_REFERENCE_UID,
            "sop_instances": [{"role": "primary", "index": 0, "uid": SR_SOP_UID}],
        },
        "controlled_metadata": {
            "patient_name": "DTS^Synthetic^Patient001",
            "patient_id": "DTS-PATIENT-001",
            "manufacturer": "dicom-test-suite",
            "model_name": "derived_sr_comprehensive3d_scoord3d",
            "software_versions": "0.1.0",
            "study_date": "20260101",
            "study_time": "000000",
            "content_date": "20260101",
            "content_time": "000000",
            "timezone_offset_from_utc": "+0000",
        },
        "sources": [{
            "role": "source_image",
            "source_case_id": "enhanced/ct/multiframe_shared_perframe_explicit_le",
            "relative_path": path.name,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "sop_class_uid": ENHANCED_CT_STORAGE,
            "sop_instance_uid": CT_SOP_UID,
            "series_instance_uid": CT_SERIES_UID,
            "frame_numbers": [1, 2],
        }],
        "parameters": {
            "tracking_identifier": TRACKING_IDENTIFIER,
            "tracking_uid": TRACKING_UID,
            "observer_uid": OBSERVER_UID,
            "fiducial_uid": FIDUCIAL_UID,
            "graphic_type": "POLYLINE",
            "graphic_data_patient_mm": [point.copy() for point in GRAPHIC_DATA_PATIENT_MM],
            "measurement_value_mm": 2.5,
        },
    }


def _content_items(dataset: Dataset) -> list[Dataset]:
    found: list[Dataset] = []
    pending = list(dataset.ContentSequence)
    while pending:
        item = pending.pop(0)
        found.append(item)
        pending.extend(getattr(item, "ContentSequence", []))
    return found


class Scoord3dGenerationTest(unittest.TestCase):
    def test_generates_exact_distance_coordinate_and_source_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = _request(root)
            response = _generate(request, root / "outputs")
            document = pydicom.dcmread(root / "outputs" / OUTPUT_RELATIVE_PATH)
            self.assertEqual(response["expected_semantics"]["fiducial_uid"], FIDUCIAL_UID)
            self.assertEqual(document.ContentTemplateSequence[0].TemplateIdentifier, "1500")
            self.assertEqual(document.InstanceCreationTime, "000000")
            items = _content_items(document)
            group = next(
                item for item in items
                if item.ValueType == "CONTAINER"
                and item.ConceptNameCodeSequence[0].CodeValue == "125007"
            )
            self.assertEqual(group.ContentTemplateSequence[0].TemplateIdentifier, "1501")
            distance = next(item for item in items if item.ValueType == "NUM")
            self.assertEqual(distance.MeasuredValueSequence[0].NumericValue, "2.5")
            scoord = next(item for item in items if item.ValueType == "SCOORD3D")
            self.assertEqual(scoord.RelationshipType, "INFERRED FROM")
            self.assertEqual(scoord.GraphicType, "POLYLINE")
            self.assertEqual(list(scoord.GraphicData), [0.0, 0.0, 0.0, 0.0, 0.0, 2.5])
            self.assertEqual(scoord.ReferencedFrameOfReferenceUID, FRAME_OF_REFERENCE_UID)
            self.assertEqual(scoord.FiducialUID, FIDUCIAL_UID)
            source = next(item for item in items if item.ValueType == "IMAGE")
            self.assertEqual(source.ConceptNameCodeSequence[0].CodeValue, "121112")
            self.assertEqual(
                list(source.ReferencedSOPSequence[0].ReferencedFrameNumber), [1, 2]
            )

    def test_rejects_coordinate_not_derived_from_source_positions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = _request(root)
            request["parameters"]["graphic_data_patient_mm"][1][2] = 3.0
            with self.assertRaisesRegex(ProtocolError, "differs from recipe"):
                generate(request, root / "outputs")


if __name__ == "__main__":
    unittest.main()
