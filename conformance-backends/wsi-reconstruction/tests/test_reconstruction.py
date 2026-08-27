from __future__ import annotations

import copy
import tempfile
import unittest
from pathlib import Path

import numpy as np
from pydicom.dataset import Dataset, FileDataset, FileMetaDataset
from pydicom.sequence import Sequence

from dts_wsi_reconstruction.__main__ import (
    MATRIX_HASH,
    SPARSE_MATRIX_HASH,
    SPARSE_OCCUPANCY_MASK,
    ReconstructionError,
    reconstruct,
)


def _dataset(path: Path) -> FileDataset:
    meta = FileMetaDataset()
    meta.MediaStorageSOPClassUID = "1.2.840.10008.5.1.4.1.1.77.1.6"
    meta.MediaStorageSOPInstanceUID = "2.25.1"
    meta.TransferSyntaxUID = "1.2.840.10008.1.2.1"
    meta.ImplementationClassUID = "2.25.2"
    ds = FileDataset(path, {}, file_meta=meta, preamble=b"\0" * 128)
    ds.SOPClassUID = meta.MediaStorageSOPClassUID
    ds.SOPInstanceUID = meta.MediaStorageSOPInstanceUID
    ds.StudyInstanceUID = "2.25.3"
    ds.SeriesInstanceUID = "2.25.4"
    ds.FrameOfReferenceUID = "2.25.5"
    ds.PatientName = "DTS^Synthetic"
    ds.PatientID = "DTS"
    ds.StudyDate = "20260101"
    ds.StudyTime = "000000"
    ds.StudyID = "WSI"
    ds.AccessionNumber = ""
    ds.ReferringPhysicianName = ""
    ds.Modality = "SM"
    ds.SeriesNumber = 1
    ds.InstanceNumber = 1
    ds.ImageType = ["ORIGINAL", "PRIMARY", "VOLUME", "NONE"]
    ds.DimensionOrganizationType = "TILED_FULL"
    ds.LossyImageCompression = "00"
    ds.Rows = 2
    ds.Columns = 2
    ds.NumberOfFrames = 4
    ds.TotalPixelMatrixRows = 4
    ds.TotalPixelMatrixColumns = 4
    ds.NumberOfOpticalPaths = 1
    ds.TotalPixelMatrixFocalPlanes = 1
    ds.SamplesPerPixel = 3
    ds.PhotometricInterpretation = "RGB"
    ds.PlanarConfiguration = 0
    ds.BitsAllocated = 8
    ds.BitsStored = 8
    ds.HighBit = 7
    ds.PixelRepresentation = 0
    ds.ImagedVolumeWidth = 2.0
    ds.ImagedVolumeHeight = 2.0
    ds.ImagedVolumeDepth = 0.001
    ds.ImageOrientationSlide = [1, 0, 0, 0, 1, 0]
    origin = Dataset()
    origin.XOffsetInSlideCoordinateSystem = 0
    origin.YOffsetInSlideCoordinateSystem = 0
    ds.TotalPixelMatrixOriginSequence = Sequence([origin])
    organization = Dataset()
    organization.DimensionOrganizationUID = "2.25.6"
    ds.DimensionOrganizationSequence = Sequence([organization])
    specimen = Dataset()
    specimen.SpecimenIdentifier = "DTS-SPECIMEN-001"
    specimen.SpecimenUID = "2.25.7"
    ds.SpecimenDescriptionSequence = Sequence([specimen])
    optical = Dataset()
    optical.OpticalPathIdentifier = "RGB"
    optical.IlluminationWaveLength = 550
    ds.OpticalPathSequence = Sequence([optical])
    shared = Dataset()
    measures = Dataset()
    measures.PixelSpacing = [0.5, 0.5]
    measures.SliceThickness = 0.001
    shared.PixelMeasuresSequence = Sequence([measures])
    frame_type = Dataset()
    frame_type.FrameType = ["ORIGINAL", "PRIMARY", "VOLUME", "NONE"]
    shared.WholeSlideMicroscopyImageFrameTypeSequence = Sequence([frame_type])
    ds.SharedFunctionalGroupsSequence = Sequence([shared])
    colors = ([255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255])
    frames = np.asarray([[color] * 4 for color in colors], dtype=np.uint8).reshape(
        4, 2, 2, 3
    )
    ds.PixelData = frames.tobytes()
    return ds


def _sparse_dataset(path: Path) -> FileDataset:
    ds = _dataset(path)
    ds.DimensionOrganizationType = "TILED_SPARSE"
    ds.NumberOfFrames = 2
    dimension_uid = str(ds.DimensionOrganizationSequence[0].DimensionOrganizationUID)
    dimension_items = []
    for pointer, label in (
        (0x0048021E, "Column Position"),
        (0x0048021F, "Row Position"),
    ):
        item = Dataset()
        item.DimensionOrganizationUID = dimension_uid
        item.DimensionIndexPointer = pointer
        item.FunctionalGroupPointer = 0x0048021A
        item.DimensionDescriptionLabel = label
        dimension_items.append(item)
    ds.DimensionIndexSequence = Sequence(dimension_items)
    per_frame_items = []
    for column, row, values, x, y in (
        (1, 1, [1, 1], 0.0, 0.0),
        (3, 3, [2, 2], 1.0, 1.0),
    ):
        per_frame = Dataset()
        frame_content = Dataset()
        frame_content.DimensionIndexValues = values
        per_frame.FrameContentSequence = Sequence([frame_content])
        plane = Dataset()
        plane.ColumnPositionInTotalImagePixelMatrix = column
        plane.RowPositionInTotalImagePixelMatrix = row
        plane.XOffsetInSlideCoordinateSystem = x
        plane.YOffsetInSlideCoordinateSystem = y
        plane.ZOffsetInSlideCoordinateSystem = 0.0
        per_frame.PlanePositionSlideSequence = Sequence([plane])
        optical_path = Dataset()
        optical_path.OpticalPathIdentifier = "RGB"
        per_frame.OpticalPathIdentificationSequence = Sequence([optical_path])
        per_frame_items.append(per_frame)
    ds.PerFrameFunctionalGroupsSequence = Sequence(per_frame_items)
    colors = ([255, 0, 0], [255, 255, 255])
    frames = np.asarray([[color] * 4 for color in colors], dtype=np.uint8).reshape(
        2, 2, 2, 3
    )
    ds.PixelData = frames.tobytes()
    return ds


class ReconstructionTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.path = Path(self.temp.name) / "wsi.dcm"
        _dataset(self.path).save_as(self.path, enforce_file_format=True)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_reconstructs_exact_total_pixel_matrix(self) -> None:
        result = reconstruct(self.path)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["total_pixel_matrix_sha256"], MATRIX_HASH)
        self.assertFalse(result["transforms_applied"])
        self.assertEqual(
            [
                (p["column_position"], p["row_position"])
                for p in result["implicit_frame_positions"]
            ],
            [(1, 1), (3, 1), (1, 3), (3, 3)],
        )

    def test_rejects_explicit_per_frame_positions(self) -> None:
        ds = copy.deepcopy(_dataset(self.path))
        ds.PerFrameFunctionalGroupsSequence = Sequence([Dataset()] * 4)
        ds.save_as(self.path, enforce_file_format=True)
        with self.assertRaisesRegex(ReconstructionError, "implicit positions"):
            reconstruct(self.path)

    def test_rejects_pixel_mutation(self) -> None:
        ds = copy.deepcopy(_dataset(self.path))
        pixels = bytearray(ds.PixelData)
        pixels[0] = 254
        ds.PixelData = bytes(pixels)
        ds.save_as(self.path, enforce_file_format=True)
        with self.assertRaisesRegex(ReconstructionError, "stored frame hashes"):
            reconstruct(self.path)

    def test_rejects_geometry_mutation(self) -> None:
        ds = copy.deepcopy(_dataset(self.path))
        ds.TotalPixelMatrixColumns = 6
        ds.save_as(self.path, enforce_file_format=True)
        with self.assertRaisesRegex(ReconstructionError, "TotalPixelMatrixColumns"):
            reconstruct(self.path)

    def test_rejects_position_metadata_mutation(self) -> None:
        ds = copy.deepcopy(_dataset(self.path))
        ds.SharedFunctionalGroupsSequence[0].PixelMeasuresSequence[0].PixelSpacing = [
            0.5,
            0.6,
        ]
        ds.save_as(self.path, enforce_file_format=True)
        with self.assertRaisesRegex(ReconstructionError, "pixel spacing"):
            reconstruct(self.path)


class SparseReconstructionTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.path = Path(self.temp.name) / "wsi-sparse.dcm"
        _sparse_dataset(self.path).save_as(self.path, enforce_file_format=True)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_reconstructs_only_encoded_tiles_with_explicit_positions(self) -> None:
        result = reconstruct(self.path)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["dimension_organization_type"], "TILED_SPARSE")
        self.assertEqual(result["occupancy_mask"], SPARSE_OCCUPANCY_MASK)
        self.assertEqual(result["total_pixel_matrix_sha256"], SPARSE_MATRIX_HASH)
        self.assertEqual(
            [
                (p["column_position"], p["row_position"])
                for p in result["explicit_frame_positions"]
            ],
            [(1, 1), (3, 3)],
        )
        self.assertEqual(
            result["absent_tile_positions"],
            [
                {"column_position": 3, "row_position": 1},
                {"column_position": 1, "row_position": 3},
            ],
        )
        self.assertFalse(result["transforms_applied"])

    def test_rejects_dimension_pointer_mutation(self) -> None:
        ds = copy.deepcopy(_sparse_dataset(self.path))
        ds.DimensionIndexSequence[0].DimensionIndexPointer = 0x0048021F
        ds.save_as(self.path, enforce_file_format=True)
        with self.assertRaisesRegex(
            ReconstructionError, "dimension index item 1 pointer"
        ):
            reconstruct(self.path)

    def test_rejects_dimension_index_value_mutation(self) -> None:
        ds = copy.deepcopy(_sparse_dataset(self.path))
        ds.PerFrameFunctionalGroupsSequence[1].FrameContentSequence[
            0
        ].DimensionIndexValues = [2, 1]
        ds.save_as(self.path, enforce_file_format=True)
        with self.assertRaisesRegex(
            ReconstructionError, "frame 2 dimension index values"
        ):
            reconstruct(self.path)

    def test_rejects_explicit_position_mutation(self) -> None:
        ds = copy.deepcopy(_sparse_dataset(self.path))
        ds.PerFrameFunctionalGroupsSequence[1].PlanePositionSlideSequence[
            0
        ].RowPositionInTotalImagePixelMatrix = 1
        ds.save_as(self.path, enforce_file_format=True)
        with self.assertRaisesRegex(ReconstructionError, "frame 2 row position"):
            reconstruct(self.path)

    def test_rejects_sparse_payload_mutation(self) -> None:
        ds = copy.deepcopy(_sparse_dataset(self.path))
        pixels = bytearray(ds.PixelData)
        pixels[-1] = 254
        ds.PixelData = bytes(pixels)
        ds.save_as(self.path, enforce_file_format=True)
        with self.assertRaisesRegex(
            ReconstructionError, "sparse Pixel Data payload hash"
        ):
            reconstruct(self.path)


if __name__ == "__main__":
    unittest.main()
