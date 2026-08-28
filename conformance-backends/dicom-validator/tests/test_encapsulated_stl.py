from __future__ import annotations

import io
import json
import math
import shutil
import struct
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

import pydicom
from pydicom.dataset import FileDataset, FileMetaDataset
from pydicom.uid import ExplicitVRLittleEndian

from dts_dicom_validator_adapter.encapsulated_stl import (
    ADAPTER_ID,
    extract_document_bytes,
    load_contract,
    parse_binary_stl,
    parser,
    validate,
)


SOP_CLASS_UID = "1.2.840.10008.5.1.4.1.1.104.3"
CONTRACT_PATH = Path(__file__).parents[1] / "encapsulated-stl-lock.json"
Point = tuple[float, float, float]


def cross(left: Point, right: Point) -> Point:
    return (
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    )


def subtract(left: Point, right: Point) -> Point:
    return tuple(left[index] - right[index] for index in range(3))  # type: ignore[return-value]


def unit_normal(face: tuple[Point, Point, Point]) -> Point:
    normal = cross(subtract(face[1], face[0]), subtract(face[2], face[0]))
    magnitude = math.sqrt(sum(value * value for value in normal))
    return tuple(value / magnitude for value in normal)  # type: ignore[return-value]


def tetrahedron_payload(extent: float = 10.0) -> bytes:
    origin = (0.0, 0.0, 0.0)
    x = (extent, 0.0, 0.0)
    y = (0.0, extent, 0.0)
    z = (0.0, 0.0, extent)
    faces = ((origin, y, x), (origin, x, z), (origin, z, y), (x, y, z))
    payload = bytearray(b"dicom-test-suite synthetic closed tetrahedron")
    payload.extend(bytes(80 - len(payload)))
    payload.extend(struct.pack("<I", len(faces)))
    for face in faces:
        payload.extend(struct.pack("<12fH", *unit_normal(face), *sum(face, ()), 0))
    return bytes(payload)


class EncapsulatedStlTests(unittest.TestCase):
    def setUp(self) -> None:
        self.contract = load_contract(CONTRACT_PATH)

    def write_dataset(
        self,
        payload: bytes | None = None,
        declared_length: int | None = None,
    ) -> Path:
        payload = tetrahedron_payload() if payload is None else payload
        meta = FileMetaDataset()
        meta.MediaStorageSOPClassUID = SOP_CLASS_UID
        meta.MediaStorageSOPInstanceUID = "2.25.1"
        meta.TransferSyntaxUID = ExplicitVRLittleEndian
        meta.ImplementationClassUID = "2.25.2"
        dataset = FileDataset(None, {}, file_meta=meta, preamble=bytes(128))
        dataset.SOPClassUID = SOP_CLASS_UID
        dataset.SOPInstanceUID = "2.25.1"
        dataset.Modality = "M3D"
        dataset.MIMETypeOfEncapsulatedDocument = "model/stl"
        dataset.add_new(0x00420011, "OB", payload)
        dataset.add_new(
            0x00420015,
            "UL",
            len(payload) if declared_length is None else declared_length,
        )
        directory = Path(tempfile.mkdtemp(prefix="dts-stl-adapter-"))
        path = directory / "mesh.dcm"
        dataset.save_as(path, enforce_file_format=True)
        self.addCleanup(shutil.rmtree, directory, True)
        return path

    def extract(self, path: Path) -> dict:
        output = io.StringIO()
        with (
            patch(
                "dts_dicom_validator_adapter.encapsulated_stl.verify_distribution"
            ),
            redirect_stdout(output),
        ):
            self.assertEqual(validate(path, CONTRACT_PATH), 0)
        return json.loads(output.getvalue())

    def test_extracts_exact_document_extent_and_closed_tetrahedron(self) -> None:
        result = self.extract(self.write_dataset())
        self.assertEqual(result["adapter_id"], ADAPTER_ID)
        self.assertEqual(result["dicom"]["encapsulated_document_length"], 284)
        self.assertEqual(result["dicom"]["stored_value_padding_bytes"], 0)
        self.assertEqual(result["mesh"]["triangle_count"], 4)
        self.assertEqual(result["mesh"]["unique_vertex_count"], 4)
        self.assertEqual(result["mesh"]["edge_count"], 6)
        self.assertEqual(result["mesh"]["signed_volume"], 1000.0 / 6.0)
        self.assertEqual(
            result["mesh"]["payload_sha256"],
            "3c3049d231f8e98c0d2fe7cb81cf6805141bcac39dd04b9cf7f8063ec44bbfb2",
        )
        for invariant in (
            "attribute_byte_counts_zero",
            "closed_manifold",
            "finite_geometry",
            "nondegenerate_faces",
            "normal_winding_agreement",
            "outward_winding",
        ):
            self.assertTrue(result["mesh"][invariant])

    def test_uses_document_length_and_rejects_non_padding_tail(self) -> None:
        path = self.write_dataset(tetrahedron_payload() + b"X", 284)
        dataset = pydicom.dcmread(path)
        with self.assertRaisesRegex(RuntimeError, "bytes beyond"):
            extract_document_bytes(dataset)
        path = self.write_dataset(declared_length=283)
        dataset = pydicom.dcmread(path)
        document, padding_length = extract_document_bytes(dataset)
        self.assertEqual(len(document), 283)
        self.assertEqual(padding_length, 1)
        with self.assertRaisesRegex(RuntimeError, "Document Length does not match"):
            self.extract(path)

    def test_rejects_count_length_nonfinite_attribute_and_degenerate_records(self) -> None:
        payload = bytearray(tetrahedron_payload())
        struct.pack_into("<I", payload, 80, 5)
        with self.assertRaisesRegex(RuntimeError, "count does not match payload length"):
            parse_binary_stl(bytes(payload), self.contract)

        payload = bytearray(tetrahedron_payload())
        struct.pack_into("<f", payload, 84, math.nan)
        with self.assertRaisesRegex(RuntimeError, "non-finite"):
            parse_binary_stl(bytes(payload), self.contract)

        payload = bytearray(tetrahedron_payload())
        struct.pack_into("<H", payload, 84 + 48, 1)
        with self.assertRaisesRegex(RuntimeError, "attributes must all be zero"):
            parse_binary_stl(bytes(payload), self.contract)

        payload = bytearray(tetrahedron_payload())
        payload[84 + 36 : 84 + 48] = payload[84 + 24 : 84 + 36]
        with self.assertRaisesRegex(RuntimeError, "degenerate"):
            parse_binary_stl(bytes(payload), self.contract)

    def test_rejects_normal_winding_outward_and_manifold_mutations(self) -> None:
        payload = bytearray(tetrahedron_payload())
        struct.pack_into("<3f", payload, 84, 0.0, 0.0, 1.0)
        with self.assertRaisesRegex(RuntimeError, "normal disagrees"):
            parse_binary_stl(bytes(payload), self.contract)

        payload = bytearray(tetrahedron_payload())
        record = 84
        first = bytes(payload[record + 12 : record + 24])
        second = bytes(payload[record + 24 : record + 36])
        payload[record + 12 : record + 24] = second
        payload[record + 24 : record + 36] = first
        struct.pack_into("<3f", payload, record, 0.0, 0.0, 1.0)
        with self.assertRaisesRegex(RuntimeError, "winding is not outward"):
            parse_binary_stl(bytes(payload), self.contract)

        payload = bytearray(tetrahedron_payload())
        record = 84 + 50
        struct.pack_into("<3f", payload, record + 36, 0.0, 0.0, 9.0)
        face = struct.unpack_from("<9f", payload, record + 12)
        vertices = tuple(tuple(face[offset : offset + 3]) for offset in (0, 3, 6))
        struct.pack_into("<3f", payload, record, *unit_normal(vertices))  # type: ignore[arg-type]
        with self.assertRaisesRegex(RuntimeError, "closed two-manifold"):
            parse_binary_stl(bytes(payload), self.contract)

    def test_rejects_valid_geometry_with_wrong_bounds_or_payload_hash(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "bounds do not match"):
            parse_binary_stl(tetrahedron_payload(9.0), self.contract)

        payload = bytearray(tetrahedron_payload())
        payload[0] ^= 1
        with self.assertRaisesRegex(RuntimeError, "payload hash"):
            parse_binary_stl(bytes(payload), self.contract)

    def test_parser_requires_an_explicit_contract_lock(self) -> None:
        args = parser().parse_args(
            ["--contract-lock", "encapsulated-stl-lock.json", "mesh.dcm"]
        )
        self.assertEqual(args.contract_lock, Path("encapsulated-stl-lock.json"))
        self.assertEqual(args.input, Path("mesh.dcm"))


if __name__ == "__main__":
    unittest.main()
