"""Fail-closed independent reconstruction of the locked small WSI cases."""

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
from pydicom.tag import Tag

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
SPARSE_FRAME_HASHES = [FRAME_HASHES[0], FRAME_HASHES[3]]
SPARSE_PAYLOAD_HASH = "94a57aca44c4a97d424e8e546b2673fa91f711694de1ccb36f062aabbc9b55ee"
SPARSE_MATRIX_HASH = "d10a587875f14a0b74a9e4935ce83cdb73377bd7357a172db8e9f7347c030eb3"
SPARSE_OCCUPANCY_MASK = [True, False, False, True]


class ReconstructionError(RuntimeError):
    """The input does not satisfy the exact locked reconstruction contract."""


def _sha256(array: np.ndarray) -> str:
    return hashlib.sha256(np.ascontiguousarray(array).tobytes()).hexdigest()


def _require_equal(actual: Any, expected: Any, name: str) -> None:
    if actual != expected:
        raise ReconstructionError(
            f"{name} mismatch: expected {expected!r}, got {actual!r}"
        )


def _validate_common(
    dataset: pydicom.Dataset,
    dimension_organization_type: str,
    number_of_frames: int,
) -> tuple[list[float], list[float], list[float]]:
    _require_equal(str(dataset.SOPClassUID), WSI_STORAGE, "SOP Class UID")
    _require_equal(
        str(dataset.file_meta.TransferSyntaxUID),
        EXPLICIT_VR_LITTLE_ENDIAN,
        "transfer syntax",
    )
    for keyword, expected in (
        ("Modality", "SM"),
        ("DimensionOrganizationType", dimension_organization_type),
        ("PhotometricInterpretation", "RGB"),
        ("LossyImageCompression", "00"),
    ):
        _require_equal(str(getattr(dataset, keyword)), expected, keyword)
    for keyword, expected in (
        ("Rows", 2),
        ("Columns", 2),
        ("NumberOfFrames", number_of_frames),
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
    _require_equal(
        len(dataset.DimensionOrganizationSequence),
        1,
        "dimension organization item count",
    )
    _require_equal(len(dataset.SpecimenDescriptionSequence), 1, "specimen item count")
    _require_equal(len(dataset.OpticalPathSequence), 1, "optical path item count")
    _require_equal(
        str(dataset.OpticalPathSequence[0].OpticalPathIdentifier),
        "RGB",
        "optical path identifier",
    )
    _require_equal(
        len(dataset.SharedFunctionalGroupsSequence),
        1,
        "shared functional groups item count",
    )
    measures = dataset.SharedFunctionalGroupsSequence[0].PixelMeasuresSequence[0]
    spacing = [float(value) for value in measures.PixelSpacing]
    _require_equal(spacing, [0.5, 0.5], "pixel spacing")
    orientation = [float(value) for value in dataset.ImageOrientationSlide]
    _require_equal(
        orientation, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0], "image orientation slide"
    )
    origin_item = dataset.TotalPixelMatrixOriginSequence[0]
    origin = [
        float(origin_item.XOffsetInSlideCoordinateSystem),
        float(origin_item.YOffsetInSlideCoordinateSystem),
        float(getattr(origin_item, "ZOffsetInSlideCoordinateSystem", 0.0)),
    ]
    _require_equal(origin, [0.0, 0.0, 0.0], "total pixel matrix origin")
    return spacing, orientation, origin


def _validate_shape(
    dataset: pydicom.Dataset,
) -> tuple[list[float], list[float], list[float]]:
    geometry = _validate_common(dataset, "TILED_FULL", 4)
    if (
        "DimensionIndexSequence" in dataset
        or "PerFrameFunctionalGroupsSequence" in dataset
    ):
        raise ReconstructionError(
            "TILED_FULL input must use implicit positions without per-frame or dimension-index sequences"
        )
    return geometry


def _require_keywords(dataset: pydicom.Dataset, expected: set[str], name: str) -> None:
    actual = {element.keyword for element in dataset}
    _require_equal(actual, expected, f"{name} attributes")


def _validate_sparse_dimensions(dataset: pydicom.Dataset) -> str:
    _require_equal(len(dataset.DimensionIndexSequence), 2, "dimension index item count")
    organization_uid = str(
        dataset.DimensionOrganizationSequence[0].DimensionOrganizationUID
    )
    expected = (
        (Tag(0x0048021E), "Column Position"),
        (Tag(0x0048021F), "Row Position"),
    )
    for number, (item, (pointer, label)) in enumerate(
        zip(dataset.DimensionIndexSequence, expected, strict=True), start=1
    ):
        _require_keywords(
            item,
            {
                "DimensionOrganizationUID",
                "DimensionIndexPointer",
                "FunctionalGroupPointer",
                "DimensionDescriptionLabel",
            },
            f"dimension index item {number}",
        )
        _require_equal(
            Tag(item.DimensionIndexPointer),
            pointer,
            f"dimension index item {number} pointer",
        )
        _require_equal(
            Tag(item.FunctionalGroupPointer),
            Tag(0x0048021A),
            f"dimension index item {number} functional group pointer",
        )
        _require_equal(
            str(item.DimensionOrganizationUID),
            organization_uid,
            f"dimension index item {number} organization UID",
        )
        _require_equal(
            str(item.DimensionDescriptionLabel),
            label,
            f"dimension index item {number} label",
        )
    return organization_uid


def _sparse_positions(
    dataset: pydicom.Dataset,
    spacing: list[float],
    orientation: list[float],
    origin: list[float],
) -> list[dict[str, int | float | str]]:
    _require_equal(
        len(dataset.PerFrameFunctionalGroupsSequence),
        2,
        "per-frame functional group item count",
    )
    expected = (
        (1, 1, [1, 1], 0.0, 0.0, 0.0),
        (3, 3, [2, 2], 1.0, 1.0, 0.0),
    )
    positions: list[dict[str, int | float | str]] = []
    for frame_number, (item, values) in enumerate(
        zip(dataset.PerFrameFunctionalGroupsSequence, expected, strict=True), start=1
    ):
        column, row, dimension_values, expected_x, expected_y, expected_z = values
        _require_keywords(
            item,
            {
                "FrameContentSequence",
                "PlanePositionSlideSequence",
                "OpticalPathIdentificationSequence",
            },
            f"per-frame item {frame_number}",
        )
        _require_equal(
            len(item.FrameContentSequence),
            1,
            f"frame {frame_number} frame content count",
        )
        _require_equal(
            [int(value) for value in item.FrameContentSequence[0].DimensionIndexValues],
            dimension_values,
            f"frame {frame_number} dimension index values",
        )
        _require_equal(
            len(item.PlanePositionSlideSequence),
            1,
            f"frame {frame_number} plane position count",
        )
        plane = item.PlanePositionSlideSequence[0]
        actual_column = int(plane.ColumnPositionInTotalImagePixelMatrix)
        actual_row = int(plane.RowPositionInTotalImagePixelMatrix)
        _require_equal(actual_column, column, f"frame {frame_number} column position")
        _require_equal(actual_row, row, f"frame {frame_number} row position")
        actual_x = float(plane.XOffsetInSlideCoordinateSystem)
        actual_y = float(plane.YOffsetInSlideCoordinateSystem)
        actual_z = float(plane.ZOffsetInSlideCoordinateSystem)
        _require_equal(actual_x, expected_x, f"frame {frame_number} X offset")
        _require_equal(actual_y, expected_y, f"frame {frame_number} Y offset")
        _require_equal(actual_z, expected_z, f"frame {frame_number} Z offset")
        zero_based_column = column - 1
        zero_based_row = row - 1
        derived_x = (
            origin[0]
            + zero_based_column * spacing[1] * orientation[0]
            + zero_based_row * spacing[0] * orientation[3]
        )
        derived_y = (
            origin[1]
            + zero_based_column * spacing[1] * orientation[1]
            + zero_based_row * spacing[0] * orientation[4]
        )
        derived_z = (
            origin[2]
            + zero_based_column * spacing[1] * orientation[2]
            + zero_based_row * spacing[0] * orientation[5]
        )
        _require_equal(
            [actual_x, actual_y, actual_z],
            [derived_x, derived_y, derived_z],
            f"frame {frame_number} derived slide position",
        )
        _require_equal(
            len(item.OpticalPathIdentificationSequence),
            1,
            f"frame {frame_number} optical path count",
        )
        optical_path = str(
            item.OpticalPathIdentificationSequence[0].OpticalPathIdentifier
        )
        _require_equal(
            optical_path, "RGB", f"frame {frame_number} optical path identifier"
        )
        positions.append(
            {
                "frame_number": frame_number,
                "optical_path_identifier": optical_path,
                "column_position": column,
                "row_position": row,
                "x_mm": actual_x,
                "y_mm": actual_y,
                "z_mm": actual_z,
                "dimension_index_values": dimension_values,
            }
        )
    return positions


def _reconstruct_tiled_full(dataset: pydicom.Dataset) -> dict[str, Any]:
    spacing, orientation, origin = _validate_shape(dataset)
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
    tiles_per_row = int(dataset.TotalPixelMatrixColumns) // int(dataset.Columns)
    for index, frame in enumerate(frames):
        tile_row, tile_column = divmod(index, tiles_per_row)
        row = tile_row * int(dataset.Rows)
        column = tile_column * int(dataset.Columns)
        reconstructed[
            row : row + int(dataset.Rows),
            column : column + int(dataset.Columns),
            :,
        ] = frame
        x_mm = (
            origin[0]
            + column * spacing[1] * orientation[0]
            + row * spacing[0] * orientation[3]
        )
        y_mm = (
            origin[1]
            + column * spacing[1] * orientation[1]
            + row * spacing[0] * orientation[4]
        )
        z_mm = (
            origin[2]
            + column * spacing[1] * orientation[2]
            + row * spacing[0] * orientation[5]
        )
        positions.append(
            {
                "frame_number": index + 1,
                "optical_path_identifier": "RGB",
                "focal_plane": 1,
                "column_position": column + 1,
                "row_position": row + 1,
                "x_mm": x_mm,
                "y_mm": y_mm,
                "z_mm": z_mm,
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
        raise ReconstructionError(
            "independent and highdicom total pixel matrices differ"
        )

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


def _reconstruct_tiled_sparse(dataset: pydicom.Dataset) -> dict[str, Any]:
    spacing, orientation, origin = _validate_common(dataset, "TILED_SPARSE", 2)
    organization_uid = _validate_sparse_dimensions(dataset)
    positions = _sparse_positions(dataset, spacing, orientation, origin)
    _require_equal(
        hashlib.sha256(bytes(dataset.PixelData)).hexdigest(),
        SPARSE_PAYLOAD_HASH,
        "sparse Pixel Data payload hash",
    )

    image = hd.Image.from_dataset(dataset, copy=True)
    frames = [image.get_stored_frame(number, as_index=False) for number in range(1, 3)]
    for number, frame in enumerate(frames, start=1):
        _require_equal(list(frame.shape), [2, 2, 3], f"frame {number} shape")
    frame_hashes = [_sha256(frame.astype(np.uint8, copy=False)) for frame in frames]
    _require_equal(frame_hashes, SPARSE_FRAME_HASHES, "stored frame hashes")

    reconstructed = np.zeros((4, 4, 3), dtype=np.uint8)
    occupancy = [False, False, False, False]
    tiles_per_row = int(dataset.TotalPixelMatrixColumns) // int(dataset.Columns)
    for frame, position in zip(frames, positions, strict=True):
        row = int(position["row_position"]) - 1
        column = int(position["column_position"]) - 1
        tile_index = row // int(dataset.Rows) * tiles_per_row + column // int(
            dataset.Columns
        )
        if occupancy[tile_index]:
            raise ReconstructionError(
                f"duplicate sparse tile position at occupancy index {tile_index}"
            )
        occupancy[tile_index] = True
        reconstructed[
            row : row + int(dataset.Rows), column : column + int(dataset.Columns), :
        ] = frame
    _require_equal(occupancy, SPARSE_OCCUPANCY_MASK, "sparse occupancy mask")
    matrix_hash = _sha256(reconstructed)
    _require_equal(
        matrix_hash, SPARSE_MATRIX_HASH, "sentinel-zero reconstructed matrix hash"
    )

    return {
        "status": "passed",
        "backend": "dts-wsi-reconstruct",
        "backend_version": __version__,
        "runtime": {
            "highdicom": version("highdicom"),
            "numpy": version("numpy"),
            "pydicom": version("pydicom"),
        },
        "dimension_organization_type": "TILED_SPARSE",
        "dimension_organization_uid": organization_uid,
        "dimension_index_pointers": ["0048021E", "0048021F"],
        "functional_group_pointer": "0048021A",
        "frame_hashes": frame_hashes,
        "explicit_frame_positions": positions,
        "occupancy_mask": occupancy,
        "absent_tile_positions": [
            {"column_position": 3, "row_position": 1},
            {"column_position": 1, "row_position": 3},
        ],
        "pixel_data_sha256": SPARSE_PAYLOAD_HASH,
        "total_pixel_matrix_shape": [4, 4, 3],
        "total_pixel_matrix_sha256": matrix_hash,
        "missing_pixel_sentinel": [0, 0, 0],
        "transforms_applied": False,
    }


def reconstruct(path: Path) -> dict[str, Any]:
    dataset = pydicom.dcmread(path)
    dimension_type = str(getattr(dataset, "DimensionOrganizationType", ""))
    if dimension_type == "TILED_FULL":
        return _reconstruct_tiled_full(dataset)
    if dimension_type == "TILED_SPARSE":
        return _reconstruct_tiled_sparse(dataset)
    raise ReconstructionError(
        f"DimensionOrganizationType mismatch: expected 'TILED_FULL' or 'TILED_SPARSE', got {dimension_type!r}"
    )


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
        print(
            json.dumps(
                {"status": "failed", "error": f"{type(error).__name__}: {error}"},
                sort_keys=True,
            )
        )
        return 1
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
