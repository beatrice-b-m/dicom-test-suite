use std::fs;

use serde_json::Value;

const IOD_CASE_IDS: &[&str] = &[
    "vl/endoscopic/rgb_explicit_le",
    "vl/microscopic/rgb_explicit_le",
    "vl/wsi/tiled_full_small",
    "vl/wsi/pyramid_multiresolution",
];
const PIXEL_CASE_IDS: &[&str] = &[
    "vl/endoscopic/rgb_explicit_le",
    "vl/microscopic/rgb_explicit_le",
];

fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON artifact"))
        .expect("parse JSON artifact")
}

#[test]
fn visible_light_iod_route_is_additive_exact_case_and_uv_locked() {
    let config = read_json("conformance/validators.json");
    let lock = read_json("conformance/validator-lock.json");
    let adapter = config["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "pydicom-dicom-validator-visible-light")
        .expect("VL IOD adapter");
    assert_eq!(adapter["role"], "secondary_iod_validator");
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!(IOD_CASE_IDS)
    );
    assert_eq!(adapter["executable_env"], "DTS_DICOM_VALIDATOR_PYTHON");
    assert!(
        adapter["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "conformance-backends/dicom-validator/standard-lock.json")
    );
    assert!(
        config["adapters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|adapter| adapter["id"] == "dicom3tools-dciodvfy"
                && adapter["role"] == "primary_iod_validator")
    );
    let locked = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-visible-light")
        .expect("VL IOD lock");
    assert_eq!(
        locked["adapter_sha256"],
        "2813c20e61cd625955429a999de42c52c9b1fec25f3e2a3b168dc0b41b46b72c"
    );
    assert_eq!(
        locked["supporting_artifacts"]["uv.lock"],
        "988c01b0da2b433a4a26cb566cbbcfb4f18b31099ddd679520119c47309afdc0"
    );
    assert_eq!(
        locked["supporting_artifacts"]["2026b/json/iod_info.json"],
        "ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683"
    );
}

#[test]
fn visible_light_pixel_and_parser_routes_are_exact_and_locked() {
    let config = read_json("conformance/validators.json");
    let lock = read_json("conformance/validator-lock.json");
    let adapter = config["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "dcmtk-dcm2img-visible-light")
        .expect("VL pixel adapter");
    assert_eq!(adapter["role"], "pixel_decoder");
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!(PIXEL_CASE_IDS)
    );
    assert_eq!(
        adapter["arguments"],
        serde_json::json!([
            "+F", "1", "-S", "-bs", "-M", "-W", "+Pid", "-O", "+op", "{input}", "{output}"
        ])
    );
    let parser = config["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "dcmtk-dcmdump")
        .expect("DCMTK parser");
    assert_eq!(parser["role"], "independent_parser");
    assert_eq!(parser["arguments"], serde_json::json!(["+fo", "{input}"]));
    let decoder_lock = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "dcmtk-dcm2img-visible-light")
        .expect("VL pixel lock");
    let parser_lock = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "dcmtk-dcmdump")
        .expect("parser lock");
    assert_eq!(
        decoder_lock["executable_sha256"],
        "6a6103a7c516814b5eb44f53d198b111cbaf1678de5952ab7d31961732f112d5"
    );
    assert_eq!(
        parser_lock["executable_sha256"],
        "d2261944ea1ceb6743df9866f2237014b284fa39119c8a5eee226ae922ead45f"
    );
}
