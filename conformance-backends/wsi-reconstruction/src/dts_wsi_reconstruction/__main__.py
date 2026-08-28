"""Fail-closed independent reconstruction of the locked WSI cases."""

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
from highdicom.spatial import iter_tiled_full_frame_data
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
VOLUME_PAYLOAD_HASH = "b40b0afc9b180d5ebfb54a7db428e13fe09a33dcc9a8f76220f395ba2c68d2db"
THUMBNAIL_HASH = "6733cdd08e5c7ef0453e2759ef0d28fbd43ea2aa7883b55422a13dac38e23ecc"
LABEL_HASH = "ad078f83d3ea66f075867d116c8c126e9c8a8a9dd873cd27280371c173d8ad02"
ICC_PROFILE_HASH = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef"
MULTI_PATH_IDENTIFIERS = ["BRIGHTFIELD", "ALTERNATE"]
MULTI_PATH_DESCRIPTIONS = [
    "Deterministic brightfield path",
    "Deterministic alternate path",
]
MULTI_PATH_WAVELENGTHS = [550.0, 650.0]
MULTI_PATH_FRAME_HASHES = [
    FRAME_HASHES,
    [
        "f7606fde280d9577c963618cc2a8fa52b15315ff63ec185029cf66bda64435ab",
        "81fd180e1f66d28018580f37d46188c02fd6709f875b3b620090718a8847c282",
        "745598fdcfa2650299b59b42f40c0750087e117d6bc236c66486087cd264ebd8",
        "15ec7bf0b50732b49f8228e07d24365338f9e3ab994b00af08e5a3bffe55fd8b",
    ],
]
MULTI_PATH_PAYLOAD_HASHES = [
    VOLUME_PAYLOAD_HASH,
    "1f7ee233e83aebb2127b56d5d728f9ca2df9170ec4eb24e929dca261f9badbed",
]
MULTI_PATH_MATRIX_HASHES = [
    MATRIX_HASH,
    "caa1a1abb84ec283bbf92a0f00d5bd89650420d0b1fa911e191ddb368f50e09f",
]
MULTI_PATH_AGGREGATE_HASH = (
    "831fe6e50cbc3f3d82e3f57c984d3c273cdb18dd3bd3ab511b3633dc293f708f"
)
GROUP_ROLES = ("volume", "thumbnail", "label")
ROLE_IMAGE_TYPES = {
    "volume": ["ORIGINAL", "PRIMARY", "VOLUME", "NONE"],
    "thumbnail": ["DERIVED", "PRIMARY", "THUMBNAIL", "RESAMPLED"],
    "label": ["ORIGINAL", "PRIMARY", "LABEL", "NONE"],
}


class ReconstructionError(RuntimeError):
    """The input does not satisfy the exact locked reconstruction contract."""


def _sha256(array: np.ndarray) -> str:
    return hashlib.sha256(np.ascontiguousarray(array).tobytes()).hexdigest()


def _require_equal(actual: Any, expected: Any, name: str) -> None:
    if actual != expected:
        raise ReconstructionError(
            f"{name} mismatch: expected {expected!r}, got {actual!r}"
        )


def _require_float_close(actual: float, expected: float, name: str) -> None:
    if not np.isclose(actual, expected, rtol=0.0, atol=1e-9):
        raise ReconstructionError(
            f"{name} mismatch: expected {expected!r}, got {actual!r}"
        )


def _validate_common(
    dataset: pydicom.Dataset,
    dimension_organization_type: str,
    number_of_frames: int,
    number_of_optical_paths: int = 1,
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
        ("NumberOfOpticalPaths", number_of_optical_paths),
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
    _require_equal(
        len(dataset.OpticalPathSequence),
        number_of_optical_paths,
        "optical path item count",
    )
    if number_of_optical_paths == 1:
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


def _validate_multiple_optical_path_structure(
    dataset: pydicom.Dataset,
) -> tuple[list[float], list[float], list[float]]:
    geometry = _validate_common(dataset, "TILED_FULL", 8, 2)
    if (
        "DimensionIndexSequence" in dataset
        or "PerFrameFunctionalGroupsSequence" in dataset
    ):
        raise ReconstructionError(
            "multiple-optical-path TILED_FULL input must use implicit positions "
            "without per-frame or dimension-index sequences"
        )
    for keyword in (
        "SpacingBetweenSlices",
        "NumberOfFocalPlanes",
        "DistanceBetweenFocalPlanes",
        "PyramidUID",
        "ConcatenationUID",
        "ReferencedSeriesSequence",
        "ICCProfile",
    ):
        if keyword in dataset:
            raise ReconstructionError(f"top-level {keyword} must be absent")
    for keyword, expected in (
        ("ImageType", ROLE_IMAGE_TYPES["volume"]),
        ("BurnedInAnnotation", "NO"),
        ("SpecimenLabelInImage", "NO"),
        ("FocusMethod", "AUTO"),
        ("ExtendedDepthOfField", "NO"),
        ("PositionReferenceIndicator", "SLIDE_CORNER"),
    ):
        actual = getattr(dataset, keyword)
        if keyword == "ImageType":
            actual = [str(value) for value in actual]
        else:
            actual = str(actual)
        _require_equal(actual, expected, keyword)
    for keyword, expected in (
        ("ImagedVolumeWidth", 2.0),
        ("ImagedVolumeHeight", 2.0),
        ("ImagedVolumeDepth", 0.001),
    ):
        _require_float_close(float(getattr(dataset, keyword)), expected, keyword)
    shared = dataset.SharedFunctionalGroupsSequence[0]
    _require_keywords(
        shared,
        {"PixelMeasuresSequence", "WholeSlideMicroscopyImageFrameTypeSequence"},
        "shared functional groups item",
    )
    _require_equal(
        [
            str(value)
            for value in shared.WholeSlideMicroscopyImageFrameTypeSequence[0].FrameType
        ],
        ROLE_IMAGE_TYPES["volume"],
        "shared WSI FrameType",
    )
    _require_float_close(
        float(shared.PixelMeasuresSequence[0].SliceThickness),
        0.001,
        "SliceThickness",
    )

    actual_identifiers: list[str] = []
    for ordinal, (item, identifier, description, wavelength) in enumerate(
        zip(
            dataset.OpticalPathSequence,
            MULTI_PATH_IDENTIFIERS,
            MULTI_PATH_DESCRIPTIONS,
            MULTI_PATH_WAVELENGTHS,
            strict=True,
        ),
        start=1,
    ):
        _require_keywords(
            item,
            {
                "OpticalPathIdentifier",
                "OpticalPathDescription",
                "IlluminationWaveLength",
                "IlluminationTypeCodeSequence",
                "ICCProfile",
                "ColorSpace",
            },
            f"optical path item {ordinal}",
        )
        actual_identifier = str(item.OpticalPathIdentifier)
        actual_description = str(item.OpticalPathDescription)
        actual_wavelength = float(item.IlluminationWaveLength)
        _require_equal(
            actual_identifier, identifier, f"optical path item {ordinal} identifier"
        )
        _require_equal(
            actual_description,
            description,
            f"optical path item {ordinal} description",
        )
        _require_float_close(
            actual_wavelength,
            wavelength,
            f"optical path item {ordinal} illumination wavelength",
        )
        _require_equal(
            len(item.IlluminationTypeCodeSequence),
            1,
            f"optical path item {ordinal} illumination type count",
        )
        illumination = item.IlluminationTypeCodeSequence[0]
        _require_equal(
            [
                str(illumination.CodeValue),
                str(illumination.CodingSchemeDesignator),
                str(illumination.CodeMeaning),
            ],
            ["111744", "DCM", "Brightfield illumination"],
            f"optical path item {ordinal} illumination type",
        )
        icc_hash = hashlib.sha256(bytes(item.ICCProfile)).hexdigest()
        _require_equal(
            icc_hash, ICC_PROFILE_HASH, f"optical path item {ordinal} ICC Profile hash"
        )
        _require_equal(
            str(item.ColorSpace),
            "SRGB",
            f"optical path item {ordinal} ColorSpace",
        )
        actual_identifiers.append(actual_identifier)
    _require_equal(
        len(set(actual_identifiers)),
        len(actual_identifiers),
        "unique optical path identifier count",
    )
    return geometry


def _reconstruct_multiple_optical_paths(
    dataset: pydicom.Dataset,
) -> dict[str, Any]:
    _validate_multiple_optical_path_structure(dataset)
    aggregate_hash = hashlib.sha256(bytes(dataset.PixelData)).hexdigest()
    _require_equal(
        aggregate_hash,
        MULTI_PATH_AGGREGATE_HASH,
        "multiple-optical-path Pixel Data aggregate hash",
    )

    image = hd.Image.from_dataset(dataset, copy=True)
    frames = [
        image.get_stored_frame(number, as_index=False).astype(np.uint8, copy=False)
        for number in range(1, 9)
    ]
    for number, frame in enumerate(frames, start=1):
        _require_equal(list(frame.shape), [2, 2, 3], f"frame {number} shape")
    frame_hashes = [_sha256(frame) for frame in frames]
    expected_frame_hashes = [
        frame_hash
        for path_hashes in MULTI_PATH_FRAME_HASHES
        for frame_hash in path_hashes
    ]
    _require_equal(frame_hashes, expected_frame_hashes, "stored frame hashes")

    implicit = list(iter_tiled_full_frame_data(dataset))
    _require_equal(len(implicit), 8, "implicit frame position count")
    matrices = [np.empty((4, 4, 3), dtype=np.uint8) for _ in range(2)]
    occupancy = [[False, False, False, False] for _ in range(2)]
    positions: list[dict[str, int | float | str]] = []
    per_path_frames: list[list[np.ndarray]] = [[], []]
    for frame_number, (frame, position) in enumerate(
        zip(frames, implicit, strict=True), start=1
    ):
        channel, focal_plane, column, row, x_mm, y_mm, z_mm = position
        if channel is None:
            raise ReconstructionError(
                f"frame {frame_number} optical path ordinal must not be absent"
            )
        path_index = int(channel) - 1
        if path_index not in (0, 1):
            raise ReconstructionError(
                f"frame {frame_number} optical path ordinal is out of range: {channel}"
            )
        expected_path_index = (frame_number - 1) // 4
        _require_equal(
            path_index,
            expected_path_index,
            f"frame {frame_number} optical path order",
        )
        _require_equal(int(focal_plane), 1, f"frame {frame_number} focal plane")
        tile_index = (frame_number - 1) % 4
        expected_column = 1 + (tile_index % 2) * 2
        expected_row = 1 + (tile_index // 2) * 2
        _require_equal(int(column), expected_column, f"frame {frame_number} column")
        _require_equal(int(row), expected_row, f"frame {frame_number} row")
        expected_x = float(expected_column - 1) * 0.5
        expected_y = float(expected_row - 1) * 0.5
        _require_float_close(float(x_mm), expected_x, f"frame {frame_number} X")
        _require_float_close(float(y_mm), expected_y, f"frame {frame_number} Y")
        _require_float_close(float(z_mm), 0.0, f"frame {frame_number} Z")
        if occupancy[path_index][tile_index]:
            raise ReconstructionError(
                f"duplicate tile {tile_index + 1} for optical path {channel}"
            )
        occupancy[path_index][tile_index] = True
        zero_row = expected_row - 1
        zero_column = expected_column - 1
        matrices[path_index][
            zero_row : zero_row + 2, zero_column : zero_column + 2
        ] = frame
        per_path_frames[path_index].append(frame)
        positions.append(
            {
                "frame_number": frame_number,
                "optical_path_ordinal": int(channel),
                "optical_path_identifier": MULTI_PATH_IDENTIFIERS[path_index],
                "focal_plane": int(focal_plane),
                "column_position": int(column),
                "row_position": int(row),
                "x_mm": float(x_mm),
                "y_mm": float(y_mm),
                "z_mm": float(z_mm),
            }
        )
    _require_equal(occupancy, [[True] * 4, [True] * 4], "per-path occupancy")

    path_results: list[dict[str, Any]] = []
    for index, identifier in enumerate(MULTI_PATH_IDENTIFIERS):
        path_frame_hashes = [_sha256(frame) for frame in per_path_frames[index]]
        _require_equal(
            path_frame_hashes,
            MULTI_PATH_FRAME_HASHES[index],
            f"{identifier} frame hashes",
        )
        path_payload_hash = hashlib.sha256(
            b"".join(
                np.ascontiguousarray(frame).tobytes()
                for frame in per_path_frames[index]
            )
        ).hexdigest()
        _require_equal(
            path_payload_hash,
            MULTI_PATH_PAYLOAD_HASHES[index],
            f"{identifier} payload hash",
        )
        matrix_hash = _sha256(matrices[index])
        _require_equal(
            matrix_hash,
            MULTI_PATH_MATRIX_HASHES[index],
            f"{identifier} reconstructed matrix hash",
        )
        optical = dataset.OpticalPathSequence[index]
        path_results.append(
            {
                "ordinal": index + 1,
                "identifier": identifier,
                "description": str(optical.OpticalPathDescription),
                "illumination_wavelength_nm": float(optical.IlluminationWaveLength),
                "illumination_type": {
                    "code_value": "111744",
                    "coding_scheme_designator": "DCM",
                    "code_meaning": "Brightfield illumination",
                },
                "icc_profile_sha256": hashlib.sha256(
                    bytes(optical.ICCProfile)
                ).hexdigest(),
                "color_space": str(optical.ColorSpace),
                "frame_numbers": list(range(index * 4 + 1, index * 4 + 5)),
                "frame_hashes": path_frame_hashes,
                "pixel_data_sha256": path_payload_hash,
                "total_pixel_matrix_shape": [4, 4, 3],
                "total_pixel_matrix_sha256": matrix_hash,
            }
        )

    try:
        image.get_total_pixel_matrix(
            dtype=np.uint8,
            apply_real_world_transform=False,
            apply_modality_transform=False,
            apply_voi_transform=False,
            apply_presentation_lut=False,
            apply_palette_color_lut=False,
            apply_icc_profile=False,
        )
    except RuntimeError as error:
        if "do not uniquely identify frames" not in str(error):
            raise ReconstructionError(
                f"unexpected unfiltered total pixel matrix rejection: {error}"
            ) from error
    else:
        raise ReconstructionError(
            "unfiltered total pixel matrix unexpectedly collapsed the optical-path dimension"
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
        "dimension_organization_type": "TILED_FULL",
        "number_of_frames": 8,
        "number_of_optical_paths": 2,
        "total_pixel_matrix_focal_planes": 1,
        "optical_path_identifiers": MULTI_PATH_IDENTIFIERS,
        "frame_hashes": frame_hashes,
        "pixel_data_sha256": aggregate_hash,
        "implicit_frame_positions": positions,
        "optical_paths": path_results,
        "presence": {
            "dimension_index_sequence": False,
            "per_frame_functional_groups_sequence": False,
            "spacing_between_slices": False,
            "number_of_focal_planes": False,
            "distance_between_focal_planes": False,
            "pyramid_uid": False,
            "concatenation_uid": False,
            "referenced_series_sequence": False,
            "top_level_icc_profile": False,
        },
        "extended_depth_of_field": "NO",
        "unfiltered_total_pixel_matrix": "rejected_ambiguous_optical_path_dimension",
        "reconstruction_scope": "separate_matrix_per_optical_path",
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
        optical_path_count = len(getattr(dataset, "OpticalPathSequence", []))
        if (
            optical_path_count == 2
            or int(getattr(dataset, "NumberOfOpticalPaths", 0)) == 2
            or int(getattr(dataset, "NumberOfFrames", 0)) == 8
        ):
            return _reconstruct_multiple_optical_paths(dataset)
        return _reconstruct_tiled_full(dataset)
    if dimension_type == "TILED_SPARSE":
        return _reconstruct_tiled_sparse(dataset)
    raise ReconstructionError(
        f"DimensionOrganizationType mismatch: expected 'TILED_FULL' or 'TILED_SPARSE', got {dimension_type!r}"
    )


def _role(dataset: pydicom.Dataset) -> str:
    image_type = [str(value) for value in getattr(dataset, "ImageType", [])]
    if len(image_type) < 3:
        raise ReconstructionError("ImageType must contain a role in value 3")
    role = image_type[2].lower()
    if role not in GROUP_ROLES:
        raise ReconstructionError(
            f"ImageType value 3 mismatch: expected VOLUME, THUMBNAIL, or LABEL, got {image_type[2]!r}"
        )
    _require_equal(image_type, ROLE_IMAGE_TYPES[role], f"{role} ImageType")
    return role


def _single_frame_matrix(dataset: pydicom.Dataset, role: str) -> np.ndarray:
    image = hd.Image.from_dataset(dataset, copy=True)
    frame = image.get_stored_frame(1, as_index=False)
    _require_equal(list(frame.shape), [2, 2, 3], f"{role} stored frame shape")
    return frame.astype(np.uint8, copy=False)


def _validate_group_member(dataset: pydicom.Dataset, role: str) -> dict[str, Any]:
    _require_equal(str(dataset.SOPClassUID), WSI_STORAGE, f"{role} SOP Class UID")
    _require_equal(
        str(dataset.file_meta.TransferSyntaxUID),
        EXPLICIT_VR_LITTLE_ENDIAN,
        f"{role} transfer syntax",
    )
    for keyword, expected in (
        ("Modality", "SM"),
        ("DimensionOrganizationType", "TILED_FULL"),
        ("PhotometricInterpretation", "RGB"),
        ("LossyImageCompression", "00"),
        ("BurnedInAnnotation", "NO"),
        ("SpecimenLabelInImage", "YES" if role == "label" else "NO"),
    ):
        _require_equal(str(getattr(dataset, keyword)), expected, f"{role} {keyword}")
    expected_frames = 4 if role == "volume" else 1
    expected_matrix = 4 if role == "volume" else 2
    for keyword, expected in (
        ("Rows", 2),
        ("Columns", 2),
        ("NumberOfFrames", expected_frames),
        ("TotalPixelMatrixRows", expected_matrix),
        ("TotalPixelMatrixColumns", expected_matrix),
        ("NumberOfOpticalPaths", 1),
        ("TotalPixelMatrixFocalPlanes", 1),
        ("SamplesPerPixel", 3),
        ("PlanarConfiguration", 0),
        ("BitsAllocated", 8),
        ("BitsStored", 8),
        ("HighBit", 7),
        ("PixelRepresentation", 0),
    ):
        _require_equal(int(getattr(dataset, keyword)), expected, f"{role} {keyword}")
    if "DimensionIndexSequence" in dataset or "PerFrameFunctionalGroupsSequence" in dataset:
        raise ReconstructionError(
            f"{role} TILED_FULL input must use implicit positions without per-frame or dimension-index sequences"
        )
    _require_equal(
        [str(value) for value in dataset.SharedFunctionalGroupsSequence[0]
         .WholeSlideMicroscopyImageFrameTypeSequence[0].FrameType],
        ROLE_IMAGE_TYPES[role],
        f"{role} shared FrameType",
    )
    measures = dataset.SharedFunctionalGroupsSequence[0].PixelMeasuresSequence[0]
    spacing = [float(value) for value in measures.PixelSpacing]
    _require_equal(
        spacing,
        [1.0, 1.0] if role == "thumbnail" else [0.5, 0.5],
        f"{role} pixel spacing",
    )
    expected_extent = 1.0 if role == "label" else 2.0
    _require_equal(float(dataset.ImagedVolumeWidth), expected_extent, f"{role} imaged width")
    _require_equal(float(dataset.ImagedVolumeHeight), expected_extent, f"{role} imaged height")
    _require_float_close(
        float(dataset.ImagedVolumeDepth), 0.001, f"{role} imaged depth"
    )
    _require_equal(
        [float(value) for value in dataset.ImageOrientationSlide],
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        f"{role} image orientation slide",
    )
    origin = dataset.TotalPixelMatrixOriginSequence[0]
    _require_equal(
        [
            float(origin.XOffsetInSlideCoordinateSystem),
            float(origin.YOffsetInSlideCoordinateSystem),
            float(getattr(origin, "ZOffsetInSlideCoordinateSystem", 0.0)),
        ],
        [0.0, 0.0, 0.0],
        f"{role} total pixel matrix origin",
    )
    payload_hash = hashlib.sha256(bytes(dataset.PixelData)).hexdigest()
    expected_payload = {
        "volume": VOLUME_PAYLOAD_HASH,
        "thumbnail": THUMBNAIL_HASH,
        "label": LABEL_HASH,
    }[role]
    _require_equal(payload_hash, expected_payload, f"{role} Pixel Data payload hash")
    if role == "volume":
        result = _reconstruct_tiled_full(dataset)
        matrix_hash = result["total_pixel_matrix_sha256"]
        shape = result["total_pixel_matrix_shape"]
        frame_hashes = result["frame_hashes"]
    else:
        matrix = _single_frame_matrix(dataset, role)
        matrix_hash = _sha256(matrix)
        _require_equal(matrix_hash, expected_payload, f"{role} reconstructed matrix hash")
        shape = list(matrix.shape)
        frame_hashes = [matrix_hash]
    return {
        "role": role,
        "sop_instance_uid": str(dataset.SOPInstanceUID),
        "image_type": ROLE_IMAGE_TYPES[role],
        "frame_type": ROLE_IMAGE_TYPES[role],
        "frame_count": expected_frames,
        "stored_frame_shape": [2, 2, 3],
        "frame_hashes": frame_hashes,
        "pixel_data_sha256": payload_hash,
        "total_pixel_matrix_shape": shape,
        "total_pixel_matrix_sha256": matrix_hash,
        "pyramid_member": role != "label",
        "pyramid_uid": str(dataset.PyramidUID) if role != "label" else None,
        "specimen_label_in_image": "YES" if role == "label" else "NO",
        "burned_in_annotation": "NO",
        "transforms_applied": False,
    }


def _shared_group_identity(dataset: pydicom.Dataset) -> dict[str, str]:
    specimen = dataset.SpecimenDescriptionSequence[0]
    optical = dataset.OpticalPathSequence[0]
    icc_profile = bytes(optical.ICCProfile)
    _require_equal(hashlib.sha256(icc_profile).hexdigest(), ICC_PROFILE_HASH, "ICC Profile hash")
    _require_equal(str(optical.OpticalPathIdentifier), "RGB", "optical path identifier")
    _require_equal(float(optical.IlluminationWaveLength), 550.0, "illumination wavelength")
    _require_equal(str(dataset.ContainerIdentifier), "DTS-SLIDE-001", "container identifier")
    _require_equal(str(specimen.SpecimenIdentifier), "DTS-SPECIMEN-001", "specimen identifier")
    return {
        "patient_id": str(dataset.PatientID),
        "patient_name": str(dataset.PatientName),
        "study_instance_uid": str(dataset.StudyInstanceUID),
        "series_instance_uid": str(dataset.SeriesInstanceUID),
        "frame_of_reference_uid": str(dataset.FrameOfReferenceUID),
        "container_identifier": str(dataset.ContainerIdentifier),
        "specimen_identifier": str(specimen.SpecimenIdentifier),
        "specimen_uid": str(specimen.SpecimenUID),
        "optical_path_identifier": str(optical.OpticalPathIdentifier),
        "illumination_wavelength_nm": str(float(optical.IlluminationWaveLength)),
        "icc_profile_sha256": hashlib.sha256(icc_profile).hexdigest(),
    }


def reconstruct_group(paths: list[Path]) -> dict[str, Any]:
    _require_equal(len(paths), 3, "group input count")
    datasets_by_role: dict[str, pydicom.Dataset] = {}
    for path in paths:
        dataset = pydicom.dcmread(path)
        role = _role(dataset)
        if role in datasets_by_role:
            raise ReconstructionError(f"duplicate group role: {role}")
        datasets_by_role[role] = dataset
    _require_equal(set(datasets_by_role), set(GROUP_ROLES), "group roles")

    volume = datasets_by_role["volume"]
    identity = _shared_group_identity(volume)
    for role in GROUP_ROLES[1:]:
        _require_equal(
            _shared_group_identity(datasets_by_role[role]),
            identity,
            f"{role} shared group identity",
        )
    sop_uids = [str(datasets_by_role[role].SOPInstanceUID) for role in GROUP_ROLES]
    _require_equal(len(set(sop_uids)), 3, "unique SOP Instance UID count")
    volume_pyramid_uid = str(volume.PyramidUID)
    _require_equal(
        str(datasets_by_role["thumbnail"].PyramidUID),
        volume_pyramid_uid,
        "thumbnail Pyramid UID",
    )
    if "PyramidUID" in datasets_by_role["label"]:
        raise ReconstructionError("label Pyramid UID must be absent")

    members = [
        _validate_group_member(datasets_by_role[role], role) for role in GROUP_ROLES
    ]
    volume_image = hd.Image.from_dataset(volume, copy=True).get_total_pixel_matrix(
        dtype=np.uint8,
        apply_real_world_transform=False,
        apply_modality_transform=False,
        apply_voi_transform=False,
        apply_presentation_lut=False,
        apply_palette_color_lut=False,
        apply_icc_profile=False,
    )
    thumbnail_image = _single_frame_matrix(datasets_by_role["thumbnail"], "thumbnail")
    quadrant_reduction = volume_image[np.ix_([0, 2], [0, 2])]
    if not np.array_equal(thumbnail_image, quadrant_reduction):
        raise ReconstructionError("thumbnail does not equal deterministic volume quadrant reduction")

    return {
        "status": "passed",
        "backend": "dts-wsi-reconstruct",
        "backend_version": __version__,
        "runtime": {
            "highdicom": version("highdicom"),
            "numpy": version("numpy"),
            "pydicom": version("pydicom"),
        },
        "ordered_roles": list(GROUP_ROLES),
        "group_identity": identity,
        "pyramid_uid": volume_pyramid_uid,
        "pyramid_roles": ["volume", "thumbnail"],
        "apex_role": "thumbnail",
        "label_excluded_from_pyramid": True,
        "thumbnail_reduction": "volume_quadrant_top_left_pixels",
        "thumbnail_reduction_sha256": _sha256(quadrant_reduction),
        "members": members,
        "transforms_applied": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path)
    parser.add_argument("--group-input", type=Path, action="append", default=[])
    parser.add_argument("--version", action="store_true")
    args = parser.parse_args()
    if args.version:
        print(f"dts-wsi-reconstruct {__version__}")
        return 0
    if args.input is not None and args.group_input:
        parser.error("--input and --group-input are mutually exclusive")
    if args.input is None and not args.group_input:
        parser.error("--input or exactly three --group-input values are required")
    if args.group_input and len(args.group_input) != 3:
        parser.error("--group-input must be repeated exactly three times")
    try:
        result = reconstruct_group(args.group_input) if args.group_input else reconstruct(args.input)
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
