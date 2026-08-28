from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import math
import struct
import sys
from collections import Counter
from pathlib import Path
from typing import Any

import pydicom

from .__main__ import EXPECTED_DISTRIBUTIONS, verify_distribution


ADAPTER_ID = "pydicom-encapsulated-stl-payload"
ADAPTER_VERSION = "0.1.0"
CONTRACT_SCHEMA_VERSION = "0.1.0"
TRIANGLE_RECORD_LENGTH = 50
STL_PREAMBLE_LENGTH = 84

Point = tuple[float, float, float]


def _vector(left: Point, right: Point) -> Point:
    return tuple(right[index] - left[index] for index in range(3))  # type: ignore[return-value]


def _cross(left: Point, right: Point) -> Point:
    return (
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    )


def _dot(left: Point, right: Point) -> float:
    return sum(left[index] * right[index] for index in range(3))


def _magnitude(vector: Point) -> float:
    return math.sqrt(_dot(vector, vector))


def _mean(points: list[Point]) -> Point:
    return tuple(
        sum(point[index] for point in points) / len(points) for index in range(3)
    )  # type: ignore[return-value]


def load_contract(path: Path) -> dict[str, Any]:
    contract = json.loads(path.read_text(encoding="utf-8"))
    if contract.get("schema_version") != CONTRACT_SCHEMA_VERSION:
        raise RuntimeError("Encapsulated STL contract schema version mismatch")
    if contract.get("adapter_id") != ADAPTER_ID:
        raise RuntimeError("Encapsulated STL contract adapter identity mismatch")
    if contract.get("adapter_version") != ADAPTER_VERSION:
        raise RuntimeError("Encapsulated STL contract adapter version mismatch")
    return contract


def extract_document_bytes(dataset: pydicom.dataset.Dataset) -> tuple[bytes, int]:
    if 0x00420011 not in dataset:
        raise RuntimeError("Encapsulated Document is missing")
    if 0x00420015 not in dataset:
        raise RuntimeError("Encapsulated Document Length is missing")
    document_element = dataset[0x00420011]
    length_element = dataset[0x00420015]
    if document_element.VR != "OB":
        raise RuntimeError("Encapsulated Document must have OB VR")
    if length_element.VR != "UL":
        raise RuntimeError("Encapsulated Document Length must have UL VR")
    declared_length = int(length_element.value)
    raw_value = bytes(document_element.value)
    if declared_length <= 0 or declared_length > len(raw_value):
        raise RuntimeError("Encapsulated Document Length exceeds the stored OB value")
    padding = raw_value[declared_length:]
    if len(padding) > 1 or any(padding):
        raise RuntimeError(
            "stored OB value contains bytes beyond Encapsulated Document Length"
        )
    return raw_value[:declared_length], len(padding)


def parse_binary_stl(payload: bytes, contract: dict[str, Any]) -> dict[str, Any]:
    if len(payload) < STL_PREAMBLE_LENGTH:
        raise RuntimeError("binary STL is shorter than its header and triangle count")
    triangle_count = struct.unpack_from("<I", payload, 80)[0]
    expected_length = STL_PREAMBLE_LENGTH + TRIANGLE_RECORD_LENGTH * triangle_count
    if len(payload) != expected_length:
        raise RuntimeError("binary STL triangle count does not match payload length")
    if triangle_count != int(contract["triangle_count"]):
        raise RuntimeError("binary STL triangle count does not match the contract")

    normals: list[Point] = []
    faces: list[tuple[Point, Point, Point]] = []
    attributes: list[int] = []
    for index in range(triangle_count):
        values = struct.unpack_from(
            "<12fH", payload, STL_PREAMBLE_LENGTH + index * TRIANGLE_RECORD_LENGTH
        )
        coordinates = values[:12]
        if not all(math.isfinite(value) for value in coordinates):
            raise RuntimeError(f"binary STL triangle {index + 1} contains non-finite values")
        normal = tuple(coordinates[:3])
        vertices = tuple(
            tuple(coordinates[offset : offset + 3]) for offset in (3, 6, 9)
        )
        normals.append(normal)  # type: ignore[arg-type]
        faces.append(vertices)  # type: ignore[arg-type]
        attributes.append(values[12])
    if any(attributes):
        raise RuntimeError("binary STL triangle attributes must all be zero")

    unique_vertices = sorted({vertex for face in faces for vertex in face})
    if len(unique_vertices) < 4:
        raise RuntimeError("binary STL does not contain enough distinct vertices")
    mesh_centroid = _mean(unique_vertices)
    tolerance = float(contract["normal_tolerance"])
    directed_edges: Counter[tuple[Point, Point]] = Counter()
    undirected_edges: Counter[tuple[Point, Point]] = Counter()
    signed_six_volume = 0.0
    for index, (normal, face) in enumerate(zip(normals, faces, strict=True), start=1):
        first_edge = _vector(face[0], face[1])
        second_edge = _vector(face[0], face[2])
        winding_normal = _cross(first_edge, second_edge)
        winding_magnitude = _magnitude(winding_normal)
        normal_magnitude = _magnitude(normal)
        if winding_magnitude <= tolerance:
            raise RuntimeError(f"binary STL triangle {index} is degenerate")
        if normal_magnitude <= tolerance:
            raise RuntimeError(f"binary STL triangle {index} has a zero normal")
        agreement = _dot(winding_normal, normal) / (
            winding_magnitude * normal_magnitude
        )
        if agreement < 1.0 - tolerance or abs(normal_magnitude - 1.0) > tolerance:
            raise RuntimeError(
                f"binary STL triangle {index} normal disagrees with its winding"
            )
        face_centroid = _mean(list(face))
        if _dot(winding_normal, _vector(mesh_centroid, face_centroid)) <= tolerance:
            raise RuntimeError(f"binary STL triangle {index} winding is not outward")
        signed_six_volume += _dot(face[0], _cross(face[1], face[2]))
        for start, end in ((face[0], face[1]), (face[1], face[2]), (face[2], face[0])):
            directed_edges[(start, end)] += 1
            undirected_edges[tuple(sorted((start, end)))] += 1

    if any(count != 2 for count in undirected_edges.values()):
        raise RuntimeError("binary STL is not a closed two-manifold")
    for start, end in undirected_edges:
        if directed_edges[(start, end)] != 1 or directed_edges[(end, start)] != 1:
            raise RuntimeError("binary STL manifold edges do not have opposite winding")
    if signed_six_volume <= tolerance:
        raise RuntimeError("binary STL has non-positive outward signed volume")

    bounds_min = [min(vertex[index] for vertex in unique_vertices) for index in range(3)]
    bounds_max = [max(vertex[index] for vertex in unique_vertices) for index in range(3)]
    if bounds_min != contract["bounds"]["min"] or bounds_max != contract["bounds"]["max"]:
        raise RuntimeError("binary STL bounds do not match the contract")
    payload_sha256 = hashlib.sha256(payload).hexdigest()
    if payload_sha256 != contract["payload_sha256"]:
        raise RuntimeError("binary STL payload hash does not match the contract")

    return {
        "attribute_byte_counts_zero": True,
        "bounds": {"max": bounds_max, "min": bounds_min},
        "closed_manifold": True,
        "edge_count": len(undirected_edges),
        "finite_geometry": True,
        "nondegenerate_faces": True,
        "normal_winding_agreement": True,
        "outward_winding": True,
        "payload_length_bytes": len(payload),
        "payload_sha256": payload_sha256,
        "signed_volume": signed_six_volume / 6.0,
        "triangle_count": triangle_count,
        "unique_vertex_count": len(unique_vertices),
    }


def validate(input_path: Path, contract_path: Path) -> int:
    for name, version in EXPECTED_DISTRIBUTIONS.items():
        verify_distribution(name, version)
    contract = load_contract(contract_path)
    dataset = pydicom.dcmread(input_path)
    transfer_syntax = str(dataset.file_meta.TransferSyntaxUID)
    expected = contract["dicom"]
    if str(getattr(dataset, "SOPClassUID", "")) != expected["sop_class_uid"]:
        raise RuntimeError("DICOM object is not Encapsulated STL Storage")
    if transfer_syntax != expected["transfer_syntax_uid"]:
        raise RuntimeError("Encapsulated STL transfer syntax does not match the contract")
    if str(getattr(dataset, "Modality", "")) != expected["modality"]:
        raise RuntimeError("Encapsulated STL modality does not match the contract")
    if str(getattr(dataset, "MIMETypeOfEncapsulatedDocument", "")) != expected["mime_type"]:
        raise RuntimeError("Encapsulated STL MIME type does not match the contract")
    payload, padding_length = extract_document_bytes(dataset)
    if len(payload) != int(contract["payload_length_bytes"]):
        raise RuntimeError("Encapsulated Document Length does not match the contract")
    mesh = parse_binary_stl(payload, contract)
    result = {
        "adapter_id": ADAPTER_ID,
        "adapter_version": ADAPTER_VERSION,
        "dicom": {
            "encapsulated_document_length": len(payload),
            "mime_type": str(dataset.MIMETypeOfEncapsulatedDocument),
            "modality": str(dataset.Modality),
            "sop_class_uid": str(dataset.SOPClassUID),
            "stored_value_padding_bytes": padding_length,
            "transfer_syntax_uid": transfer_syntax,
        },
        "mesh": mesh,
    }
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    result.add_argument("--version", action="store_true")
    result.add_argument("--contract-lock", type=Path)
    result.add_argument("input", nargs="?", type=Path)
    return result


def main() -> None:
    args = parser().parse_args()
    if args.version:
        versions = " ".join(
            f"{name}={importlib.metadata.version(name)}"
            for name in sorted(EXPECTED_DISTRIBUTIONS)
        )
        print(
            f"{ADAPTER_ID} {ADAPTER_VERSION} python={sys.version.split()[0]} {versions}"
        )
        return
    if args.input is None:
        raise SystemExit("input DICOM path is required")
    if args.contract_lock is None:
        raise SystemExit("--contract-lock is required")
    try:
        raise SystemExit(validate(args.input, args.contract_lock))
    except Exception as error:
        print(f"Error: Encapsulated STL adapter failure: {error}", file=sys.stderr)
        raise SystemExit(126) from error


if __name__ == "__main__":
    main()
