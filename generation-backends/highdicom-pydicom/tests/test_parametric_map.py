from __future__ import annotations

import unittest

import numpy as np

from dts_highdicom_backend.parametric_map import _float_pixel_array


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


if __name__ == "__main__":
    unittest.main()
