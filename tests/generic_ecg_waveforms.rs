//! Caller-owned ECG waveforms retain independent metadata, groups and sample semantics.
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::sdk::{
    CorpusSelector, DicomTestSuite, GenerateCorpusOutcome, GenerateCorpusRequest,
    InspectCorpusRequest, ReportRequest, ValidateRequest,
};

#[test]
fn caller_waveforms_are_reproducible_and_semantically_bound() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "caller-waveforms-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir(&root).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/generic-ecg-waveform-corpus");
    let definition: Value =
        serde_json::from_slice(&fs::read(fixture.join("definition.json")).unwrap()).unwrap();
    let descriptor = root.join("definition.json");
    fs::copy(fixture.join("definition.json"), &descriptor).unwrap();
    let members = root.join("members");
    for reference in std::iter::once(&definition["registry"])
        .chain(
            definition["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| &c["recipe"]),
        )
        .chain(definition["evidence"].as_array().unwrap())
    {
        let path = reference["path"].as_str().unwrap();
        let dest = members.join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(fixture.join("members").join(path), dest).unwrap();
    }
    let product = DicomTestSuite::embedded().unwrap();
    product.version().unwrap();
    let selector = || CorpusSelector::Profile {
        profile: "core".into(),
        include_stress: false,
    };
    product
        .inspect_corpus(
            InspectCorpusRequest::from_file(&descriptor, &members).with_selection(selector()),
        )
        .unwrap();
    let invoke = |args: &[&str]| {
        let result = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{args:?}: {} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice::<Value>(&result.stdout).unwrap()
    };
    invoke(&[
        "generate",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--seed",
        "23",
        "--parallelism",
        "2",
        "--out",
        "cli-output",
        "--format",
        "json",
    ]);
    let manifest_path = root.join("cli-output/manifest.json");
    let original_manifest = fs::read(&manifest_path).unwrap();
    let manifest: Value = serde_json::from_slice(&original_manifest).unwrap();
    assert_eq!(manifest["files"].as_array().unwrap().len(), 2);
    for name in ["sdk-output", "repeat-output"] {
        let GenerateCorpusOutcome::Published(output) = product
            .generate_corpus(
                GenerateCorpusRequest::from_file(
                    &descriptor,
                    &members,
                    root.join(name),
                    selector(),
                )
                .with_seed(23)
                .with_parallelism(2),
            )
            .unwrap()
        else {
            panic!("must publish")
        };
        assert_eq!(
            fs::read(output.output_root().join("manifest.json")).unwrap(),
            original_manifest
        );
        for file in manifest["files"].as_array().unwrap() {
            let path = file["path"].as_str().unwrap();
            assert_eq!(
                fs::read(root.join("cli-output").join(path)).unwrap(),
                fs::read(output.output_root().join(path)).unwrap()
            );
        }
        assert!(
            product
                .validate(ValidateRequest::new(output.output_root()))
                .unwrap()
                .is_valid()
        );
    }
    assert_eq!(
        invoke(&["validate", "cli-output", "--format", "json"])["result"]["valid"],
        true
    );
    let report = invoke(&[
        "report",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    assert_eq!(
        report["result"]["report"],
        product
            .report(ReportRequest::new(root.join("sdk-output")))
            .unwrap()
            .deserialize::<Value>()
            .unwrap()
    );
    for (index, file) in manifest["files"].as_array().unwrap().iter().enumerate() {
        let input: synth_dicom_gen::recipes::WaveformPlanInput = serde_json::from_value(
            file["recipe"]["recipe_parameters"]["waveform_contract"].clone(),
        )
        .unwrap();
        assert_eq!(file["expected_waveform"]["aggregate"]["group_count"], 2);
        for group in file["expected_waveform"]["multiplex_groups"]
            .as_array()
            .unwrap()
        {
            assert_ne!(group["storage"]["sample_min"], -1000);
            assert_ne!(group["storage"]["sample_max"], 1000);
        }
        for mode in 0..8 {
            let mut invalid = input.clone();
            match mode {
                0 => invalid
                    .caller_metadata
                    .as_mut()
                    .unwrap()
                    .instance_number
                    .clear(),
                1 => invalid.groups[0].sampling_frequency_hz = "199".into(),
                2 => invalid.groups[0].declared_sha256 = "0".repeat(64),
                3 => invalid.formula.modulus = 0,
                4 => {
                    invalid.groups[0].channels[0]
                        .caller_calibration
                        .as_mut()
                        .unwrap()
                        .sensitivity = "-1".into()
                }
                5 => invalid
                    .caller_metadata
                    .as_mut()
                    .unwrap()
                    .content_date
                    .clear(),
                6 => invalid
                    .caller_metadata
                    .as_mut()
                    .unwrap()
                    .content_time
                    .clear(),
                _ => invalid
                    .caller_metadata
                    .as_mut()
                    .unwrap()
                    .acquisition_datetime
                    .clear(),
            }
            assert!(synth_dicom_gen::recipes::validate_caller_waveform_input(&invalid).is_err());
        }
        let path = root.join("cli-output").join(file["path"].as_str().unwrap());
        let original = fs::read(&path).unwrap();
        for needle in [
            b"CALLER-TRACE-71".as_slice(),
            b"0.5 ".as_slice(),
            &[0x00, 0x54, 0x10, 0x10, b'O', b'W', 0, 0],
        ] {
            let mut bytes = original.clone();
            let mut offset = bytes
                .windows(needle.len())
                .position(|v| v == needle)
                .unwrap();
            if needle[0] == 0 {
                offset += 12;
            }
            bytes[offset] ^= 1;
            fs::write(&path, &bytes).unwrap();
            let mut changed = manifest.clone();
            changed["files"][index]["sha256"] = json!(synth_dicom_gen::sha256_hex(&bytes));
            fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
            let outcome = product.validate(ValidateRequest::new(root.join("cli-output")));
            assert!(outcome.is_err() || !outcome.unwrap().is_valid());
        }
        fs::write(&path, original).unwrap();
        for fields in [
            vec!["waveform_contract"],
            vec!["waveform_capability_version"],
            vec!["waveform_contract", "waveform_capability_version"],
        ] {
            let mut changed = manifest.clone();
            for field in fields {
                changed["files"][index]["recipe"]["recipe_parameters"]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
            }
            fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
            let outcome = product.validate(ValidateRequest::new(root.join("cli-output")));
            assert!(outcome.is_err() || !outcome.unwrap().is_valid());
            assert!(
                product
                    .report(ReportRequest::new(root.join("cli-output")))
                    .is_err()
            );
        }
    }
    fs::write(&manifest_path, original_manifest).unwrap();
    assert!(
        product
            .validate(ValidateRequest::new(root.join("cli-output")))
            .unwrap()
            .is_valid()
    );
}
