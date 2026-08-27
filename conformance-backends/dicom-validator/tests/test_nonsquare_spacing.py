from __future__ import annotations

import io
import json
import shutil
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

from pydicom.dataset import FileDataset, FileMetaDataset
from pydicom.uid import ExplicitVRLittleEndian, SecondaryCaptureImageStorage

from dts_dicom_validator_adapter.__main__ import extract_nonsquare_spacing


PIXELS = bytes(
    [0, 255, 0, 255, 0, 255, 255, 0, 255, 0, 255, 0] * 2
)


class NonsquareSpacingTests(unittest.TestCase):
    def write_dataset(self, variant: str, *, forbidden: bool = False) -> Path:
        meta = FileMetaDataset()
        meta.MediaStorageSOPClassUID = SecondaryCaptureImageStorage
        meta.MediaStorageSOPInstanceUID = "2.25.1"
        meta.TransferSyntaxUID = ExplicitVRLittleEndian
        meta.ImplementationClassUID = "2.25.2"
        dataset = FileDataset(None, {}, file_meta=meta, preamble=b"\0" * 128)
        dataset.SOPClassUID = SecondaryCaptureImageStorage
        dataset.SOPInstanceUID = "2.25.1"
        dataset.Rows = 4
        dataset.Columns = 6
        dataset.SamplesPerPixel = 1
        dataset.PhotometricInterpretation = "MONOCHROME2"
        dataset.BitsAllocated = 8
        dataset.BitsStored = 8
        dataset.HighBit = 7
        dataset.PixelRepresentation = 0
        dataset.PixelData = PIXELS
        dataset[0x7FE00010].VR = "OB"
        if variant in ("pixel_spacing", "mixed"):
            dataset.PixelSpacing = ["0.6", "0.3"]
            dataset.NominalScannedPixelSpacing = ["0.6", "0.3"]
        if variant in ("pixel_aspect_ratio", "mixed"):
            dataset.PixelAspectRatio = [2, 1]
        if forbidden:
            dataset.FrameOfReferenceUID = "2.25.9"
        directory = Path(tempfile.mkdtemp(prefix="dts-nonsquare-adapter-"))
        path = directory / f"{variant}.dcm"
        dataset.save_as(path, enforce_file_format=True)
        self.addCleanup(shutil.rmtree, directory, True)
        return path

    def extract(self, path: Path) -> dict:
        output = io.StringIO()
        with (
            patch(
                "dts_dicom_validator_adapter.__main__.verify_distribution"
            ),
            patch("dts_dicom_validator_adapter.__main__.verify_standard"),
            redirect_stdout(output),
        ):
            self.assertEqual(
                extract_nonsquare_spacing(path, Path("unused"), Path("unused")),
                0,
            )
        return json.loads(output.getvalue())

    def test_extracts_both_exclusive_variants(self) -> None:
        spacing = self.extract(self.write_dataset("pixel_spacing"))
        self.assertEqual(spacing["variant_id"], "pixel_spacing")
        self.assertEqual(spacing["pixel_spacing"]["values"], ["0.6", "0.3"])
        self.assertEqual(spacing["pixel_spacing"]["vr"], "DS")
        self.assertEqual(spacing["pixel_spacing"]["vm"], 2)
        self.assertIsNone(spacing["pixel_aspect_ratio"])

        aspect = self.extract(self.write_dataset("pixel_aspect_ratio"))
        self.assertEqual(aspect["variant_id"], "pixel_aspect_ratio")
        self.assertEqual(aspect["pixel_aspect_ratio"]["values"], ["2", "1"])
        self.assertEqual(aspect["pixel_aspect_ratio"]["vr"], "IS")
        self.assertEqual(aspect["pixel_aspect_ratio"]["vm"], 2)
        self.assertIsNone(aspect["pixel_spacing"])
        self.assertEqual(
            aspect["pixel_data_sha256"],
            "e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6",
        )

    def test_rejects_mixed_and_patient_space_geometry(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "exactly one"):
            self.extract(self.write_dataset("mixed"))
        with self.assertRaisesRegex(RuntimeError, "frame_of_reference_uid"):
            self.extract(self.write_dataset("pixel_spacing", forbidden=True))


if __name__ == "__main__":
    unittest.main()
