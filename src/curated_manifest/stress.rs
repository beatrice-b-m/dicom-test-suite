//! Typed compatibility and resource qualification projection for stress cases.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::composition::CompositionUidRole;
use crate::corpus_plan::{PlannedArtifact, PlannedDicomArtifact};
use crate::curated_plan::{CuratedArtifactProjectionContext, CuratedScProjectionContext};
use crate::executor::adapters::{ManifestProjectionArtifact, ManifestProjectionInput};
use crate::executor::evidence::{ExecutionStatus, MaterializedContentEvidence, ResultStatus};
use crate::recipes::{
    STRESS_CT_PLAN_PROVIDER_ID, STRESS_SC_PLAN_PROVIDER_ID, StressCtArtifactParameters,
    StressCtParameters, StressScParameters,
};

use super::{
    CuratedManifestError, err, fail, legacy_validation, only, required, transfer_syntax_name, uid,
    validation_checks,
};

const RLE: &str = "1.2.840.10008.1.2.5";

pub(super) fn project_file_entry(
    context: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
) -> Result<Value, CuratedManifestError> {
    let PlannedArtifact::Dicom(planned) = &pair.planned else {
        return fail("stress artifact is not DICOM");
    };
    let (output, pixels) = verified_output(planned, pair)?;
    let mut checks = validation_checks(&pair.execution)?;
    if context.case_recipe.plan_provider_id == STRESS_CT_PLAN_PROVIDER_ID {
        // Reduced/full resource policy belongs to the run-level stress
        // qualification record, not the historical per-file validation block.
        checks.retain(|check| check.name != "stress_ct_reduced_qualification");
    } else if context.case_recipe.plan_provider_id == STRESS_SC_PLAN_PROVIDER_ID {
        checks.retain(|check| {
            !matches!(
                check.name.as_str(),
                "curated_composition_plan" | "extended_offset_table_arithmetic"
            )
        });
    }
    let (recipe_parameters, image, pixel_data, capabilities, semantics, pattern, stressors, extra) =
        if context.case_recipe.plan_provider_id == STRESS_CT_PLAN_PROVIDER_ID {
            project_ct(context, planned, pixels, &pair.execution)?
        } else if context.case_recipe.plan_provider_id == STRESS_SC_PLAN_PROVIDER_ID {
            project_sc(context, planned, pixels, &pair.execution)?
        } else {
            return fail("unsupported stress projection provider");
        };
    let public_recipe_id = if context.case_recipe.plan_provider_id == STRESS_CT_PLAN_PROVIDER_ID {
        // Preserve the versioned public manifest identity while the modular
        // document retains its registry-disambiguated recipe identifier.
        "stress_high_instance_count_ct"
    } else {
        context.case_recipe.recipe_id.as_str()
    };
    let mut manifest = json!({
        "case_id":context.registry_case.case_id,
        "profile_membership":context.registry_case.profiles,
        "path":output.relative_path,"sha256":output.sha256,"size_bytes":output.size_bytes,
        "determinism":context.registry_case.determinism,
        "recipe":{"recipe_id":public_recipe_id,"recipe_version":context.case_recipe.recipe_version,"recipe_parameters":recipe_parameters},
        "dicom":{"sop_class_uid":required(&context.registry_case.sop_class_uid,"registry SOP Class UID")?,
            "sop_class_name":required(&context.registry_case.sop_class_name,"registry SOP Class name")?,
            "iod_name":required(&context.registry_case.iod_name,"registry IOD name")?,"modality":required(&context.registry_case.modality,"registry modality")?,
            "transfer_syntax_uid":planned.encoding.transfer_syntax_uid,"transfer_syntax_name":transfer_syntax_name(&planned.encoding.transfer_syntax_uid)?},
        "uids":{"study_instance_uid":uid(planned,CompositionUidRole::StudyInstance)?,"series_instance_uid":uid(planned,CompositionUidRole::SeriesInstance)?,
            "sop_instance_uid":uid(planned,CompositionUidRole::SopInstance)?,"implementation_class_uid":planned.encoding.implementation.class_uid},
        "image":image,"pixel_data":pixel_data,"references":[],"expected_capabilities":capabilities,
        "expected_semantics":semantics,"expected_visual_checks":{"pattern":pattern},
        "validation":legacy_validation(&checks),"known_stressors":stressors,
        "standards_evidence":context.registry_case.standards_evidence
    });
    if context.case_recipe.plan_provider_id == STRESS_CT_PLAN_PROVIDER_ID {
        manifest["uids"]["frame_of_reference_uid"] = json!(
            planned
                .instance
                .identities
                .get(&CompositionUidRole::FrameOfReference, 0)
                .ok_or_else(|| err("stress CT lacks Frame of Reference UID"))?
        );
    }
    for (name, value) in extra {
        manifest[name] = value;
    }
    Ok(manifest)
}

fn verified_output<'a>(
    planned: &PlannedDicomArtifact,
    pair: &'a ManifestProjectionArtifact,
) -> Result<
    (
        &'a crate::executor::evidence::OutputEvidence,
        &'a MaterializedContentEvidence,
    ),
    CuratedManifestError,
> {
    let execution = &pair.execution;
    let output = execution
        .output
        .as_ref()
        .ok_or_else(|| err("stress artifact has no output evidence"))?;
    let materialization = execution
        .materialization
        .as_ref()
        .ok_or_else(|| err("stress artifact has no materialization evidence"))?;
    if execution.status != ExecutionStatus::Succeeded
        || !output.publish
        || output.relative_path != planned.output.relative_path.as_str()
        || materialization.transfer_syntax_uid.as_deref()
            != Some(&planned.encoding.transfer_syntax_uid)
        || materialization.implementation_class_uid.as_deref()
            != Some(&planned.encoding.implementation.class_uid)
        || materialization.materialized_artifact_sha256.as_deref() != Some(&output.sha256)
        || execution.resources.actual_output_bytes != output.size_bytes
        || execution.resources.actual_output_bytes > execution.resources.planned_output_bytes
        || execution
            .resources
            .actual_peak_working_bytes
            .is_none_or(|actual| actual > execution.resources.planned_peak_working_bytes)
    {
        return fail("stress output, identity, or resource evidence differs from its plan");
    }
    let pixels = materialization
        .content
        .iter()
        .find(|content| content.slot == "pixels")
        .ok_or_else(|| err("stress pixel evidence is absent"))?;
    Ok((output, pixels))
}

type StressFields = (
    Value,
    Value,
    Value,
    Vec<&'static str>,
    Value,
    &'static str,
    Vec<String>,
    BTreeMap<&'static str, Value>,
);

fn project_sc(
    context: &CuratedArtifactProjectionContext,
    _planned: &PlannedDicomArtifact,
    pixels: &MaterializedContentEvidence,
    execution: &crate::executor::evidence::ArtifactExecutionEvidence,
) -> Result<StressFields, CuratedManifestError> {
    let parameters: StressScParameters = serde_json::from_value(Value::Object(
        context.case_recipe.provider_parameters.clone(),
    ))
    .map_err(|error| err(format!("invalid typed stress SC parameters: {error}")))?;
    let native_pixel = |vr: &str, expected_value_length: u64| {
        if !execution.codecs.is_empty() || pixels.fragment_count != 0 {
            return fail("native stress SC has codec or fragment evidence");
        }
        let value_length = pixels
            .native_value_field_size_bytes
            .unwrap_or(pixels.size_bytes);
        if value_length != expected_value_length || pixels.size_bytes != expected_value_length {
            return fail("native stress SC Value Field size differs from its typed plan");
        }
        Ok(
            json!({"vr":vr,"native_or_encapsulated":"native","value_length":value_length,
            "frame_count":1,"frame_hashes":[pixels.sha256]}),
        )
    };
    let common_image = |rows, columns, frames, bits| {
        json!({"rows":rows,"columns":columns,"frames":frames,"samples_per_pixel":1,
            "photometric_interpretation":"MONOCHROME2","bits_allocated":bits,"bits_stored":bits,
            "high_bit":bits-1,"pixel_representation":0,"planar_configuration":Value::Null})
    };
    match parameters {
        StressScParameters::LargeBulk {
            rows,
            columns,
            payload_bytes,
            policy: _,
            ..
        } => Ok((
            json!({"rows":rows,"columns":columns,"payload_bytes":payload_bytes}),
            common_image(rows, columns, 1, 16),
            native_pixel("OW", payload_bytes)?,
            vec![
                "open_file",
                "read_metadata",
                "render_native_pixels",
                "stream_large_bulk_data",
            ],
            json!({"synthetic_data":"YES","conversion_type":"SYN","pixel_min":0,"pixel_max":0}),
            "uniform_zero_reduced_64_mib_native_pixel_data",
            context.artifact_recipe.stressors.clone(),
            BTreeMap::new(),
        )),
        StressScParameters::DeepNestedSequences {
            sequence_depth,
            payload_bytes,
            policy: _,
            ..
        } => Ok((
            json!({"sequence_depth":sequence_depth,"payload_bytes":payload_bytes}),
            common_image(2, 2, 1, 8),
            native_pixel("OB", 4)?,
            vec![
                "open_file",
                "read_metadata",
                "render_native_pixels",
                "bounded_metadata_traversal",
            ],
            json!({"synthetic_data":"YES","conversion_type":"SYN","sequence_depth":sequence_depth,"nested_payload_bytes":payload_bytes}),
            "tiny_gradient_with_32_level_private_sequence",
            context.artifact_recipe.stressors.clone(),
            BTreeMap::new(),
        )),
        StressScParameters::LongValueMetadata {
            creator_blocks,
            values_per_block,
            metadata_value_bytes,
            policy: _,
            ..
        } => {
            let metadata_values = u64::from(creator_blocks) * u64::from(values_per_block);
            let payload_bytes = metadata_values * u64::from(metadata_value_bytes);
            Ok((
                json!({"metadata_values":metadata_values,"metadata_value_bytes":metadata_value_bytes,"payload_bytes":payload_bytes}),
                common_image(2, 2, 1, 8),
                native_pixel("OB", 4)?,
                vec![
                    "open_file",
                    "read_metadata",
                    "render_native_pixels",
                    "bounded_metadata_traversal",
                ],
                json!({"synthetic_data":"YES","conversion_type":"SYN","metadata_values":metadata_values,"metadata_total_value_bytes":payload_bytes}),
                "tiny_gradient_with_1024_private_ut_values",
                context.artifact_recipe.stressors.clone(),
                BTreeMap::new(),
            ))
        }
        StressScParameters::LargeEncapsulatedMultifragment {
            rows,
            columns,
            frames,
            fragments_per_frame,
            policy: _,
            ..
        } => {
            let codec = only(&execution.codecs, "stress codec evidence")?;
            if codec.status != ResultStatus::Passed
                || codec.transfer_syntax_uid != RLE
                || codec.encoded_frame_sha256 != pixels.compressed_frame_sha256
                || pixels.fragments_per_frame
                    != vec![u64::from(fragments_per_frame); frames as usize]
                || pixels.fragment_count != u64::from(frames) * u64::from(fragments_per_frame)
                || pixels.extended_offset_table.len() != frames as usize
                || !pixels.basic_offset_table.is_empty()
            {
                return fail("stress multifragment codec evidence differs from its typed plan");
            }
            let compressed_payload_bytes = pixels.compressed_lengths.iter().sum::<u64>();
            let native_payload_bytes = u64::from(rows) * u64::from(columns) * u64::from(frames);
            Ok((
                json!({"rows":rows,"columns":columns,"frames":frames,"fragments_per_frame":fragments_per_frame,
                    "fragment_count":pixels.fragment_count,"native_payload_bytes":native_payload_bytes,"compressed_payload_bytes":compressed_payload_bytes}),
                common_image(rows, columns, frames, 8),
                json!({"vr":"OB","native_or_encapsulated":"encapsulated","value_length":Value::Null,
                    "frame_count":frames,"frame_hashes":codec.decoded_frame_sha256,
                    "codec":{"backend_id":codec.backend_id,"backend_kind":codec.backend_kind,"display_name":codec.display_name,
                        "version":codec.backend_version,"transfer_syntax_uid":codec.transfer_syntax_uid,
                        "feature_gate":codec.feature_gate,"determinism":codec.determinism},
                    "encapsulated_pixel_data":{"basic_offset_table":{"present":true,"populated":false,"offset_count":0,"offsets":[]},
                        "fragments_per_frame":pixels.fragments_per_frame,
                        "extended_offset_table":{"present":true,"lengths_present":true,
                            "offset_count":pixels.extended_offset_table.len(),"length_count":pixels.extended_offset_table_lengths.len(),
                            "offsets":pixels.extended_offset_table,"lengths":pixels.extended_offset_table_lengths},
                        "compressed_frame_hashes":pixels.compressed_frame_sha256}}),
                vec![
                    "open_file",
                    "read_metadata",
                    "decode_rle_lossless_pixels",
                    "parse_extended_offset_table",
                    "stream_multifragment_pixel_data",
                ],
                json!({"synthetic_data":"YES","conversion_type":"SYN","pixel_min":0,"pixel_max":255}),
                "256_deterministic_pseudorandom_monochrome_frames",
                context.artifact_recipe.stressors.clone(),
                BTreeMap::new(),
            ))
        }
    }
}

fn project_ct(
    context: &CuratedArtifactProjectionContext,
    _planned: &PlannedDicomArtifact,
    pixels: &MaterializedContentEvidence,
    execution: &crate::executor::evidence::ArtifactExecutionEvidence,
) -> Result<StressFields, CuratedManifestError> {
    let parameters: StressCtParameters = serde_json::from_value(Value::Object(
        context.case_recipe.provider_parameters.clone(),
    ))
    .map_err(|error| err(format!("invalid typed stress CT parameters: {error}")))?;
    let artifact: StressCtArtifactParameters =
        serde_json::from_value(Value::Object(context.artifact_recipe.parameters.clone()))
            .map_err(|error| err(format!("invalid typed stress CT artifact: {error}")))?;
    if !execution.codecs.is_empty() || pixels.fragment_count != 0 {
        return fail("stress CT has codec or fragment evidence");
    }
    let ordinal = u64::from(context.artifact_recipe.order) + 1;
    let stored_values = (0..parameters.rows * parameters.columns)
        .map(|index| i64::from((index % parameters.pixel_modulus) as i32 + parameters.pixel_offset))
        .collect::<Vec<_>>();
    let adjacent = vec![2.5; parameters.instances.saturating_sub(1) as usize];
    let position = artifact
        .image_position_patient
        .iter()
        .map(|value| value.parse::<f64>().map_err(|error| err(error.to_string())))
        .collect::<Result<Vec<_>, _>>()?;
    let recipe = json!({"rows":parameters.rows,"columns":parameters.columns,"samples_per_pixel":1,
        "photometric_interpretation":"MONOCHROME2","bits_allocated":16,"bits_stored":12,"high_bit":11,
        "pixel_representation":1,"pixel_values":stored_values,"geometry":{"pixel_spacing":parameters.pixel_spacing_mm.join("\\"),
            "image_orientation_patient":"1\\0\\0\\0\\1\\0","image_position_patient":artifact.image_position_patient.join("\\"),
            "slice_thickness":parameters.slice_spacing_mm,"spacing_between_slices":parameters.slice_spacing_mm,
            "position_along_normal":artifact.position_along_normal,"slice_order_index":ordinal,"slice_count":parameters.instances,
            "series_ordinal":1,"study_series_count":1},"kvp":"120","acquisition_number":"1","series_number":"1",
        "rescale":{"intercept":"-1024","slope":"1","type":"HU"},"window":{"center":"40","width":"400"}});
    let geometry = json!({"image_orientation_patient":[1.0,0.0,0.0,0.0,1.0,0.0],"image_position_patient":position,
        "position_along_normal_mm":artifact.position_along_normal,"geometric_order_index":ordinal,
        "instance_number":ordinal,"instance_number_state":"numeric","instance_number_order_index":ordinal,
        "series_instance_count":parameters.instances,"sort_basis":"image_position_patient_projected_on_slice_normal",
        "sort_direction":"ascending","sorting_conflict_expected":false,"adjacent_spacing_mm":adjacent,
        "spacing_uniform":true,"spacing_tolerance_mm":0.00001,"position_tolerance_mm":0.00001});
    let mut extra = BTreeMap::new();
    extra.insert("expected_geometry", geometry);
    Ok((
        recipe,
        json!({"rows":parameters.rows,"columns":parameters.columns,"frames":1,"samples_per_pixel":1,
            "photometric_interpretation":"MONOCHROME2","bits_allocated":16,"bits_stored":12,"high_bit":11,
            "pixel_representation":1,"planar_configuration":Value::Null}),
        json!({"vr":"OW","native_or_encapsulated":"native",
            "value_length":pixels.native_value_field_size_bytes.unwrap_or(pixels.size_bytes),
            "frame_count":pixels.native_frame_sha256.len(),"frame_hashes":pixels.native_frame_sha256}),
        vec![
            "open_file",
            "read_metadata",
            "render_native_pixels",
            "apply_modality_rescale",
            "apply_window",
            "sort_series_by_geometry",
        ],
        json!({"synthetic_data":"YES","image_type":"ORIGINAL\\PRIMARY\\AXIAL","pixel_min":parameters.pixel_min,
            "pixel_max":parameters.pixel_max,"rescale":{"intercept":"-1024","slope":"1","type":"HU","output_min":-2048,"output_max":1023},
            "window":{"center":"40","width":"400"},"geometry_sort_key":{"image_orientation_patient":"1\\0\\0\\0\\1\\0",
                "position_along_normal":artifact.position_along_normal,"slice_order_index":ordinal},
            "series_instance_count":parameters.instances,"shared_study_series_frame_of_reference":true}),
        "2x2_signed_ct_hu_gradient",
        vec![
            "ct_image_storage",
            "signed_12_bit_pixels",
            "modality_rescale",
            "window_center_width",
            "multi_instance_series",
            "geometry_slice_sorting",
            "reduced_stress_scale",
            "high_instance_count_study",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        extra,
    ))
}

/// Project U7.5 stress qualification records entirely from typed plan and
/// execution resource evidence. No filesystem or wall-clock readback occurs.
pub fn project_qualifications(
    context: &CuratedScProjectionContext,
    input: &ManifestProjectionInput,
) -> Result<Vec<Value>, CuratedManifestError> {
    let artifacts = input
        .artifacts
        .iter()
        .filter(|pair| !matches!(pair.planned, PlannedArtifact::Qualification(_)))
        .collect::<Vec<_>>();
    if context.artifacts.len() != artifacts.len() {
        return fail("stress projection context and artifact counts differ");
    }
    let mut groups: BTreeMap<
        &str,
        Vec<(
            &CuratedArtifactProjectionContext,
            &ManifestProjectionArtifact,
        )>,
    > = BTreeMap::new();
    for (ctx, pair) in context.artifacts.iter().zip(artifacts) {
        if ctx.artifact_id != pair.planned.logical_id() {
            return fail("stress projection context and planned identity differ");
        }
        groups
            .entry(&ctx.registry_case.case_id)
            .or_default()
            .push((ctx, pair));
    }
    let mut grouped = BTreeMap::new();
    for (case_id, pairs) in groups {
        let active = pairs.iter().any(|(ctx, pair)| {
            captured_stress(ctx) || stress_intent(ctx) || has_reduced_stress_evidence(&pair.planned)
        });
        if !active {
            continue;
        }
        let expected_count = stress_contract_members(pairs[0].0)?
            .ok_or_else(|| err("stress intent lacks captured typed stress membership"))?;
        if pairs.len() != expected_count {
            return fail("stress qualification group is incomplete or contains extra members");
        }
        let mut orders = std::collections::BTreeSet::new();
        for (ctx, pair) in &pairs {
            if stress_contract_members(ctx)? != Some(expected_count)
                || ctx.case_recipe != pairs[0].0.case_recipe
                || ctx.registry_case != pairs[0].0.registry_case
                || !orders.insert(ctx.artifact_recipe.order)
                || !has_reduced_stress_evidence(&pair.planned)
            {
                return fail("stress qualification group has crossed or partial captured evidence");
            }
            let PlannedArtifact::Dicom(planned) = &pair.planned else {
                return fail("stress qualification member is not DICOM");
            };
            let template = ctx
                .artifact_recipe
                .template
                .as_ref()
                .ok_or_else(|| err("stress template absent"))?;
            if planned.instance.template_id.0 != template.template_id
                || planned.instance.template_version.to_string() != template.template_version
                || ctx.registry_case.sop_class_uid.as_deref()
                    != Some(planned.instance.sop_class_uid.as_str())
                || planned.encoding.transfer_syntax_uid
                    != ctx.artifact_recipe.encoding.transfer_syntax_uid
                || planned.instance.transfer_syntax_uid
                    != ctx.artifact_recipe.encoding.transfer_syntax_uid
                || pair.execution.status != ExecutionStatus::Succeeded
                || pair.execution.logical_id != planned.logical_id
            {
                return fail(
                    "stress captured template, SOP, or execution identity differs from plan",
                );
            }
        }
        grouped.insert(case_id, pairs);
    }
    let mut qualifications = grouped
        .into_iter()
        .map(|(case_id, pairs)| qualification(case_id, &pairs))
        .collect::<Result<Vec<_>, _>>()?;
    qualifications.sort_by_key(|value| {
        value["case_id"]
            .as_str()
            .and_then(stress_qualification_order)
            .unwrap_or(usize::MAX)
    });
    Ok(qualifications)
}

fn captured_stress(ctx: &CuratedArtifactProjectionContext) -> bool {
    ctx.registry_case
        .profiles
        .iter()
        .any(|profile| profile == "stress")
}

fn stress_intent(ctx: &CuratedArtifactProjectionContext) -> bool {
    matches!(
        ctx.case_recipe.plan_provider_id.as_str(),
        STRESS_CT_PLAN_PROVIDER_ID | STRESS_SC_PLAN_PROVIDER_ID
    ) || matches!(
        ctx.artifact_recipe.algorithm_provider_id.as_deref(),
        Some("algorithm.stress_ct" | "algorithm.stress_sc")
    ) || ctx.artifact_recipe.content.provider_id == "content.stress.synthetic"
        || ctx
            .case_recipe
            .provider_parameters
            .get("stress")
            .is_some_and(|value| value != &Value::Bool(false))
        || ctx
            .artifact_recipe
            .parameters
            .get("pixel_algorithm")
            .and_then(|value| value.get("algorithm"))
            == Some(&json!("reduced_stress"))
}

/// Select only existing qualified stress tuples; caller names never manufacture intent.
fn stress_contract_members(
    ctx: &CuratedArtifactProjectionContext,
) -> Result<Option<usize>, CuratedManifestError> {
    if !captured_stress(ctx) && !stress_intent(ctx) {
        return Ok(None);
    }
    if !captured_stress(ctx) || !stress_intent(ctx) {
        return fail("captured stress profile and typed stress intent differ");
    }
    let recipe = &ctx.case_recipe;
    let artifact = &ctx.artifact_recipe;
    let id = ctx.registry_case.case_id.as_str();
    if recipe.binding.case_id != id || stress_qualification_order(id).is_none() {
        return fail("stress qualification is not an existing approved recipe");
    }
    let (provider, algorithm, template, sop, content, transfer, count) = match id {
        "stress/enhanced-ct/many_frames" => {
            #[derive(serde::Deserialize)]
            struct EnhancedStressIntent {
                family: String,
                stress: bool,
                concatenation: bool,
            }
            let input: EnhancedStressIntent =
                serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
                    .map_err(|error| err(error.to_string()))?;
            if input.family != "ct" || !input.stress || input.concatenation {
                return fail("enhanced stress requires the typed reduced CT contract");
            }
            (
                "native.enhanced_plan",
                "algorithm.enhanced",
                "enhanced/ct",
                "1.2.840.10008.5.1.4.1.1.2.1",
                "content.native_pixels",
                "1.2.840.10008.1.2.1",
                1,
            )
        }
        "stress/wsi/large_pyramid" => {
            let pixels: crate::recipes::WsiPixelAlgorithm = serde_json::from_value(
                artifact
                    .parameters
                    .get("pixel_algorithm")
                    .cloned()
                    .ok_or_else(|| err("stress WSI pixels absent"))?,
            )
            .map_err(|error| err(error.to_string()))?;
            if !matches!(
                pixels,
                crate::recipes::WsiPixelAlgorithm::ReducedStress { .. }
            ) || recipe.provider_parameters.get("dependency_mode")
                != Some(&json!("ordered_level_chain"))
            {
                return fail("stress WSI requires reduced ordered levels");
            }
            (
                "native.wsi_plan",
                "algorithm.wsi",
                "vl/wsi/pyramid-volume",
                "1.2.840.10008.5.1.4.1.1.77.1.6",
                "content.native_pixels",
                "1.2.840.10008.1.2.1",
                3,
            )
        }
        "stress/study/high_instance_count_ct" => {
            let parameters: StressCtParameters =
                serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
                    .map_err(|error| err(error.to_string()))?;
            (
                STRESS_CT_PLAN_PROVIDER_ID,
                "algorithm.stress_ct",
                "classic/ct",
                "1.2.840.10008.5.1.4.1.1.2",
                "content.native_pixels",
                "1.2.840.10008.1.2.1",
                parameters.instances as usize,
            )
        }
        _ => {
            let parameters: StressScParameters =
                serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
                    .map_err(|error| err(error.to_string()))?;
            let (expected_id, template, sop, transfer) = match parameters {
                StressScParameters::LargeBulk { .. } => (
                    "stress/sc/large_bulk_data",
                    "classic/secondary-capture/monochrome",
                    "1.2.840.10008.5.1.4.1.1.7",
                    "1.2.840.10008.1.2.1",
                ),
                StressScParameters::DeepNestedSequences { .. } => (
                    "stress/sc/deep_nested_sequences",
                    "classic/secondary-capture/monochrome",
                    "1.2.840.10008.5.1.4.1.1.7",
                    "1.2.840.10008.1.2.1",
                ),
                StressScParameters::LongValueMetadata { .. } => (
                    "stress/sc/long_value_metadata",
                    "classic/secondary-capture/monochrome",
                    "1.2.840.10008.5.1.4.1.1.7",
                    "1.2.840.10008.1.2.1",
                ),
                StressScParameters::LargeEncapsulatedMultifragment { .. } => (
                    "stress/sc/large_encapsulated_multifragment",
                    "classic/secondary-capture/multiframe-grayscale-byte",
                    "1.2.840.10008.5.1.4.1.1.7.2",
                    RLE,
                ),
            };
            if id != expected_id {
                return fail("stress SC typed variant and approved recipe differ");
            }
            (
                STRESS_SC_PLAN_PROVIDER_ID,
                "algorithm.stress_sc",
                template,
                sop,
                "content.stress.synthetic",
                transfer,
                1,
            )
        }
    };
    let declared = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| err("stress DICOM recipe absent"))?;
    if count == 0
        || declared.artifacts.len() != count
        || !declared
            .artifacts
            .iter()
            .any(|candidate| candidate == artifact)
        || recipe.plan_provider_id != provider
        || artifact.algorithm_provider_id.as_deref() != Some(algorithm)
        || artifact.template.as_ref().is_none_or(|reference| {
            reference.template_id != template || reference.template_version != "1.0.0"
        })
        || ctx.registry_case.sop_class_uid.as_deref() != Some(sop)
        || artifact.content.provider_id != content
        || artifact.encoding.transfer_syntax_uid != transfer
    {
        return fail("captured stress capability tuple is incomplete or crossed");
    }
    Ok(Some(count))
}

fn stress_qualification_order(case_id: &str) -> Option<usize> {
    [
        "stress/wsi/large_pyramid",
        "stress/study/high_instance_count_ct",
        "stress/sc/large_bulk_data",
        "stress/sc/deep_nested_sequences",
        "stress/sc/long_value_metadata",
        "stress/sc/large_encapsulated_multifragment",
        "stress/enhanced-ct/many_frames",
    ]
    .iter()
    .position(|approved| *approved == case_id)
}

fn qualification(
    case_id: &str,
    pairs: &[(
        &CuratedArtifactProjectionContext,
        &ManifestProjectionArtifact,
    )],
) -> Result<Value, CuratedManifestError> {
    let first = pairs
        .first()
        .ok_or_else(|| err("empty stress qualification group"))?;
    let (recipe, requested, policy) = match case_id {
        "stress/wsi/large_pyramid" => (
            "wsi_pyramid",
            scale_with_tiles(3, 0, 0, 0, 1024, 1024, 256, 256, 3, 0, 0),
            None,
        ),
        "stress/enhanced-ct/many_frames" => {
            ("enhanced_ct", scale(1, 256, 0, 0, 64, 64, 0, 0), None)
        }
        _ if first.0.case_recipe.plan_provider_id == STRESS_CT_PLAN_PROVIDER_ID => {
            let p: StressCtParameters = serde_json::from_value(Value::Object(
                first.0.case_recipe.provider_parameters.clone(),
            ))
            .map_err(|e| err(e.to_string()))?;
            (
                "ct_study",
                scale(p.instances, p.instances, 0, 0, p.rows, p.columns, 0, 0),
                Some(p.policy),
            )
        }
        _ if first.0.case_recipe.plan_provider_id == STRESS_SC_PLAN_PROVIDER_ID => {
            let p: StressScParameters = serde_json::from_value(Value::Object(
                first.0.case_recipe.provider_parameters.clone(),
            ))
            .map_err(|e| err(e.to_string()))?;
            match p {
                StressScParameters::LargeBulk {
                    payload_bytes,
                    policy,
                    ..
                } => (
                    "native_bulk_data",
                    scale(0, 0, 0, payload_bytes, 0, 0, 0, 0),
                    Some(policy),
                ),
                StressScParameters::DeepNestedSequences {
                    sequence_depth,
                    payload_bytes,
                    policy,
                    ..
                } => (
                    "nested_sequences",
                    scale(0, 0, 0, payload_bytes, 0, 0, sequence_depth, 0),
                    Some(policy),
                ),
                StressScParameters::LongValueMetadata {
                    creator_blocks,
                    values_per_block,
                    metadata_value_bytes,
                    policy,
                    ..
                } => {
                    let values = creator_blocks * values_per_block;
                    (
                        "long_metadata",
                        scale(
                            0,
                            0,
                            0,
                            u64::from(values) * u64::from(metadata_value_bytes),
                            0,
                            0,
                            0,
                            values,
                        ),
                        Some(policy),
                    )
                }
                StressScParameters::LargeEncapsulatedMultifragment {
                    frames,
                    fragments_per_frame,
                    rows,
                    columns,
                    policy,
                    ..
                } => (
                    "encapsulated_eot",
                    scale(
                        0,
                        frames,
                        fragments_per_frame,
                        u64::from(rows) * u64::from(columns) * u64::from(frames),
                        0,
                        0,
                        0,
                        0,
                    ),
                    Some(policy),
                ),
            }
        }
        _ => return fail("approved stress case has no stress plan provider"),
    };
    if policy.as_ref().is_some_and(|policy| {
        policy.qualification_scale != "reduced"
            || policy.full_scale_available
            || policy.full_scale_reason.trim().is_empty()
    }) || pairs
        .iter()
        .any(|(_, pair)| !has_reduced_stress_evidence(&pair.planned))
    {
        return fail("stress qualification policy is not reduced-only");
    }
    let actual_output = pairs
        .iter()
        .try_fold(0_u64, |sum, (_, p)| {
            sum.checked_add(p.execution.resources.actual_output_bytes)
        })
        .ok_or_else(|| err("actual stress output overflow"))?;
    let elapsed = pairs
        .iter()
        .try_fold(0_u64, |sum, (_, p)| {
            sum.checked_add(p.execution.resources.elapsed_milliseconds)
        })
        .ok_or_else(|| err("stress elapsed overflow"))?;
    let mut actual = requested.clone();
    actual["output_bytes"] = json!(actual_output);
    const PUBLIC_OUTPUT_CEILING: u64 = 256 * 1024 * 1024;
    const PUBLIC_PEAK_RSS_CEILING: u64 = 512 * 1024 * 1024;
    if actual_output > PUBLIC_OUTPUT_CEILING {
        return fail("stress output exceeds the versioned public resource envelope");
    }
    Ok(
        json!({"case_id":case_id,"kind":"stress_case_run","contract_version":"0.1.0","profile":"stress","recipe":recipe,
        "scale":"reduced","requested":requested,"actual":actual,
        "resource_envelope":{"output_bytes":PUBLIC_OUTPUT_CEILING,"peak_rss_bytes":PUBLIC_PEAK_RSS_CEILING,"case_wall_milliseconds":120000,
            "job_wall_milliseconds":600000,"recipe_output_bytes":Value::Null},
        "observation":{"output_bytes":actual_output,"elapsed_milliseconds":elapsed,"peak_rss_bytes":Value::Null},
        "outcome":"completed","unavailable_scales":[{"scale":"full","reason_code":"full_scale_runner_unimplemented",
            "message":"The scheduled full-scale streaming runner and independent resource qualification are not implemented."}],
        "payload_policy":"generated_payloads_uncommitted","status":"passed"}),
    )
}

fn has_reduced_stress_evidence(artifact: &crate::corpus_plan::PlannedArtifact) -> bool {
    let Some(evidence) = (match artifact {
        crate::corpus_plan::PlannedArtifact::Dicom(artifact) => Some(&artifact.evidence),
        _ => None,
    }) else {
        return false;
    };
    evidence.obligations.iter().any(|obligation| {
        obligation.obligation_id == "curated_generation_validation"
            && obligation.route_id == "shared_corpus_executor"
            && obligation.required
            && obligation.independence == crate::corpus_plan::EvidenceIndependence::SameProject
            && obligation.parameters.get("qualification_scale") == Some(&Value::from("reduced"))
            && obligation.parameters.get("full_scale_available") == Some(&Value::from(false))
            && obligation
                .parameters
                .get("full_scale_reason")
                .and_then(Value::as_str)
                .is_some_and(|reason| !reason.trim().is_empty())
    })
}

fn scale(
    instances: u32,
    frames: u32,
    fragments: u32,
    payload_bytes: u64,
    rows: u32,
    columns: u32,
    sequence_depth: u32,
    metadata_values: u32,
) -> Value {
    json!({"instances":instances,"frames":frames,"fragments":fragments,"payload_bytes":payload_bytes,"output_bytes":0,
        "rows":rows,"columns":columns,"tile_rows":0,"tile_columns":0,"pyramid_levels":0,"sequence_depth":sequence_depth,"metadata_values":metadata_values})
}

#[allow(clippy::too_many_arguments)]
fn scale_with_tiles(
    instances: u32,
    frames: u32,
    fragments: u32,
    payload_bytes: u64,
    rows: u32,
    columns: u32,
    tile_rows: u32,
    tile_columns: u32,
    pyramid_levels: u32,
    sequence_depth: u32,
    metadata_values: u32,
) -> Value {
    json!({"instances":instances,"frames":frames,"fragments":fragments,"payload_bytes":payload_bytes,"output_bytes":0,
        "rows":rows,"columns":columns,"tile_rows":tile_rows,"tile_columns":tile_columns,"pyramid_levels":pyramid_levels,
        "sequence_depth":sequence_depth,"metadata_values":metadata_values})
}

#[cfg(test)]
mod stress_projection_tests {
    use super::*;
    use crate::curated_plan::{
        CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
    };

    #[test]
    fn stress_projection_requires_captured_typed_complete_evidence() {
        let catalog = crate::recipes::RecipeCatalog::load(
            "cases/recipes",
            "cases/registry.json",
            "templates/catalog.json",
        )
        .unwrap();
        let registry: Value =
            serde_json::from_str(include_str!("../../cases/registry.json")).unwrap();
        for case in
            registry["cases"].as_array().unwrap().iter().filter(|case| {
                stress_qualification_order(case["case_id"].as_str().unwrap()).is_some()
            })
        {
            let recipe = catalog
                .recipes()
                .values()
                .find(|recipe| recipe.binding.case_id == case["case_id"].as_str().unwrap())
                .unwrap();
            for artifact in &recipe.dicom.as_ref().unwrap().artifacts {
                let ctx = CuratedArtifactProjectionContext {
                    artifact_id: artifact.logical_id.clone(),
                    plan_order: 0,
                    registry_order: 0,
                    historical_recipe_order: recipe.planning_order.unwrap(),
                    historical_artifact_order: artifact.order,
                    registry_case: serde_json::from_value(case.clone()).unwrap(),
                    case_recipe: recipe.clone(),
                    artifact_recipe: artifact.clone(),
                };
                assert_eq!(
                    stress_contract_members(&ctx).unwrap(),
                    Some(recipe.dicom.as_ref().unwrap().artifacts.len())
                );
                let mut core = ctx.clone();
                core.registry_case.profiles = vec!["core".into()];
                assert!(stress_contract_members(&core).is_err());
                let mut crossed = ctx.clone();
                crossed.artifact_recipe.algorithm_provider_id =
                    Some("algorithm.classic_nuclear".into());
                assert!(stress_contract_members(&crossed).is_err());
                let mut unknown = ctx.clone();
                unknown.registry_case.case_id = "caller/stress".into();
                assert!(stress_contract_members(&unknown).is_err());
            }
        }
        let provider =
            CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
                .unwrap();
        let bundle = provider
            .plan(&CuratedScPlanRequest {
                selection: CuratedScSelection::CaseIds(vec![
                    "stress/enhanced-ct/many_frames".into(),
                ]),
                seed: 1,
                max_parallelism: 1,
            })
            .unwrap();
        let context = bundle.projection;
        let artifacts = bundle.plan.artifacts.iter().map(|planned| {
            let PlannedArtifact::Dicom(dicom) = planned else { panic!("DICOM plan required"); };
            ManifestProjectionArtifact { planned:planned.clone(), execution:serde_json::from_value(json!({
                "logical_id":dicom.logical_id,"order":dicom.order,"artifact_kind":"dicom","status":"succeeded", "corpus_plan_sha256":"0".repeat(64),"instance_plan_sha256":dicom.instance.canonical_sha256(),
                "output":{"relative_path":"synthetic/stress.dcm","publish":true,"size_bytes":123,"sha256":"0".repeat(64)},"materialization":null,"validation":[],"obligations":[],"providers":[],"codecs":[],
                "resources":{"planned_output_bytes":1000,"planned_peak_working_bytes":1000,"actual_output_bytes":123,"actual_peak_working_bytes":null,"elapsed_milliseconds":1}
            })).unwrap() }
        }).collect();
        let input = ManifestProjectionInput { corpus_plan_sha256:"0".repeat(64), artifacts, unavailable:vec![],
            resources:serde_json::from_value(json!({"planned_max_artifacts":1,"planned_max_total_output_bytes":1000,"planned_max_peak_working_bytes":1000,"requested_parallelism":1,"used_parallelism":1,"actual_artifact_output_bytes":123,"actual_publication_bytes":0,"actual_peak_working_bytes":null})).unwrap(),
            publication:serde_json::from_value(json!({"manifest_relative_path":"manifest.json","state":"staging","private_staging":true,"no_overwrite":true,"validation_complete":false,"cleanup_complete":false,"manifest_sha256":null})).unwrap() };
        let expected = qualification(
            "stress/enhanced-ct/many_frames",
            &[(&context.artifacts[0], &input.artifacts[0])],
        )
        .unwrap();
        assert_eq!(
            project_qualifications(&context, &input).unwrap(),
            vec![expected]
        );
        let PlannedArtifact::Dicom(first) = &input.artifacts[0].planned else {
            unreachable!()
        };
        let mut qualification_pair = input.artifacts[0].clone();
        qualification_pair.planned =
            PlannedArtifact::Qualification(crate::corpus_plan::PlannedQualification {
                logical_id: "synthetic-qualification".into(),
                order: 0,
                provenance: first.provenance.clone(),
                case_binding: None,
                profile: None,
                run_seed: None,
                qualification_kind: "synthetic".into(),
                parameters: Default::default(),
                sources: vec![],
                payload_policy: crate::corpus_plan::QualificationPayloadPolicy::NoPayload,
                validation: first.validation.clone(),
                evidence: first.evidence.clone(),
                resources: first.resources.clone(),
            });
        for index in [0, 1] {
            let mut interleaved = input.clone();
            interleaved
                .artifacts
                .insert(index, qualification_pair.clone());
            assert_eq!(
                project_qualifications(&context, &interleaved).unwrap(),
                project_qualifications(&context, &input).unwrap()
            );
        }
        let mut empty = context.clone();
        empty.artifacts.clear();
        let mut empty_input = input.clone();
        empty_input.artifacts = vec![qualification_pair];
        assert!(
            project_qualifications(&empty, &empty_input)
                .unwrap()
                .is_empty()
        );
        for mutation in 0..6 {
            let mut changed = input.clone();
            let PlannedArtifact::Dicom(plan) = &mut changed.artifacts[0].planned else {
                unreachable!()
            };
            match mutation {
                0 => plan.evidence.obligations[0].route_id = "unrelated".into(),
                1 => plan.evidence.obligations[0].required = false,
                2 => {
                    plan.evidence.obligations[0].independence =
                        crate::corpus_plan::EvidenceIndependence::IndependentTool
                }
                3 => {
                    plan.evidence.obligations[0]
                        .parameters
                        .insert("qualification_scale".into(), json!("full"));
                }
                4 => plan.encoding.transfer_syntax_uid = RLE.into(),
                _ => plan.instance.transfer_syntax_uid = RLE.into(),
            }
            assert!(
                project_qualifications(&context, &changed).is_err(),
                "mutation {mutation}"
            );
        }
        let mut extra = context.clone();
        extra.artifacts.push(context.artifacts[0].clone());
        let mut extra_input = input.clone();
        extra_input.artifacts.push(input.artifacts[0].clone());
        assert!(project_qualifications(&extra, &extra_input).is_err());
        let mut wrong = context.clone();
        wrong.artifacts[0].artifact_id = "crossed".into();
        assert!(project_qualifications(&wrong, &input).is_err());
        let mut absent = input.clone();
        absent.artifacts.clear();
        assert!(project_qualifications(&context, &absent).is_err());
        let mut core = context.clone();
        core.artifacts[0].registry_case.profiles = vec!["core".into()];
        assert!(project_qualifications(&core, &input).is_err());
        let mut no_obligation = input.clone();
        let PlannedArtifact::Dicom(plan) = &mut no_obligation.artifacts[0].planned else {
            unreachable!()
        };
        plan.evidence.obligations.clear();
        assert!(project_qualifications(&context, &no_obligation).is_err());
        let us = provider
            .plan(&CuratedScPlanRequest {
                selection: CuratedScSelection::CaseIds(vec![
                    "classic/us/mono2_u8_explicit_le".into(),
                ]),
                seed: 1,
                max_parallelism: 1,
            })
            .unwrap();
        let mut ordinary = us.projection;
        ordinary.artifacts[0].registry_case.case_id = "stress/enhanced-ct/many_frames".into();
        assert_eq!(
            stress_contract_members(&ordinary.artifacts[0]).unwrap(),
            None
        );
        let mut ordinary_input = input.clone();
        ordinary_input.artifacts[0].planned = us.plan.artifacts[0].clone();
        assert!(
            project_qualifications(&ordinary, &ordinary_input)
                .unwrap()
                .is_empty()
        );
    }
}
