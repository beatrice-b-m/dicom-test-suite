from __future__ import annotations

import copy
import io
import sys
import types
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

from dts_dicom_validator_adapter.__main__ import (
    C_ARM_CONTROL_POINT_SEQUENCE,
    C_ARM_PHOTON_ELECTRON_BEAM_MODULE,
    DEFINITION_CORRECTION_ID,
    LOCKED_MALFORMED_RECORDED_DATETIME,
    RECORDED_RT_CONTROL_POINT_DATETIME,
    RT_CONTROL_POINT_MODULE,
    RT_DELIVERY_CONTROL_POINT_MODULE,
    RT_RECORD_FLAG_YES_ALTERNATIVE,
    correct_locked_definition,
    main,
)


class DefinitionCorrectionTests(unittest.TestCase):
    @staticmethod
    def dicom_info(attribute: object) -> object:
        return types.SimpleNamespace(
            modules={
                "unrelated": {"sentinel": "preserved"},
                C_ARM_PHOTON_ELECTRON_BEAM_MODULE: {
                    C_ARM_CONTROL_POINT_SEQUENCE: {
                        "items": {
                            "include": [
                                {"ref": RT_DELIVERY_CONTROL_POINT_MODULE},
                                {"ref": "C.36.2.2.9-1"},
                                {"ref": "C.36.2.2.11-1"},
                            ]
                        },
                        "name": "C-Arm Photon-Electron Control Point Sequence",
                        "type": "1",
                    }
                },
                RT_DELIVERY_CONTROL_POINT_MODULE: {
                    "include": [{"ref": RT_CONTROL_POINT_MODULE}]
                },
                RT_CONTROL_POINT_MODULE: {
                    "unrelated": {"sentinel": "preserved"},
                    RECORDED_RT_CONTROL_POINT_DATETIME: attribute,
                },
            }
        )

    def test_injects_only_locked_rt_record_flag_yes_alternative(self) -> None:
        malformed = copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
        info = self.dicom_info(malformed)
        before_unrelated_module = copy.deepcopy(info.modules["unrelated"])
        before_unrelated_attribute = copy.deepcopy(
            info.modules[RT_CONTROL_POINT_MODULE]["unrelated"]
        )

        correct_locked_definition(info)

        corrected = info.modules[RT_CONTROL_POINT_MODULE][
            RECORDED_RT_CONTROL_POINT_DATETIME
        ]
        expected = copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
        expected["cond"]["other_cond"] = RT_RECORD_FLAG_YES_ALTERNATIVE
        self.assertEqual(corrected, expected)
        self.assertEqual(info.modules["unrelated"], before_unrelated_module)
        self.assertEqual(
            info.modules[RT_CONTROL_POINT_MODULE]["unrelated"],
            before_unrelated_attribute,
        )
        self.assertEqual(malformed, LOCKED_MALFORMED_RECORDED_DATETIME)

    def test_rejects_every_locked_shape_drift(self) -> None:
        mutations = []
        for path, value in (
            (("name",), "Recorded DateTime"),
            (("type",), "2C"),
            (("cond", "tag"), "(300A,0600)"),
            (("cond", "type"), "MU"),
            (("cond", "op"), "!="),
            (("cond", "index"), 1),
            (("cond", "values"), ["NO"]),
        ):
            mutation = copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
            target = mutation
            for key in path[:-1]:
                target = target[key]
            target[path[-1]] = value
            mutations.append(mutation)
        already_corrected = copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
        already_corrected["cond"]["other_cond"] = copy.deepcopy(
            RT_RECORD_FLAG_YES_ALTERNATIVE
        )
        mutations.extend((None, {}, already_corrected))

        for mutation in mutations:
            with self.subTest(mutation=mutation):
                with self.assertRaisesRegex(
                    RuntimeError,
                    f"definition correction {DEFINITION_CORRECTION_ID} "
                    "attribute shape mismatch",
                ):
                    correct_locked_definition(self.dicom_info(mutation))

    def test_rejects_missing_or_malformed_module_map(self) -> None:
        missing_target = self.dicom_info(
            copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
        )
        del missing_target.modules[RT_CONTROL_POINT_MODULE]
        malformed_target = self.dicom_info(
            copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
        )
        malformed_target.modules[RT_CONTROL_POINT_MODULE] = []
        for info in (
            object(),
            types.SimpleNamespace(modules=None),
            missing_target,
            malformed_target,
        ):
            with self.subTest(info=info):
                with self.assertRaisesRegex(
                    RuntimeError,
                    f"definition correction {DEFINITION_CORRECTION_ID} "
                    "module shape mismatch",
                ):
                    correct_locked_definition(info)

    def test_rejects_parent_sequence_path_drift(self) -> None:
        mutations = []
        missing_beam = self.dicom_info(
            copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
        )
        del missing_beam.modules[C_ARM_PHOTON_ELECTRON_BEAM_MODULE]
        mutations.append(missing_beam)
        wrong_sequence = self.dicom_info(
            copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
        )
        wrong_sequence.modules[C_ARM_PHOTON_ELECTRON_BEAM_MODULE][
            C_ARM_CONTROL_POINT_SEQUENCE
        ]["type"] = "2"
        mutations.append(wrong_sequence)
        wrong_include = self.dicom_info(
            copy.deepcopy(LOCKED_MALFORMED_RECORDED_DATETIME)
        )
        wrong_include.modules[RT_DELIVERY_CONTROL_POINT_MODULE]["include"] = []
        mutations.append(wrong_include)

        for info in mutations:
            with self.subTest(info=info):
                with self.assertRaisesRegex(
                    RuntimeError,
                    f"definition correction {DEFINITION_CORRECTION_ID} "
                    "path shape mismatch",
                ):
                    correct_locked_definition(info)

    def test_version_exposes_correction_identity(self) -> None:
        output = io.StringIO()
        with (
            patch.object(sys, "argv", ["dts-dicom-validator", "--version"]),
            redirect_stdout(output),
        ):
            main()
        self.assertIn(
            f"definition_correction={DEFINITION_CORRECTION_ID}", output.getvalue()
        )


if __name__ == "__main__":
    unittest.main()
