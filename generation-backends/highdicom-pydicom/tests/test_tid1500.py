from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import pydicom
from pydicom.dataset import Dataset, FileDataset, FileMetaDataset
from pydicom.sequence import Sequence

from dts_highdicom_backend.protocol import ProtocolError
from dts_highdicom_backend.__main__ import _generate
from dts_highdicom_backend.parametric_map import FLOAT32_CASE_ID
from dts_highdicom_backend.tid1500 import (
    CASE_ID,
    COMPREHENSIVE_3D_SR_STORAGE,
    ENHANCED_CT_STORAGE,
    EXPLICIT_VR_LITTLE_ENDIAN,
    MEASUREMENT_VALUE,
    OUTPUT_RELATIVE_PATH,
    RECIPE_ID,
    SEGMENTATION_STORAGE,
    TRACKING_IDENTIFIER,
    _normalize_source_image_meaning,
    generate,
)


STUDY_UID = "2.25.100"
FRAME_OF_REFERENCE_UID = "2.25.101"
CT_SERIES_UID = "2.25.102"
CT_SOP_UID = "2.25.103"
SEG_SERIES_UID = "2.25.104"
SEG_SOP_UID = "2.25.105"
SR_SERIES_UID = "2.25.106"
SR_SOP_UID = "2.25.107"
TRACKING_UID = "2.25.108"
OBSERVER_UID = "2.25.109"


def _base_dataset(path: Path, sop_class_uid: str, sop_instance_uid: str) -> FileDataset:
    meta = FileMetaDataset()
    meta.MediaStorageSOPClassUID = sop_class_uid
    meta.MediaStorageSOPInstanceUID = sop_instance_uid
    meta.TransferSyntaxUID = EXPLICIT_VR_LITTLE_ENDIAN
    meta.ImplementationClassUID = "2.25.999"
    dataset = FileDataset(path, {}, file_meta=meta, preamble=b"\0" * 128)
    dataset.SOPClassUID = sop_class_uid
    dataset.SOPInstanceUID = sop_instance_uid
    dataset.StudyInstanceUID = STUDY_UID
    dataset.FrameOfReferenceUID = FRAME_OF_REFERENCE_UID
    dataset.PatientName = "DTS^Synthetic^Patient001"
    dataset.PatientID = "DTS-PATIENT-001"
    dataset.PatientBirthDate = ""
    dataset.PatientSex = ""
    dataset.AccessionNumber = ""
    dataset.StudyID = "DTS-STUDY"
    dataset.StudyDate = "20260101"
    dataset.StudyTime = "000000"
    return dataset


def _write_sources(root: Path) -> tuple[Path, Path]:
    ct_path = root / "ct.dcm"
    ct = _base_dataset(ct_path, ENHANCED_CT_STORAGE, CT_SOP_UID)
    ct.SeriesInstanceUID = CT_SERIES_UID
    ct.Modality = "CT"
    ct.NumberOfFrames = 2
    pixel_measures = Dataset()
    pixel_measures.PixelSpacing = [0.75, 0.75]
    pixel_measures.SliceThickness = 2.5
    shared = Dataset()
    shared.PixelMeasuresSequence = Sequence([pixel_measures])
    ct.SharedFunctionalGroupsSequence = Sequence([shared])
    ct.save_as(ct_path, enforce_file_format=True)

    seg_path = root / "seg.dcm"
    seg = _base_dataset(seg_path, SEGMENTATION_STORAGE, SEG_SOP_UID)
    seg.SeriesInstanceUID = SEG_SERIES_UID
    seg.Modality = "SEG"
    seg.NumberOfFrames = 2
    seg.SegmentationType = "BINARY"
    segment = Dataset()
    segment.SegmentNumber = 1
    seg.SegmentSequence = Sequence([segment])
    functional_groups = []
    for frame_number in [1, 2]:
        identification = Dataset()
        identification.ReferencedSegmentNumber = 1
        source = Dataset()
        source.ReferencedSOPClassUID = ENHANCED_CT_STORAGE
        source.ReferencedSOPInstanceUID = CT_SOP_UID
        source.ReferencedFrameNumber = frame_number
        derivation = Dataset()
        derivation.SourceImageSequence = Sequence([source])
        group = Dataset()
        group.SegmentIdentificationSequence = Sequence([identification])
        group.DerivationImageSequence = Sequence([derivation])
        functional_groups.append(group)
    seg.PerFrameFunctionalGroupsSequence = Sequence(functional_groups)
    seg.save_as(seg_path, enforce_file_format=True)
    return ct_path, seg_path


def _source(path: Path, role: str, dataset: Dataset, frames: list[int] | None) -> dict:
    return {
        "role": role,
        "source_case_id": "source-case",
        "relative_path": path.name,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "sop_class_uid": str(dataset.SOPClassUID),
        "sop_instance_uid": str(dataset.SOPInstanceUID),
        "series_instance_uid": str(dataset.SeriesInstanceUID),
        "frame_numbers": frames,
    }


def _request(root: Path) -> dict:
    ct_path, seg_path = _write_sources(root)
    ct = pydicom.dcmread(ct_path)
    seg = pydicom.dcmread(seg_path)
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
            "sop_instances": [
                {"role": "primary", "index": 0, "uid": SR_SOP_UID},
            ],
        },
        "controlled_metadata": {
            "patient_name": "DTS^Synthetic^Patient001",
            "patient_id": "DTS-PATIENT-001",
            "manufacturer": "dicom-test-suite",
            "model_name": "derived_sr_tid1500_ct_measurement_report",
            "software_versions": "0.1.0",
            "study_date": "20260101",
            "study_time": "000000",
            "content_date": "20260101",
            "content_time": "000000",
            "timezone_offset_from_utc": "+0000",
        },
        "sources": [
            _source(ct_path, "source_image", ct, [1, 2]),
            _source(seg_path, "segmentation", seg, None),
        ],
        "parameters": {
            "segment_number": 1,
            "measurement_value": MEASUREMENT_VALUE,
            "tracking_identifier": TRACKING_IDENTIFIER,
            "tracking_uid": TRACKING_UID,
            "observer_uid": OBSERVER_UID,
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


class Tid1500GenerationTest(unittest.TestCase):
    def test_dispatch_preserves_parametric_map_operation(self) -> None:
        request = {"case": {"case_id": FLOAT32_CASE_ID}}
        expected = {"relative_path": "parametric-map.dcm"}
        with patch(
            "dts_highdicom_backend.__main__.generate_parametric_map",
            return_value=expected,
        ) as operation:
            actual = _generate(request, Path("outputs"))
        self.assertIs(actual, expected)
        operation.assert_called_once_with(request, Path("outputs"))

    def test_generates_locked_measurement_and_reference_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = _request(root)
            output_root = root / "outputs"
            response = generate(request, output_root)
            document = pydicom.dcmread(output_root / OUTPUT_RELATIVE_PATH)

            self.assertEqual(response["sop_instance_uid"], SR_SOP_UID)
            self.assertEqual(
                response["expected_semantics"]["measurement"]["value"],
                MEASUREMENT_VALUE,
            )
            self.assertEqual(document.SOPClassUID, COMPREHENSIVE_3D_SR_STORAGE)
            self.assertEqual(document.CompletionFlag, "COMPLETE")
            self.assertEqual(document.PreliminaryFlag, "FINAL")
            self.assertEqual(document.VerificationFlag, "UNVERIFIED")
            self.assertEqual(document.ContentTemplateSequence[0].TemplateIdentifier, "1500")
            self.assertEqual(document.InstanceCreationDate, "20260101")
            self.assertEqual(document.InstanceCreationTime, "000000")
            self.assertEqual(
                {
                    str(item.ContributionDateTime)
                    for item in document.ContributingEquipmentSequence
                },
                {"20260101000000+0000"},
            )

            items = _content_items(document)
            volume = next(
                item
                for item in items
                if item.ValueType == "NUM"
                and item.ConceptNameCodeSequence[0].CodeValue == "118565006"
            )
            self.assertEqual(volume.MeasuredValueSequence[0].NumericValue, "5.625")
            referenced_segment = next(
                item
                for item in items
                if item.ValueType == "IMAGE"
                and item.ConceptNameCodeSequence[0].CodeValue == "121191"
            ).ReferencedSOPSequence[0]
            self.assertEqual(referenced_segment.ReferencedSegmentNumber, 1)
            self.assertFalse(hasattr(referenced_segment, "ReferencedFrameNumber"))
            source_image = next(
                item
                for item in items
                if item.ValueType == "IMAGE"
                and item.ConceptNameCodeSequence[0].CodeValue == "121233"
            )
            self.assertEqual(
                source_image.ConceptNameCodeSequence[0].CodeMeaning,
                "Source image for segmentation",
            )
            self.assertEqual(
                list(source_image.ReferencedSOPSequence[0].ReferencedFrameNumber),
                [1, 2],
            )

            evidence = {
                str(instance.ReferencedSOPInstanceUID)
                for study in document.CurrentRequestedProcedureEvidenceSequence
                for series in study.ReferencedSeriesSequence
                for instance in series.ReferencedSOPSequence
            }
            self.assertEqual(evidence, {CT_SOP_UID, SEG_SOP_UID})

    def test_rejects_noncanonical_source_order(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = _request(root)
            request["sources"].reverse()
            with self.assertRaisesRegex(ProtocolError, "ordered as CT then SEG"):
                generate(request, root / "outputs")

    def test_normalizes_highdicom_source_image_code_meaning(self) -> None:
        root = Dataset()
        item = Dataset()
        concept = Dataset()
        concept.CodeValue = "121233"
        concept.CodingSchemeDesignator = "DCM"
        concept.CodeMeaning = "Source Image for Segmentation"
        item.ConceptNameCodeSequence = Sequence([concept])
        root.ContentSequence = Sequence([item])
        _normalize_source_image_meaning(root)
        self.assertEqual(concept.CodeMeaning, "Source image for segmentation")


if __name__ == "__main__":
    unittest.main()
