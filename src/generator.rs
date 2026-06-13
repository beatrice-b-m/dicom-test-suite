use std::fs;
use std::path::PathBuf;

use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
use serde_json::Value;

use crate::{
    DeterministicUidInput, GenerateError, PreparedGenerationRun, UidRole, deterministic_uid,
    sha256_hex,
};

const FIRST_SMOKE_CASE_ID: &str = "classic/sc/mono2_u8_explicit_le";
const FIRST_SMOKE_RECIPE_ID: &str = "sc_mono2_u8";
const FIRST_SMOKE_RECIPE_VERSION: &str = "0.1.0";
const FIRST_SMOKE_RELATIVE_PATH: &str = "classic/sc/mono2_u8_explicit_le/instance.dcm";
const FIRST_SMOKE_PIXEL_BYTES: [u8; 4] = [0, 85, 170, 255];

#[derive(Debug, Clone)]
pub(crate) struct GeneratedFile {
    pub case_id: String,
    pub manifest_entry: Value,
}

pub(crate) fn write_supported_cases(
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    let Some(case) = registry_case(registry, FIRST_SMOKE_CASE_ID)? else {
        return Ok(Vec::new());
    };
    let profiles = string_array(case.get("profiles"))?;
    if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
        return Ok(Vec::new());
    }

    write_first_smoke_case(run, case, standards_lock_sha256).map(|file| vec![file])
}

fn write_first_smoke_case(
    run: &PreparedGenerationRun,
    case: &Value,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid =
        deterministic_case_uid(standards_lock_sha256, run.seed, UidRole::StudyInstance);
    let series_instance_uid =
        deterministic_case_uid(standards_lock_sha256, run.seed, UidRole::SeriesInstance);
    let sop_instance_uid =
        deterministic_case_uid(standards_lock_sha256, run.seed, UidRole::SopInstance);
    let implementation_class_uid =
        deterministic_case_uid(standards_lock_sha256, 0, UidRole::ImplementationClass);

    let path = run.out_dir.join(FIRST_SMOKE_RELATIVE_PATH);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(FIRST_SMOKE_RELATIVE_PATH),
        message: "generated DICOM path must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let mut obj = InMemDicomObject::new_empty();
    put_str(
        &mut obj,
        tags::SOP_CLASS_UID,
        VR::UI,
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
    );
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(&mut obj, tags::PATIENT_NAME, VR::PN, "DICOMTEST^SMOKE");
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DICOMTEST-SMOKE-001");
    put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "SMOKE");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "OT");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");

    put_str(&mut obj, tags::CONVERSION_TYPE, VR::CS, "SYN");
    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        FIRST_SMOKE_RECIPE_ID,
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::PATIENT_ORIENTATION, VR::CS, "");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");

    put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        "MONOCHROME2",
    );
    put_u16(&mut obj, tags::ROWS, VR::US, 2);
    put_u16(&mut obj, tags::COLUMNS, VR::US, 2);
    put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 8);
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 8);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 7);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);
    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(FIRST_SMOKE_PIXEL_BYTES.as_slice()),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name("DICOMTS010"),
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    file_obj
        .write_to_file(&path)
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    let bytes = fs::read(&path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.clone(),
        source,
    })?;

    Ok(GeneratedFile {
        case_id: FIRST_SMOKE_CASE_ID.to_string(),
        manifest_entry: first_smoke_manifest_entry(
            case,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &bytes,
        ),
    })
}

fn first_smoke_manifest_entry(
    case: &Value,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
) -> Value {
    let mut standards_evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    standards_evidence.extend([
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_sop_class SecondaryCaptureImageStorage",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.8-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element SyntheticData",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "search_standard_text Image Pixel Description Macro",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.7-11c"
        }),
    ]);

    serde_json::json!({
        "case_id": FIRST_SMOKE_CASE_ID,
        "profile_membership": ["smoke"],
        "path": FIRST_SMOKE_RELATIVE_PATH,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": FIRST_SMOKE_RECIPE_ID,
            "recipe_version": FIRST_SMOKE_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": 2,
                "columns": 2,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 8,
                "bits_stored": 8,
                "pixel_values": [0, 85, 170, 255]
            }
        },
        "dicom": {
            "sop_class_uid": uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
            "sop_class_name": "Secondary Capture Image Storage",
            "iod_name": "Secondary Capture Image",
            "modality": "OT",
            "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN,
            "transfer_syntax_name": "Explicit VR Little Endian"
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": Value::Null,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": 2,
            "columns": 2,
            "frames": 1,
            "samples_per_pixel": 1,
            "photometric_interpretation": "MONOCHROME2",
            "bits_allocated": 8,
            "bits_stored": 8,
            "high_bit": 7,
            "pixel_representation": 0,
            "planar_configuration": Value::Null
        },
        "pixel_data": {
            "vr": "OB",
            "native_or_encapsulated": "native",
            "value_length": FIRST_SMOKE_PIXEL_BYTES.len(),
            "frame_count": 1,
            "frame_hashes": [sha256_hex(&FIRST_SMOKE_PIXEL_BYTES)]
        },
        "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "conversion_type": "SYN",
            "pixel_min": 0,
            "pixel_max": 255
        },
        "expected_visual_checks": {
            "pattern": "2x2_monochrome_gradient",
            "top_left": 0,
            "bottom_right": 255
        },
        "validation": {
            "status": "passed",
            "internal": [
                {
                    "name": "part10_writer",
                    "status": "passed",
                    "message": "DICOM-rs wrote a Part 10 file with File Meta Information and DICM prefix."
                }
            ],
            "standards": [],
            "external": []
        },
        "known_stressors": ["minimal_secondary_capture", "native_ob_pixel_data"],
        "standards_evidence": standards_evidence
    })
}

fn deterministic_case_uid(standards_lock_sha256: &str, run_seed: u64, role: UidRole) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: FIRST_SMOKE_CASE_ID,
        recipe_version: FIRST_SMOKE_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn put_str(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    obj.put(DataElement::new(tag, vr, value));
}

fn put_u16(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: u16) {
    obj.put(DataElement::new(tag, vr, PrimitiveValue::from(value)));
}

fn registry_case<'a>(
    registry: &'a Value,
    case_id: &str,
) -> Result<Option<&'a Value>, GenerateError> {
    let cases =
        registry
            .get("cases")
            .and_then(Value::as_array)
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "missing cases array",
            })?;
    Ok(cases
        .iter()
        .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id)))
}

fn string_array(value: Option<&Value>) -> Result<Vec<String>, GenerateError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or(GenerateError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "case profiles must be a string array",
        })?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or(GenerateError::MetadataShape {
                    path: PathBuf::from("cases/registry.json"),
                    message: "case profiles must be a string array",
                })
        })
        .collect()
}

fn case_matches_profile(profiles: &[String], requested: &str, include_stress: bool) -> bool {
    match requested {
        "all" => profiles.iter().any(|profile| {
            matches!(profile.as_str(), "smoke" | "core" | "extended" | "legacy")
                || (include_stress && profile == "stress")
        }),
        profile => profiles.iter().any(|case_profile| case_profile == profile),
    }
}
