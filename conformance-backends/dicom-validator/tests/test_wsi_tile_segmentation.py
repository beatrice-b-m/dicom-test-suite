from __future__ import annotations

from copy import deepcopy
import os
from pathlib import Path
from types import SimpleNamespace

import pydicom
import pytest

from dts_dicom_validator_adapter.wsi_tile_segmentation import (
    LOCKED_MODULES,
    SEGMENTATION_GROUP_MACROS,
    SEGMENTATION_MACRO_MODULE,
    SEGMENTATION_STORAGE_UIDS,
    correct_segmentation_group_macros,
    verify_exact_case_functional_groups,
)


def locked_definition() -> SimpleNamespace:
    modules = {
        macro["ref"]: {"locked": True}
        for name, macro in SEGMENTATION_GROUP_MACROS.items()
        if name != "Segmentation"
    }
    iods = {
        sop_class_uid: {
            "title": "Segmentation IOD",
            "modules": deepcopy(LOCKED_MODULES),
            "group_macros": {},
        }
        for sop_class_uid in SEGMENTATION_STORAGE_UIDS
    }
    return SimpleNamespace(iods=iods, modules=modules)


def test_correction_restores_only_the_locked_segmentation_macro_table() -> None:
    dicom_info = locked_definition()
    correct_segmentation_group_macros(dicom_info)
    for sop_class_uid in SEGMENTATION_STORAGE_UIDS:
        assert dicom_info.iods[sop_class_uid]["group_macros"] == SEGMENTATION_GROUP_MACROS
    assert dicom_info.modules["C.8.20.3.1"] == SEGMENTATION_MACRO_MODULE


@pytest.mark.parametrize("drift", ["macros", "title", "module", "reference"])
def test_correction_fails_closed_on_definition_drift(drift: str) -> None:
    dicom_info = locked_definition()
    if drift == "macros":
        dicom_info.iods[SEGMENTATION_STORAGE_UIDS[0]]["group_macros"] = {
            "upstream": {"ref": "fixed", "use": "U"}
        }
    elif drift == "title":
        dicom_info.iods[SEGMENTATION_STORAGE_UIDS[0]]["title"] = "Changed"
    elif drift == "module":
        del dicom_info.iods[SEGMENTATION_STORAGE_UIDS[0]]["modules"][
            "Segmentation Image"
        ]
    else:
        del dicom_info.modules[SEGMENTATION_GROUP_MACROS["Pixel Measures"]["ref"]]
    with pytest.raises(RuntimeError, match="definition correction"):
        correct_segmentation_group_macros(dicom_info)


@pytest.mark.parametrize(
    ("scope", "keyword"),
    [
        ("shared", "PixelMeasuresSequence"),
        ("shared", "SegmentIdentificationSequence"),
        ("per-frame", "FrameContentSequence"),
        ("per-frame", "PlanePositionSlideSequence"),
        ("per-frame", "DerivationImageSequence"),
    ],
)
def test_real_generated_file_rejects_each_required_macro_removal(
    tmp_path: Path, scope: str, keyword: str
) -> None:
    configured = os.environ.get("DTS_M6_SEGMENTATION_FIXTURE")
    if not configured:
        pytest.skip("DTS_M6_SEGMENTATION_FIXTURE is required for generated-file qualification")
    fixture = Path(configured)
    assert fixture.is_file(), fixture
    verify_exact_case_functional_groups(fixture)
    dataset = pydicom.dcmread(fixture)
    if scope == "shared":
        delattr(dataset.SharedFunctionalGroupsSequence[0], keyword)
    else:
        for frame in dataset.PerFrameFunctionalGroupsSequence:
            delattr(frame, keyword)
    mutated = tmp_path / f"missing-{keyword}.dcm"
    dataset.save_as(mutated, enforce_file_format=True)
    with pytest.raises(RuntimeError, match=keyword):
        verify_exact_case_functional_groups(mutated)
