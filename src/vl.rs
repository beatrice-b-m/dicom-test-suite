//! Caller-owned native single-frame visible-light manifest and reopening checks.
use crate::recipes::classic_vl_projection::{VlArtifactParameters, VlProviderParameters};
use crate::recipes::{AttributeOperation, ClassicIccProjection};
use serde_json::{Value, json};

fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}
fn declaration(
    file: &Value,
) -> Result<
    Option<(
        VlProviderParameters,
        VlArtifactParameters,
        Vec<AttributeOperation>,
        Option<ClassicIccProjection>,
    )>,
    String,
> {
    let p = &file["recipe"]["recipe_parameters"];
    if p.get("vl_capability_version").is_none()
        && ["vl_provider", "vl_artifact", "icc_projection"]
            .iter()
            .all(|k| p.get(k).is_none())
    {
        let sop = file["dicom"]["sop_class_uid"].as_str().unwrap_or("");
        if matches!(
            sop,
            "1.2.840.10008.5.1.4.1.1.77.1.1"
                | "1.2.840.10008.5.1.4.1.1.77.1.2"
                | "1.2.840.10008.5.1.4.1.1.77.1.4"
        ) {
            // Frozen recipe0.1 native VL admitted only this image shape. Wider
            // caller inputs require their versioned declaration even if stripped.
            if file["image"]["rows"] != 2
                || file["image"]["columns"] != 2
                || (sop != "1.2.840.10008.5.1.4.1.1.77.1.4"
                    && file.get("expected_vl_single_frame").is_none())
            {
                return Err("VL image requires its versioned declaration".into());
            }
        }
        return Ok(None);
    }
    if p["vl_capability_version"] != "1.0.0" {
        return Err("missing/unsupported VL capability version".into());
    }
    Ok(Some((
        decode(&p["vl_provider"])?,
        decode(&p["vl_artifact"])?,
        decode(&p["metadata_overrides"])?,
        p.get("icc_projection")
            .filter(|v| !v.is_null())
            .map(decode)
            .transpose()?,
    )))
}
fn fields(p: &VlProviderParameters) -> [(&'static str, &'static str, &str); 9] {
    [
        ("0010,0010", "PN", &p.patient_name),
        ("0010,0020", "LO", &p.patient_id),
        ("0010,0030", "DA", &p.patient_birth_date),
        ("0010,0040", "CS", &p.patient_sex),
        ("0008,0020", "DA", &p.study_date),
        ("0008,0030", "TM", &p.study_time),
        ("0020,0010", "SH", &p.study_id),
        ("0008,0070", "LO", &p.manufacturer),
        ("0018,1020", "LO", &p.software_versions),
    ]
}
fn profile_bytes(a: &VlArtifactParameters) -> Result<Option<Vec<u8>>, String> {
    a.icc_profile_hex
        .as_ref()
        .map(|s| {
            if s.len() > 2 * 1024 * 1024 || s.len() % 2 != 0 || !s.is_ascii() {
                return Err("invalid ICC hex bound".into());
            }
            s.as_bytes()
                .chunks_exact(2)
                .map(|b| {
                    u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16)
                        .map_err(|_| "invalid ICC hex".into())
                })
                .collect()
        })
        .transpose()
}
pub(crate) fn validate_manifest(file: &Value) -> Result<bool, String> {
    let Some((p, a, ops, icc)) = declaration(file)? else {
        return Ok(false);
    };
    crate::recipes::validate_caller_vl_parameters(&p, &a, &ops, icc.as_ref())
        .map_err(|error| error.to_string())?;
    let sop = a.sop_class_uid.as_str();
    let params = &file["recipe"]["recipe_parameters"];
    for (key, value) in [
        ("rows", json!(a.rows)),
        ("columns", json!(a.columns)),
        ("samples_per_pixel", json!(3)),
        ("photometric_interpretation", json!("RGB")),
        ("bits_allocated", json!(8)),
        ("bits_stored", json!(8)),
        ("planar_configuration", json!(0)),
        ("pixel_values", json!(a.stored_values)),
        ("palette", Value::Null),
        ("pixel_padding", Value::Null),
        ("body_part_examined", json!(a.body_part_examined)),
        ("laterality", json!(a.laterality)),
    ] {
        if params[key] != value {
            return Err(format!("VL recipe projection differs: {key}"));
        }
    }
    if file["references"] != json!([]) {
        return Err("VL references must be empty".into());
    }

    let pixels = a.stored_values.iter().map(|v| *v as u8).collect::<Vec<_>>();
    let hash = crate::sha256_hex(&pixels);
    if a.frame_sha256 != hash
        || Some(&a.pixel_min) != a.stored_values.iter().min()
        || Some(&a.pixel_max) != a.stored_values.iter().max()
    {
        return Err("VL pixel extrema/hash differ".into());
    }
    let image = json!({"rows":a.rows,"columns":a.columns,"frames":1,"samples_per_pixel":3,"photometric_interpretation":"RGB","bits_allocated":8,"bits_stored":8,"high_bit":7,"pixel_representation":0,"planar_configuration":0});
    if file["image"] != image
        || file["dicom"]["sop_class_uid"] != sop
        || file["dicom"]["modality"] != a.modality
        || file["dicom"]["transfer_syntax_uid"] != "1.2.840.10008.1.2.1"
        || file["pixel_data"]
            != json!({"vr":"OB","native_or_encapsulated":"native","value_length":pixels.len()+pixels.len()%2,"frame_count":1,"frame_hashes":[hash]})
    {
        return Err("VL image/pixel manifest differs".into());
    }
    for (key, value) in [
        ("pixel_min", json!(a.pixel_min)),
        ("pixel_max", json!(a.pixel_max)),
        ("image_type", json!("ORIGINAL\\PRIMARY")),
        ("body_part_examined", json!(a.body_part_examined)),
        ("laterality", json!(a.laterality)),
        ("synthetic_data", json!("YES")),
        ("lossy_image_compression", json!("00")),
    ] {
        if file["expected_semantics"][key] != value {
            return Err(format!("VL semantic field differs: {key}"));
        }
    }
    let raw = profile_bytes(&a)?;
    match (&raw, &a.icc_profile_sha256, &icc) {
        (Some(bytes), Some(sha), Some(projection)) => {
            crate::icc::validate_profile(bytes, projection, sha, a.color_space.as_deref())?;
            let mut expected = json!(projection);
            expected.as_object_mut().unwrap().extend([
                ("profile_sha256".into(), json!(sha)),
                ("profile_size_bytes".into(), json!(bytes.len())),
                ("declared_profile_size_bytes".into(), json!(bytes.len())),
                ("color_space".into(), Value::Null),
            ]);
            if file["expected_icc_profile"] != expected {
                return Err("VL ICC projection differs".into());
            }
        }
        (None, None, None) if a.color_space.is_none() => {
            if file.get("expected_icc_profile").is_some() {
                return Err("undeclared ICC projection".into());
            }
        }
        _ => return Err("incomplete VL ICC declaration".into()),
    }
    let mut absent = vec![
        "number_of_frames",
        "frame_of_reference_uid",
        "specimen_module",
        "optical_path_module",
    ];
    if raw.is_none() {
        absent.push("icc_profile_module");
    }
    let mut single = image;
    single.as_object_mut().unwrap().remove("frames");
    let expected = json!({"iod_kind":match a.modality.as_str(){"XC"=>"vl_photographic_single_frame","ES"=>"vl_endoscopic_single_frame",_=>"vl_microscopic_single_frame"},"sop_class_uid":sop,"sop_class_name":file["dicom"]["sop_class_name"],"iod_name":file["dicom"]["iod_name"],"modality":a.modality,"transfer_syntax_uid":"1.2.840.10008.1.2.1","image_type":["ORIGINAL","PRIMARY"],"body_part_examined":a.body_part_examined,"laterality":a.laterality,"acquisition_context_items":0,"image":single,"absent_content":absent});
    if file["expected_vl_single_frame"] != expected {
        return Err("VL single-frame projection differs".into());
    }
    Ok(true)
}

pub(crate) fn validate_object(file: &Value, obj: &crate::OpenedObject) -> Result<(), String> {
    if !validate_manifest(file)? {
        return Ok(());
    }
    let (p, a, ops, icc) = declaration(file)?.unwrap();
    validate_parameters_object(&p, &a, &ops, icc.as_ref(), obj)
}

pub(crate) fn validate_parameters_object(
    p: &VlProviderParameters,
    a: &VlArtifactParameters,
    ops: &[AttributeOperation],
    icc: Option<&ClassicIccProjection>,
    obj: &crate::OpenedObject,
) -> Result<(), String> {
    crate::recipes::classic_vl_projection::validate_caller_vl_parameters(p, a, ops, icc)
        .map_err(|error| error.to_string())?;
    let check = |tag: &str, vr: &str, value: &str| -> Result<(), String> {
        let tag = crate::composition::AttributeAddress::from_normalized_tag(tag)
            .map_err(|e| e.to_string())?
            .tag();
        let element = obj.element(tag).map_err(|e| e.to_string())?;
        if element.vr().to_string() != vr
            || element.to_str().map_err(|e| e.to_string())?.as_ref() != value
        {
            return Err(format!("VL reopened text/VR differs: {tag}"));
        }
        Ok(())
    };
    for (tag, vr, value) in fields(p) {
        check(tag, vr, value)?;
    }
    for op in ops {
        check(
            &op.tag,
            op.vr.as_deref().unwrap(),
            op.value.as_ref().and_then(Value::as_str).unwrap(),
        )?;
    }
    for (tag, vr, value) in [
        ("0008,0008", "CS", "ORIGINAL\\PRIMARY"),
        ("0008,001C", "CS", "YES"),
        ("0028,2110", "CS", "00"),
        ("0018,0015", "CS", a.body_part_examined.as_deref().unwrap()),
        ("0020,0060", "CS", a.laterality.as_deref().unwrap()),
    ] {
        check(tag, vr, value)?;
    }
    let context = obj
        .element(dicom_core::Tag(0x0040, 0x0555))
        .map_err(|e| e.to_string())?;
    if context.vr() != dicom_core::VR::SQ || !context.items().is_some_and(|items| items.is_empty())
    {
        return Err("VL Acquisition Context must be an empty SQ".into());
    }
    for tag in [
        dicom_core::Tag(0x0028, 0x0008),
        dicom_core::Tag(0x0020, 0x0052),
        dicom_core::Tag(0x0040, 0x0512),
        dicom_core::Tag(0x0040, 0x0513),
        dicom_core::Tag(0x0040, 0x0560),
        dicom_core::Tag(0x0048, 0x0105),
        dicom_core::Tag(0x0048, 0x0106),
        dicom_core::Tag(0x0048, 0x0207),
        dicom_core::Tag(0x0028, 0x2002),
    ] {
        if obj.element(tag).is_ok() {
            return Err(format!("VL contains excluded attribute {tag}"));
        }
    }
    let profile = profile_bytes(a)?;
    if let Some(raw) = profile {
        let element = obj
            .element(dicom_core::Tag(0x0028, 0x2000))
            .map_err(|e| e.to_string())?;
        let actual = element.to_bytes().map_err(|e| e.to_string())?;
        if element.vr() != dicom_core::VR::OB || actual.as_ref() != raw.as_slice() {
            return Err("VL ICC reopened bytes differ".into());
        }
        crate::icc::validate_profile(
            actual.as_ref(),
            icc.unwrap(),
            a.icc_profile_sha256.as_deref().unwrap(),
            None,
        )?;
    } else if obj.element(dicom_core::Tag(0x0028, 0x2000)).is_ok() {
        return Err("VL contains undeclared ICC profile".into());
    }
    let mut pixels = a.stored_values.iter().map(|v| *v as u8).collect::<Vec<_>>();
    if pixels.len() % 2 != 0 {
        pixels.push(0);
    }
    let element = obj
        .element(dicom_core::Tag(0x7fe0, 0x0010))
        .map_err(|e| e.to_string())?;
    if element.vr() != dicom_core::VR::OB
        || element.to_bytes().map_err(|e| e.to_string())?.as_ref() != pixels.as_slice()
    {
        return Err("VL reopened native pixels differ".into());
    }
    Ok(())
}
