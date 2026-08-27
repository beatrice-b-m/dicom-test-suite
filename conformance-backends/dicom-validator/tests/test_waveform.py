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
    GENERAL_AGGREGATE_PAYLOAD_SHA256,
    GENERAL_AUXILIARY_CHANNELS,
    GENERAL_ECG_STORAGE,
    GENERAL_GROUP_PAYLOAD_SHA256,
    TWELVE_LEAD_ECG_STORAGE,
    WAVEFORM_CHANNELS,
    extract_waveform,
    parser,
)


class WaveformTests(unittest.TestCase):
    def write_dataset(
        self,
        sop_class_uid: str = TWELVE_LEAD_ECG_STORAGE,
        mutation: str | None = None,
    ) -> Path:
        meta = FileMetaDataset()
        meta.MediaStorageSOPClassUID = sop_class_uid
        meta.MediaStorageSOPInstanceUID = "2.25.1"
        meta.TransferSyntaxUID = ExplicitVRLittleEndian
        meta.ImplementationClassUID = "2.25.2"
        dataset = FileDataset(None, {}, file_meta=meta, preamble=b"\0" * 128)
        dataset.SOPClassUID = sop_class_uid
        dataset.SOPInstanceUID = "2.25.1"
        dataset.Modality = "ECG"
        dataset.AcquisitionContextSequence = Sequence([])

        if sop_class_uid == TWELVE_LEAD_ECG_STORAGE:
            specs = [("RESTING_12_LEAD", 500, 500, WAVEFORM_CHANNELS)]
        elif sop_class_uid == GENERAL_ECG_STORAGE:
            specs = [
                ("STD12_250HZ", 1000, 250, WAVEFORM_CHANNELS),
                ("AUX4_1000HZ", 4000, 1000, GENERAL_AUXILIARY_CHANNELS),
            ]
        else:
            specs = [("UNSUPPORTED", 1, 1, WAVEFORM_CHANNELS[:1])]

        groups = [
            self.waveform_group(
                group_index,
                label,
                sample_count,
                sampling_frequency,
                channels,
                sop_class_uid,
            )
            for group_index, (
                label,
                sample_count,
                sampling_frequency,
                channels,
            ) in enumerate(specs)
        ]
        if mutation == "payload":
            payload = bytearray(groups[-1][0x54001010].value)
            payload[100] ^= 1
            groups[-1][0x54001010].value = bytes(payload)
        elif mutation == "duplicate_lead":
            definitions = groups[-1].ChannelDefinitionSequence
            definitions[-1].ChannelSourceSequence = Sequence(
                [definitions[0].ChannelSourceSequence[0]]
            )
        elif mutation == "reverse_groups":
            groups.reverse()
        elif mutation == "missing_group":
            groups.pop()
        elif mutation == "sample_count":
            groups[-1].NumberOfWaveformSamples += 1
        elif mutation == "sample_interpretation":
            groups[-1].WaveformSampleInterpretation = "US"
        dataset.WaveformSequence = Sequence(groups)

        directory = Path(tempfile.mkdtemp(prefix="dts-waveform-adapter-"))
        path = directory / "waveform.dcm"
        dataset.save_as(path, enforce_file_format=True)
        self.addCleanup(shutil.rmtree, directory, True)
        return path

    def waveform_group(
        self,
        group_index: int,
        label: str,
        sample_count: int,
        sampling_frequency: int,
        channels: tuple,
        sop_class_uid: str,
    ) -> Dataset:
        group = Dataset()
        group.WaveformOriginality = "ORIGINAL"
        group.NumberOfWaveformChannels = len(channels)
        group.NumberOfWaveformSamples = sample_count
        group.SamplingFrequency = str(sampling_frequency)
        group.MultiplexGroupLabel = label
        group.WaveformBitsAllocated = 16
        group.WaveformSampleInterpretation = "SS"
        group.ChannelDefinitionSequence = Sequence(
            [
                self.channel_definition(ordinal, channel)
                for ordinal, channel in enumerate(channels, start=1)
            ]
        )
        if sop_class_uid == TWELVE_LEAD_ECG_STORAGE:
            values = [
                ((sample * (channel + 1) * 37 + channel * 101) % 2001) - 1000
                for sample in range(sample_count)
                for channel in range(len(channels))
            ]
        else:
            values = [
                (
                    (
                        sample
                        * (channel + 1)
                        * (group_index + 1)
                        * 37
                        + channel * 101
                        + group_index * 307
                    )
                    % 2001
                )
                - 1000
                for sample in range(sample_count)
                for channel in range(len(channels))
            ]
        group.add_new(
            0x54001010,
            "OW",
            struct.pack(f"<{len(values)}h", *values),
        )
        return group

    @staticmethod
    def channel_definition(ordinal: int, channel: tuple) -> Dataset:
        label, code_value, code_meaning, _ = channel
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
        return definition

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

    def test_extracts_locked_twelve_lead_group_and_aggregate(self) -> None:
        result = self.extract(self.write_dataset())
        self.assertEqual(result["adapter_id"], "pydicom-dicom-validator-waveform")
        self.assertEqual(result["sop_class_uid"], TWELVE_LEAD_ECG_STORAGE)
        self.assertEqual(result["acquisition_context_items"], 0)
        self.assertEqual(
            result["absent_content"],
            {
                "annotation_module": True,
                "synchronization_module": True,
                "references": True,
                "image": True,
                "pixel_data": True,
            },
        )
        self.assertEqual(result["aggregate"]["group_count"], 1)
        self.assertEqual(result["aggregate"]["total_channel_count"], 12)
        self.assertEqual(result["aggregate"]["common_duration_seconds"], 1)
        self.assertEqual(result["aggregate"]["total_payload_length_bytes"], 12000)
        group = result["multiplex_groups"][0]
        self.assertEqual(group["ordinal"], 1)
        self.assertEqual(group["label"], "RESTING_12_LEAD")
        self.assertEqual(group["channel_count"], 12)
        self.assertEqual(group["samples_per_channel"], 500)
        self.assertEqual(group["sampling_frequency_hz"], 500)
        self.assertEqual(group["storage"]["payload_length_bytes"], 12000)
        self.assertEqual(
            group["storage"]["payload_sha256"],
            "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
        )
        self.assertEqual(group["storage"]["sample_min"], -1000)
        self.assertEqual(group["storage"]["sample_max"], 1000)
        self.assertTrue(group["storage"]["formula_match"])
        self.assertEqual(len(group["storage"]["channel_sha256"]), 12)
        self.assertEqual(group["channels"][0]["label"], "I")
        self.assertEqual(group["channels"][-1]["source"]["code_value"], "2:8")

    def test_extracts_locked_general_ecg_groups_and_aggregate(self) -> None:
        result = self.extract(self.write_dataset(GENERAL_ECG_STORAGE))
        self.assertEqual(result["sop_class_uid"], GENERAL_ECG_STORAGE)
        self.assertEqual(
            result["aggregate"],
            {
                "group_count": 2,
                "total_channel_count": 16,
                "common_duration_seconds": 4,
                "total_payload_length_bytes": 56000,
                "group_payload_sha256": list(GENERAL_GROUP_PAYLOAD_SHA256),
                "aggregate_payload_sha256": GENERAL_AGGREGATE_PAYLOAD_SHA256,
            },
        )
        first, second = result["multiplex_groups"]
        self.assertEqual(
            (first["ordinal"], first["label"], first["channel_count"]),
            (1, "STD12_250HZ", 12),
        )
        self.assertEqual(
            (first["samples_per_channel"], first["sampling_frequency_hz"]),
            (1000, 250),
        )
        self.assertEqual(first["storage"]["payload_length_bytes"], 24000)
        self.assertEqual(
            (second["ordinal"], second["label"], second["channel_count"]),
            (2, "AUX4_1000HZ", 4),
        )
        self.assertEqual(
            (second["samples_per_channel"], second["sampling_frequency_hz"]),
            (4000, 1000),
        )
        self.assertEqual(second["storage"]["payload_length_bytes"], 32000)
        self.assertEqual(
            [channel["label"] for channel in second["channels"]],
            ["A1", "A2", "A3", "A4"],
        )
        self.assertEqual(
            [channel["source"]["code_value"] for channel in second["channels"]],
            ["2:75", "2:76", "2:77", "2:78"],
        )
        self.assertTrue(
            all(group["storage"]["formula_match"] for group in (first, second))
        )

    def test_rejects_payload_and_channel_mutations(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "sample formula"):
            self.extract(self.write_dataset(mutation="payload"))
        with self.assertRaisesRegex(RuntimeError, "channel 12 metadata"):
            self.extract(self.write_dataset(mutation="duplicate_lead"))
        with self.assertRaisesRegex(RuntimeError, "sample formula"):
            self.extract(self.write_dataset(GENERAL_ECG_STORAGE, "payload"))
        with self.assertRaisesRegex(RuntimeError, "channel 4 metadata"):
            self.extract(self.write_dataset(GENERAL_ECG_STORAGE, "duplicate_lead"))

    def test_rejects_general_group_order_cardinality_and_shape_mutations(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "group 1.*shape"):
            self.extract(self.write_dataset(GENERAL_ECG_STORAGE, "reverse_groups"))
        with self.assertRaisesRegex(RuntimeError, "exactly 2 multiplex groups"):
            self.extract(self.write_dataset(GENERAL_ECG_STORAGE, "missing_group"))
        with self.assertRaisesRegex(RuntimeError, "group 2.*shape"):
            self.extract(self.write_dataset(GENERAL_ECG_STORAGE, "sample_count"))
        with self.assertRaisesRegex(RuntimeError, "group 2.*shape"):
            self.extract(
                self.write_dataset(GENERAL_ECG_STORAGE, "sample_interpretation")
            )

    def test_rejects_unsupported_waveform_sop_class(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "unsupported waveform SOP Class UID"):
            self.extract(self.write_dataset("1.2.840.10008.5.1.4.1.1.9.1.3"))

    def test_parser_exposes_waveform_route(self) -> None:
        args = parser().parse_args(["--waveform", "input.dcm"])
        self.assertTrue(args.waveform)
        self.assertEqual(args.input, Path("input.dcm"))


if __name__ == "__main__":
    unittest.main()
