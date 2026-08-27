from __future__ import annotations

import hashlib
import unittest

import numpy as np
from pydicom.dataset import Dataset

from dts_highdicom_backend.parametric_map import (
    FLOAT32_CASE_ID,
    FLOAT32_RECIPE_ID,
    FLOAT64_CASE_ID,
    FLOAT64_RECIPE_ID,
    FLOAT64_SPATIAL_RANK_INCREMENT,
    _float64_pixel_array,
    _float_pixel_array,
    _normalize_metadata,
    _recipe_for_request,
)
from dts_highdicom_backend.protocol import ProtocolError


class _Source:
    def __init__(self, values: list[list[int]]) -> None:
        self.pixel_array = np.asarray(values, dtype=np.int16)


class FloatPixelFormulaTest(unittest.TestCase):
    def test_formula_is_exact_and_spatial_rank_sensitive(self) -> None:
        sources = [
            ({}, _Source([[-1024, 0], [1024, 2047]])),
            ({}, _Source([[-1024, 0], [1024, 2047]])),
            ({}, _Source([[-1024, 0], [1024, 2047]])),
        ]
        pixels = _float_pixel_array(
            sources,  # type: ignore[arg-type]
            {"stored_value_scale": 0.25, "spatial_rank_increment": 0.25},
        )
        self.assertEqual(pixels.dtype, np.dtype("<f4"))
        self.assertEqual(
            pixels.view("<u4").reshape(3, 4).tolist(),
            [
                [3279945728, 0, 1132462080, 1140842496],
                [3279929344, 1048576000, 1132470272, 1140850688],
                [3279912960, 1056964608, 1132478464, 1140854784],
            ],
        )

    def test_float64_formula_has_exact_sub_float32_rank_distinctions(self) -> None:
        sources = [
            ({}, _Source([[-1024, 0], [1024, 2047]])),
            ({}, _Source([[-1024, 0], [1024, 2047]])),
            ({}, _Source([[-1024, 0], [1024, 2047]])),
        ]
        pixels = _float64_pixel_array(
            sources,  # type: ignore[arg-type]
            {
                "stored_value_scale": 0.25,
                "spatial_rank_increment": FLOAT64_SPATIAL_RANK_INCREMENT,
            },
        )

        self.assertEqual(pixels.dtype, np.dtype("<f8"))
        self.assertEqual(
            pixels.view("<u8").reshape(3, 4).tolist(),
            [
                [
                    13866583252673757184,
                    0,
                    4643211215818981376,
                    4647710417399840768,
                ],
                [
                    13866583252673724416,
                    4472074429978902528,
                    4643211215818997760,
                    4647710417399857152,
                ],
                [
                    13866583252673691648,
                    4476578029606273024,
                    4643211215819014144,
                    4647710417399873536,
                ],
            ],
        )
        self.assertEqual(
            [
                hashlib.sha256(frame.tobytes(order="C")).hexdigest()
                for frame in pixels
            ],
            [
                "ce1600d46bb7468f4a0f60c2d58cf96430234a89e50f0cacdd56bfd86bc3ec90",
                "be480ba76c1931f10052029005c539dd45b565f7020cc94a41a89825c3b6ea44",
                "921a8e74cc86e767d5436be2a4eb0c6d383bf3f210ec4c32e8f8c43c239f8abe",
            ],
        )
        self.assertNotEqual(pixels[0, 0, 0], pixels[1, 0, 0])
        self.assertEqual(
            np.float32(pixels[0, 0, 0]),
            np.float32(pixels[1, 0, 0]),
        )

    def test_float64_formula_rejects_noncanonical_rank_increment(self) -> None:
        sources = [
            ({}, _Source([[0]])),
            ({}, _Source([[0]])),
            ({}, _Source([[0]])),
        ]
        with self.assertRaisesRegex(ProtocolError, "must be 2\\^-30"):
            _float64_pixel_array(
                sources,  # type: ignore[arg-type]
                {"stored_value_scale": 0.25, "spatial_rank_increment": 0.25},
            )

    def test_recipe_dispatches_by_case_and_sample_type(self) -> None:
        float32 = _recipe_for_request(
            {
                "case": {
                    "case_id": FLOAT32_CASE_ID,
                    "recipe_id": FLOAT32_RECIPE_ID,
                },
                "parameters": {"sample_type": "float32"},
            }
        )
        float64 = _recipe_for_request(
            {
                "case": {
                    "case_id": FLOAT64_CASE_ID,
                    "recipe_id": FLOAT64_RECIPE_ID,
                },
                "parameters": {"sample_type": "float64"},
            }
        )

        self.assertEqual(float32.pixel_data_keyword, "FloatPixelData")
        self.assertEqual(float32.payload_vr, "OF")
        self.assertEqual(float32.output_relative_path, "parametric-map.dcm")
        self.assertEqual(float64.pixel_data_keyword, "DoubleFloatPixelData")
        self.assertEqual(float64.payload_vr, "OD")
        self.assertEqual(
            float64.output_relative_path,
            "parametric-map-float64.dcm",
        )

    def test_recipe_rejects_mismatched_sample_type(self) -> None:
        with self.assertRaisesRegex(ProtocolError, "requires sample_type float64"):
            _recipe_for_request(
                {
                    "case": {
                        "case_id": FLOAT64_CASE_ID,
                        "recipe_id": FLOAT64_RECIPE_ID,
                    },
                    "parameters": {"sample_type": "float32"},
                }
            )

    def test_normalized_series_laterality_is_valid_for_iod_validation(self) -> None:
        dataset = Dataset()
        dataset.ContributingEquipmentSequence = [Dataset()]
        _normalize_metadata(
            dataset,
            {
                "controlled_metadata": {
                    "patient_name": "DTS^Synthetic",
                    "patient_id": "DTS-PATIENT",
                    "manufacturer": "dicom-test-suite",
                    "model_name": "Parametric Map",
                    "software_versions": "0.1.0",
                    "study_date": "20260101",
                    "study_time": "000000",
                    "content_date": "20260101",
                    "content_time": "000000",
                    "timezone_offset_from_utc": "+0000",
                }
            },
        )

        self.assertEqual(dataset.Laterality, "R")
        self.assertEqual(dataset.SyntheticData, "YES")
        self.assertEqual(
            dataset.ContributingEquipmentSequence[0].ContributionDateTime,
            "20260101000000+0000",
        )


if __name__ == "__main__":
    unittest.main()
