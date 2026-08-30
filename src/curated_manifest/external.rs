//! Filesystem-free projection of pinned externally constructed Part 10 objects.

use serde_json::{Value, json};

use crate::composition::CompositionUidRole;
use crate::corpus_plan::{PlannedArtifact, PlannedImportedDicomArtifact};
use crate::curated_plan::CuratedArtifactProjectionContext;
use crate::executor::adapters::{ManifestProjectionArtifact, ManifestProjectionCompatibilityInput};
use crate::executor::evidence::{ExecutionStatus, ImportedDicomObservation};

use super::{CuratedManifestError, err, fail, required};

pub(super) fn project_file_entry(
    ctx: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
    input: &ManifestProjectionCompatibilityInput,
) -> Result<Value, CuratedManifestError> {
    let PlannedArtifact::ImportedDicom(planned) = &pair.planned else {
        return fail("external projector received a non-imported artifact");
    };
    validate_contract(ctx, pair, planned)?;
    let output = pair.execution.output.as_ref().unwrap();
    let materialization = pair.execution.materialization.as_ref().unwrap();
    let observation = materialization.imported_dicom.as_ref().unwrap();
    let response = pair
        .execution
        .providers
        .first()
        .and_then(|provider| provider.claims.get("response"))
        .ok_or_else(|| err("external provider response evidence is missing"))?;
    let response_output = response
        .get("outputs")
        .and_then(Value::as_array)
        .and_then(|outputs| outputs.first())
        .ok_or_else(|| err("external provider response has no output"))?;
    let references = references(planned, input)?;
    let backend = backend(pair, response)?;
    let validation = json!({
        "status":"passed",
        "internal":pair.execution.validation.iter().map(|result|json!({"name":result.rule_id,"status":"passed","message":result.message})).collect::<Vec<_>>(),
        "standards":[],
        "external":[]
    });
    let parameters = &ctx.artifact_recipe.parameters;
    let kind = ctx
        .case_recipe
        .provider_parameters
        .get("import")
        .and_then(|value| value.get("kind"))
        .or_else(|| {
            ctx.case_recipe
                .provider_parameters
                .get("document")
                .and_then(|value| value.get("kind"))
        })
        .and_then(Value::as_str)
        .ok_or_else(|| err("external recipe kind is missing"))?;

    let mut entry = base_entry(
        ctx,
        output.relative_path.as_str(),
        &output.sha256,
        output.size_bytes,
        observation,
        backend,
        references,
        validation,
    )?;
    if entry["uids"]["frame_of_reference_uid"].is_null() {
        entry["uids"]["frame_of_reference_uid"] = identity(
            &planned.declared_instance.identities,
            CompositionUidRole::FrameOfReference,
        )
        .map(Value::from)
        .unwrap_or(Value::Null);
    }
    match kind {
        "parametric_map_float32" | "parametric_map_float64" => {
            project_parametric_map(&mut entry, kind, parameters, response_output, planned)?
        }
        "whole_slide_tile_segmentation" => {
            project_wsi_seg(&mut entry, parameters, response_output, planned, input)?
        }
        "tid1500" => project_tid1500(&mut entry, response_output, planned, input)?,
        "comprehensive3d" => project_scoord3d(&mut entry, response_output, planned, input)?,
        other => return fail(format!("unsupported external projection kind {other}")),
    }
    Ok(entry)
}

fn validate_contract(
    ctx: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
    planned: &PlannedImportedDicomArtifact,
) -> Result<(), CuratedManifestError> {
    let execution = &pair.execution;
    let binding = planned
        .case_binding
        .as_ref()
        .ok_or_else(|| err("imported artifact has no case binding"))?;
    let output = execution
        .output
        .as_ref()
        .ok_or_else(|| err("imported artifact has no output evidence"))?;
    let materialization = execution
        .materialization
        .as_ref()
        .ok_or_else(|| err("imported artifact has no materialization evidence"))?;
    let observed = materialization
        .imported_dicom
        .as_ref()
        .ok_or_else(|| err("imported DICOM observation is missing"))?;
    if execution.status != ExecutionStatus::Succeeded
        || execution.order != ctx.plan_order
        || planned.order != ctx.plan_order
        || binding.case_id != ctx.registry_case.case_id
        || binding.recipe_id != ctx.case_recipe.recipe_id
        || binding.recipe_version != ctx.case_recipe.recipe_version
        || output.relative_path != planned.output.relative_path.as_str()
        || !output.publish
        || materialization.backend_id != "imported_part10_materializer"
        || materialization.materialized_artifact_sha256.as_deref() != Some(output.sha256.as_str())
        || observed.sop_class_uid != planned.declared_instance.sop_class_uid
        || observed.transfer_syntax_uid != planned.provider.transfer_syntax_uid
        || observed.sop_instance_uid
            != identity(
                &planned.declared_instance.identities,
                CompositionUidRole::SopInstance,
            )?
        || execution.providers.len() != 1
        || execution.providers[0].provider_id != planned.provider.provider_id
        || execution
            .obligations
            .iter()
            .any(|obligation| obligation.status != crate::executor::evidence::ResultStatus::Passed)
    {
        return fail("external execution evidence differs from the immutable import plan");
    }
    Ok(())
}

fn base_entry(
    ctx: &CuratedArtifactProjectionContext,
    path: &str,
    sha256: &str,
    size_bytes: u64,
    observed: &ImportedDicomObservation,
    backend: Value,
    references: Vec<Value>,
    validation: Value,
) -> Result<Value, CuratedManifestError> {
    Ok(json!({
        "case_id":ctx.registry_case.case_id,
        "profile_membership":ctx.artifact_recipe.public_profile_membership.as_ref().unwrap_or(&ctx.registry_case.profiles),
        "path":path,"sha256":sha256,"size_bytes":size_bytes,
        "determinism":ctx.registry_case.determinism,
        "recipe":{"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,"recipe_parameters":{}},
        "dicom":{"sop_class_uid":required(&ctx.registry_case.sop_class_uid,"registry SOP Class UID")?,"sop_class_name":required(&ctx.registry_case.sop_class_name,"registry SOP Class name")?,"iod_name":required(&ctx.registry_case.iod_name,"registry IOD name")?,"modality":required(&ctx.registry_case.modality,"registry modality")?,"transfer_syntax_uid":observed.transfer_syntax_uid,"transfer_syntax_name":"Explicit VR Little Endian"},
        "uids":{"study_instance_uid":observed.study_instance_uid,"series_instance_uid":observed.series_instance_uid,"sop_instance_uid":observed.sop_instance_uid,"frame_of_reference_uid":observed.frame_of_reference_uid,"implementation_class_uid":observed.implementation_class_uid,"implementation_version_name":crate::IMPLEMENTATION_VERSION_NAME},
        "image":Value::Null,"pixel_data":Value::Null,"generation_backend":backend,
        "references":references,"validation":validation,
        "standards_evidence":ctx.registry_case.standards_evidence
    }))
}

fn backend(
    pair: &ManifestProjectionArtifact,
    response: &Value,
) -> Result<Value, CuratedManifestError> {
    let provider = pair.execution.providers.first().unwrap();
    let backend = response
        .get("backend")
        .ok_or_else(|| err("backend response identity is missing"))?;
    Ok(json!({
        "backend_id":provider.provider_id,"protocol_version":response["protocol_version"],
        "name":backend["name"],"version":backend["version"],
        "dependency_lock_sha256":backend["dependency_lock_sha256"],
        "executable_fingerprint":provider.executable_sha256,
        "entrypoint_fingerprint":provider.claims["entrypoint_fingerprint"],
        "environment_fingerprint":provider.claims["environment_fingerprint"],
        "runtime_identity":provider.claims["runtime_identity"],
        "invocation_elapsed_milliseconds":provider.claims.get("invocation_elapsed_milliseconds").cloned().unwrap_or(Value::Null),
        "determinism":"semantic_stable","warnings":response["warnings"]
    }))
}

fn references(
    planned: &PlannedImportedDicomArtifact,
    input: &ManifestProjectionCompatibilityInput,
) -> Result<Vec<Value>, CuratedManifestError> {
    planned.declared_instance.references.iter().map(|reference| {
        let source = input.artifacts.iter().find(|candidate| candidate.planned.logical_id() == reference.target_instance_id).ok_or_else(|| err("external source artifact is absent"))?;
        let source_output = source.execution.output.as_ref().ok_or_else(|| err("external source output is absent"))?;
        let (binding, identities) = match &source.planned {
            PlannedArtifact::Dicom(source) => (source.case_binding.as_ref(), &source.instance.identities),
            PlannedArtifact::ImportedDicom(source) => (source.case_binding.as_ref(), &source.declared_instance.identities),
            _ => return fail("external source is not DICOM"),
        };
        let binding = binding.ok_or_else(|| err("external source binding is absent"))?;
        let mut value = json!({"relationship":reference.role,"source_case_id":binding.case_id,"source_path":source_output.relative_path,"sop_class_uid":reference.referenced_sop_class_uid,"sop_instance_uid":reference.referenced_sop_instance_uid});
        if let Some(series) = identities.get(&CompositionUidRole::SeriesInstance, 0) { value["series_instance_uid"] = json!(series); }
        if !reference.referenced_frames.is_empty() { value["frame_numbers"] = json!(reference.referenced_frames); }
        Ok(value)
    }).collect()
}

fn project_parametric_map(
    entry: &mut Value,
    kind: &str,
    parameters: &serde_json::Map<String, Value>,
    output: &Value,
    planned: &PlannedImportedDicomArtifact,
) -> Result<(), CuratedManifestError> {
    if let Some(references) = entry["references"].as_array_mut() {
        for reference in references {
            reference["relationship"] = json!("source_image");
        }
    }
    let semantics = &output["expected_semantics"];
    let payload = &output["payload_expectations"];
    let float64 = kind.ends_with("float64");
    let bits_key = if float64 {
        "little_endian_float64_bits"
    } else {
        "little_endian_float32_bits"
    };
    let sample = if float64 { "float64" } else { "float32" };
    let pixel_stressor = if float64 {
        "double_float_pixel_data"
    } else {
        "float_pixel_data"
    };
    let dimension = identity(
        &planned.declared_instance.identities,
        CompositionUidRole::DimensionOrganization,
    )?;
    entry["uids"]["dimension_organization_uid"] = json!(dimension);
    entry["recipe"]["recipe_parameters"] = json!({"stored_value_scale":parameters["stored_value_scale"],"spatial_rank_increment":parameters["spatial_rank_increment"],"dimension_organization_uid":dimension,bits_key:payload[bits_key]});
    entry["image"] = json!({"sample_type":sample,"rows":semantics["rows"],"columns":semantics["columns"],"frames":semantics["frames"],"samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2","bits_allocated":if float64 {64} else {32},"planar_configuration":Value::Null});
    entry["pixel_data"] = json!({"vr":payload["vr"],"native_or_encapsulated":"native","value_length":payload["value_length"],"frame_count":semantics["frames"],"frame_hashes":payload["frame_sha256"]});
    entry["expected_capabilities"] = json!([
        "open_file",
        "read_metadata",
        if float64 {
            "render_double_float_pixels"
        } else {
            "render_float_pixels"
        },
        "parse_multiframe_functional_groups",
        "apply_real_world_value_mapping"
    ]);
    entry["expected_semantics"] = json!({"synthetic_data":"YES","sample_type":sample,"pixel_min":semantics["minimum"],"pixel_max":semantics["maximum"],"shared_functional_groups_sequence_items":1,"per_frame_functional_groups_sequence_items":semantics["frames"],"dimension_organization_uid":dimension,"source_reference_count":entry["references"].as_array().map(Vec::len).unwrap_or(0),"real_world_value_mapping":{"lut_label":semantics["real_world_value_mapping"]["lut_label"],"slope":semantics["real_world_value_mapping"]["slope"],"intercept":semantics["real_world_value_mapping"]["intercept"],"units":{"code_value":semantics["real_world_value_mapping"]["unit"]["value"],"coding_scheme_designator":semantics["real_world_value_mapping"]["unit"]["scheme"],"code_meaning":semantics["real_world_value_mapping"]["unit"]["meaning"]},"quantity_definition":{"code_value":semantics["real_world_value_mapping"]["quantity"]["value"],"coding_scheme_designator":semantics["real_world_value_mapping"]["quantity"]["scheme"],"code_meaning":semantics["real_world_value_mapping"]["quantity"]["meaning"]}},bits_key:payload[bits_key]});
    entry["expected_visual_checks"] = json!({"pattern":if float64 {"three_frame_ct_derived_float64_parametric_map"} else {"three_frame_ct_derived_float32_parametric_map"}});
    entry["known_stressors"] = json!([
        "parametric_map_storage",
        pixel_stressor,
        "native_multiframe_pixel_data",
        "real_world_value_mapping",
        "cross_instance_references",
        "external_generation_backend"
    ]);
    Ok(())
}

fn project_wsi_seg(
    entry: &mut Value,
    parameters: &serde_json::Map<String, Value>,
    output: &Value,
    planned: &PlannedImportedDicomArtifact,
    input: &ManifestProjectionCompatibilityInput,
) -> Result<(), CuratedManifestError> {
    entry["references"][0]["relationship"] = json!("source_image_for_segmentation");
    let semantics = &output["expected_semantics"];
    let payload = &output["payload_expectations"];
    let source_ref = entry["references"]
        .as_array()
        .and_then(|v| v.first())
        .ok_or_else(|| err("WSI SEG source is absent"))?
        .clone();
    let source = input
        .artifacts
        .iter()
        .find(|candidate| {
            candidate.planned.logical_id()
                == planned.declared_instance.references[0].target_instance_id
        })
        .ok_or_else(|| err("WSI source evidence is absent"))?;
    let PlannedArtifact::Dicom(source_plan) = &source.planned else {
        return fail("WSI source is not native DICOM");
    };
    let specimen = identity(
        &source_plan.instance.identities,
        CompositionUidRole::TemplateDefined("specimen_uid".into()),
    )?;
    let dimension = identity(
        &planned.declared_instance.identities,
        CompositionUidRole::DimensionOrganization,
    )?;
    entry["uids"]["dimension_organization_uid"] = json!(dimension);
    entry["recipe"]["recipe_parameters"] = json!({"source_case_id":source_ref["source_case_id"],"source_frame_numbers":parameters["source_frame_numbers"],"dimension_organization_uid":dimension,"segmentation_type":"FRACTIONAL","segmentation_fractional_type":"OCCUPANCY","maximum_fractional_value":255,"segment_count":1,"segment_label":"DTS_SYNTHETIC_REGION"});
    entry["image"] = json!({"rows":2,"columns":2,"frames":2,"samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2","bits_allocated":8,"bits_stored":8,"high_bit":7,"pixel_representation":0,"planar_configuration":Value::Null});
    entry["pixel_data"] = json!({"vr":"OB","native_or_encapsulated":"native","value_length":payload["value_length"],"frame_count":2,"frame_hashes":payload["frame_sha256"]});
    entry["expected_capabilities"] = json!([
        "open_file",
        "read_metadata",
        "parse_segmentation",
        "reconstruct_wsi_tile_segmentation",
        "resolve_frame_references"
    ]);
    entry["expected_semantics"] = json!({"synthetic_data":"YES","pixel_min":0,"pixel_max":255,"segmentation_type":"FRACTIONAL","segmentation_fractional_type":"OCCUPANCY","maximum_fractional_value":255,"segment_sequence_items":1,"shared_functional_groups_sequence_items":1,"per_frame_functional_groups_sequence_items":2,"source_case_id":source_ref["source_case_id"],"source_sop_instance_uid":source_ref["sop_instance_uid"],"referenced_frame_numbers":[1,4]});
    entry["expected_wsi_tile_segmentation"] =
        crate::wsi_tile_segmentation_locked_contract(crate::WsiTileSegmentationLockedInputs {
            source_case_id: source_ref["source_case_id"].as_str().unwrap(),
            source_path: source_ref["source_path"].as_str().unwrap(),
            source_sha256: &source.execution.output.as_ref().unwrap().sha256,
            source_study_instance_uid: identity(
                &source_plan.instance.identities,
                CompositionUidRole::StudyInstance,
            )?,
            source_series_instance_uid: identity(
                &source_plan.instance.identities,
                CompositionUidRole::SeriesInstance,
            )?,
            source_sop_class_uid: source_ref["sop_class_uid"].as_str().unwrap(),
            source_sop_instance_uid: source_ref["sop_instance_uid"].as_str().unwrap(),
            frame_of_reference_uid: identity(
                &planned.declared_instance.identities,
                CompositionUidRole::FrameOfReference,
            )?,
            specimen_uid: specimen,
            dimension_organization_uid: dimension,
        });
    entry["expected_visual_checks"] = json!({"pattern":"two_diagonal_wsi_tile_occupancy_masks"});
    entry["known_stressors"] = json!([
        "segmentation_storage",
        "fractional_occupancy_pixel_data",
        "tiled_sparse",
        "wsi_tile_references",
        "slide_coordinate_system",
        "external_generation_backend"
    ]);
    let _ = semantics;
    Ok(())
}

fn project_tid1500(
    entry: &mut Value,
    output: &Value,
    planned: &PlannedImportedDicomArtifact,
    _input: &ManifestProjectionCompatibilityInput,
) -> Result<(), CuratedManifestError> {
    entry["references"][0]["relationship"] = json!("source_image_for_segmentation");
    entry["references"][1]["relationship"] = json!("referenced_segment");
    let s = &output["expected_semantics"];
    let refs = entry["references"]
        .as_array()
        .ok_or_else(|| err("TID1500 references missing"))?
        .clone();
    if refs.len() != 2 {
        return fail("TID1500 reference closure differs");
    };
    let code = |v: &str, scheme: &str, m: &str| json!({"code_value":v,"coding_scheme_designator":scheme,"code_meaning":m});
    let tracking = identity(
        &planned.declared_instance.identities,
        CompositionUidRole::TemplateDefined("tracking_uid".into()),
    )?;
    let observer = identity(
        &planned.declared_instance.identities,
        CompositionUidRole::TemplateDefined("observer_uid".into()),
    )?;
    entry["recipe"]["recipe_parameters"] = json!({"segment_number":1,"measurement_value_mm3":5.625,"tracking_identifier":"DTS-TID1500-ROI-1","source_frame_numbers":[1,2]});
    entry["expected_capabilities"] = json!([
        "open_file",
        "read_metadata",
        "parse_structured_report",
        "resolve_references",
        "interpret_tid1500_measurements"
    ]);
    entry["expected_semantics"] = json!({"synthetic_data":"YES","source_sop_instance_uid":refs[0]["sop_instance_uid"],"structured_report":{"completion_flag":"COMPLETE","preliminary_flag":"FINAL","verification_flag":"UNVERIFIED","root_value_type":"CONTAINER","root_continuity_of_content":"CONTINUOUS","content_sequence_items":8}});
    entry["expected_tid1500"] = json!({"completion_flag":"COMPLETE","preliminary_flag":"FINAL","verification_flag":"UNVERIFIED","root_template":{"mapping_resource":"DCMR","template_identifier":"1500"},"document_title":code("126000","DCM","Imaging Measurement Report"),"observation_context":{"observer_type":"DEVICE","device_observer_uid":observer},"procedure_reported":code("25045-6","LN","CT unspecified body region"),"imaging_measurements":code("126010","DCM","Imaging Measurements"),"measurement_group":{"container":code("125007","DCM","Measurement Group"),"tracking_identifier":"DTS-TID1500-ROI-1","tracking_uid":tracking,"finding":code("123037004","SCT","Body structure"),"referenced_segment":{"source_case_id":refs[1]["source_case_id"],"sop_class_uid":refs[1]["sop_class_uid"],"sop_instance_uid":refs[1]["sop_instance_uid"],"series_instance_uid":refs[1]["series_instance_uid"],"segment_number":1,"referenced_frame_numbers":Value::Null,"source_image":{"source_case_id":refs[0]["source_case_id"],"sop_class_uid":refs[0]["sop_class_uid"],"sop_instance_uid":refs[0]["sop_instance_uid"],"series_instance_uid":refs[0]["series_instance_uid"],"referenced_frame_numbers":[1,2]}},"measurement":{"name":code("118565006","SCT","Volume"),"numeric_value":"5.625","units":code("mm3","UCUM","cubic millimeter")}},"evidence":[{"role":"source_image","source_case_id":refs[0]["source_case_id"],"sop_class_uid":refs[0]["sop_class_uid"],"sop_instance_uid":refs[0]["sop_instance_uid"],"series_instance_uid":refs[0]["series_instance_uid"]},{"role":"referenced_segmentation","source_case_id":refs[1]["source_case_id"],"sop_class_uid":refs[1]["sop_class_uid"],"sop_instance_uid":refs[1]["sop_instance_uid"],"series_instance_uid":refs[1]["series_instance_uid"]}]});
    entry["expected_visual_checks"] =
        json!({"pattern":"tid1500_volume_measurement_from_binary_segmentation"});
    entry["known_stressors"] = json!([
        "comprehensive_3d_sr_storage",
        "tid1500_measurement_report",
        "tid1411_measurement_group",
        "referenced_segment",
        "cross_instance_references",
        "external_generation_backend"
    ]);
    let _ = s;
    Ok(())
}

fn project_scoord3d(
    entry: &mut Value,
    output: &Value,
    planned: &PlannedImportedDicomArtifact,
    _input: &ManifestProjectionCompatibilityInput,
) -> Result<(), CuratedManifestError> {
    entry["references"][0]["relationship"] = json!("source_of_measurement");
    let s = &output["expected_semantics"];
    let r = entry["references"]
        .as_array()
        .and_then(|v| v.first())
        .ok_or_else(|| err("SCOORD3D reference missing"))?
        .clone();
    let code = |v: &str, scheme: &str, m: &str| json!({"code_value":v,"coding_scheme_designator":scheme,"code_meaning":m});
    let tracking = identity(
        &planned.declared_instance.identities,
        CompositionUidRole::TemplateDefined("tracking_uid".into()),
    )?;
    let observer = identity(
        &planned.declared_instance.identities,
        CompositionUidRole::TemplateDefined("observer_uid".into()),
    )?;
    let fiducial = identity(
        &planned.declared_instance.identities,
        CompositionUidRole::TemplateDefined("fiducial_uid".into()),
    )?;
    let frame = identity(
        &planned.declared_instance.identities,
        CompositionUidRole::FrameOfReference,
    )?;
    entry["recipe"]["recipe_parameters"] = json!({"tracking_identifier":"DTS-SCOORD3D-ROI-1","source_frame_numbers":[1,2],"graphic_type":"POLYLINE","graphic_data_patient_mm":[[0.0,0.0,0.0],[0.0,0.0,2.5]],"measurement_value_mm":2.5});
    entry["expected_capabilities"] = json!([
        "parse_structured_report",
        "parse_scoord3d",
        "resolve_references",
        "render_spatial_annotation"
    ]);
    entry["expected_semantics"] = json!({"synthetic_data":"YES","source_sop_instance_uid":r["sop_instance_uid"],"structured_report":{"completion_flag":"COMPLETE","preliminary_flag":"FINAL","verification_flag":"UNVERIFIED","root_value_type":"CONTAINER","root_continuity_of_content":"CONTINUOUS","content_sequence_items":8},"scoord3d":{"graphic_type":"POLYLINE","graphic_data_patient_mm":[[0.0,0.0,0.0],[0.0,0.0,2.5]],"frame_of_reference_uid":frame,"fiducial_uid":fiducial}});
    entry["expected_scoord3d"] = json!({"completion_flag":"COMPLETE","preliminary_flag":"FINAL","verification_flag":"UNVERIFIED","root_template":{"mapping_resource":"DCMR","template_identifier":"1500"},"document_title":code("126000","DCM","Imaging Measurement Report"),"observation_context":{"observer_type":"DEVICE","device_observer_uid":observer},"procedure_reported":code("25045-6","LN","CT unspecified body region"),"imaging_measurements":code("126010","DCM","Imaging Measurements"),"measurement_group":{"template":{"mapping_resource":"DCMR","template_identifier":"1501"},"container":code("125007","DCM","Measurement Group"),"tracking_identifier":"DTS-SCOORD3D-ROI-1","tracking_uid":tracking,"finding":code("123037004","SCT","Body structure"),"measurement":{"name":code("121206","DCM","Distance"),"numeric_value":"2.5","units":code("mm","UCUM","millimeter"),"spatial_coordinates":{"relationship":"INFERRED FROM","value_type":"SCOORD3D","concept_name":code("260753009","SCT","Source"),"graphic_type":"POLYLINE","graphic_data_mm":[0.0,0.0,0.0,0.0,0.0,2.5],"frame_of_reference_uid":frame,"fiducial_uid":fiducial}},"source_image":{"relationship":"CONTAINS","value_type":"IMAGE","concept_name":code("121112","DCM","Source of Measurement"),"source_case_id":r["source_case_id"],"sop_class_uid":r["sop_class_uid"],"sop_instance_uid":r["sop_instance_uid"],"series_instance_uid":r["series_instance_uid"],"referenced_frame_numbers":[1,2]}},"image_library_present":false,"evidence":[{"role":"source_image","source_case_id":r["source_case_id"],"sop_class_uid":r["sop_class_uid"],"sop_instance_uid":r["sop_instance_uid"],"series_instance_uid":r["series_instance_uid"]}]});
    entry["expected_visual_checks"] =
        json!({"pattern":"scoord3d_polyline_between_enhanced_ct_frames"});
    entry["known_stressors"] = json!([
        "comprehensive_3d_sr_storage",
        "tid1500_measurement_report",
        "tid1501_measurement_group",
        "scoord3d_patient_coordinates",
        "frame_of_reference_geometry",
        "cross_instance_references",
        "external_generation_backend"
    ]);
    let _ = s;
    Ok(())
}

fn identity(
    identities: &crate::composition::IdentityPlan,
    role: CompositionUidRole,
) -> Result<&str, CuratedManifestError> {
    identities
        .get(&role, 0)
        .ok_or_else(|| err(format!("missing planned identity {}", role.as_str())))
}
