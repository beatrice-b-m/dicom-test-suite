from __future__ import annotations

import io
import json
import shutil
import struct
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

from pydicom.dataset import Dataset, FileDataset, FileMetaDataset
from pydicom.sequence import Sequence
from pydicom.uid import ExplicitVRLittleEndian

from dts_dicom_validator_adapter.__main__ import (
    TWELVE_LEAD_ECG_STORAGE,
    WAVEFORM_CHANNELS,
    extract_waveform,
    parser,
)


class WaveformTests(unittest.TestCase):
    def write_dataset(self, mutation: str | None = None) -> Path:
        meta = FileMetaDataset()
        meta.MediaStorageSOPClassUID = TWELVE_LEAD_ECG_STORAGE
        meta.MediaStorageSOPInstanceUID = "2.25.1"
        meta.TransferSyntaxUID = ExplicitVRLittleEndian
        meta.ImplementationClassUID = "2.25.2"
        dataset = FileDataset(None, {}, file_meta=meta, preamble=b"\0" * 128)
        dataset.SOPClassUID = TWELVE_LEAD_ECG_STORAGE
        dataset.SOPInstanceUID = "2.25.1"
        dataset.Modality = "ECG"

        group = Dataset()
        group.WaveformOriginality = "ORIGINAL"
        group.NumberOfWaveformChannels = 12
        group.NumberOfWaveformSamples = 500
        group.SamplingFrequency = "500"
        group.MultiplexGroupLabel = "RESTING_12_LEAD"
        group.WaveformBitsAllocated = 16
        group.WaveformSampleInterpretation = "SS"
        definitions = []
        for ordinal, (label, code_value, code_meaning, _) in enumerate(
            WAVEFORM_CHANNELS, start=1
        ):
            definition = Dataset()
            definition.WaveformChannelNumber = ordinal
            definition.ChannelLabel = label
            source = Dataset()
            source.CodeValue = code_value
            source.CodingSchemeDesignator = "MDC"
            source.CodeMeaning = code_meaning
            definition.ChannelSourceSequence = Sequence([source])
            definition.ChannelSensitivity = "1"
            unit = Dataset()
            unit.CodeValue = "uV"
            unit.CodingSchemeDesignator = "UCUM"
            unit.CodeMeaning = "microvolt"
            definition.ChannelSensitivityUnitsSequence = Sequence([unit])
            definition.ChannelSensitivityCorrectionFactor = "1"
            definition.ChannelBaseline = "0"
            definition.ChannelTimeSkew = "0"
            definition.WaveformBitsStored = 16
            definitions.append(definition)
        if mutation == "duplicate_lead":
            definitions[-1].ChannelSourceSequence = Sequence(
                [definitions[0].ChannelSourceSequence[0]]
            )
        group.ChannelDefinitionSequence = Sequence(definitions)

        values = [
            ((sample * (channel + 1) * 37 + channel * 101) % 2001) - 1000
            for sample in range(500)
            for channel in range(12)
        ]
        payload = bytearray(struct.pack("<6000h", *values))
        if mutation == "payload":
            payload[100] ^= 1
        group.add_new(0x54001010, "OW", bytes(payload))
        dataset.WaveformSequence = Sequence([group])

        directory = Path(tempfile.mkdtemp(prefix="dts-waveform-adapter-"))
        path = directory / "twelve-lead-ecg.dcm"
        dataset.save_as(path, enforce_file_format=True)
        self.addCleanup(shutil.rmtree, directory, True)
        return path

    def extract(self, path: Path) -> dict:
        output = io.StringIO()
        with (
            patch("dts_dicom_validator_adapter.__main__.verify_distribution"),
            patch("dts_dicom_validator_adapter.__main__.verify_standard"),
            redirect_stdout(output),
        ):
            self.assertEqual(
                extract_waveform(path, Path("unused"), Path("unused")),
                0,
            )
        return json.loads(output.getvalue())

    def test_extracts_locked_waveform_metadata_and_hashes(self) -> None:
        result = self.extract(self.write_dataset())
        self.assertEqual(result["adapter_id"], "pydicom-dicom-validator-waveform")
        self.assertEqual(result["channel_count"], 12)
        self.assertEqual(result["sample_count"], 500)
        self.assertEqual(result["sampling_frequency_hz"], 500)
        self.assertEqual(result["waveform_data_length"], 12000)
        self.assertEqual(
            result["waveform_data_sha256"],
            "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
        )
        self.assertEqual(result["stored_value_min"], -1000)
        self.assertEqual(result["stored_value_max"], 1000)
        self.assertEqual(result["interleave_order"], "channel_then_sample")
        self.assertEqual(len(result["channel_hashes"]), 12)
        self.assertEqual(result["channel_definitions"][0]["label"], "I")
        self.assertEqual(
            result["channel_definitions"][-1]["source"]["code_value"], "2:8"
        )

    def test_rejects_payload_and_channel_mutations(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "sample formula"):
            self.extract(self.write_dataset("payload"))
        with self.assertRaisesRegex(RuntimeError, "channel 12 metadata"):
            self.extract(self.write_dataset("duplicate_lead"))

    def test_parser_exposes_waveform_route(self) -> None:
        args = parser().parse_args(["--waveform", "input.dcm"])
        self.assertTrue(args.waveform)
        self.assertEqual(args.input, Path("input.dcm"))


if __name__ == "__main__":
    unittest.main()
