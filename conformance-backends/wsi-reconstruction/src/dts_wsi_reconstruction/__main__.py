"""Fail-closed independent reconstruction of the locked small TILED_FULL WSI."""

from __future__ import annotations

import argparse
import hashlib
import json
from importlib.metadata import version
from pathlib import Path
from typing import Any

import highdicom as hd
import numpy as np
import pydicom

from . import __version__

WSI_STORAGE = "1.2.840.10008.5.1.4.1.1.77.1.6"
EXPLICIT_VR_LITTLE_ENDIAN = "1.2.840.10008.1.2.1"
FRAME_HASHES = [
    "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
    "6c8f6d772829d493618e079a099cf4f20d8524ed3656f49db234f5bbf60a4e65",
    "7263ad3fd60c6620abd423516d748baedf5e393b1fbdaaf780ff5803a443cc4f",
    "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21",
]
MATRIX_HASH = "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a"


class ReconstructionError(RuntimeError):
    """The input does not satisfy the exact locked reconstruction contract."""


def _sha256(array: np.ndarray) -> str:
    return hashlib.sha256(np.ascontiguousarray(array).tobytes()).hexdigest()


def _require_equal(actual: Any, expected: Any, name: str) -> None:
    if actual != expected:
        raise ReconstructionError(f"{name} mismatch: expected {expected!r}, got {actual!r}")


def _validate_shape(dataset: pydicom.Dataset) -> None:
    _require_equal(str(dataset.SOPClassUID), WSI_STORAGE, "SOP Class UID")
    _require_equal(str(dataset.file_meta.TransferSyntaxUID), EXPLICIT_VR_LITTLE_ENDIAN, "transfer syntax")
    for keyword, expected in (
        ("Modality", "SM"),
        ("DimensionOrganizationType", "TILED_FULL"),
        ("PhotometricInterpretation", "RGB"),
        ("LossyImageCompression", "00"),
    ):
        _require_equal(str(getattr(dataset, keyword)), expected, keyword)
    for keyword, expected in (
        ("Rows", 2),
        ("Columns", 2),
        ("NumberOfFrames", 4),
        ("TotalPixelMatrixRows", 4),
        ("TotalPixelMatrixColumns", 4),
        ("NumberOfOpticalPaths", 1),
        ("TotalPixelMatrixFocalPlanes", 1),
        ("SamplesPerPixel", 3),
        ("PlanarConfiguration", 0),
        ("BitsAllocated", 8),
        ("BitsStored", 8),
        ("HighBit", 7),
        ("PixelRepresentation", 0),
    ):
        _require_equal(int(getattr(dataset, keyword)), expected, keyword)
    if "DimensionIndexSequence" in dataset or "PerFrameFunctionalGroupsSequence" in dataset:
        raise ReconstructionError("TILED_FULL input must use implicit positions without per-frame or dimension-index sequences")
    _require_equal(len(dataset.DimensionOrganizationSequence), 1, "dimension organization item count")
    _require_equal(len(dataset.SpecimenDescriptionSequence), 1, "specimen item count")
    _require_equal(len(dataset.OpticalPathSequence), 1, "optical path item count")
    _require_equal(str(dataset.OpticalPathSequence[0].OpticalPathIdentifier), "RGB", "optical path identifier")


def reconstruct(path: Path) -> dict[str, Any]:
    dataset = pydicom.dcmread(path)
    _validate_shape(dataset)

    image = hd.Image.from_dataset(dataset, copy=True)
    frames = [image.get_stored_frame(number, as_index=False) for number in range(1, 5)]
    for number, frame in enumerate(frames, start=1):
        _require_equal(list(frame.shape), [2, 2, 3], f"frame {number} shape")
    frame_hashes = [_sha256(frame.astype(np.uint8, copy=False)) for frame in frames]
    _require_equal(frame_hashes, FRAME_HASHES, "stored frame hashes")

    # Derive the canonical implicit order without consulting generator code:
    # columns vary fastest, followed by rows, focal planes, and optical paths.
    reconstructed = np.empty((4, 4, 3), dtype=np.uint8)
    positions: list[dict[str, int | float | str]] = []
    for index, frame in enumerate(frames):
        tile_row, tile_column = divmod(index, 2)
        row = tile_row * 2
        column = tile_column * 2
        reconstructed[row : row + 2, column : column + 2, :] = frame
        positions.append(
            {
                "frame_number": index + 1,
                "optical_path_identifier": "RGB",
                "focal_plane": 1,
                "column_position": column + 1,
                "row_position": row + 1,
                "x_mm": float(column) * 0.5,
                "y_mm": float(row) * 0.5,
                "z_mm": 0.0,
            }
        )

    matrix_hash = _sha256(reconstructed)
    _require_equal(matrix_hash, MATRIX_HASH, "independently reconstructed matrix hash")
    library_matrix = image.get_total_pixel_matrix(
        dtype=np.uint8,
        apply_real_world_transform=False,
        apply_modality_transform=False,
        apply_voi_transform=False,
        apply_presentation_lut=False,
        apply_palette_color_lut=False,
        apply_icc_profile=False,
    )
    _require_equal(list(library_matrix.shape), [4, 4, 3], "highdicom matrix shape")
    _require_equal(_sha256(library_matrix), MATRIX_HASH, "highdicom matrix hash")
    if not np.array_equal(reconstructed, library_matrix):
        raise ReconstructionError("independent and highdicom total pixel matrices differ")

    return {
        "status": "passed",
        "backend": "dts-wsi-reconstruct",
        "backend_version": __version__,
        "runtime": {
            "highdicom": version("highdicom"),
            "numpy": version("numpy"),
            "pydicom": version("pydicom"),
        },
        "frame_hashes": frame_hashes,
        "implicit_frame_positions": positions,
        "total_pixel_matrix_shape": [4, 4, 3],
        "total_pixel_matrix_sha256": matrix_hash,
        "transforms_applied": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path)
    parser.add_argument("--version", action="store_true")
    args = parser.parse_args()
    if args.version:
        print(f"dts-wsi-reconstruct {__version__}")
        return 0
    if args.input is None:
        parser.error("--input is required")
    try:
        result = reconstruct(args.input)
    except Exception as error:
        print(json.dumps({"status": "failed", "error": f"{type(error).__name__}: {error}"}, sort_keys=True))
        return 1
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
