use std::fs;

use serde_json::{Value, json};

const WSI_CASE_ID: &str = "vl/wsi/tiled_full_small";
const WSI_SPARSE_CASE_ID: &str = "vl/wsi/tiled_sparse_small";
const WSI_PYRAMID_CASE_ID: &str = "vl/wsi/pyramid_multiresolution";

fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON artifact"))
        .expect("parse JSON artifact")
}

#[test]
fn wsi_iod_and_reconstruction_routes_are_exact_uv_locked_and_additive() {
    let config = read_json("conformance/validators.json");
    let lock = read_json("conformance/validator-lock.json");
    let adapters = config["adapters"].as_array().unwrap();

    let iod = adapters
        .iter()
        .find(|adapter| adapter["id"] == "pydicom-dicom-validator-visible-light")
        .expect("visible-light IOD adapter");
    assert_eq!(
        iod["supported_case_ids"],
        json!([
            "vl/endoscopic/rgb_explicit_le",
            "vl/microscopic/rgb_explicit_le",
            WSI_CASE_ID,
            WSI_PYRAMID_CASE_ID
        ])
    );
    assert!(adapters.iter().any(|adapter| {
        adapter["id"] == "dicom3tools-dciodvfy" && adapter["role"] == "primary_iod_validator"
    }));

    let reconstruction = adapters
        .iter()
        .find(|adapter| adapter["id"] == "highdicom-wsi-reconstruction")
        .expect("WSI reconstruction adapter");
    assert_eq!(reconstruction["role"], "pixel_decoder");
    assert_eq!(
        reconstruction["supported_case_ids"],
        json!([WSI_CASE_ID, WSI_SPARSE_CASE_ID, WSI_PYRAMID_CASE_ID])
    );
    assert_eq!(
        reconstruction["executable_env"],
        "DTS_WSI_RECONSTRUCTION_PYTHON"
    );
    assert_eq!(
        reconstruction["arguments"],
        json!(["-m", "dts_wsi_reconstruction", "--input", "{input}"])
    );
    assert_eq!(
        reconstruction["group_arguments"],
        json!([
            "-m",
            "dts_wsi_reconstruction",
            "--group-input",
            "{group_input_1}",
            "--group-input",
            "{group_input_2}",
            "--group-input",
            "{group_input_3}"
        ])
    );
    assert_eq!(
        reconstruction["artifacts"],
        json!([
            {"path": "conformance-backends/wsi-reconstruction/.python-version"},
            {"path": "conformance-backends/wsi-reconstruction/pyproject.toml"},
            {"path": "conformance-backends/wsi-reconstruction/uv.lock"},
            {"path": "conformance-backends/wsi-reconstruction/src/dts_wsi_reconstruction/__init__.py"},
            {"path": "conformance-backends/wsi-reconstruction/src/dts_wsi_reconstruction/__main__.py"}
        ])
    );
    assert_eq!(
        reconstruction["capabilities"],
        json!([
            "tiled_full_implicit_positions",
            "tiled_sparse_explicit_positions",
            "dimension_index_validation",
            "exact_stored_frame_hashes",
            "pixel_data_payload_hash",
            "sparse_occupancy_and_absent_positions",
            "zero_sentinel_reconstruction",
            "total_pixel_matrix_reconstruction",
            "pyramid_group_role_derivation",
            "pyramid_uid_membership",
            "thumbnail_reduction",
            "label_companion_exclusion",
            "transforms_disabled"
        ])
    );

    let locked = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "highdicom-wsi-reconstruction")
        .expect("WSI reconstruction lock");
    assert_eq!(
        locked["adapter_sha256"],
        "9822f1672f41288e81725b4f9cd58ae3c4861e595c320514dcd827d8b045d4c8"
    );
    assert_eq!(
        locked["supporting_artifacts"]["uv.lock"],
        "9e4b7c03d240f549c9e0032c7f143d17c4dfb8bc9ae76dcf54f550f39842f238"
    );
    assert_eq!(
        locked["supporting_artifacts"]["adapter/__main__.py"],
        "20a43469d14a6a972830add077320a842259b5d8d81784380937fdcab5d798a5"
    );
    assert!(
        locked["version"]
            .as_str()
            .unwrap()
            .contains("adapter 0.3.0")
    );
}

#[test]
fn sparse_wsi_uses_locked_primary_authority_and_visible_characterization() {
    let config = read_json("conformance/validators.json");
    let lock = read_json("conformance/validator-lock.json");
    let adapters = config["adapters"].as_array().unwrap();

    let primary = adapters
        .iter()
        .find(|adapter| adapter["id"] == "pydicom-dicom-validator-wsi-sparse")
        .expect("sparse WSI primary adapter");
    assert_eq!(primary["role"], "primary_iod_validator");
    assert_eq!(primary["supported_case_ids"], json!([WSI_SPARSE_CASE_ID]));
    assert_eq!(primary["executable_env"], "DTS_DICOM_VALIDATOR_PYTHON");
    assert_eq!(primary["artifacts"].as_array().unwrap().len(), 14);

    let characterization = adapters
        .iter()
        .find(|adapter| adapter["id"] == "dicom3tools-dciodvfy-wsi-sparse-characterization")
        .expect("sparse WSI characterization adapter");
    assert_eq!(characterization["role"], "iod_characterization");
    assert_eq!(
        characterization["supported_case_ids"],
        json!([WSI_SPARSE_CASE_ID])
    );
    assert_eq!(characterization["expected_exit_code"], 1);
    assert_eq!(
        characterization["expected_findings"],
        json!([{
            "severity": "error",
            "message": "Error - </NumberOfFrames(0028,0008)> - NumberOfFrames does not match expected value for tiled total pixel matrix = <2 > - expected 4 for 1 optical paths, 1 focal planes, 2 rows of tiles, 2 columns of tiles"
        }])
    );

    let sparse_lock = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-wsi-sparse")
        .expect("sparse WSI primary lock");
    assert_eq!(sparse_lock["role"], "primary_iod_validator");
    assert_eq!(
        sparse_lock["adapter_sha256"],
        "2813c20e61cd625955429a999de42c52c9b1fec25f3e2a3b168dc0b41b46b72c"
    );
    assert_eq!(
        sparse_lock["supporting_artifacts"]["2026b/json/iod_info.json"],
        "ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683"
    );

    let characterization_lock = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "dicom3tools-dciodvfy-wsi-sparse-characterization")
        .expect("sparse WSI characterization lock");
    assert_eq!(characterization_lock["role"], "iod_characterization");
    assert_eq!(
        characterization_lock["executable_sha256"],
        "1aeb75d6ccd3f193e3b322b6da77742cdce2e0604868eaf2a2669c786cbc27e5"
    );

    assert!(!adapters.iter().any(|adapter| {
        adapter["id"] == "pydicom-dicom-validator-visible-light"
            && adapter["supported_case_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == WSI_SPARSE_CASE_ID))
    }));
}
