//! Caller waveform declaration agreement and actual multiplex-group validation.
use crate::recipes::WaveformPlanInput;
use dicom_core::{Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_object::InMemDicomObject;
use serde_json::{Value, json};
type Dataset = InMemDicomObject<StandardDataDictionary>;

fn input(file: &Value) -> Result<Option<WaveformPlanInput>, String> {
    let p = &file["recipe"]["recipe_parameters"];
    if p.get("waveform_capability_version").is_none() && p.get("waveform_contract").is_none() {
        let frozen_field = match file["dicom"]["sop_class_uid"].as_str() {
            Some("1.2.840.10008.5.1.4.1.1.9.1.1") => Some("expected_twelve_lead_ecg_waveform"),
            Some("1.2.840.10008.5.1.4.1.1.9.1.2") => Some("expected_general_ecg_waveform"),
            _ => None,
        };
        if let Some(field) = frozen_field {
            if !crate::manifest_contract::legacy_field_matches(field, &file["expected_waveform"])? {
                return Err("waveform requires a complete versioned declaration or frozen historical contract".into());
            }
        }
        return Ok(None);
    }
    if p["waveform_capability_version"] != "1.0.0" {
        return Err("missing/unsupported waveform capability version".into());
    }
    let input: WaveformPlanInput =
        serde_json::from_value(p["waveform_contract"].clone()).map_err(|e| e.to_string())?;
    crate::recipes::validate_caller_waveform_input(&input).map_err(|e| e.to_string())?;
    Ok(Some(input))
}
pub(crate) fn validate_manifest(file: &Value) -> Result<bool, String> {
    let Some(input) = input(file)? else {
        return Ok(false);
    };
    let projected = crate::recipes::project_waveform(&input)
        .map_err(|e| e.to_string())?
        .legacy_fields();
    if file.get("image") != Some(&Value::Null)
        || file.get("pixel_data") != Some(&Value::Null)
        || file["case_id"] != input.case_id
        || file["path"] != input.output_path
        || file["dicom"]["sop_class_uid"] != input.sop_class_uid
        || file["dicom"]["modality"] != "ECG"
        || file["dicom"]["transfer_syntax_uid"] != "1.2.840.10008.1.2.1"
        || file["references"] != json!([])
        || file["recipe"]["recipe_id"] != input.recipe.recipe_id
        || file["recipe"]["recipe_version"] != input.recipe.recipe_version
    {
        return Err("waveform identity/closure differs".into());
    }
    for key in [
        "expected_waveform",
        "expected_semantics",
        "expected_capabilities",
        "expected_visual_checks",
        "known_stressors",
    ] {
        if file[key] != projected[key] {
            return Err(format!("waveform projection differs: {key}"));
        }
    }
    for (key, value) in projected["recipe"]["recipe_parameters"]
        .as_object()
        .ok_or("waveform projected parameters missing")?
    {
        if file["recipe"]["recipe_parameters"][key] != *value {
            return Err(format!("waveform recipe parameter differs: {key}"));
        }
    }
    Ok(true)
}
fn text(obj: &Dataset, tag: Tag, vr: VR, expected: &str) -> Result<(), String> {
    let element = obj.element(tag).map_err(|e| e.to_string())?;
    if element.vr() != vr || element.to_str().map_err(|e| e.to_string())?.as_ref() != expected {
        return Err(format!("waveform value/VR differs: {tag}"));
    }
    Ok(())
}
fn integer(obj: &Dataset, tag: Tag, vr: VR, expected: u64) -> Result<(), String> {
    let element = obj.element(tag).map_err(|e| e.to_string())?;
    if element.vr() != vr || element.to_int::<u64>().map_err(|e| e.to_string())? != expected {
        return Err(format!("waveform integer/VR differs: {tag}"));
    }
    Ok(())
}
fn sequence(obj: &Dataset, tag: Tag) -> Result<&[Dataset], String> {
    let element = obj.element(tag).map_err(|e| e.to_string())?;
    if element.vr() != VR::SQ {
        return Err(format!("waveform sequence VR differs: {tag}"));
    }
    element
        .items()
        .map(|v| &v[..])
        .ok_or_else(|| format!("waveform sequence missing: {tag}"))
}
fn code(obj: &Dataset, tag: Tag, value: &str, scheme: &str, meaning: &str) -> Result<(), String> {
    let items = sequence(obj, tag)?;
    if items.len() != 1 {
        return Err("waveform code sequence cardinality differs".into());
    }
    text(&items[0], Tag(8, 0x0100), VR::SH, value)?;
    text(&items[0], Tag(8, 0x0102), VR::SH, scheme)?;
    text(&items[0], Tag(8, 0x0104), VR::LO, meaning)
}
pub(crate) fn validate_object(file: &Value, obj: &crate::OpenedObject) -> Result<(), String> {
    if !validate_manifest(file)? {
        return Ok(());
    }
    validate_parameters_object(&input(file)?.unwrap(), obj)
}
pub(crate) fn validate_parameters_object(
    input: &WaveformPlanInput,
    obj: &crate::OpenedObject,
) -> Result<(), String> {
    crate::recipes::validate_caller_waveform_input(input).map_err(|e| e.to_string())?;
    let metadata = serde_json::to_value(&input.caller_metadata).map_err(|e| e.to_string())?;
    for (group, element, vr, key) in [
        (0x10, 0x10, VR::PN, "patient_name"),
        (0x10, 0x20, VR::LO, "patient_id"),
        (0x10, 0x30, VR::DA, "patient_birth_date"),
        (0x10, 0x40, VR::CS, "patient_sex"),
        (8, 0x20, VR::DA, "study_date"),
        (8, 0x30, VR::TM, "study_time"),
        (8, 0x23, VR::DA, "content_date"),
        (8, 0x33, VR::TM, "content_time"),
        (8, 0x2a, VR::DT, "acquisition_datetime"),
        (8, 0x70, VR::LO, "manufacturer"),
        (0x18, 0x1020, VR::LO, "software_versions"),
        (0x20, 0x13, VR::IS, "instance_number"),
        (8, 0x90, VR::PN, "referring_physician_name"),
        (8, 0x50, VR::SH, "accession_number"),
        (8, 0x80, VR::LO, "institution_name"),
        (8, 0x81, VR::ST, "institution_address"),
    ] {
        text(
            obj,
            Tag(group, element),
            vr,
            metadata[key]
                .as_str()
                .ok_or("waveform caller metadata missing")?,
        )?;
    }
    for (tag, vr, value) in [
        (Tag(8, 0x16), VR::UI, input.sop_class_uid.as_str()),
        (Tag(8, 0x60), VR::CS, "ECG"),
        (Tag(8, 0x1c), VR::CS, "YES"),
        (Tag(0x20, 0x10), VR::SH, input.study_id.as_str()),
        (Tag(0x20, 0x11), VR::IS, input.series_number.as_str()),
        (
            Tag(8, 0x1090),
            VR::LO,
            input.manufacturer_model_name.as_str(),
        ),
        (
            Tag(0x18, 0x1000),
            VR::LO,
            input.device_serial_number.as_str(),
        ),
    ] {
        text(obj, tag, vr, value)?;
    }
    if !sequence(obj, Tag(0x40, 0x555))?.is_empty() {
        return Err("waveform Acquisition Context must be empty".into());
    }
    use dicom_dictionary_std::tags;
    for tag in [
        tags::WAVEFORM_ANNOTATION_SEQUENCE,
        tags::STRUCTURED_WAVEFORM_ANNOTATION_SEQUENCE,
        tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
        tags::SYNCHRONIZATION_TRIGGER,
        tags::SYNCHRONIZATION_CHANNEL,
        tags::ACQUISITION_TIME_SYNCHRONIZED,
        tags::TIME_SOURCE,
        tags::TIME_DISTRIBUTION_PROTOCOL,
        tags::NTP_SOURCE_ADDRESS,
        tags::REFERENCED_STUDY_SEQUENCE,
        tags::REFERENCED_SERIES_SEQUENCE,
        tags::REFERENCED_WAVEFORM_SEQUENCE,
        tags::REFERENCED_IMAGE_SEQUENCE,
        tags::REFERENCED_INSTANCE_SEQUENCE,
        tags::SOURCE_IMAGE_SEQUENCE,
        tags::ROWS,
        tags::COLUMNS,
        tags::SAMPLES_PER_PIXEL,
        tags::NUMBER_OF_FRAMES,
        tags::PHOTOMETRIC_INTERPRETATION,
        tags::BITS_ALLOCATED,
        tags::BITS_STORED,
        tags::HIGH_BIT,
        tags::PIXEL_REPRESENTATION,
        tags::PLANAR_CONFIGURATION,
        tags::PIXEL_DATA,
        tags::FLOAT_PIXEL_DATA,
        tags::DOUBLE_FLOAT_PIXEL_DATA,
        tags::FRAME_OF_REFERENCE_UID,
    ] {
        if obj.element(tag).is_ok() {
            return Err(format!("waveform excluded content present: {tag}"));
        }
    }
    let groups = sequence(obj, Tag(0x5400, 0x100))?;
    if groups.len() != input.groups.len() {
        return Err("waveform group count differs".into());
    }
    for (index, (group, expected)) in groups.iter().zip(&input.groups).enumerate() {
        for tag in [
            Tag(0x0018, 0x1068),
            Tag(0x0018, 0x1069),
            Tag(0x0018, 0x106e),
        ] {
            if group.element(tag).is_ok() {
                return Err(format!(
                    "waveform contains undeclared group timing attribute {tag}"
                ));
            }
        }

        text(group, Tag(0x3a, 4), VR::CS, "ORIGINAL")?;
        text(group, Tag(0x3a, 0x20), VR::SH, &expected.label)?;
        integer(group, Tag(0x3a, 5), VR::US, expected.channels.len() as u64)?;
        integer(
            group,
            Tag(0x3a, 0x10),
            VR::UL,
            expected.samples_per_channel.into(),
        )?;
        text(
            group,
            Tag(0x3a, 0x1a),
            VR::DS,
            &expected.sampling_frequency_hz,
        )?;
        integer(group, Tag(0x5400, 0x1004), VR::US, 16)?;
        text(group, Tag(0x5400, 0x1006), VR::CS, "SS")?;
        if group.element(Tag(0x5400, 0x100a)).is_ok() {
            return Err("waveform padding value must be absent".into());
        }
        let channels = sequence(group, Tag(0x3a, 0x200))?;
        if channels.len() != expected.channels.len() {
            return Err("waveform channel count differs".into());
        }
        for (ordinal, (channel, declared)) in channels.iter().zip(&expected.channels).enumerate() {
            text(
                channel,
                Tag(0x3a, 0x202),
                VR::IS,
                &(ordinal + 1).to_string(),
            )?;
            text(channel, Tag(0x3a, 0x203), VR::SH, &declared.label)?;
            code(
                channel,
                Tag(0x3a, 0x208),
                &declared.code_value,
                "MDC",
                &declared.code_meaning,
            )?;
            let calibration = declared
                .caller_calibration
                .as_ref()
                .ok_or("caller channel calibration missing")?;
            code(
                channel,
                Tag(0x3a, 0x211),
                &calibration.unit_code_value,
                &calibration.unit_coding_scheme,
                &calibration.unit_code_meaning,
            )?;
            for (tag, value) in [
                (Tag(0x3a, 0x210), calibration.sensitivity.as_str()),
                (
                    Tag(0x3a, 0x212),
                    calibration.sensitivity_correction_factor.as_str(),
                ),
                (Tag(0x3a, 0x213), calibration.baseline.as_str()),
                (Tag(0x3a, 0x214), calibration.time_skew_seconds.as_str()),
            ] {
                text(channel, tag, VR::DS, value)?;
            }
            integer(channel, Tag(0x3a, 0x21a), VR::US, 16)?;
            if channel.element(Tag(0x3a, 0x215)).is_ok() {
                return Err("waveform sample skew must be absent".into());
            }
        }
        let bytes =
            crate::recipes::caller_waveform_group_bytes(input, index).map_err(|e| e.to_string())?;
        let element = group
            .element(Tag(0x5400, 0x1010))
            .map_err(|e| e.to_string())?;
        if element.vr() != VR::OW
            || element.to_bytes().map_err(|e| e.to_string())?.as_ref() != bytes.as_slice()
        {
            return Err("waveform OW data differs".into());
        }
    }
    Ok(())
}
