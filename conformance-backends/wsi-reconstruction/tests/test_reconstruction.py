from __future__ import annotations

import copy
import tempfile
import unittest
from pathlib import Path

import numpy as np
from pydicom.dataset import Dataset, FileDataset, FileMetaDataset
from pydicom.sequence import Sequence

from dts_wsi_reconstruction.__main__ import MATRIX_HASH, ReconstructionError, reconstruct


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
    frames = np.asarray([[color] * 4 for color in colors], dtype=np.uint8).reshape(4, 2, 2, 3)
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
            [(p["column_position"], p["row_position"]) for p in result["implicit_frame_positions"]],
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
        ds.SharedFunctionalGroupsSequence[0].PixelMeasuresSequence[0].PixelSpacing = [0.5, 0.6]
        ds.save_as(self.path, enforce_file_format=True)
        with self.assertRaisesRegex(ReconstructionError, "pixel spacing"):
            reconstruct(self.path)


if __name__ == "__main__":
    unittest.main()
