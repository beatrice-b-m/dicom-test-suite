use std::fs;

use serde_json::{Value, json};

const WSI_CASE_ID: &str = "vl/wsi/tiled_full_small";

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
            WSI_CASE_ID
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
    assert_eq!(reconstruction["supported_case_ids"], json!([WSI_CASE_ID]));
    assert_eq!(
        reconstruction["executable_env"],
        "DTS_WSI_RECONSTRUCTION_PYTHON"
    );
    assert_eq!(
        reconstruction["arguments"],
        json!(["-m", "dts_wsi_reconstruction", "--input", "{input}"])
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

    let locked = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "highdicom-wsi-reconstruction")
        .expect("WSI reconstruction lock");
    assert_eq!(
        locked["adapter_sha256"],
        "6b3f67bfc1aae4609ba7ccc399d78119e326556a64613621403b3b7b7a788716"
    );
    assert_eq!(
        locked["supporting_artifacts"]["uv.lock"],
        "0f7a560ec5a875c5a5bbc8bfcfd1f5223c4b770043319ebd15aa3cf0705d8882"
    );
    assert_eq!(
        locked["supporting_artifacts"]["adapter/__main__.py"],
        "5a06fab2ce499598cdff78adce3be355b4f03c8cdea7050f6f85f0bb3811fc94"
    );
}
