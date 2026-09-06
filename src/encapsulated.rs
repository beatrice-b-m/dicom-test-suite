//! Versioned caller-owned encapsulated PDF/STL evidence and object validation.
use crate::recipes::EncapsulatedPayloadPlanInput;
use serde_json::{Value, json};

fn input(file: &Value) -> Result<Option<EncapsulatedPayloadPlanInput>, String> {
    let parameters = &file["recipe"]["recipe_parameters"];
    let marker = parameters.get("encapsulated_capability_version");
    let declaration = parameters.get("encapsulated_contract");
    if marker.is_none() && declaration.is_none() {
        if file["dicom"]["sop_class_uid"] == "1.2.840.10008.5.1.4.1.1.104.3"
            && file.get("expected_encapsulated_stl").is_none()
        {
            return Err("STL requires declared payload evidence".into());
        }
        if file["dicom"]["sop_class_uid"] == "1.2.840.10008.5.1.4.1.1.104.1" {
            // An absent caller marker is only the frozen original PDF contract.
            let frozen = json!({"document_title":"DTS Minimal Synthetic PDF","mime_type":"application/pdf",
                "document_length":327,"document_sha256":"4028af3714fa07d2f20e758649532faef11b4818c99a2b8dc0c88170a0dc8784",
                "burned_in_annotation":"NO","recognizable_visual_features":"NO"});
            if file["expected_semantics"]["encapsulated_document"] != frozen
                || *parameters != frozen
                || file["expected_semantics"]["conversion_type"] != "SYN"
            {
                return Err(
                    "PDF requires caller capability or complete historical payload evidence".into(),
                );
            }
        }
        return Ok(None);
    }
    if marker.and_then(Value::as_str) != Some("1.0.0") {
        return Err("missing/unsupported encapsulated capability version".into());
    }
    let input: EncapsulatedPayloadPlanInput =
        serde_json::from_value(declaration.cloned().unwrap_or(Value::Null))
            .map_err(|e| e.to_string())?;
    crate::recipes::validate_caller_encapsulated_input(&input).map_err(|e| e.to_string())?;
    Ok(Some(input))
}

pub(crate) fn validate_manifest(file: &Value) -> Result<bool, String> {
    let Some(input) = input(file)? else {
        return Ok(false);
    };
    let raw = crate::recipes::caller_encapsulated_bytes(&input).map_err(|e| e.to_string())?;
    let projection = serde_json::to_value(
        crate::recipes::project_encapsulated_payload(&input).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if file["case_id"] != input.case_id
        || file["path"] != input.output_path
        || file["recipe"]["recipe_id"] != input.recipe.recipe_id
        || file["recipe"]["recipe_version"] != input.recipe.recipe_version
        || file["dicom"]["sop_class_uid"] != input.sop_class_uid
        || file["dicom"]["modality"] != input.modality
        || file["dicom"]["transfer_syntax_uid"] != "1.2.840.10008.1.2.1"
        || file["references"] != json!([])
        || !file.get("image").is_some_and(Value::is_null)
        || !file.get("pixel_data").is_some_and(Value::is_null)
    {
        return Err("encapsulated identity/closure differs".into());
    }
    for key in [
        "expected_semantics",
        "expected_capabilities",
        "known_stressors",
    ] {
        if file[key] != projection[key] {
            return Err(format!("encapsulated projection differs: {key}"));
        }
    }
    if let Some(stl) = projection
        .get("expected_encapsulated_stl")
        .filter(|v| !v.is_null())
    {
        if file["expected_encapsulated_stl"] != *stl {
            return Err("encapsulated STL projection differs".into());
        }
    } else if file.get("expected_encapsulated_stl").is_some() {
        return Err("PDF declares STL evidence".into());
    }
    if file["expected_visual_checks"] != json!({"pattern":projection["expected_visual_pattern"]}) {
        return Err("encapsulated visual declaration differs".into());
    }
    if let Some(parameters) = projection["recipe_parameters"].as_object() {
        for (key, value) in parameters {
            if file["recipe"]["recipe_parameters"][key] != *value {
                return Err(format!("encapsulated recipe projection differs: {key}"));
            }
        }
    }
    let document = &file["expected_semantics"]["encapsulated_document"];
    if document["document_length"] != raw.len()
        || document["document_sha256"] != crate::sha256_hex(&raw)
    {
        return Err("encapsulated payload projection differs".into());
    }
    Ok(true)
}

pub(crate) fn validate_object(file: &Value, obj: &crate::OpenedObject) -> Result<(), String> {
    if !validate_manifest(file)? {
        return Ok(());
    }
    let input = input(file)?.unwrap();
    validate_parameters_object(&input, obj)?;
    if input.sop_class_uid == "1.2.840.10008.5.1.4.1.1.104.3" {
        let actual = obj
            .element(dicom_core::Tag(0x0020, 0x0052))
            .map_err(|e| e.to_string())?
            .to_str()
            .map_err(|e| e.to_string())?;
        if file["uids"]["frame_of_reference_uid"].as_str() != Some(actual.as_ref()) {
            return Err("STL Frame of Reference differs from manifest".into());
        }
    }
    Ok(())
}

pub(crate) fn validate_parameters_object(
    input: &EncapsulatedPayloadPlanInput,
    obj: &crate::OpenedObject,
) -> Result<(), String> {
    crate::recipes::validate_caller_encapsulated_input(input).map_err(|e| e.to_string())?;
    let raw = crate::recipes::caller_encapsulated_bytes(input).map_err(|e| e.to_string())?;
    if input.sop_class_uid == "1.2.840.10008.5.1.4.1.1.104.3" {
        stl_bounds(&raw)?;
    }
    let metadata = input
        .caller_metadata
        .as_ref()
        .ok_or("missing caller metadata")?;
    let stl = input.sop_class_uid == "1.2.840.10008.5.1.4.1.1.104.3";
    let check = |tag: dicom_core::Tag, vr: &str, expected: &str| -> Result<(), String> {
        let element = obj.element(tag).map_err(|e| e.to_string())?;
        if element.vr().to_string() != vr
            || element.to_str().map_err(|e| e.to_string())?.as_ref() != expected
        {
            return Err(format!("encapsulated reopened text/VR differs: {tag}"));
        }
        Ok(())
    };
    for (group, element, vr, value) in [
        (0x0010, 0x0010, "PN", input.patient_name.as_str()),
        (0x0010, 0x0020, "LO", input.patient_id.as_str()),
        (0x0010, 0x0030, "DA", metadata.patient_birth_date.as_str()),
        (0x0010, 0x0040, "CS", metadata.patient_sex.as_str()),
        (0x0008, 0x0020, "DA", metadata.study_date.as_str()),
        (0x0008, 0x0030, "TM", metadata.study_time.as_str()),
        (0x0008, 0x0023, "DA", metadata.content_date.as_str()),
        (0x0008, 0x0033, "TM", metadata.content_time.as_str()),
        (
            0x0008,
            0x0090,
            "PN",
            metadata.referring_physician_name.as_str(),
        ),
        (0x0008, 0x0050, "SH", metadata.accession_number.as_str()),
        (0x0008, 0x0070, "LO", metadata.manufacturer.as_str()),
        (0x0018, 0x1020, "LO", metadata.software_versions.as_str()),
        (0x0020, 0x0010, "SH", input.study_id.as_str()),
        (0x0020, 0x0011, "IS", input.series_number.as_str()),
        (0x0020, 0x0013, "IS", metadata.instance_number.as_str()),
        (0x0008, 0x103e, "LO", input.series_description.as_str()),
        (0x0008, 0x1090, "LO", input.manufacturer_model_name.as_str()),
        (0x0018, 0x1000, "LO", input.device_serial_number.as_str()),
        (0x0008, 0x0016, "UI", input.sop_class_uid.as_str()),
        (0x0008, 0x0060, "CS", input.modality.as_str()),
        (0x0008, 0x002a, "DT", input.acquisition_datetime.as_str()),
        (0x0028, 0x0301, "CS", input.burned_in_annotation.as_str()),
        (
            0x0028,
            0x0302,
            "CS",
            input.recognizable_visual_features.as_str(),
        ),
        (0x0042, 0x0010, "ST", input.document_title.as_str()),
        (0x0008, 0x001c, "CS", "YES"),
        (
            0x0042,
            0x0012,
            "LO",
            if stl { "model/stl" } else { "application/pdf" },
        ),
    ] {
        check(dicom_core::Tag(group, element), vr, value)?;
    }
    let length = obj
        .element(dicom_core::Tag(0x0042, 0x0015))
        .map_err(|e| e.to_string())?;
    if length.vr() != dicom_core::VR::UL
        || length.to_int::<u64>().map_err(|e| e.to_string())? != raw.len() as u64
    {
        return Err("encapsulated unpadded document length differs".into());
    }
    let payload = obj
        .element(dicom_core::Tag(0x0042, 0x0011))
        .map_err(|e| e.to_string())?;
    let mut padded = raw;
    if padded.len() % 2 != 0 {
        padded.push(0);
    }
    if payload.vr() != dicom_core::VR::OB
        || payload.to_bytes().map_err(|e| e.to_string())?.as_ref() != padded.as_slice()
    {
        return Err("encapsulated payload or zero padding differs".into());
    }
    for tag in [
        dicom_core::Tag(0x7fe0, 0x0010),
        dicom_core::Tag(0x7fe0, 0x0008),
        dicom_core::Tag(0x7fe0, 0x0009),
    ] {
        if obj.element(tag).is_ok() {
            return Err("encapsulated object contains pixel data".into());
        }
    }
    let concept = obj
        .element(dicom_core::Tag(0x0040, 0xa043))
        .map_err(|e| e.to_string())?;
    if concept.vr() != dicom_core::VR::SQ {
        return Err("Concept Name VR must be SQ".into());
    }
    let items = concept.items().ok_or("concept name is not SQ")?;
    if stl {
        if items.len() != 1 {
            return Err("STL concept item count differs".into());
        }
        for (tag, vr, value) in [
            (dicom_core::Tag(8, 0x0100), dicom_core::VR::SH, "129006"),
            (dicom_core::Tag(8, 0x0102), dicom_core::VR::SH, "DCM"),
            (
                dicom_core::Tag(8, 0x0104),
                dicom_core::VR::LO,
                "Anatomical Model",
            ),
        ] {
            if items[0].element(tag).map_err(|e| e.to_string())?.vr() != vr {
                return Err("STL concept code VR differs".into());
            }
            if items[0]
                .element(tag)
                .map_err(|e| e.to_string())?
                .to_str()
                .map_err(|e| e.to_string())?
                .as_ref()
                != value
            {
                return Err("STL concept differs".into());
            }
        }
        for (g, e, vr, value) in [
            (8, 0x0012, "DA", metadata.instance_creation_date.as_str()),
            (8, 0x0013, "TM", metadata.instance_creation_time.as_str()),
            (
                0x0020,
                0x1040,
                "LO",
                metadata.position_reference_indicator.as_str(),
            ),
            (
                0x0070,
                0x0081,
                "LO",
                input.content_description.as_deref().unwrap_or(""),
            ),
        ] {
            check(dicom_core::Tag(g, e), vr, value)?;
        }
        check(dicom_core::Tag(0x0068, 0x7001), "CS", "NO")?;
        check(dicom_core::Tag(0x0068, 0x7002), "CS", "NO")?;
        check(dicom_core::Tag(0x0008, 0x0005), "CS", "ISO_IR 192")?;
        if obj.element(dicom_core::Tag(0x0008, 0x0064)).is_ok() {
            return Err("STL contains Conversion Type".into());
        }
        let uid = obj
            .element(dicom_core::Tag(0x0020, 0x0052))
            .map_err(|e| e.to_string())?;
        if uid.vr() != dicom_core::VR::UI || uid.to_str().map_err(|e| e.to_string())?.is_empty() {
            return Err("STL Frame of Reference missing".into());
        }
        let encoded = serde_json::to_value(&input.payload).map_err(|e| e.to_string())?;
        let units = obj
            .element(dicom_core::Tag(0x0040, 0x08ea))
            .map_err(|e| e.to_string())?;
        if units.vr() != dicom_core::VR::SQ {
            return Err("STL units VR must be SQ".into());
        }
        let units = units.items().ok_or("STL units are not SQ")?;
        if units.len() != 1 {
            return Err("STL units cardinality differs".into());
        }
        for (tag, vr, key) in [
            (
                dicom_core::Tag(8, 0x0100),
                dicom_core::VR::SH,
                "unit_code_value",
            ),
            (
                dicom_core::Tag(8, 0x0102),
                dicom_core::VR::SH,
                "unit_coding_scheme",
            ),
            (
                dicom_core::Tag(8, 0x0104),
                dicom_core::VR::LO,
                "unit_code_meaning",
            ),
        ] {
            if units[0].element(tag).map_err(|e| e.to_string())?.vr() != vr {
                return Err("STL units code VR differs".into());
            }
            if units[0]
                .element(tag)
                .map_err(|e| e.to_string())?
                .to_str()
                .map_err(|e| e.to_string())?
                .as_ref()
                != encoded[key].as_str().ok_or("missing units")?
            {
                return Err("STL units differ".into());
            }
        }
    } else {
        if !items.is_empty() {
            return Err("PDF Concept Name must be empty".into());
        }
        check(
            dicom_core::Tag(8, 0x0012),
            "DA",
            &metadata.instance_creation_date,
        )?;
        check(
            dicom_core::Tag(8, 0x0013),
            "TM",
            &metadata.instance_creation_time,
        )?;
        if obj.element(dicom_core::Tag(0x0020, 0x1040)).is_ok() {
            return Err("PDF contains Position Reference Indicator".into());
        }
        check(dicom_core::Tag(8, 0x0064), "CS", "SYN")?;
        if obj.element(dicom_core::Tag(0x0020, 0x0052)).is_ok() {
            return Err("PDF contains Frame of Reference".into());
        }
    }
    Ok(())
}

/// Structural binary STL bounds; no manifold, winding or degeneracy claim.
pub(crate) fn stl_bounds(bytes: &[u8]) -> Result<([f64; 3], [f64; 3]), String> {
    crate::recipes::caller_stl_bounds(bytes).map_err(|e| e.to_string())
}
