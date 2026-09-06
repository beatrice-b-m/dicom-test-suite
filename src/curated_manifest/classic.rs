//! Typed compatibility projection for plan-first classic image artifacts.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::composition::CompositionUidRole;
use crate::corpus_plan::{OffsetTablePolicy, PlannedArtifact, PlannedDicomArtifact};
use crate::curated_plan::CuratedArtifactProjectionContext;
use crate::executor::adapters::ManifestProjectionArtifact;
use crate::executor::evidence::{
    ArtifactExecutionEvidence, MaterializedContentEvidence, ResultStatus,
};
use crate::recipes::classic_ct::{ClassicCtInstanceNumber, inspect_ct_capability};
use crate::recipes::classic_dx_mg::DxMgArtifactParameters;
use crate::recipes::classic_mr_cr::{CrArtifactParameters, inspect_mr_capability};
use crate::recipes::classic_nuclear::{
    ClassicNuclearArtifactParameters, ClassicNuclearPixels, ClassicNuclearProviderParameters,
};
use crate::recipes::classic_vl_projection::{
    ProjectionArtifactParameters, VlArtifactParameters, VlPhotometricInterpretation,
};
use crate::recipes::{ClassicProjectionFamily, ClassicSemanticLabels};

use super::{
    CuratedManifestError, err, fail, legacy_validation, only, required, transfer_syntax_name, uid,
    validation_checks,
};

const RLE: &str = "1.2.840.10008.1.2.5";

struct Facts {
    recipe: Value,
    image: Value,
    semantics: Value,
    specials: Map<String, Value>,
}

pub(super) fn project_classic_file_entry(
    context: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
) -> Result<serde_json::Value, CuratedManifestError> {
    let PlannedArtifact::Dicom(planned) = &pair.planned else {
        return fail("classic artifact is not DICOM");
    };
    let execution = &pair.execution;
    let output = execution
        .output
        .as_ref()
        .ok_or_else(|| err("missing output evidence"))?;
    let materialization = execution
        .materialization
        .as_ref()
        .ok_or_else(|| err("missing materialization evidence"))?;
    if output.relative_path != planned.output.relative_path.as_str()
        || output.relative_path != context.artifact_recipe.output.path.as_deref().unwrap_or("")
        || !output.publish
        || materialization.transfer_syntax_uid.as_deref()
            != Some(&planned.encoding.transfer_syntax_uid)
        || materialization.implementation_class_uid.as_deref()
            != Some(&planned.encoding.implementation.class_uid)
        || materialization.materialized_artifact_sha256.as_deref() != Some(&output.sha256)
    {
        return fail("classic output/materialization evidence differs from plan");
    }
    let pixels = materialization
        .content
        .iter()
        .find(|item| item.slot == "pixels")
        .ok_or_else(|| err("missing classic pixel evidence"))?;
    let projection = context
        .artifact_recipe
        .classic_projection
        .as_ref()
        .ok_or_else(|| err("missing classic_projection contract"))?;
    let facts = family_facts(context, &projection.family)?;
    let mut checks = validation_checks(execution)?;
    checks.retain(|check| check.name != "classic_ct_group_topology");
    if projection.include_implementation_version_name {
        checks.retain(|check| {
            !matches!(
                check.name.as_str(),
                "native_frame_hash_count" | "native_frame_hashes"
            )
        });
    }
    if projection.icc.is_some() {
        let index = checks
            .iter()
            .position(|check| check.name == "icc_profile_round_trip")
            .ok_or_else(|| err("missing typed ICC validation check"))?;
        let mut check = checks.remove(index);
        check.message = "ICC Profile OB bytes, DICOM-constrained header, tag table, hash, and SRGB declaration match.".into();
        checks.push(check);
    }
    let mut manifest = json!({
        "case_id":context.registry_case.case_id,
        "profile_membership":context.artifact_recipe.public_profile_membership.as_ref().unwrap_or(&context.registry_case.profiles),
        "path":output.relative_path,"sha256":output.sha256,"size_bytes":output.size_bytes,
        "determinism":context.registry_case.determinism,
        "recipe":{"recipe_id":context.case_recipe.recipe_id,"recipe_version":context.case_recipe.recipe_version,"recipe_parameters":facts.recipe},
        "dicom":{"sop_class_uid":required(&context.registry_case.sop_class_uid,"registry SOP Class UID")?,
            "sop_class_name":required(&context.registry_case.sop_class_name,"registry SOP Class name")?,
            "iod_name":required(&context.registry_case.iod_name,"registry IOD name")?,"modality":required(&context.registry_case.modality,"registry modality")?,
            "transfer_syntax_uid":planned.encoding.transfer_syntax_uid,"transfer_syntax_name":transfer_syntax_name(&planned.encoding.transfer_syntax_uid)?},
        "uids":{"study_instance_uid":uid(planned,CompositionUidRole::StudyInstance)?,"series_instance_uid":uid(planned,CompositionUidRole::SeriesInstance)?,
            "sop_instance_uid":uid(planned,CompositionUidRole::SopInstance)?,"frame_of_reference_uid":planned.instance.identities.get(&CompositionUidRole::FrameOfReference,0),
            "implementation_class_uid":planned.encoding.implementation.class_uid},
        "image":facts.image,"pixel_data":pixel_data(planned,execution,pixels)?,
        "expected_capabilities":projection.expected_capabilities,"expected_semantics":facts.semantics,
        "expected_visual_checks":{"pattern":projection.visual_pattern},"validation":legacy_validation(&checks),
        "known_stressors":context.artifact_recipe.stressors,
        "standards_evidence":context.registry_case.standards_evidence.iter().cloned().chain(
            projection.standards_evidence_append.iter().map(|value|serde_json::to_value(value).expect("typed standards evidence serializes"))
        ).collect::<Vec<_>>(),"references":[]
    });
    if projection.include_implementation_version_name {
        manifest["uids"]["implementation_version_name"] =
            json!(planned.encoding.implementation.version_name);
    }
    for (key, value) in facts.specials {
        manifest[key] = value;
    }
    Ok(manifest)
}

fn family_facts(
    ctx: &CuratedArtifactProjectionContext,
    family: &ClassicProjectionFamily,
) -> Result<Facts, CuratedManifestError> {
    match family {
        ClassicProjectionFamily::Ct => ct(ctx),
        ClassicProjectionFamily::DxMg => dx_mg(ctx),
        ClassicProjectionFamily::MrCr => mr_cr(ctx),
        ClassicProjectionFamily::Nuclear => nuclear(ctx),
        ClassicProjectionFamily::VlProjection => vl_projection(ctx),
    }
}
fn decode<T: DeserializeOwned>(value: Value) -> Result<T, CuratedManifestError> {
    serde_json::from_value(value)
        .map_err(|e| err(format!("invalid typed classic projection input: {e}")))
}
fn artifact<T: DeserializeOwned>(
    ctx: &CuratedArtifactProjectionContext,
) -> Result<T, CuratedManifestError> {
    decode(Value::Object(ctx.artifact_recipe.parameters.clone()))
}
fn provider<T: DeserializeOwned>(
    ctx: &CuratedArtifactProjectionContext,
) -> Result<T, CuratedManifestError> {
    decode(Value::Object(ctx.case_recipe.provider_parameters.clone()))
}
fn joined(values: &[String]) -> String {
    values.join("\\")
}
fn parse(value: &str) -> Result<f64, CuratedManifestError> {
    value
        .parse()
        .map_err(|e| err(format!("invalid decimal {value}: {e}")))
}
fn numbers(values: &[String]) -> Result<Vec<f64>, CuratedManifestError> {
    values.iter().map(|v| parse(v)).collect()
}
fn labels(
    ctx: &CuratedArtifactProjectionContext,
) -> Result<&ClassicSemanticLabels, CuratedManifestError> {
    ctx.artifact_recipe
        .classic_projection
        .as_ref()
        .and_then(|v| v.semantic_labels.as_ref())
        .ok_or_else(|| err("missing classic semantic labels"))
}
fn image(
    rows: u64,
    columns: u64,
    frames: u64,
    spp: u64,
    photo: &str,
    bits: u64,
    stored: u64,
    repr: u64,
    planar: Option<u64>,
) -> Value {
    json!({"rows":rows,"columns":columns,"frames":frames,"samples_per_pixel":spp,"photometric_interpretation":photo,"bits_allocated":bits,"bits_stored":stored,"high_bit":stored-1,"pixel_representation":repr,"planar_configuration":planar})
}

fn ct(ctx: &CuratedArtifactProjectionContext) -> Result<Facts, CuratedManifestError> {
    let capability = inspect_ct_capability(&ctx.case_recipe)
        .map_err(|error| err(format!("invalid typed classic CT input: {error}")))?
        .ok_or_else(|| err("classic CT projection lacks the CT capability tuple"))?;
    let inspected = capability
        .artifacts
        .iter()
        .find(|artifact| artifact.order == ctx.artifact_recipe.order)
        .ok_or_else(|| err("classic CT artifact missing from inspected capability"))?;
    let p = &capability.provider;
    let a = &inspected.parameters;
    let px = &a.pixels;
    let slice_count = inspected.series_instance_count;
    let study_series_count = capability.study_series_count;
    let slice_order_index = inspected.geometric_order_index;
    let mut geometry = json!({"pixel_spacing":joined(&p.pixel_spacing),"image_orientation_patient":joined(&p.image_orientation_patient),"image_position_patient":joined(&a.image_position_patient),"slice_thickness":p.slice_thickness});
    if let Some(value) = &p.spacing_between_slices {
        geometry["spacing_between_slices"] = json!(value);
    }
    if let Some(value) = &p.gantry_detector_tilt {
        geometry["gantry_detector_tilt_degrees"] = json!(parse(value)?);
    }
    if capability.artifacts.len() > 1 {
        geometry.as_object_mut().unwrap().extend(Map::from_iter([
            (
                "position_along_normal".into(),
                json!(a.position_along_normal),
            ),
            ("slice_order_index".into(), json!(slice_order_index)),
            ("slice_count".into(), json!(slice_count)),
            ("series_ordinal".into(), json!(inspected.series_ordinal)),
            ("study_series_count".into(), json!(study_series_count)),
        ]));
    }
    let recipe = json!({"rows":px.rows,"columns":px.columns,"samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2","bits_allocated":16,"bits_stored":12,"high_bit":11,"pixel_representation":1,"pixel_values":px.stored_values,"geometry":geometry,"kvp":p.kvp,"acquisition_number":a.acquisition_number,"series_number":a.series_number,"rescale":{"intercept":p.rescale_intercept,"slope":p.rescale_slope,"type":p.rescale_type},"window":{"center":p.window_center,"width":p.window_width}});
    let output_min = px.pixel_min + parse(&p.rescale_intercept)? as i64;
    let output_max = px.pixel_max + parse(&p.rescale_intercept)? as i64;
    let mut semantics = json!({"synthetic_data":"YES","image_type":joined(&p.image_type),"pixel_min":px.pixel_min,"pixel_max":px.pixel_max,"rescale":{"intercept":p.rescale_intercept,"slope":p.rescale_slope,"type":p.rescale_type,"output_min":output_min,"output_max":output_max},"window":{"center":p.window_center,"width":p.window_width}});
    let mut specials = Map::new();
    if capability.artifacts.len() > 1 {
        semantics["geometry_sort_key"] = json!({"image_orientation_patient":joined(&p.image_orientation_patient),"position_along_normal":a.position_along_normal,"slice_order_index":slice_order_index});
        semantics["series_instance_count"] = json!(slice_count);
        semantics["shared_study_series_frame_of_reference"] = json!(true);
        let instance = match &a.instance_number {
            ClassicCtInstanceNumber::Value { value } => value.parse::<i64>().ok(),
            ClassicCtInstanceNumber::Empty => None,
        };
        let mut expected = json!({"image_orientation_patient":numbers(&p.image_orientation_patient)?,"image_position_patient":numbers(&a.image_position_patient)?,"position_along_normal_mm":a.position_along_normal,"geometric_order_index":inspected.geometric_order_index,"instance_number":instance,"instance_number_state":if instance.is_some(){"numeric"}else{"empty"},"instance_number_order_index":inspected.instance_number_order_index,"sort_basis":"image_position_patient_projected_on_slice_normal","sort_direction":"ascending","position_tolerance_mm":0.00001,"spacing_tolerance_mm":0.00001,"adjacent_spacing_mm":inspected.adjacent_spacing_mm,"spacing_uniform":inspected.spacing_uniform,"sorting_conflict_expected":inspected.sorting_conflict_expected,"series_instance_count":slice_count});
        if let Some(value) = &p.gantry_detector_tilt {
            expected["gantry_detector_tilt_degrees"] = json!(parse(value)?);
        }
        specials.insert("expected_geometry".into(), expected);
        if let Some(series) = &p.series_organization {
            semantics["series_ordinal"] = json!(inspected.series_ordinal);
            semantics["study_series_count"] = json!(study_series_count);
            specials.insert(
                "expected_series_organization".into(),
                json!({"group_id":series.group_id,"shared_study_instance_uid_expected":series.shared_study_instance_uid,
                    "shared_frame_of_reference_uid_expected":series.shared_frame_of_reference_uid,
                    "distinct_series_instance_uids_expected":series.distinct_series_instance_uids,
                    "series_instance_count":slice_count,"series_ordinal":inspected.series_ordinal,"study_series_count":study_series_count}),
            );
        }
    }
    Ok(Facts {
        recipe,
        image: image(
            px.rows.into(),
            px.columns.into(),
            1,
            1,
            "MONOCHROME2",
            16,
            12,
            1,
            None,
        ),
        semantics,
        specials,
    })
}

fn photo(
    value: &crate::native_pixel::PhotometricInterpretation,
) -> Result<&'static str, CuratedManifestError> {
    match value {
        crate::native_pixel::PhotometricInterpretation::Monochrome1 => Ok("MONOCHROME1"),
        crate::native_pixel::PhotometricInterpretation::Monochrome2 => Ok("MONOCHROME2"),
        _ => fail("unsupported classic DX/MG photometric"),
    }
}
fn dx_mg(ctx: &CuratedArtifactProjectionContext) -> Result<Facts, CuratedManifestError> {
    let a: DxMgArtifactParameters = artifact(ctx)?;
    let photo = photo(&a.photometric_interpretation)?;
    let window = json!({"center":a.window_center,"width":a.window_width});
    let shutter=a.shutter.as_ref().map(|s|json!({"shape":s.shape,"left_vertical_edge":s.left_vertical_edge,"right_vertical_edge":s.right_vertical_edge,"upper_horizontal_edge":s.upper_horizontal_edge,"lower_horizontal_edge":s.lower_horizontal_edge,"presentation_value":s.presentation_value}));
    let mut recipe = json!({"rows":a.rows,"columns":a.columns,"samples_per_pixel":1,"photometric_interpretation":photo,"bits_allocated":16,"bits_stored":12,"high_bit":11,"pixel_representation":0,"pixel_values":a.stored_values,"body_part_examined":a.body_part_examined,"image_laterality":a.image_laterality,"presentation_intent_type":a.presentation_intent_type,"imager_pixel_spacing":joined(&a.imager_pixel_spacing),"presentation_lut_shape":a.presentation_lut_shape,"view_position":a.view_position,"window":window,"display_shutter":shutter});
    recipe.as_object_mut().unwrap().retain(|_, v| !v.is_null());
    let mut semantics = json!({"synthetic_data":"YES","pixel_min":a.pixel_min,"pixel_max":a.pixel_max,"presentation_intent_type":a.presentation_intent_type});
    if a.modality == "MG" {
        semantics["window"] = window;
    }
    if let Some(value) = shutter {
        semantics["display_shutter"] = value;
    }
    if let Some(v) = ctx
        .artifact_recipe
        .classic_projection
        .as_ref()
        .and_then(|p| p.semantic_labels.as_ref())
        .and_then(|l| l.photometric_semantics.as_ref())
    {
        semantics["photometric_semantics"] = json!(v);
    }
    Ok(Facts {
        recipe,
        image: image(
            a.rows.into(),
            a.columns.into(),
            1,
            1,
            photo,
            16,
            12,
            0,
            None,
        ),
        semantics,
        specials: Map::new(),
    })
}

fn mr_cr(ctx: &CuratedArtifactProjectionContext) -> Result<Facts, CuratedManifestError> {
    if let Some(capability) = inspect_mr_capability(&ctx.case_recipe)
        .map_err(|error| err(format!("invalid classic MR capability: {error}")))?
    {
        let a = capability
            .artifacts
            .get(ctx.artifact_recipe.order as usize)
            .ok_or_else(|| err("classic MR artifact order is out of range"))?;
        let mr = &capability.mr;
        let slice_count = ctx
            .case_recipe
            .dicom
            .as_ref()
            .map(|value| value.artifacts.len())
            .unwrap_or(1);
        let slice_order_index = ctx.artifact_recipe.order as usize + 1;
        let image_type = capability.provider.image_type.join("\\");
        Ok(Facts {
            recipe: json!({"rows":a.rows,"columns":a.columns,"samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2","bits_allocated":16,"bits_stored":16,"high_bit":15,"pixel_representation":0,"pixel_values":a.stored_values,"geometry":{"pixel_spacing":joined(&a.pixel_spacing),"image_orientation_patient":joined(&a.image_orientation_patient),"image_position_patient":joined(&a.image_position_patient),"slice_thickness":a.slice_thickness,"spacing_between_slices":a.spacing_between_slices,"slice_location":a.slice_location,"position_along_normal":a.position_along_normal,"slice_count":slice_count,"slice_order_index":slice_order_index},"mr":mr}),
            image: image(
                a.rows.into(),
                a.columns.into(),
                1,
                1,
                "MONOCHROME2",
                16,
                16,
                0,
                None,
            ),
            semantics: json!({"synthetic_data":"YES","image_type":image_type,"pixel_min":a.pixel_min,"pixel_max":a.pixel_max,"geometry_sort_key":{"image_orientation_patient":joined(&a.image_orientation_patient),"position_along_normal":a.position_along_normal,"slice_order_index":slice_order_index},"series_instance_count":slice_count,"shared_study_series_frame_of_reference":true}),
            specials: Map::new(),
        })
    } else {
        let a: CrArtifactParameters = artifact(ctx)?;
        let l = labels(ctx)?;
        let overlay = json!({"rows":a.overlay.rows,"columns":a.overlay.columns,"type":a.overlay.overlay_type,"origin":a.overlay.origin,"bits_allocated":a.overlay.bits_allocated,"bit_position":a.overlay.bit_position,"value_length":a.overlay.data.len()});
        let modality = json!({"descriptor":a.modality_lut.descriptor,"type":a.modality_lut.lut_type,"data_value_length":a.modality_lut.data.len()});
        let voi =
            json!({"descriptor":a.voi_lut.descriptor,"data_value_length":a.voi_lut.data.len()});
        Ok(Facts {
            recipe: json!({"rows":a.rows,"columns":a.columns,"samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2","bits_allocated":8,"bits_stored":8,"high_bit":7,"pixel_representation":0,"pixel_values":a.stored_values,"body_part_examined":a.body_part_examined,"view_position":a.view_position,"overlay":overlay,"modality_lut":modality,"voi_lut":voi}),
            image: image(
                a.rows.into(),
                a.columns.into(),
                1,
                1,
                "MONOCHROME2",
                8,
                8,
                0,
                None,
            ),
            semantics: json!({"synthetic_data":"YES","image_type":"ORIGINAL\\PRIMARY","pixel_min":a.pixel_min,"pixel_max":a.pixel_max,"overlay_pattern":l.overlay_pattern,"modality_lut":l.modality_lut,"voi_lut":l.voi_lut}),
            specials: Map::new(),
        })
    }
}

fn nuclear(ctx: &CuratedArtifactProjectionContext) -> Result<Facts, CuratedManifestError> {
    let provider: ClassicNuclearProviderParameters = provider(ctx)?;
    let body_part = provider.body_part_examined;
    let value: ClassicNuclearArtifactParameters = artifact(ctx)?;
    match value {
        ClassicNuclearArtifactParameters::UltrasoundSingleFrame {
            pixels,
            image_type,
            lossy_image_compression,
            ultrasound_color_data_present,
        } => {
            let recipe = json!({"lossy_image_compression":lossy_image_compression,"ultrasound_color_data_present":ultrasound_color_data_present});
            let mut semantics = json!({"synthetic_data":"YES","image_type":joined(&image_type),"lossy_image_compression":lossy_image_compression,"ultrasound_color_data_present":ultrasound_color_data_present,"pixel_min":pixels.pixel_min,"pixel_max":pixels.pixel_max});
            if let Some(value) = &body_part {
                semantics["body_part_examined"] = json!(value);
            }
            let mut facts = nuclear_base(&pixels, recipe, semantics, Map::new())?;
            facts.recipe.as_object_mut().unwrap().remove("frames");
            Ok(facts)
        }
        ClassicNuclearArtifactParameters::UltrasoundMultiframe {
            pixels,
            image_type,
            frame_increment_pointer,
            frame_time_ms,
            frame_relative_times_ms,
            payload_sha256,
            lossy_image_compression,
            color_data_present,
            spatially_related_frames,
            region_calibrated,
            ..
        } => {
            let frame_size = pixels.rows as usize * pixels.columns as usize;
            let frames = pixels.stored_values.chunks(frame_size).enumerate().map(|(index,values)|json!({"frame_number":index+1,"frame_sha256":pixels.frame_sha256[index],"pixel_values":values})).collect::<Vec<_>>();
            let expected = json!({"frame_count":pixels.frames,"frame_increment_pointer":frame_increment_pointer,"frame_time_ms":f64::from(frame_time_ms),"frame_relative_times_ms":frame_relative_times_ms.iter().map(|v|f64::from(*v)).collect::<Vec<_>>(),"frames":frames,"image_type":image_type,"lossy_image_compression":lossy_image_compression,"color_data_present":color_data_present,"spatially_related_frames":spatially_related_frames,"region_calibrated":region_calibrated});
            let mut specials = Map::new();
            specials.insert("expected_us_multiframe".into(), expected.clone());
            let mut facts = nuclear_base(
                &pixels,
                json!({"frame_time_ms":frame_time_ms,"payload_sha256":payload_sha256,"us_multiframe":expected}),
                json!({"synthetic_data":"YES","image_type":joined(&image_type),"body_part_examined":body_part,"pixel_min":pixels.pixel_min,"pixel_max":pixels.pixel_max}),
                specials,
            )?;
            facts.recipe.as_object_mut().unwrap().remove("pixel_values");
            Ok(facts)
        }
        ClassicNuclearArtifactParameters::NuclearMedicine {
            pixels,
            image_type,
            pixel_spacing,
            energy_window_vector,
            detector_vector,
            energy_windows,
            detectors,
            actual_frame_duration_ms,
            counts_accumulated,
            ..
        } => {
            let energy=energy_windows.iter().map(|v|Ok(json!({"index":v.index,"name":v.name,"lower_limit_kev":parse(&v.lower_limit_kev)?,"upper_limit_kev":parse(&v.upper_limit_kev)?}))).collect::<Result<Vec<_>,CuratedManifestError>>()?;
            let detector=detectors.iter().map(|v|Ok(json!({"index":v.index,"collimator_type":v.collimator_type,"focal_distance_mm":parse(&v.focal_distance_mm)?,"start_angle_degrees":parse(&v.start_angle_degrees)?,"image_orientation_patient":numbers(&v.image_orientation_patient)?,"image_position_patient":numbers(&v.image_position_patient)?}))).collect::<Result<Vec<_>,CuratedManifestError>>()?;
            let dimensions=(0..pixels.frames as usize).map(|i|json!({"frame_number":i+1,"energy_window_index":energy_window_vector[i],"detector_index":detector_vector[i],"frame_sha256":pixels.frame_sha256[i]})).collect::<Vec<_>>();
            let expected = json!({"image_type":image_type,"frame_increment_pointers":["0054,0010","0054,0020"],"number_of_energy_windows":energy.len(),"number_of_detectors":detector.len(),"energy_window_vector":energy_window_vector,"detector_vector":detector_vector,"energy_windows":energy,"detectors":detector,"actual_frame_duration_ms":actual_frame_duration_ms,"counts_accumulated":counts_accumulated,"frame_dimensions":dimensions});
            let recipe = json!({"nm_dimensions":{"frame_increment_pointers":["0054,0010","0054,0020"],"number_of_energy_windows":energy_windows.len(),"number_of_detectors":detectors.len(),"energy_window_vector":energy_window_vector,"detector_vector":detector_vector}});
            let mut specials = Map::new();
            specials.insert("expected_nm_multiframe".into(), expected);
            nuclear_base(
                &pixels,
                recipe,
                json!({"synthetic_data":"YES","body_part_examined":body_part,"pixel_spacing_mm":numbers(&pixel_spacing)?,"pixel_min":pixels.pixel_min,"pixel_max":pixels.pixel_max}),
                specials,
            )
        }
        ClassicNuclearArtifactParameters::Pet {
            pixels,
            image_type,
            units,
            counts_source,
            series_type,
            number_of_slices,
            corrected_image,
            decay_correction,
            dose_calibration_factor,
            frame_reference_time_ms,
            actual_frame_duration_ms,
            image_index,
            pixel_spacing,
            image_orientation_patient,
            image_position_patient,
            slice_thickness,
            rescale_intercept,
            rescale_slope,
            expected_activity_bqml,
            ..
        } => {
            let image_type_semantics = joined(&image_type);
            let activity_values = expected_activity_bqml
                .iter()
                .map(|value| parse(value))
                .collect::<Result<Vec<_>, _>>()?;
            let intercept = parse(&rescale_intercept)?;
            let slope = parse(&rescale_slope)?;
            let expected = json!({"units":units,"counts_source":counts_source,"series_type":series_type,"number_of_slices":number_of_slices,"corrected_image":corrected_image,"decay_correction":decay_correction,"dose_calibration_factor":parse(&dose_calibration_factor)?,"frame_reference_time_ms":parse(&frame_reference_time_ms)?,"actual_frame_duration_ms":actual_frame_duration_ms.parse::<u64>().map_err(|e|err(e.to_string()))?,"image_index":image_index,"rescale_intercept":intercept,"rescale_slope":slope,"activity_values_bqml":activity_values,"stored_values":pixels.stored_values,"image_type":image_type,"radiopharmaceutical_information_item_count":0});
            let recipe = json!({"geometry":{"pixel_spacing":joined(&pixel_spacing),"image_orientation_patient":joined(&image_orientation_patient),"image_position_patient":joined(&image_position_patient),"slice_thickness":slice_thickness},"pet_activity":{"units":units,"rescale_intercept":intercept,"rescale_slope":slope,"stored_values":pixels.stored_values,"activity_values_bqml":activity_values}});
            let mut specials = Map::new();
            specials.insert("expected_pet_activity".into(), expected);
            nuclear_base(
                &pixels,
                recipe,
                json!({"synthetic_data":"YES","body_part_examined":body_part,"image_type":image_type_semantics,"pixel_min":pixels.pixel_min,"pixel_max":pixels.pixel_max,"rescale":{"intercept":intercept,"slope":slope,"type":units}}),
                specials,
            )
        }
    }
}
fn nuclear_base(
    pixels: &ClassicNuclearPixels,
    extra: Value,
    semantics: Value,
    specials: Map<String, Value>,
) -> Result<Facts, CuratedManifestError> {
    let bits = if pixels.stored_value_type == "u8" {
        8
    } else {
        16
    };
    let frame_size = pixels.rows as usize * pixels.columns as usize;
    let pixel_values = if pixels.frames == 1 {
        json!(pixels.stored_values)
    } else {
        json!(pixels.stored_values.chunks(frame_size).collect::<Vec<_>>())
    };
    let mut recipe = json!({"rows":pixels.rows,"columns":pixels.columns,"frames":pixels.frames,"samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2","bits_allocated":bits,"bits_stored":bits,"high_bit":bits-1,"pixel_representation":0,"pixel_values":pixel_values});
    recipe
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().cloned().unwrap_or_default());
    Ok(Facts {
        recipe,
        image: image(
            pixels.rows.into(),
            pixels.columns.into(),
            pixels.frames.into(),
            1,
            "MONOCHROME2",
            bits,
            bits,
            0,
            None,
        ),
        semantics,
        specials,
    })
}

fn vl_projection(ctx: &CuratedArtifactProjectionContext) -> Result<Facts, CuratedManifestError> {
    if matches!(ctx.registry_case.modality.as_deref(), Some("XA" | "RF")) {
        let a: ProjectionArtifactParameters = artifact(ctx)?;
        let key = if a.modality == "XA" {
            "xa_projection"
        } else {
            "xrf_projection"
        };
        let mut projected = serde_json::to_value(&a.non_claims).unwrap();
        let object = projected.as_object_mut().unwrap();
        object.extend(Map::from_iter([
            ("body_part_examined".into(), json!(a.body_part_examined)),
            ("image_type".into(), json!(a.image_type)),
            ("frame_count".into(), json!(1)),
            ("patient_orientation_empty".into(), json!(true)),
            (
                "pixel_intensity_relationship".into(),
                json!(a.pixel_intensity_relationship),
            ),
            (
                "lossy_image_compression".into(),
                json!(a.lossy_image_compression),
            ),
            ("kvp".into(), json!(parse(&a.kvp)?)),
            ("radiation_setting".into(), json!(a.radiation_setting)),
            (
                "exposure_mas".into(),
                json!(a.exposure.parse::<i64>().map_err(|e| err(e.to_string()))?),
            ),
            (
                "imager_pixel_spacing_mm".into(),
                json!(numbers(&a.imager_pixel_spacing)?),
            ),
            (
                "distance_source_to_detector_mm".into(),
                json!(parse(&a.distance_source_to_detector)?),
            ),
            (
                "distance_source_to_patient_mm".into(),
                json!(parse(&a.distance_source_to_patient)?),
            ),
            (
                "estimated_radiographic_magnification_factor".into(),
                json!(parse(&a.estimated_magnification_factor)?),
            ),
        ]));
        for (name, value) in [
            (
                "positioner_primary_angle_degrees",
                &a.positioner_primary_angle,
            ),
            (
                "positioner_secondary_angle_degrees",
                &a.positioner_secondary_angle,
            ),
            ("column_angulation_degrees", &a.column_angulation),
        ] {
            if let Some(value) = value {
                object.insert(name.into(), json!(parse(value)?));
            }
        }
        if key == "xa_projection" {
            for name in [
                "table_position_present",
                "table_tilt_present",
                "tomography_present",
                "xa_positioner_angles_present",
            ] {
                object.remove(name);
            }
        }
        let mut nested = Map::new();
        nested.insert(key.into(), projected.clone());
        let mut recipe = json!({"rows":a.rows,"columns":a.columns,"frames":1,"samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2","bits_allocated":8,"bits_stored":8,"high_bit":7,"pixel_representation":0,"payload_sha256":a.frame_sha256});
        recipe.as_object_mut().unwrap().extend(nested);
        let mut specials = Map::new();
        specials.insert(format!("expected_{key}"), projected);
        return Ok(Facts {
            recipe,
            image: image(
                a.rows.into(),
                a.columns.into(),
                1,
                1,
                "MONOCHROME2",
                8,
                8,
                0,
                None,
            ),
            semantics: json!({"synthetic_data":"YES","image_type":joined(&a.image_type),"body_part_examined":a.body_part_examined,"pixel_min":a.pixel_min,"pixel_max":a.pixel_max}),
            specials,
        });
    }
    let a: VlArtifactParameters = artifact(ctx)?;
    let photo = match a.photometric_interpretation {
        VlPhotometricInterpretation::Rgb => "RGB",
        VlPhotometricInterpretation::PaletteColor => "PALETTE COLOR",
    };
    let label = labels(ctx)?;
    let palette=a.palette.as_ref().map(|p|json!({"descriptor":p.descriptor,"red_data_value_length":p.red.len()*2,"green_data_value_length":p.green.len()*2,"blue_data_value_length":p.blue.len()*2}));
    let mut recipe = json!({"rows":a.rows,"columns":a.columns,"samples_per_pixel":a.samples_per_pixel,"photometric_interpretation":photo,"bits_allocated":8,"bits_stored":8,"planar_configuration":a.planar_configuration,"pixel_values":a.stored_values,"palette":palette,"pixel_padding":Value::Null});
    if let Some(v) = &a.body_part_examined {
        recipe["body_part_examined"] = json!(v)
    }
    if a.body_part_examined.is_some() {
        if let Some(v) = &a.laterality {
            recipe["laterality"] = json!(v)
        }
    }
    let mut semantics = json!({"synthetic_data":"YES","conversion_type":Value::Null,"image_type":"ORIGINAL\\PRIMARY","pixel_min":a.pixel_min,"pixel_max":a.pixel_max,"pixel_padding":Value::Null,"lossy_image_compression":"00","lossy_image_compression_ratio":Value::Null,"lossy_image_compression_method":Value::Null,"photometric_semantics":label.photometric_semantics});
    if let Some(v) = &a.body_part_examined {
        semantics["body_part_examined"] = json!(v)
    }
    if a.body_part_examined.is_some() {
        if let Some(v) = &a.laterality {
            semantics["laterality"] = json!(v)
        }
    }
    let mut specials = Map::new();
    if let Some(hash) = &a.icc_profile_sha256 {
        let icc = ctx
            .artifact_recipe
            .classic_projection
            .as_ref()
            .and_then(|value| value.icc.as_ref())
            .ok_or_else(|| err("missing classic ICC projection contract"))?;
        let size = a.icc_profile_hex.as_ref().map(|v| v.len() / 2).unwrap_or(0);
        let mut expected = serde_json::to_value(icc).expect("typed ICC projection serializes");
        expected.as_object_mut().unwrap().extend(Map::from_iter([
            ("color_space".into(), json!(a.color_space)),
            ("profile_sha256".into(), json!(hash)),
            ("profile_size_bytes".into(), json!(size)),
            ("declared_profile_size_bytes".into(), json!(size)),
        ]));
        specials.insert("expected_icc_profile".into(), expected);
    }
    if a.body_part_examined.is_some() {
        let mut vl_image = image(
            a.rows.into(),
            a.columns.into(),
            1,
            a.samples_per_pixel.into(),
            photo,
            8,
            8,
            0,
            a.planar_configuration.map(Into::into),
        );
        vl_image.as_object_mut().unwrap().remove("frames");
        specials.insert("expected_vl_single_frame".into(),json!({"iod_kind":if a.modality=="ES"{"vl_endoscopic_single_frame"}else{"vl_microscopic_single_frame"},"sop_class_uid":a.sop_class_uid,"sop_class_name":required(&ctx.registry_case.sop_class_name,"SOP name")?,"iod_name":required(&ctx.registry_case.iod_name,"IOD name")?,"modality":a.modality,"transfer_syntax_uid":"1.2.840.10008.1.2.1","image_type":["ORIGINAL","PRIMARY"],"body_part_examined":a.body_part_examined,"laterality":a.laterality,"acquisition_context_items":0,"image":vl_image,"absent_content":["number_of_frames","frame_of_reference_uid","specimen_module","optical_path_module","icc_profile_module"]}));
    }
    Ok(Facts {
        recipe,
        image: image(
            a.rows.into(),
            a.columns.into(),
            1,
            a.samples_per_pixel.into(),
            photo,
            8,
            8,
            0,
            a.planar_configuration.map(Into::into),
        ),
        semantics,
        specials,
    })
}

fn pixel_data(
    planned: &PlannedDicomArtifact,
    execution: &ArtifactExecutionEvidence,
    pixels: &MaterializedContentEvidence,
) -> Result<Value, CuratedManifestError> {
    let vr = pixels.vr.as_str();
    if planned.encoding.transfer_syntax_uid != RLE {
        if !execution.codecs.is_empty() || pixels.fragment_count != 0 {
            return fail("native classic artifact has codec evidence");
        };
        return Ok(
            json!({"vr":vr,"native_or_encapsulated":"native","value_length":pixels.native_value_field_size_bytes.ok_or_else(||err("missing native Value Field size"))?,"frame_count":pixels.decoded_frame_sha256.len(),"frame_hashes":pixels.decoded_frame_sha256}),
        );
    }
    let codec = only(&execution.codecs, "classic codec evidence")?;
    if codec.status != ResultStatus::Passed
        || codec.encoded_frame_sha256 != pixels.compressed_frame_sha256
    {
        return fail("classic codec evidence differs from materialization");
    };
    let eot = matches!(planned.encoding.offset_table, OffsetTablePolicy::Extended);
    Ok(
        json!({"vr":vr,"native_or_encapsulated":"encapsulated","value_length":Value::Null,"frame_count":codec.decoded_frame_sha256.len(),"frame_hashes":codec.decoded_frame_sha256,"codec":{"backend_id":codec.backend_id,"backend_kind":codec.backend_kind,"display_name":codec.display_name,"version":codec.backend_version,"transfer_syntax_uid":codec.transfer_syntax_uid,"feature_gate":codec.feature_gate,"determinism":codec.determinism},"encapsulated_pixel_data":{"basic_offset_table":{"present":true,"populated":!pixels.basic_offset_table.is_empty(),"offset_count":pixels.basic_offset_table.len(),"offsets":pixels.basic_offset_table},"fragments_per_frame":pixels.fragments_per_frame,"fragments":pixels.fragments,"extended_offset_table":if eot{json!({"present":true,"lengths_present":true,"offset_count":pixels.extended_offset_table.len(),"length_count":pixels.extended_offset_table_lengths.len(),"offsets":pixels.extended_offset_table,"lengths":pixels.extended_offset_table_lengths})}else{json!({"present":false,"lengths_present":false,"offset_count":0,"length_count":0})},"compressed_frame_hashes":pixels.compressed_frame_sha256}}),
    )
}
