from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import pydicom
from pydicom.dataset import Dataset, FileDataset, FileMetaDataset
from pydicom.sequence import Sequence

from dts_highdicom_backend.__main__ import _generate
from dts_highdicom_backend.protocol import ProtocolError
from dts_highdicom_backend.wsi_tile_segmentation import (
    CASE_ID,
    DIMENSION_INDEX_VALUES,
    DIMENSION_INDICES,
    EXPLICIT_VR_LITTLE_ENDIAN,
    FRAME_SHA256,
    FRAME_VALUES,
    OUTPUT_RELATIVE_PATH,
    PAYLOAD_SHA256,
    RECIPE_ID,
    RECONSTRUCTED_MATRIX_SHA256,
    SEGMENTATION_STORAGE,
    SOURCE_CASE_ID,
    WSI_STORAGE,
    generate,
    reconstructed_total_pixel_matrix,
)

STUDY_UID = "2.25.200"
SOURCE_SERIES_UID = "2.25.201"
SOURCE_SOP_UID = "2.25.202"
FRAME_OF_REFERENCE_UID = "2.25.203"
SPECIMEN_UID = "2.25.204"
SOURCE_DIMENSION_UID = "2.25.205"
SEG_SERIES_UID = "2.25.206"
SEG_SOP_UID = "2.25.207"
SEG_DIMENSION_UID = "2.25.208"


def _item(**values: object) -> Dataset:
    item = Dataset()
    for keyword, value in values.items():
        setattr(item, keyword, value)
    return item


def _write_source(path: Path) -> FileDataset:
    meta = FileMetaDataset()
    meta.MediaStorageSOPClassUID = WSI_STORAGE
    meta.MediaStorageSOPInstanceUID = SOURCE_SOP_UID
    meta.TransferSyntaxUID = EXPLICIT_VR_LITTLE_ENDIAN
    meta.ImplementationClassUID = "2.25.999"
    source = FileDataset(path, {}, file_meta=meta, preamble=b"\0" * 128)
    source.SOPClassUID = WSI_STORAGE
    source.SOPInstanceUID = SOURCE_SOP_UID
    source.PatientName = "DTS^Synthetic^Patient001"
    source.PatientID = "DTS-PATIENT-001"
    source.PatientBirthDate = "19700101"
    source.PatientSex = "O"
    source.StudyInstanceUID = STUDY_UID
    source.StudyDate = "20260101"
    source.StudyTime = "000000"
    source.ReferringPhysicianName = ""
    source.StudyID = "DTS-WSI"
    source.AccessionNumber = ""
    source.SeriesInstanceUID = SOURCE_SERIES_UID
    source.SeriesNumber = 41
    source.Modality = "SM"
    source.FrameOfReferenceUID = FRAME_OF_REFERENCE_UID
    source.PositionReferenceIndicator = "SLIDE_CORNER"
    source.Manufacturer = "dicom-test-suite"
    source.ManufacturerModelName = "Native TILED_FULL WSI"
    source.DeviceSerialNumber = "DTS-WSI-001"
    source.SoftwareVersions = "0.1.0"
    source.ImageType = ["ORIGINAL", "PRIMARY", "VOLUME", "NONE"]
    source.BurnedInAnnotation = "NO"
    source.LossyImageCompression = "00"
    source.Rows = 2
    source.Columns = 2
    source.NumberOfFrames = 4
    source.SamplesPerPixel = 3
    source.PhotometricInterpretation = "RGB"
    source.PlanarConfiguration = 0
    source.BitsAllocated = 8
    source.BitsStored = 8
    source.HighBit = 7
    source.PixelRepresentation = 0
    source.TotalPixelMatrixRows = 4
    source.TotalPixelMatrixColumns = 4
    source.TotalPixelMatrixFocalPlanes = 1
    source.TotalPixelMatrixOriginSequence = Sequence(
        [
            _item(
                XOffsetInSlideCoordinateSystem="0",
                YOffsetInSlideCoordinateSystem="0",
                ZOffsetInSlideCoordinateSystem="0",
            )
        ]
    )
    source.ImageOrientationSlide = [1, 0, 0, 0, 1, 0]
    source.DimensionOrganizationType = "TILED_FULL"
    source.DimensionOrganizationSequence = Sequence(
        [_item(DimensionOrganizationUID=SOURCE_DIMENSION_UID)]
    )
    measures = _item(PixelSpacing=[0.5, 0.5], SliceThickness="0.001")
    frame_type = _item(FrameType=["ORIGINAL", "PRIMARY", "VOLUME", "NONE"])
    source.SharedFunctionalGroupsSequence = Sequence(
        [
            _item(
                PixelMeasuresSequence=Sequence([measures]),
                WholeSlideMicroscopyImageFrameTypeSequence=Sequence([frame_type]),
            )
        ]
    )
    source.ContainerIdentifier = "DTS-SLIDE-001"
    source.IssuerOfTheContainerIdentifierSequence = Sequence([])
    source.ContainerTypeCodeSequence = Sequence([])
    specimen = _item(
        SpecimenIdentifier="DTS-SPECIMEN-001",
        SpecimenUID=SPECIMEN_UID,
        IssuerOfTheSpecimenIdentifierSequence=Sequence([]),
        SpecimenPreparationSequence=Sequence([]),
    )
    source.SpecimenDescriptionSequence = Sequence([specimen])
    colors = ([255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255])
    source.PixelData = bytes(channel for color in colors for channel in color * 4)
    source.save_as(path, enforce_file_format=True)
    return source


def _request(root: Path) -> dict:
    source_path = root / "source.dcm"
    source = _write_source(source_path)
    return {
        "case": {
            "case_id": CASE_ID,
            "recipe_id": RECIPE_ID,
            "expected_sop_class_uid": SEGMENTATION_STORAGE,
            "expected_transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN,
        },
        "staging": {"inputs_directory": str(root)},
        "identities": {
            "study_instance_uid": STUDY_UID,
            "series_instance_uid": SEG_SERIES_UID,
            "frame_of_reference_uid": FRAME_OF_REFERENCE_UID,
            "sop_instances": [{"role": "primary", "index": 0, "uid": SEG_SOP_UID}],
        },
        "controlled_metadata": {
            "patient_name": "DTS^Synthetic^Patient001",
            "patient_id": "DTS-PATIENT-001",
            "manufacturer": "dicom-test-suite",
            "model_name": RECIPE_ID,
            "software_versions": "0.1.0",
            "study_date": "20260101",
            "study_time": "000000",
            "content_date": "20260101",
            "content_time": "000000",
            "timezone_offset_from_utc": "+0000",
        },
        "sources": [
            {
                "role": "source_image",
                "source_case_id": SOURCE_CASE_ID,
                "relative_path": source_path.name,
                "sha256": hashlib.sha256(source_path.read_bytes()).hexdigest(),
                "sop_class_uid": WSI_STORAGE,
                "sop_instance_uid": SOURCE_SOP_UID,
                "series_instance_uid": SOURCE_SERIES_UID,
                "frame_numbers": [1, 4],
            }
        ],
        "parameters": {
            "dimension_organization_uid": SEG_DIMENSION_UID,
            "segmentation_type": "FRACTIONAL",
            "fractional_type": "OCCUPANCY",
            "maximum_fractional_value": 255,
        },
    }


class WsiTileSegmentationTest(unittest.TestCase):
    def test_dispatches_exact_case(self) -> None:
        request = {"case": {"case_id": CASE_ID}}
        expected = {"relative_path": OUTPUT_RELATIVE_PATH}
        with patch(
            "dts_highdicom_backend.__main__.generate_wsi_tile_segmentation",
            return_value=expected,
        ) as operation:
            actual = _generate(request, Path("outputs"))
        self.assertIs(actual, expected)
        operation.assert_called_once_with(request, Path("outputs"))

    def test_locked_hash_constants_reconstruct_exact_matrix(self) -> None:
        frames = [bytes(values) for values in FRAME_VALUES]
        self.assertEqual(
            [hashlib.sha256(frame).hexdigest() for frame in frames],
            list(FRAME_SHA256),
        )
        self.assertEqual(hashlib.sha256(b"".join(frames)).hexdigest(), PAYLOAD_SHA256)
        matrix = reconstructed_total_pixel_matrix()
        self.assertEqual(matrix.shape, (4, 4))
        self.assertEqual(hashlib.sha256(matrix.tobytes()).hexdigest(), RECONSTRUCTED_MATRIX_SHA256)

    def test_generates_exact_sparse_segmentation_and_response(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = _request(root)
            output_root = root / "outputs"
            response = generate(request, output_root)
            output_path = output_root / OUTPUT_RELATIVE_PATH
            segmentation = pydicom.dcmread(output_path)

            self.assertLessEqual(output_path.stat().st_size, 16 * 1024)
            self.assertEqual(str(segmentation.SOPClassUID), SEGMENTATION_STORAGE)
            self.assertEqual(str(segmentation.SOPInstanceUID), SEG_SOP_UID)
            self.assertEqual(str(segmentation.StudyInstanceUID), STUDY_UID)
            self.assertEqual(str(segmentation.FrameOfReferenceUID), FRAME_OF_REFERENCE_UID)
            self.assertEqual(str(segmentation.SeriesInstanceUID), SEG_SERIES_UID)
            self.assertEqual(segmentation.DimensionOrganizationType, "TILED_SPARSE")
            self.assertEqual(segmentation.SegmentationType, "FRACTIONAL")
            self.assertEqual(segmentation.SegmentationFractionalType, "OCCUPANCY")
            self.assertEqual(segmentation.MaximumFractionalValue, 255)
            self.assertEqual(segmentation.SegmentsOverlap, "NO")
            segment = segmentation.SegmentSequence[0]
            category = segment.SegmentedPropertyCategoryCodeSequence[0]
            property_type = segment.SegmentedPropertyTypeCodeSequence[0]
            self.assertEqual(
                (category.CodeValue, category.CodingSchemeDesignator, category.CodeMeaning),
                ("85756007", "SCT", "Tissue"),
            )
            self.assertEqual(
                (property_type.CodeValue, property_type.CodingSchemeDesignator, property_type.CodeMeaning),
                ("113343", "DCM", "Organ"),
            )
            self.assertEqual(segment.SegmentAlgorithmType, "MANUAL")
            self.assertNotIn("SegmentationAlgorithmIdentificationSequence", segment)
            self.assertEqual(int(segmentation.NumberOfFrames), 2)
            self.assertEqual(bytes(segmentation.PixelData), bytes(sum(FRAME_VALUES, ())))
            self.assertEqual(
                [pydicom.datadict.keyword_for_tag(item.DimensionIndexPointer) for item in segmentation.DimensionIndexSequence],
                list(DIMENSION_INDICES),
            )
            self.assertEqual(
                {str(item.DimensionOrganizationUID) for item in segmentation.DimensionIndexSequence},
                {SEG_DIMENSION_UID},
            )
            self.assertEqual(
                {str(item.DimensionOrganizationUID) for item in segmentation.DimensionOrganizationSequence},
                {SEG_DIMENSION_UID},
            )
            shared = segmentation.SharedFunctionalGroupsSequence[0]
            self.assertEqual(
                {element.keyword for element in shared},
                {"PixelMeasuresSequence", "SegmentIdentificationSequence"},
            )
            self.assertEqual(shared.SegmentIdentificationSequence[0].ReferencedSegmentNumber, 1)
            self.assertEqual(segmentation.ContainerIdentifier, "DTS-SLIDE-001")
            self.assertEqual(segmentation.SpecimenDescriptionSequence[0].SpecimenUID, SPECIMEN_UID)

            for frame, expected_values, source_frame in zip(
                segmentation.PerFrameFunctionalGroupsSequence,
                DIMENSION_INDEX_VALUES,
                [1, 4],
                strict=True,
            ):
                self.assertEqual(
                    {element.keyword for element in frame},
                    {"FrameContentSequence", "PlanePositionSlideSequence", "DerivationImageSequence"},
                )
                self.assertEqual(
                    tuple(frame.FrameContentSequence[0].DimensionIndexValues),
                    expected_values,
                )
                derivation = frame.DerivationImageSequence[0]
                source_item = derivation.SourceImageSequence[0]
                self.assertEqual(source_item.ReferencedSOPClassUID, WSI_STORAGE)
                self.assertEqual(source_item.ReferencedSOPInstanceUID, SOURCE_SOP_UID)
                self.assertEqual(source_item.ReferencedFrameNumber, source_frame)
                self.assertEqual(source_item.SpatialLocationsPreserved, "YES")
                purpose = source_item.PurposeOfReferenceCodeSequence[0]
                self.assertEqual((purpose.CodeValue, purpose.CodingSchemeDesignator), ("121322", "DCM"))
                code = derivation.DerivationCodeSequence[0]
                self.assertEqual((code.CodeValue, code.CodingSchemeDesignator), ("113076", "DCM"))

            referenced_series = segmentation.ReferencedSeriesSequence
            self.assertEqual(len(referenced_series), 1)
            self.assertEqual(referenced_series[0].SeriesInstanceUID, SOURCE_SERIES_UID)
            self.assertEqual(len(referenced_series[0].ReferencedInstanceSequence), 1)
            self.assertEqual(referenced_series[0].ReferencedInstanceSequence[0].ReferencedSOPInstanceUID, SOURCE_SOP_UID)
            for absent in (
                "PlaneOrientationSequence",
                "ICCProfile",
                "PixelPaddingValue",
                "PyramidUID",
                "ConcatenationUID",
                "TrackingID",
                "TrackingUID",
            ):
                self.assertNotIn(absent, segmentation)

            self.assertEqual(
                response["references"],
                [
                    {
                        "role": "source_image",
                        "relationship": "derivation",
                        "sop_class_uid": WSI_STORAGE,
                        "sop_instance_uid": SOURCE_SOP_UID,
                        "series_instance_uid": SOURCE_SERIES_UID,
                        "frame_numbers": [1, 4],
                    }
                ],
            )
            semantics = response["expected_semantics"]
            self.assertEqual(semantics["dimension_indices"], list(DIMENSION_INDICES))
            self.assertEqual(semantics["dimension_index_values"], [[1, 1, 1], [1, 2, 2]])
            self.assertEqual(response["payload_expectations"]["frame_values"], [list(values) for values in FRAME_VALUES])
            self.assertEqual(response["payload_expectations"]["payload_sha256"], PAYLOAD_SHA256)

    def test_rejects_wrong_source_frame_contract_before_generation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = _request(root)
            request["sources"][0]["frame_numbers"] = [1, 2, 3, 4]
            with self.assertRaisesRegex(ProtocolError, "source frames mismatch"):
                generate(request, root / "outputs")


if __name__ == "__main__":
    unittest.main()
