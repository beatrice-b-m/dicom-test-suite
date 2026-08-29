//! Pure compatibility projection for plan-first curated generation.

mod classic;

use std::collections::BTreeSet;
use std::fmt;

use serde_json::{Value, json};

use crate::composition::CompositionUidRole;
use crate::corpus_plan::{OffsetTablePolicy, PlannedArtifact, PlannedDicomArtifact};
use crate::curated_execution::{
    AdvancedCompatibilityProvider, advanced_artifact_parameters, advanced_provider_parameters,
    wsi_artifact_parameters,
};
use crate::curated_plan::{CuratedArtifactProjectionContext, CuratedScProjectionContext};
use crate::curated_validation::{CheckLayer, MetadataObservation, TypedValidationCheck};
use crate::executor::adapters::{ManifestProjectionArtifact, ManifestProjectionCompatibilityInput};
use crate::executor::evidence::{
    ArtifactExecutionEvidence, MaterializedContentEvidence, ResultStatus,
};
use crate::recipes::{
    EnhancedMrFrameAxis, MetadataScParameters, PRESENTATION_ADVANCED_PROVIDER_ID, PresentationKind,
    PrivateElementValue, REGISTRATION_PLAN_PROVIDER_ID, RegistrationKindInput, StringValueSource,
    WsiPixelAlgorithm,
};

const RLE: &str = "1.2.840.10008.1.2.5";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuratedManifestError(pub String);

impl fmt::Display for CuratedManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for CuratedManifestError {}

pub fn project_curated_file_entries(
    context: &CuratedScProjectionContext,
    input: &ManifestProjectionCompatibilityInput,
) -> Result<Vec<Value>, CuratedManifestError> {
    if context.artifacts.len() != input.artifacts.len() {
        return fail("projection context and execution artifact counts differ");
    }
    let mut entries = context
        .artifacts
        .iter()
        .zip(&input.artifacts)
        .map(|(ctx, artifact)| {
            if ctx.artifact_id != artifact.execution.logical_id
                || artifact.planned.logical_id() != ctx.artifact_id
                || artifact.execution.corpus_plan_sha256 != input.corpus_plan_sha256
            {
                return fail(format!(
                    "projection artifact identity mismatch for {}",
                    ctx.artifact_id
                ));
            }
            project_one(ctx, artifact, input)
        })
        .collect::<Result<Vec<_>, _>>()?;
    project_wsi_pyramid_group(context, input, &mut entries)?;
    Ok(entries)
}

fn project_wsi_pyramid_group(
    context: &CuratedScProjectionContext,
    input: &ManifestProjectionCompatibilityInput,
    entries: &mut [Value],
) -> Result<(), CuratedManifestError> {
    const CASE_ID: &str = "vl/wsi/pyramid_multiresolution";
    let indexes = context
        .artifacts
        .iter()
        .enumerate()
        .filter_map(|(index, artifact)| {
            (artifact.registry_case.case_id == CASE_ID).then_some(index)
        })
        .collect::<Vec<_>>();
    if indexes.is_empty() {
        return Ok(());
    }
    if indexes.len() != 3 {
        return fail("WSI pyramid compatibility projection requires all three members");
    }

    let by_role = |role: &str| {
        indexes
            .iter()
            .copied()
            .find(|index| context.artifacts[*index].artifact_recipe.output.role == role)
            .ok_or_else(|| err(format!("WSI pyramid is missing its {role} member")))
    };
    let ordered = [by_role("volume")?, by_role("thumbnail")?, by_role("label")?];
    let planned = ordered
        .map(|index| match &input.artifacts[index].planned {
            PlannedArtifact::Dicom(artifact) => Ok(artifact),
            _ => fail("WSI pyramid member is not DICOM"),
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let output = ordered
        .map(|index| {
            input.artifacts[index]
                .execution
                .output
                .as_ref()
                .ok_or_else(|| err("WSI pyramid member has no output evidence"))
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let first = planned[0];
    let identity = |role| {
        first
            .instance
            .identities
            .get(&role, 0)
            .ok_or_else(|| err("WSI pyramid is missing a shared identity"))
    };
    let study = identity(CompositionUidRole::StudyInstance)?;
    let series = identity(CompositionUidRole::SeriesInstance)?;
    let frame_of_reference = identity(CompositionUidRole::FrameOfReference)?;
    let specimen = identity(CompositionUidRole::TemplateDefined("specimen_uid".into()))?;
    let pyramid = identity(CompositionUidRole::TemplateDefined("pyramid_uid".into()))?;
    let sops = planned
        .iter()
        .map(|artifact| {
            artifact
                .instance
                .identities
                .get(&CompositionUidRole::SopInstance, 0)
                .ok_or_else(|| err("WSI pyramid member is missing its SOP Instance UID"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let contract = crate::wsi_pyramid_locked_contract(crate::WsiPyramidLockedInputs {
        study_instance_uid: study,
        series_instance_uid: series,
        frame_of_reference_uid: frame_of_reference,
        specimen_uid: specimen,
        pyramid_uid: pyramid,
        members: [
            crate::WsiPyramidMemberIdentity {
                role: crate::WsiPyramidRole::Volume,
                path: &output[0].relative_path,
                sha256: &output[0].sha256,
                size_bytes: output[0].size_bytes,
                sop_instance_uid: sops[0],
            },
            crate::WsiPyramidMemberIdentity {
                role: crate::WsiPyramidRole::Thumbnail,
                path: &output[1].relative_path,
                sha256: &output[1].sha256,
                size_bytes: output[1].size_bytes,
                sop_instance_uid: sops[1],
            },
            crate::WsiPyramidMemberIdentity {
                role: crate::WsiPyramidRole::Label,
                path: &output[2].relative_path,
                sha256: &output[2].sha256,
                size_bytes: output[2].size_bytes,
                sop_instance_uid: sops[2],
            },
        ],
    });

    for (ordinal, index) in ordered.into_iter().enumerate() {
        let role = ["volume", "thumbnail", "label"][ordinal];
        let membership = ["pyramid_layer", "pyramid_apex", "non_member_companion"][ordinal];
        let pattern = [
            "4x4_red_green_blue_white_quadrants",
            "2x2_volume_quadrant_reduction",
            "2x2_synthetic_label_companion",
        ][ordinal];
        let entry = &mut entries[index];
        entry["wsi_pyramid_role"] = json!(role);
        entry["wsi_pyramid_ordinal"] = json!(ordinal + 1);
        entry["recipe"] = json!({
            "recipe_id":"vl_wsi_pyramid_multiresolution",
            "recipe_version":"0.1.0",
            "recipe_parameters":{
                "group_role":role,
                "group_ordinal":ordinal + 1,
                "ordered_roles":["volume","thumbnail","label"],
                "pyramid_membership":membership,
                "thumbnail_provenance":if ordinal == 1 { json!("deterministic_quadrant_reduction_of_volume") } else { Value::Null },
                "icc_profile_sha256":"8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef"
            }
        });
        entry["expected_capabilities"] = json!([
            "open_file",
            "read_metadata",
            "render_native_pixels",
            "navigate_multiframe",
            "reconstruct_wsi_pyramid"
        ]);
        entry["expected_semantics"] = json!({
            "synthetic_data":"YES",
            "image_type":planned_string(planned[ordinal], "0008,0008")?
                .split('\\')
                .collect::<Vec<_>>(),
            "shared_study_series_frame_of_reference":true,
            "shared_specimen_and_optical_path":true,
            "pyramid_membership":membership,
            "ordered_group_member":true,
            "reference_free":true
        });
        entry["expected_wsi_pyramid"] = contract.clone();
        entry["expected_visual_checks"] = json!({"pattern":pattern});
        entry["known_stressors"] = json!([
            "vl_whole_slide_microscopy_image_storage",
            "multi_resolution_pyramid_membership",
            "thumbnail_apex_reduction",
            "label_non_member_companion",
            "shared_specimen_and_optical_path_metadata",
            "three_instance_group_closure"
        ]);
    }
    Ok(())
}

fn project_one(
    ctx: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
    input: &ManifestProjectionCompatibilityInput,
) -> Result<Value, CuratedManifestError> {
    let PlannedArtifact::Dicom(planned) = &pair.planned else {
        return fail("curated artifact is not DICOM");
    };
    let execution = &pair.execution;
    if planned.order != ctx.plan_order
        || execution.order != ctx.plan_order
        || planned.case_binding.as_ref().is_none_or(|binding| {
            binding.case_id != ctx.registry_case.case_id
                || binding.recipe_id != ctx.case_recipe.recipe_id
                || binding.recipe_version != ctx.case_recipe.recipe_version
        })
    {
        return fail("projection context differs from planned artifact");
    }
    if ctx.case_recipe.plan_provider_id == "native.classic_plan" {
        return classic::project_classic_file_entry(ctx, pair);
    }
    if matches!(
        ctx.case_recipe.plan_provider_id.as_str(),
        "native.enhanced_plan" | "native.wsi_plan"
    ) {
        return project_advanced_file_entry(ctx, pair, planned);
    }
    if matches!(
        ctx.case_recipe.plan_provider_id.as_str(),
        REGISTRATION_PLAN_PROVIDER_ID | PRESENTATION_ADVANCED_PROVIDER_ID
    ) {
        return project_reference_file_entry(ctx, pair, planned, input);
    }
    let sc = ctx
        .artifact_recipe
        .secondary_capture
        .as_ref()
        .ok_or_else(|| err("missing secondary_capture"))?;
    let output = execution
        .output
        .as_ref()
        .ok_or_else(|| err("missing output evidence"))?;
    if output.relative_path != planned.output.relative_path.as_str()
        || output.relative_path != ctx.artifact_recipe.output.path.as_deref().unwrap_or("")
        || !output.publish
        || execution.status != crate::executor::evidence::ExecutionStatus::Succeeded
    {
        return fail("output evidence differs from plan/recipe");
    }
    let materialization = execution
        .materialization
        .as_ref()
        .ok_or_else(|| err("missing materialization evidence"))?;
    if materialization.transfer_syntax_uid.as_deref() != Some(&planned.encoding.transfer_syntax_uid)
        || materialization.implementation_class_uid.as_deref()
            != Some(&planned.encoding.implementation.class_uid)
        || materialization.materialized_artifact_sha256.as_deref() != Some(&output.sha256)
    {
        return fail("materialization identity differs from plan/output");
    }
    let pixels = materialization
        .content
        .iter()
        .find(|content| content.slot == "pixels")
        .ok_or_else(|| err("missing pixel materialization evidence"))?;
    let frame_count = usize::try_from(sc.frames).map_err(|_| err("frame count overflow"))?;
    let observed_frames = if pixels.decoded_frame_sha256.is_empty() {
        execution
            .codecs
            .first()
            .map(|codec| codec.decoded_frame_sha256.as_slice())
            .unwrap_or(&[])
    } else {
        pixels.decoded_frame_sha256.as_slice()
    };
    if observed_frames.len() != frame_count || observed_frames != sc.frame_sha256 {
        return fail(format!(
            "decoded frame evidence differs from recipe for {}: {:?} != {:?}",
            ctx.artifact_id, observed_frames, sc.frame_sha256
        ));
    }
    let checks = validation_checks(execution)?;
    let observation = metadata_observation(execution)?;
    let observation_required = ctx.artifact_recipe.metadata_sc.is_some()
        || ctx.artifact_recipe.nonsquare_geometry.is_some()
        || sc.encapsulation_projection.is_some();
    if observation_required != observation.is_some() {
        return fail(format!(
            "metadata observation presence differs from recipe for {}: required={observation_required}, present={}",
            ctx.artifact_id,
            observation.is_some()
        ));
    }

    let palette = sc.palette.as_ref().map(|palette| {
        json!({
            "descriptor": palette.descriptor,
            "red_data_value_length": palette.red.len() * 2,
            "green_data_value_length": palette.green.len() * 2,
            "blue_data_value_length": palette.blue.len() * 2,
        })
    });
    let padding = sc
        .padding
        .as_ref()
        .map(|padding| json!({"value":padding.value,"range_limit":padding.range_limit}));
    let pixel_data = pixel_data(planned, execution, pixels, sc)?;
    let study = uid(planned, CompositionUidRole::StudyInstance)?;
    let series = uid(planned, CompositionUidRole::SeriesInstance)?;
    let sop = uid(planned, CompositionUidRole::SopInstance)?;
    let mut manifest = json!({
        "case_id": ctx.registry_case.case_id,
        "profile_membership": ctx.artifact_recipe.public_profile_membership.as_ref().unwrap_or(&ctx.registry_case.profiles),
        "path": output.relative_path,
        "sha256": output.sha256,
        "size_bytes": output.size_bytes,
        "determinism": ctx.registry_case.determinism,
        "recipe": {"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,"recipe_parameters":{
            "rows":sc.rows,"columns":sc.columns,"samples_per_pixel":sc.samples_per_pixel,
            "photometric_interpretation":sc.photometric_interpretation,"bits_allocated":sc.bits_allocated,
            "bits_stored":sc.bits_stored,"planar_configuration":sc.color.as_ref().and_then(|c| c.planar_configuration),
            "pixel_values":sc.stored_values,"palette":palette,"pixel_padding":padding
        }},
        "dicom": {
            "sop_class_uid": required(&ctx.registry_case.sop_class_uid,"registry SOP Class UID")?,
            "sop_class_name": required(&ctx.registry_case.sop_class_name,"registry SOP Class name")?,
            "iod_name": required(&ctx.registry_case.iod_name,"registry IOD name")?,
            "modality": required(&ctx.registry_case.modality,"registry modality")?,
            "transfer_syntax_uid": planned.encoding.transfer_syntax_uid,
            "transfer_syntax_name": transfer_syntax_name(&planned.encoding.transfer_syntax_uid)?
        },
        "uids":{"study_instance_uid":study,"series_instance_uid":series,"sop_instance_uid":sop,
            "frame_of_reference_uid":Value::Null,"implementation_class_uid":planned.encoding.implementation.class_uid,
            "implementation_version_name":planned.encoding.implementation.version_name},
        "image":{"rows":sc.rows,"columns":sc.columns,"frames":sc.frames,"samples_per_pixel":sc.samples_per_pixel,
            "photometric_interpretation":sc.photometric_interpretation,"bits_allocated":sc.bits_allocated,
            "bits_stored":sc.bits_stored,"high_bit":sc.high_bit,"pixel_representation":sc.pixel_representation,
            "planar_configuration":sc.color.as_ref().and_then(|c| c.planar_configuration)},
        "pixel_data":pixel_data,
        "expected_capabilities":capabilities(sc, &planned.encoding.transfer_syntax_uid, ctx.artifact_recipe.nonsquare_geometry.is_some()),
        "expected_semantics":{"synthetic_data":"YES","conversion_type":"SYN","image_type":Value::Null,
            "pixel_min":sc.pixel_min,"pixel_max":sc.pixel_max,"pixel_padding":padding,
            "lossy_image_compression":"00","lossy_image_compression_ratio":Value::Null,
            "lossy_image_compression_method":Value::Null,"photometric_semantics":sc.semantic_note},
        "expected_visual_checks":{"pattern":sc.visual_pattern},
        "validation":legacy_validation(&checks),
        "known_stressors":ctx.artifact_recipe.stressors,
        "standards_evidence":standards(ctx, sc, &planned.encoding.transfer_syntax_uid),
        "references":[]
    });
    add_special(&mut manifest, ctx, planned, pixels, observation.as_ref())?;
    Ok(manifest)
}

fn project_advanced_file_entry(
    ctx: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
    planned: &PlannedDicomArtifact,
) -> Result<Value, CuratedManifestError> {
    let execution = &pair.execution;
    let output = execution
        .output
        .as_ref()
        .ok_or_else(|| err("missing advanced output evidence"))?;
    if output.relative_path != planned.output.relative_path.as_str()
        || output.relative_path != ctx.artifact_recipe.output.path.as_deref().unwrap_or("")
        || !output.publish
        || execution.status != crate::executor::evidence::ExecutionStatus::Succeeded
    {
        return fail("advanced output evidence differs from plan/recipe");
    }
    let materialization = execution
        .materialization
        .as_ref()
        .ok_or_else(|| err("missing advanced materialization evidence"))?;
    if materialization.transfer_syntax_uid.as_deref() != Some(&planned.encoding.transfer_syntax_uid)
        || materialization.implementation_class_uid.as_deref()
            != Some(&planned.encoding.implementation.class_uid)
        || materialization.materialized_artifact_sha256.as_deref() != Some(&output.sha256)
    {
        return fail("advanced materialization identity differs from plan/output");
    }
    let pixels = materialization
        .content
        .iter()
        .find(|content| content.slot == "pixels")
        .ok_or_else(|| err("missing advanced pixel materialization evidence"))?;
    let frame_hashes = if pixels.decoded_frame_sha256.is_empty() {
        &pixels.native_frame_sha256
    } else {
        &pixels.decoded_frame_sha256
    };
    let rows = planned_unsigned(planned, "0028,0010")?;
    let columns = planned_unsigned(planned, "0028,0011")?;
    let samples_per_pixel = planned_unsigned(planned, "0028,0002")?;
    let photometric = planned_string(planned, "0028,0004")?;
    let frames = planned_optional_unsigned(planned, "0028,0008")?.unwrap_or(1);
    let bits_allocated = planned_unsigned(planned, "0028,0100")?;
    let bits_stored = planned_unsigned(planned, "0028,0101")?;
    let high_bit = planned_unsigned(planned, "0028,0102")?;
    let pixel_representation = planned_unsigned(planned, "0028,0103")?;
    let planar_configuration = planned_optional_unsigned(planned, "0028,0006")?;
    if frame_hashes.len() as u64 != frames {
        return fail("advanced frame evidence cardinality differs from Number of Frames");
    }
    let checks = validation_checks(execution)?;
    let study = uid(planned, CompositionUidRole::StudyInstance)?;
    let series = uid(planned, CompositionUidRole::SeriesInstance)?;
    let sop = uid(planned, CompositionUidRole::SopInstance)?;
    let frame_of_reference = planned
        .instance
        .identities
        .get(&CompositionUidRole::FrameOfReference, 0);
    let dimension = planned
        .instance
        .identities
        .get(&CompositionUidRole::DimensionOrganization, 0);
    let reduced_stress_wsi = ctx.case_recipe.plan_provider_id == "native.wsi_plan"
        && wsi_artifact_parameters(ctx).is_ok_and(|item| {
            matches!(
                item.pixel_algorithm,
                WsiPixelAlgorithm::ReducedStress { .. }
            )
        });
    let compatibility_checks = checks
        .iter()
        .filter(|check| {
            !matches!(
                check.name.as_str(),
                "enhanced_plan_materialization_round_trip" | "wsi_plan_materialization_round_trip"
            ) && !(reduced_stress_wsi && check.name == "curated_composition_plan")
        })
        .cloned()
        .collect::<Vec<_>>();
    let common = AdvancedManifestCommon {
        output,
        pixels,
        frame_hashes,
        rows,
        columns,
        frames,
        samples_per_pixel,
        photometric: &photometric,
        bits_allocated,
        bits_stored,
        high_bit,
        pixel_representation,
        planar_configuration,
        study,
        series,
        sop,
        frame_of_reference,
        dimension,
        validation: legacy_validation(&compatibility_checks),
    };
    if ctx.case_recipe.plan_provider_id == "native.enhanced_plan" {
        project_enhanced_manifest(ctx, planned, &common)
    } else {
        project_wsi_manifest(ctx, planned, &common)
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationProjectionParameters {
    #[serde(default)]
    uid_reference_index: Option<u32>,
    presentation: PresentationKind,
    sources: Vec<Value>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationProjectionParameters {
    series_number: String,
    study_id: String,
    laterality: String,
    manufacturer_model_name: String,
    device_serial_number: String,
    content_label: String,
    content_description: String,
    registration: RegistrationKindInput,
    sources: Vec<Value>,
}

fn project_reference_file_entry(
    ctx: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
    planned: &PlannedDicomArtifact,
    input: &ManifestProjectionCompatibilityInput,
) -> Result<Value, CuratedManifestError> {
    let execution = &pair.execution;
    let output = execution
        .output
        .as_ref()
        .ok_or_else(|| err("missing reference output evidence"))?;
    if output.relative_path != planned.output.relative_path.as_str()
        || !output.publish
        || execution.status != crate::executor::evidence::ExecutionStatus::Succeeded
    {
        return fail("reference output evidence differs from plan");
    }
    let materialization = execution
        .materialization
        .as_ref()
        .ok_or_else(|| err("missing reference materialization evidence"))?;
    if materialization.materialized_artifact_sha256.as_deref() != Some(&output.sha256)
        || !materialization.content.is_empty()
    {
        return fail("reference materialization evidence is inconsistent");
    }
    let checks = validation_checks(execution)?;
    let source = |reference: &crate::composition::MaterializedReference| {
        let pair = input
            .artifacts
            .iter()
            .find(|pair| pair.planned.logical_id() == reference.target_instance_id)
            .ok_or_else(|| {
                err(format!(
                    "missing reference source {}",
                    reference.target_instance_id
                ))
            })?;
        let PlannedArtifact::Dicom(artifact) = &pair.planned else {
            return fail("reference source is not DICOM");
        };
        let binding = artifact
            .case_binding
            .as_ref()
            .ok_or_else(|| err("reference source has no case binding"))?;
        let output = pair
            .execution
            .output
            .as_ref()
            .ok_or_else(|| err("reference source has no output evidence"))?;
        Ok((
            artifact,
            binding,
            output,
            json!({
                "source_case_id":binding.case_id,
                "source_path":output.relative_path,
                "series_instance_uid":uid(artifact, CompositionUidRole::SeriesInstance)?,
                "sop_class_uid":artifact.instance.sop_class_uid,
                "sop_instance_uid":uid(artifact, CompositionUidRole::SopInstance)?,
                "relationship":reference.role,
                "frame_numbers": if reference.referenced_frames.is_empty() { Value::Null } else { json!(reference.referenced_frames) }
            }),
        ))
    };
    let sources = planned
        .instance
        .references
        .iter()
        .map(source)
        .collect::<Result<Vec<_>, CuratedManifestError>>()?;
    let references = sources
        .iter()
        .map(|(_, _, _, value)| {
            let mut value = value.clone();
            if value["frame_numbers"].is_null() {
                value.as_object_mut().unwrap().remove("frame_numbers");
            }
            value
        })
        .collect::<Vec<_>>();
    let mut entry = json!({
        "case_id":ctx.registry_case.case_id,
        "profile_membership":ctx.registry_case.profiles,
        "path":output.relative_path,
        "sha256":output.sha256,
        "size_bytes":output.size_bytes,
        "determinism":ctx.registry_case.determinism,
        "recipe":{"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,"recipe_parameters":{}},
        "dicom":{"sop_class_uid":required(&ctx.registry_case.sop_class_uid,"SOP Class UID")?,
            "sop_class_name":required(&ctx.registry_case.sop_class_name,"SOP Class name")?,
            "iod_name":required(&ctx.registry_case.iod_name,"IOD name")?,"modality":required(&ctx.registry_case.modality,"modality")?,
            "transfer_syntax_uid":planned.encoding.transfer_syntax_uid,
            "transfer_syntax_name":transfer_syntax_name(&planned.encoding.transfer_syntax_uid)?},
        "uids":{"study_instance_uid":uid(planned,CompositionUidRole::StudyInstance)?,
            "series_instance_uid":uid(planned,CompositionUidRole::SeriesInstance)?,
            "sop_instance_uid":uid(planned,CompositionUidRole::SopInstance)?,
            "implementation_class_uid":planned.encoding.implementation.class_uid,
            "implementation_version_name":planned.encoding.implementation.version_name},
        "image":Value::Null,"pixel_data":Value::Null,"references":references,
        "validation":legacy_validation(&checks),"standards_evidence":deduplicate_reference_standards(&ctx.registry_case.standards_evidence)
    });
    if ctx.case_recipe.plan_provider_id == PRESENTATION_ADVANCED_PROVIDER_ID {
        project_presentation_reference_fields(ctx, planned, &sources, &mut entry)?;
    } else {
        project_registration_reference_fields(ctx, planned, &sources, &mut entry)?;
    }
    Ok(entry)
}

fn deduplicate_reference_standards(values: &[Value]) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .filter(|value| {
            let source = value.get("source").and_then(Value::as_str).unwrap_or("");
            let edition = value.get("edition").and_then(Value::as_str).unwrap_or("");
            let key = if let Some(query) = value.get("query").and_then(Value::as_str) {
                format!("query|{source}|{edition}|{query}")
            } else {
                format!(
                    "anchor|{source}|{edition}|{}|{}",
                    value.get("part").and_then(Value::as_str).unwrap_or(""),
                    value.get("anchor").and_then(Value::as_str).unwrap_or("")
                )
            };
            seen.insert(key)
        })
        .cloned()
        .collect()
}

fn project_presentation_reference_fields(
    ctx: &CuratedArtifactProjectionContext,
    _: &PlannedDicomArtifact,
    sources: &[(
        &PlannedDicomArtifact,
        &crate::corpus_plan::CaseBinding,
        &crate::executor::evidence::OutputEvidence,
        Value,
    )],
    entry: &mut Value,
) -> Result<(), CuratedManifestError> {
    const ICC: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";
    const PALETTE: &str = "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5";
    let parameters: PresentationProjectionParameters =
        serde_json::from_value(Value::Object(ctx.case_recipe.provider_parameters.clone()))
            .map_err(|error| err(format!("presentation projection parameters: {error}")))?;
    let _ = (parameters.uid_reference_index, parameters.sources.len());
    let source_case = &sources[0].1.case_id;
    entry["expected_capabilities"] = match parameters.presentation {
        PresentationKind::Grayscale(_) => json!([
            "open_file",
            "read_metadata",
            "show_unsupported_but_recognized",
            "apply_presentation_state"
        ]),
        PresentationKind::Color(_) => json!([
            "open_file",
            "read_metadata",
            "resolve_references",
            "apply_color_presentation_state",
            "apply_displayed_area",
            "color_manage_icc_profile"
        ]),
        PresentationKind::Blending(_) => json!([
            "open_file",
            "read_metadata",
            "resolve_references",
            "apply_blending_presentation_state",
            "render_palette_color_blend",
            "color_manage_icc_profile"
        ]),
        PresentationKind::AdvancedBlending(_) => json!([
            "open_file",
            "read_metadata",
            "resolve_references",
            "apply_advanced_blending_presentation_state",
            "render_true_color_blend",
            "color_manage_icc_profile"
        ]),
    };
    entry["known_stressors"] = match &parameters.presentation {
        PresentationKind::Grayscale(_) => json!([
            "grayscale_softcopy_presentation_state",
            "derived_source_reference",
            "softcopy_voi_window",
            "displayed_area"
        ]),
        PresentationKind::Color(_) => json!([
            "color_softcopy_presentation_state_storage",
            "same_study_reference",
            "distinct_presentation_series",
            "complete_instance_reference",
            "global_displayed_area",
            "one_based_display_coordinates",
            "mandatory_exact_icc_profile",
            "optional_rendering_modules_absent"
        ]),
        PresentationKind::Blending(_) => json!([
            "blending_softcopy_presentation_state_storage",
            "two_source_series",
            "four_complete_instance_references",
            "underlying_superimposed_positions",
            "relative_opacity",
            "per_item_rescale",
            "global_displayed_area",
            "mandatory_palette_color_lut",
            "mandatory_exact_icc_profile",
            "optional_modules_absent"
        ]),
        PresentationKind::AdvancedBlending(_) => json!([
            "advanced_blending_presentation_state_storage",
            "two_source_series",
            "four_complete_instance_references",
            "ordered_blending_graph",
            "single_geometry_source",
            "mandatory_exact_icc_profile",
            "common_instance_reference_closure",
            "optional_transformations_absent"
        ]),
    };
    match &parameters.presentation {
        PresentationKind::Grayscale(item) => {
            entry["references"][0]["relationship"] = json!("source_image");
            entry["recipe"]["recipe_parameters"] = json!({"source_case_id":source_case,"content_label":item.content_label,
                "content_description":item.content_description,"displayed_area_top_left":item.displayed_area.top_left,
                "displayed_area_bottom_right":item.displayed_area.bottom_right,"presentation_size_mode":item.displayed_area.size_mode,
                "presentation_pixel_aspect_ratio":item.displayed_area.pixel_aspect_ratio,"window_center":item.window_center,
                "window_width":item.window_width,"window_explanation":item.window_explanation,"presentation_lut_shape":item.presentation_lut_shape});
            entry["expected_semantics"] = json!({"synthetic_data":"YES","source_case_id":source_case,
                "source_sop_instance_uid":uid(sources[0].0,CompositionUidRole::SopInstance)?,"presentation_state":{
                    "displayed_area_top_left":item.displayed_area.top_left,"displayed_area_bottom_right":item.displayed_area.bottom_right,
                    "presentation_size_mode":item.displayed_area.size_mode,"presentation_pixel_aspect_ratio":item.displayed_area.pixel_aspect_ratio,
                    "window_center":item.window_center,"window_width":item.window_width,"presentation_lut_shape":item.presentation_lut_shape}});
            entry["expected_visual_checks"] = json!({"pattern":"source_ct_with_softcopy_window"});
        }
        PresentationKind::Color(item) => {
            entry["references"][0]["relationship"] = json!("source_image");
            let (src, binding, output, _) = &sources[0];
            let source_fact = json!({"source_case_id":binding.case_id,"source_path":output.relative_path,"source_sha256":output.sha256,
                "study_instance_uid":uid(src,CompositionUidRole::StudyInstance)?,"series_instance_uid":uid(src,CompositionUidRole::SeriesInstance)?,
                "sop_class_uid":src.instance.sop_class_uid,"sop_instance_uid":uid(src,CompositionUidRole::SopInstance)?,"rows":2,"columns":2,
                "photometric_interpretation":"RGB","samples_per_pixel":3,"planar_configuration":0,"complete_instance":true});
            let displayed = json!({"items":1,"applies_to_all_references":true,"top_left":item.displayed_area.top_left,
                "bottom_right":item.displayed_area.bottom_right,"presentation_size_mode":item.displayed_area.size_mode,
                "presentation_pixel_aspect_ratio":item.displayed_area.pixel_aspect_ratio,"presentation_pixel_spacing":Value::Null,
                "presentation_pixel_magnification_ratio":Value::Null});
            let icc = json!({"vr":"OB","size_bytes":736,"sha256":ICC,"device_class":"scnr","data_color_space":"RGB ",
                "profile_connection_space":"XYZ ","signature":"acsp","dicom_color_space":"SRGB"});
            entry["recipe"]["recipe_parameters"] = json!({"source_case_id":source_case,"complete_instance":true,
                "displayed_area_top_left":item.displayed_area.top_left,"displayed_area_bottom_right":item.displayed_area.bottom_right,
                "presentation_size_mode":item.displayed_area.size_mode,"presentation_pixel_aspect_ratio":item.displayed_area.pixel_aspect_ratio,
                "icc_profile_sha256":ICC});
            entry["expected_semantics"] = json!({"synthetic_data":"YES","same_study_as_source":true,"different_series_from_source":true,
                "complete_instance_reference":true,"global_displayed_area":true,"pixel_data_absent":true});
            entry["expected_color_softcopy_presentation_state"] = json!({"presentation_state":{"modality":"PR","body_part_examined":"HAND",
                "laterality":"R","content_label":item.content_label,"content_description":item.content_description,"presentation_creation_date":"20260101",
                "presentation_creation_time":"000000","instance_number":1,"series_number":62},"source":source_fact,"same_study":true,
                "different_series":true,"relationship":{"referenced_series_items":1,"referenced_image_items":1,"referenced_frame_numbers":[],
                "applies_to_complete_instance":true},"displayed_area":displayed,"icc_profile":icc,"shutter_items":0,"graphic_annotation_items":0,
                "graphic_layer_items":0,"overlay_items":0,"spatial_transform_present":false,"pixel_data_absent":true});
            entry["expected_visual_checks"] =
                json!({"pattern":"color_pr_displays_entire_2x2_rgb_source_with_srgb_profile"});
        }
        PresentationKind::Blending(item) => {
            for reference in entry["references"].as_array_mut().unwrap() {
                reference["relationship"] = json!("blending_source");
            }
            let source_manifest = presentation_source_manifest(sources)?;
            let study = uid(sources[0].0, CompositionUidRole::StudyInstance)?;
            let series = [
                uid(sources[0].0, CompositionUidRole::SeriesInstance)?,
                uid(sources[2].0, CompositionUidRole::SeriesInstance)?,
            ];
            let channel = |name| json!({"channel":name,"descriptor":[256,0,16],"data_vr":"OW","data_size_bytes":512,"data_sha256":PALETTE,"storage":"identity_u16_little_endian"});
            entry["recipe"]["recipe_parameters"] = json!({"source_case_id":source_case,"blending_positions":item.positions,"relative_opacity":item.relative_opacity,
                "palette_channel_sha256":PALETTE,"icc_profile_sha256":ICC});
            entry["expected_semantics"] = json!({"synthetic_data":"YES","same_study_as_sources":true,"shared_source_frame_of_reference":true,
                "underlying_superimposed_blend":true,"pixel_data_absent":true});
            entry["expected_blending_presentation_state"] = json!({"presentation_state":{"study_instance_uid":study,
                "series_instance_uid":entry["uids"]["series_instance_uid"],"sop_instance_uid":entry["uids"]["sop_instance_uid"],"modality":"PR","laterality":"R",
                "content_label":item.content_label,"content_description":item.content_description,"content_creator_name":"DTS^Generator",
                "presentation_creation_date":"20260101","presentation_creation_time":"000000","instance_number":1,"series_number":81},
                "sources":source_manifest,"same_study":true,"shared_frame_of_reference":true,"different_series":true,
                "blending_items":[{"blending_position":item.positions[0],"source_series_order":1,"study_instance_uid":study,"series_instance_uid":series[0],
                    "referenced_source_indices":[1,2],"referenced_frame_numbers":[],"rescale_intercept":-1024,"rescale_slope":1,"rescale_type":item.rescale_type,
                    "softcopy_voi_lut_items":0,"referenced_spatial_registration_items":0,"complete_instances":true},
                    {"blending_position":item.positions[1],"source_series_order":2,"study_instance_uid":study,"series_instance_uid":series[1],
                    "referenced_source_indices":[3,4],"referenced_frame_numbers":[],"rescale_intercept":-1024,"rescale_slope":1,"rescale_type":item.rescale_type,
                    "softcopy_voi_lut_items":0,"referenced_spatial_registration_items":0,"complete_instances":true}],
                "relative_opacity":item.relative_opacity,"displayed_area":{"items":1,"applies_to_all_references":true,"referenced_image_items":0,
                    "top_left":item.displayed_area.top_left,"bottom_right":item.displayed_area.bottom_right,"presentation_size_mode":item.displayed_area.size_mode,
                    "presentation_pixel_aspect_ratio":item.displayed_area.pixel_aspect_ratio,"presentation_pixel_spacing":Value::Null,"presentation_pixel_magnification_ratio":Value::Null},
                "palette_color_lut":{"channels":[channel("red"),channel("green"),channel("blue")],"segmented_data_present":false,"palette_uid_present":false},
                "icc_profile":{"vr":"OB","size_bytes":736,"sha256":ICC,"device_class":"scnr","data_color_space":"RGB ","profile_connection_space":"XYZ ","signature":"acsp","dicom_color_space":"SRGB"},
                "absent_modules":blending_absent_modules(),"pixel_data_absent":true});
            entry["expected_visual_checks"] = json!({"pattern":"equal_opacity_identity_palette_blend_of_two_registered_ct_series"});
        }
        PresentationKind::AdvancedBlending(item) => {
            for reference in entry["references"].as_array_mut().unwrap() {
                reference["relationship"] = json!("blending_input");
            }
            let source_manifest = presentation_source_manifest(sources)?;
            let study = uid(sources[0].0, CompositionUidRole::StudyInstance)?;
            let series = [
                uid(sources[0].0, CompositionUidRole::SeriesInstance)?,
                uid(sources[2].0, CompositionUidRole::SeriesInstance)?,
            ];
            let frame = uid(sources[0].0, CompositionUidRole::FrameOfReference)?;
            entry["uids"]["frame_of_reference_uid"] = json!(frame);
            entry["recipe"]["recipe_parameters"] = json!({"source_case_id":source_case,"blending_input_numbers":item.input_numbers,
                "display_input_numbers":item.input_numbers,"blending_mode":item.blending_mode,"icc_profile_sha256":ICC});
            entry["expected_capabilities"] = json!([
                "open_file",
                "read_metadata",
                "resolve_references",
                "apply_advanced_blending_presentation_state",
                "render_true_color_blend",
                "color_manage_icc_profile"
            ]);
            entry["expected_semantics"] = json!({"synthetic_data":"YES","same_study_as_sources":true,"shared_frame_of_reference":true,"two_input_equal_blend":true,"pixel_data_absent":true});
            entry["expected_advanced_blending_presentation_state"] = json!({"presentation_state":{"study_instance_uid":study,"series_instance_uid":entry["uids"]["series_instance_uid"],
                "sop_instance_uid":entry["uids"]["sop_instance_uid"],"frame_of_reference_uid":frame,"position_reference_indicator":"","modality":"PR","laterality":"R",
                "content_label":item.content_label,"content_description":item.content_description,"content_creator_name":"DTS^Generator","presentation_creation_date":"20260101",
                "presentation_creation_time":"000000","instance_number":1,"series_number":80},"sources":source_manifest,"same_study":true,"shared_frame_of_reference":true,
                "different_series":true,"blending_inputs":[{"input_number":item.input_numbers[0],"source_series_order":1,"study_instance_uid":study,
                    "series_instance_uid":series[0],"referenced_source_indices":[1,2],"time_series_blending":"FALSE","geometry_for_display":"TRUE","complete_instances":true},
                    {"input_number":item.input_numbers[1],"source_series_order":2,"study_instance_uid":study,"series_instance_uid":series[1],"referenced_source_indices":[3,4],
                    "time_series_blending":"FALSE","geometry_for_display":"FALSE","complete_instances":true}],"pixel_presentation":item.pixel_presentation,
                "display_operation":{"items":1,"input_numbers":item.input_numbers,"blending_mode":item.blending_mode,"relative_opacity":Value::Null,
                    "output_blending_input_number":Value::Null,"final_output":true},"icc_profile":{"vr":"OB","size_bytes":736,"sha256":ICC,"device_class":"scnr",
                    "data_color_space":"RGB ","profile_connection_space":"XYZ ","signature":"acsp","dicom_color_space":"SRGB"},
                "common_instance_reference":{"series":[{"series_order":1,"series_instance_uid":series[0],"referenced_source_indices":[1,2]},
                    {"series_order":2,"series_instance_uid":series[1],"referenced_source_indices":[3,4]}],"other_study_items":0,"mirrors_blending_inputs":true},
                "optional_transforms":{"referenced_spatial_registration_items":0,"optical_path_selection_items":0,"softcopy_voi_lut_items":0,"palette_color_lut_items":0,
                    "threshold_items":0,"displayed_area_items":0,"graphic_annotation_items":0,"graphic_group_items":0,"specimen_items":0,"spatial_transform_present":false,
                    "graphic_layer_items":0},"pixel_data_absent":true});
            entry["expected_visual_checks"] =
                json!({"pattern":"equal_true_color_blend_of_two_registered_ct_series"});
        }
    }
    Ok(())
}

fn presentation_source_manifest(
    sources: &[(
        &PlannedDicomArtifact,
        &crate::corpus_plan::CaseBinding,
        &crate::executor::evidence::OutputEvidence,
        Value,
    )],
) -> Result<Vec<Value>, CuratedManifestError> {
    sources.iter().enumerate().map(|(index,(source,binding,output,_))| {
        let image_order=index%2+1;
        Ok(json!({"source_case_id":binding.case_id,"source_path":output.relative_path,"source_sha256":output.sha256,
            "study_instance_uid":uid(source,CompositionUidRole::StudyInstance)?,"series_instance_uid":uid(source,CompositionUidRole::SeriesInstance)?,
            "frame_of_reference_uid":uid(source,CompositionUidRole::FrameOfReference)?,"sop_class_uid":source.instance.sop_class_uid,
            "sop_instance_uid":uid(source,CompositionUidRole::SopInstance)?,"series_order":index/2+1,"image_order":image_order,"rows":2,"columns":2,
            "image_orientation_patient":[1,0,0,0,1,0],"image_position_patient_mm":[0,0,if image_order==1{0}else{5}],"referenced_frame_numbers":[],"complete_instance":true}))
    }).collect()
}

fn blending_absent_modules() -> Value {
    json!({"clinical_trial_subject":true,"clinical_trial_study":true,"clinical_trial_series":true,"clinical_trial_equipment":true,
        "patient_study":true,"specimen":true,"graphic_annotation":true,"graphic_layer":true,"graphic_group":true,"spatial_transformation":true,
        "frame_of_reference":true,"common_instance_reference":true,"softcopy_presentation_lut":true,"voi_lut":true,"softcopy_voi_lut":true,
        "overlay_plane":true,"overlay_activation":true,"display_shutter":true,"bitmap_display_shutter":true})
}

fn project_registration_reference_fields(
    ctx: &CuratedArtifactProjectionContext,
    planned: &PlannedDicomArtifact,
    sources: &[(
        &PlannedDicomArtifact,
        &crate::corpus_plan::CaseBinding,
        &crate::executor::evidence::OutputEvidence,
        Value,
    )],
    entry: &mut Value,
) -> Result<(), CuratedManifestError> {
    let params: RegistrationProjectionParameters =
        serde_json::from_value(Value::Object(ctx.case_recipe.provider_parameters.clone()))
            .map_err(|error| err(format!("registration projection parameters: {error}")))?;
    let _ = (
        &params.series_number,
        &params.study_id,
        &params.laterality,
        &params.manufacturer_model_name,
        &params.device_serial_number,
        &params.content_label,
        &params.content_description,
        params.sources.len(),
    );
    let identities = sources.iter().map(|(source,binding,output,_)| Ok(json!({
        "source_case_id":binding.case_id,"source_path":output.relative_path,"source_sha256":output.sha256,
        "study_instance_uid":uid(source,CompositionUidRole::StudyInstance)?,"series_instance_uid":uid(source,CompositionUidRole::SeriesInstance)?,
        "sop_class_uid":source.instance.sop_class_uid,"sop_instance_uid":uid(source,CompositionUidRole::SopInstance)?,
        "frame_of_reference_uid":uid(source,CompositionUidRole::FrameOfReference)?
    }))).collect::<Result<Vec<Value>,CuratedManifestError>>()?;
    let target_for = uid(sources[0].0, CompositionUidRole::FrameOfReference)?;
    entry["uids"]["frame_of_reference_uid"] = json!(target_for);
    match &params.registration {
        RegistrationKindInput::Spatial(spatial) => {
            entry["references"][0]["relationship"] = json!("registered_target");
            entry["references"][1]["relationship"] = json!("moving_source");
            entry["known_stressors"] = json!([
                "spatial_registration_storage",
                "two_frames_of_reference",
                "identity_and_nonidentity_rigid_matrices",
                "matrix_directionality",
                "cross_study_references",
                "landmark_transform"
            ]);
            let target_matrix = spatial
                .fixed_matrix
                .iter()
                .map(|v| v.parse::<f64>().map_err(|e| err(e.to_string())))
                .collect::<Result<Vec<_>, _>>()?;
            let source_matrix = spatial
                .moving_matrix
                .iter()
                .map(|v| v.parse::<f64>().map_err(|e| err(e.to_string())))
                .collect::<Result<Vec<_>, _>>()?;
            entry["recipe"]["recipe_parameters"] = json!({"matrix_direction":"source_to_registered","target_identity_matrix":target_matrix,
                "source_to_registered_matrix":source_matrix,"landmark_source_mm":[-0.625,-0.625,0.0],"landmark_registered_mm":[0.0,0.0,2.5]});
            entry["expected_capabilities"] = json!([
                "open_file",
                "read_metadata",
                "resolve_references",
                "read_spatial_registration",
                "apply_rigid_transform",
                "fuse_registered_images"
            ]);
            entry["expected_semantics"] = json!({"synthetic_data":"YES","registered_frame_of_reference_uid":target_for,"matrix_direction":"source_to_registered","pixel_data_absent":true});
            entry["expected_spatial_registration"] = json!({"registered_frame_of_reference_uid":target_for,"matrix_direction":"source_to_registered",
                "registration_items":[{"role":"registered_target","source":identities[0],"complete_instance":true,"matrix_registration_items":1,
                    "registration_type_code_items":0,"matrix_items":1,"matrix":{"type":"RIGID","values":target_matrix}},
                    {"role":"moving_source","source":identities[1],"complete_instance":true,"matrix_registration_items":1,"registration_type_code_items":0,
                    "matrix_items":1,"matrix":{"type":"RIGID","values":source_matrix}}],"rigid_tolerances":{"orthonormal_abs":0.000001,
                    "determinant_abs":0.000001,"homogeneous_abs":0.000001},"landmark":{"source_point_mm":[-0.625,-0.625,0.0],
                    "registered_point_mm":[0.0,0.0,2.5],"tolerance_mm":0.000001},"common_instance_reference":{"same_study":identities[0],
                    "other_studies":[identities[1]]},"pixel_data_absent":true});
            entry["expected_visual_checks"] =
                json!({"pattern":"moving_ct_origin_maps_to_enhanced_ct_frame_2_origin"});
        }
        RegistrationKindInput::Deformable(deformable) => {
            entry["references"][0]["relationship"] = json!("registered_target");
            entry["references"][1]["relationship"] = json!("deformation_source");
            entry["known_stressors"] = json!([
                "deformable_spatial_registration_storage",
                "two_frames_of_reference",
                "identity_pre_and_post_matrices",
                "registered_to_source_sampling",
                "nonuniform_vector_grid",
                "of_little_endian_binary32",
                "i_fastest_vector_order",
                "cross_study_references"
            ]);
            let matrix = deformable
                .pre_deformation_matrix
                .iter()
                .map(|v| v.parse::<f64>().map_err(|e| err(e.to_string())))
                .collect::<Result<Vec<_>, _>>()?;
            let vectors = deformable
                .vector_grid_data
                .chunks_exact(3)
                .map(|v| vec![v[0], v[1], v[2]])
                .collect::<Vec<_>>();
            let registered = [
                [0.0, 0.0, 2.5],
                [0.75, 0.0, 2.5],
                [0.0, 0.75, 2.5],
                [0.75, 0.75, 2.5],
            ];
            let source = [
                [-0.625, -0.625, 0.0],
                [0.0, -0.625, 0.0],
                [-0.625, 0.0, 0.0],
                [0.0, 0.0, 0.0],
            ];
            let mappings=registered.iter().zip(source.iter()).map(|(r,s)|json!({"registered_point_mm":r,"source_point_mm":s,"tolerance_mm":0.000001})).collect::<Vec<_>>();
            entry["recipe"]["recipe_parameters"] = json!({"sampling_direction":"registered_to_source","pre_deformation_matrix":matrix,
                "post_deformation_matrix":matrix,"grid_dimensions":deformable.grid_dimensions,"grid_resolution_mm":deformable.grid_resolution,
                "vector_grid_data_sha256":"d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47","vector_index_order":"i_fastest_then_j_then_k"});
            entry["expected_capabilities"] = json!([
                "open_file",
                "read_metadata",
                "resolve_references",
                "read_deformable_spatial_registration",
                "apply_deformation_field",
                "resample_registered_image"
            ]);
            entry["expected_semantics"] = json!({"synthetic_data":"YES","registered_frame_of_reference_uid":target_for,"sampling_direction":"registered_to_source","pixel_data_absent":true});
            entry["expected_deformable_spatial_registration"] = json!({"registered_frame_of_reference_uid":target_for,"sampling_direction":"registered_to_source",
                "source":identities[1],"complete_instance":true,"deformable_registration_items":1,"registration_type_code_items":0,
                "pre_deformation_matrix":{"items":1,"type":"RIGID","values":matrix},"post_deformation_matrix":{"items":1,"type":"RIGID","values":matrix},
                "grid":{"items":1,"image_position_patient_mm":[0.0,0.0,2.5],"image_orientation_patient":[1.0,0.0,0.0,0.0,1.0,0.0],
                    "dimensions":deformable.grid_dimensions,"resolution_mm":deformable.grid_resolution,"vector_data_vr":"OF","vector_data_vm":1,
                    "vector_count":vectors.len(),"component_count":deformable.vector_grid_data.len(),"byte_length":deformable.vector_grid_data.len()*4,
                    "payload_sha256":"d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47","byte_order":"little_endian_ieee754_binary32",
                    "index_order":"i_fastest_then_j_then_k","vectors_mm":vectors},"point_mappings":mappings,
                "common_instance_reference":{"same_study":identities[0],"other_studies":[identities[1]]},"pixel_data_absent":true});
            entry["expected_visual_checks"] =
                json!({"pattern":"enhanced_ct_frame_2_grid_maps_to_classic_ct_pixel_centers"});
        }
    }
    let _ = planned;
    Ok(())
}

struct AdvancedManifestCommon<'a> {
    output: &'a crate::executor::evidence::OutputEvidence,
    pixels: &'a MaterializedContentEvidence,
    frame_hashes: &'a [String],
    rows: u64,
    columns: u64,
    frames: u64,
    samples_per_pixel: u64,
    photometric: &'a str,
    bits_allocated: u64,
    bits_stored: u64,
    high_bit: u64,
    pixel_representation: u64,
    planar_configuration: Option<u64>,
    study: &'a str,
    series: &'a str,
    sop: &'a str,
    frame_of_reference: Option<&'a str>,
    dimension: Option<&'a str>,
    validation: Value,
}

fn advanced_base(
    ctx: &CuratedArtifactProjectionContext,
    planned: &PlannedDicomArtifact,
    common: &AdvancedManifestCommon<'_>,
) -> Result<Value, CuratedManifestError> {
    Ok(json!({
        "case_id":ctx.registry_case.case_id,
        "profile_membership":ctx.artifact_recipe.public_profile_membership.as_ref().unwrap_or(&ctx.registry_case.profiles),
        "path":common.output.relative_path,"sha256":common.output.sha256,"size_bytes":common.output.size_bytes,
        "determinism":ctx.registry_case.determinism,
        "dicom":{"sop_class_uid":required(&ctx.registry_case.sop_class_uid,"registry SOP Class UID")?,
            "sop_class_name":required(&ctx.registry_case.sop_class_name,"registry SOP Class name")?,
            "iod_name":required(&ctx.registry_case.iod_name,"registry IOD name")?,
            "modality":required(&ctx.registry_case.modality,"registry modality")?,
            "transfer_syntax_uid":planned.encoding.transfer_syntax_uid,
            "transfer_syntax_name":transfer_syntax_name(&planned.encoding.transfer_syntax_uid)?},
        "image":{"rows":common.rows,"columns":common.columns,"frames":common.frames,
            "samples_per_pixel":common.samples_per_pixel,"photometric_interpretation":common.photometric,
            "bits_allocated":common.bits_allocated,"bits_stored":common.bits_stored,"high_bit":common.high_bit,
            "pixel_representation":common.pixel_representation,"planar_configuration":common.planar_configuration},
        "pixel_data":{"vr":common.pixels.vr,"native_or_encapsulated":"native",
            "value_length":common.pixels.native_value_field_size_bytes.unwrap_or(common.pixels.size_bytes),
            "frame_count":common.frames,"frame_hashes":common.frame_hashes},
        "validation":common.validation,
        "standards_evidence":ctx.registry_case.standards_evidence,
    }))
}

fn project_enhanced_manifest(
    ctx: &CuratedArtifactProjectionContext,
    planned: &PlannedDicomArtifact,
    common: &AdvancedManifestCommon<'_>,
) -> Result<Value, CuratedManifestError> {
    let provider = advanced_provider_parameters(ctx).map_err(|error| err(error.to_string()))?;
    let artifact = advanced_artifact_parameters(ctx).map_err(|error| err(error.to_string()))?;
    let frames = artifact.frames.expand().map_err(CuratedManifestError)?;
    let positions = frames
        .iter()
        .map(|frame| frame.image_position_patient.as_str())
        .collect::<Vec<_>>();
    let dimensions = frames
        .iter()
        .map(|frame| frame.dimension_index_value)
        .collect::<Vec<_>>();
    let pixel_count = usize::try_from(common.rows)
        .ok()
        .and_then(|rows| {
            usize::try_from(common.columns)
                .ok()
                .map(|columns| rows * columns)
        })
        .and_then(|pixels| {
            usize::try_from(common.frames)
                .ok()
                .map(|frames| pixels * frames)
        })
        .ok_or_else(|| err("advanced pixel count overflow"))?;
    let (stored_values, pixel_min, pixel_max) = artifact
        .pixels
        .values(pixel_count)
        .map_err(CuratedManifestError)?;
    let mut manifest = advanced_base(ctx, planned, common)?;
    manifest["references"] = json!([]);
    match provider {
        AdvancedCompatibilityProvider::Ct {
            common: parameters,
            pixel_spacing,
            image_orientation_patient,
            slice_thickness,
            spacing_between_slices,
            rescale_intercept,
            rescale_slope,
            rescale_type,
            concatenation,
            stress: _,
        } => {
            let (case_pixel_min, case_pixel_max) = enhanced_case_pixel_range(ctx)?;
            let dimension = common
                .dimension
                .ok_or_else(|| err("missing CT dimension UID"))?;
            let irradiation = planned
                .instance
                .identities
                .get(&CompositionUidRole::IrradiationEvent, 0)
                .ok_or_else(|| err("missing CT irradiation UID"))?;
            let concatenation_value = if concatenation {
                json!({
                    "concatenation_uid":planned_string(planned,"0020,9161")?,
                    "in_concatenation_number":artifact.in_concatenation_number,
                    "in_concatenation_total_number":ctx.case_recipe.dicom.as_ref().map_or(0,|dicom|dicom.artifacts.len()),
                    "concatenation_frame_offset_number":artifact.concatenation_frame_offset_number,
                    "sop_instance_uid_of_concatenation_source":planned_string(planned,"0020,0242")?
                })
            } else {
                Value::Null
            };
            manifest["recipe"] = json!({"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,
                "recipe_parameters":{"rows":common.rows,"columns":common.columns,"frames":common.frames,
                    "samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2","bits_allocated":16,
                    "bits_stored":16,"high_bit":15,"pixel_representation":0,"pixel_values":stored_values,
                    "frame_type":parameters.frame_type,"dimension_index":{"dimension_organization_uid":dimension,
                        "dimension_index_pointer":"ImagePositionPatient","functional_group_pointer":"PlanePositionSequence"},
                    "shared_functional_groups":{"pixel_measures":{"pixel_spacing":pixel_spacing,
                        "slice_thickness":slice_thickness,"spacing_between_slices":spacing_between_slices},
                        "plane_orientation_patient":image_orientation_patient,"frame_anatomy":{"frame_laterality":"U",
                            "anatomic_region_code_value":"T-D3000"},"irradiation_event_uid":irradiation,
                        "ct_pixel_value_transformation":{"intercept":rescale_intercept,"slope":rescale_slope,"type":rescale_type}},
                    "per_frame_functional_groups":{"image_position_patient":positions},"concatenation":concatenation_value}});
            manifest["uids"] = json!({"study_instance_uid":common.study,"series_instance_uid":common.series,
                "sop_instance_uid":common.sop,"frame_of_reference_uid":common.frame_of_reference,
                "dimension_organization_uid":dimension,"irradiation_event_uid":irradiation,
                "implementation_class_uid":planned.encoding.implementation.class_uid,
                "implementation_version_name":"DICOMTS010"});
            manifest["expected_capabilities"] = json!([
                "open_file",
                "read_metadata",
                "render_native_pixels",
                "parse_multiframe_functional_groups"
            ]);
            manifest["expected_semantics"] = json!({"synthetic_data":"YES","pixel_min":case_pixel_min,"pixel_max":case_pixel_max,
                "shared_functional_groups_sequence_items":1,"per_frame_functional_groups_sequence_items":common.frames,
                "dimension_index_values":dimensions,"concatenation":concatenation_value});
            manifest["expected_visual_checks"] = json!({"pattern":if concatenation {
                "single_member_enhanced_ct_concatenation_gradient"} else {"two_frame_enhanced_ct_unsigned_gradient_stack"}});
            let mut stressors = vec![
                "enhanced_ct_image_storage",
                "native_multiframe_pixel_data",
                "shared_functional_groups_sequence",
                "per_frame_functional_groups_sequence",
                "multi_frame_dimension",
            ];
            if concatenation {
                stressors.push("concatenation");
            }
            manifest["known_stressors"] = json!(stressors);
        }
        AdvancedCompatibilityProvider::Mr {
            common: parameters,
            pixel_spacing,
            image_orientation_patient,
            slice_thickness,
            spacing_between_slices,
            rescale_intercept,
            rescale_slope,
            rescale_type,
            repetition_time,
            flip_angle,
            echo_train_length,
            rf_echo_train_length,
            gradient_echo_train_length,
            axis,
        } => {
            let dimension = common
                .dimension
                .ok_or_else(|| err("missing MR dimension UID"))?;
            let operating_modes = json!([{"type":"STATIC FIELD","mode":"IEC_NORMAL"},{"type":"RF","mode":"IEC_NORMAL"},{"type":"GRADIENT","mode":"IEC_NORMAL"}]);
            let (pointer, group, name, values, pattern, stressor) = match &axis {
                EnhancedMrFrameAxis::EffectiveEchoTime { values } => (
                    "EffectiveEchoTime",
                    "MREchoSequence",
                    "effective_echo_time",
                    json!(values),
                    "two_frame_enhanced_mr_echo_gradient_stack",
                    "per_frame_mr_echo",
                ),
                EnhancedMrFrameAxis::TemporalPositionTimeOffset { values } => (
                    "TemporalPositionTimeOffset",
                    "TemporalPositionSequence",
                    "temporal_position_time_offset",
                    json!(values),
                    "two_frame_enhanced_mr_temporal_gradient_stack",
                    "per_frame_temporal_position",
                ),
                EnhancedMrFrameAxis::VelocityEncoding { directions, .. } => (
                    "VelocityEncodingDirection",
                    "MRVelocityEncodingSequence",
                    "velocity_encoding_direction",
                    json!(directions),
                    "two_frame_enhanced_mr_phase_velocity_encoding_stack",
                    "per_frame_mr_velocity_encoding",
                ),
            };
            let mut per_frame = json!({"image_position_patient":positions});
            per_frame[name] = values.clone();
            let mut semantics = json!({"synthetic_data":"YES","patient_position":"","content_qualification":"RESEARCH",
                "applicable_safety_standard_agency":"IEC","complex_image_component":"MAGNITUDE","acquisition_contrast":"UNKNOWN",
                "burned_in_annotation":"NO","lossy_image_compression":"00","presentation_lut_shape":"IDENTITY",
                "pixel_min":pixel_min,"pixel_max":pixel_max,"shared_functional_groups_sequence_items":1,
                "per_frame_functional_groups_sequence_items":common.frames,"dimension_index_values":[1,2]});
            semantics[name] = values.clone();
            if let EnhancedMrFrameAxis::TemporalPositionTimeOffset { .. } = &axis {
                per_frame["temporal_position_index"] = json!([1, 2]);
                per_frame["dimension_index_values"] = json!([1, 2]);
                per_frame["frame_acquisition_number"] = json!([1, 2]);
                semantics["temporal_position_indices"] = json!([1, 2]);
                semantics["frame_acquisition_numbers"] = json!([1, 2]);
                semantics["temporal_position_time_offset_unit"] = json!("seconds");
            }
            if let EnhancedMrFrameAxis::VelocityEncoding {
                minimum, maximum, ..
            } = &axis
            {
                per_frame["velocity_encoding_minimum_value"] = json!(minimum);
                per_frame["velocity_encoding_maximum_value"] = json!(maximum);
                semantics["velocity_encoding_minimum_value"] = json!(minimum);
                semantics["velocity_encoding_maximum_value"] = json!(maximum);
            }
            manifest["recipe"] = json!({"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,
                "recipe_parameters":{"rows":common.rows,"columns":common.columns,"frames":common.frames,"samples_per_pixel":1,
                "photometric_interpretation":"MONOCHROME2","bits_allocated":16,"bits_stored":16,"high_bit":15,
                "pixel_representation":0,"pixel_values":stored_values,"frame_type":parameters.frame_type,"presentation_lut_shape":"IDENTITY",
                "dimension_index":{"dimension_organization_uid":dimension,"dimension_index_pointer":pointer,"functional_group_pointer":group},
                "shared_functional_groups":{"pixel_measures":{"pixel_spacing":pixel_spacing,"slice_thickness":slice_thickness,
                    "spacing_between_slices":spacing_between_slices},"plane_orientation_patient":image_orientation_patient,
                    "frame_anatomy":{"frame_laterality":"U","anatomic_region_code_value":"69536005","anatomic_region_coding_scheme":"SCT","anatomic_region_code_meaning":"Head"},
                    "mr_image_frame_type":{"complex_image_component":"MAGNITUDE","acquisition_contrast":"UNKNOWN"},
                    "mr_timing":{"repetition_time":repetition_time,"flip_angle":flip_angle,"echo_train_length":echo_train_length,
                        "rf_echo_train_length":rf_echo_train_length,"gradient_echo_train_length":gradient_echo_train_length,
                        "specific_absorption_rate":{"definition":"IEC_HEAD","value":0.1},"operating_modes":operating_modes},
                    "pixel_value_transformation":{"intercept":rescale_intercept,"slope":rescale_slope,"type":rescale_type}},
                "per_frame_functional_groups":per_frame}});
            manifest["uids"] = json!({"study_instance_uid":common.study,"series_instance_uid":common.series,"sop_instance_uid":common.sop,
                "frame_of_reference_uid":common.frame_of_reference,"dimension_organization_uid":dimension,
                "implementation_class_uid":planned.encoding.implementation.class_uid,
                "implementation_version_name":"DICOMTS010"});
            manifest["expected_capabilities"] = json!([
                "open_file",
                "read_metadata",
                "render_native_pixels",
                "parse_multiframe_functional_groups"
            ]);
            manifest["expected_semantics"] = semantics;
            manifest["expected_visual_checks"] = json!({"pattern":pattern});
            manifest["known_stressors"] = json!([
                "enhanced_mr_image_storage",
                "native_multiframe_pixel_data",
                "shared_functional_groups_sequence",
                "per_frame_functional_groups_sequence",
                stressor,
                "multi_frame_dimension"
            ]);
        }
        AdvancedCompatibilityProvider::Pet {
            common: parameters,
            pixel_spacing: _,
            image_orientation_patient: _,
            slice_thickness: _,
            spacing_between_slices: _,
            rescale_intercept: _,
            rescale_slope,
            units: _,
            counts_source,
            stack_id,
        } => {
            let dimension = common
                .dimension
                .ok_or_else(|| err("missing PET dimension UID"))?;
            let activity = stored_values
                .iter()
                .map(|value| *value as f64 * rescale_slope.parse::<f64>().unwrap_or(0.0))
                .collect::<Vec<_>>();
            let expected = enhanced_pet_contract(
                &parameters,
                &frames,
                &dimensions,
                &artifact,
                &stored_values,
                &activity,
                &counts_source,
                &stack_id,
                common.frame_hashes,
                common.pixels,
            );
            manifest["recipe"] = json!({"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,
                "recipe_parameters":{"rows":common.rows,"columns":common.columns,"frames":common.frames,"pixel_values":stored_values,
                    "image_positions":positions,"dimension_index_values":dimensions,"enhanced_pet":expected}});
            manifest["uids"] = json!({"study_instance_uid":common.study,"series_instance_uid":common.series,"sop_instance_uid":common.sop,
                "frame_of_reference_uid":common.frame_of_reference,"dimension_organization_uid":dimension,
                "implementation_class_uid":planned.encoding.implementation.class_uid,
                "implementation_version_name":"DICOMTS010"});
            manifest["expected_capabilities"] = json!([
                "open_file",
                "render_native_pixels",
                "apply_real_world_value_mapping"
            ]);
            manifest["expected_semantics"] = json!({"synthetic_data":"YES","pixel_min":0,"pixel_max":400,
                "shared_functional_groups_item_count":1,"per_frame_functional_groups_item_count":common.frames,
                "dimension_index_values":dimensions,"temporal_position_indices":artifact.temporal_position_indices,
                "quantitative_mapping":"synthetic_bqml_not_suv_or_clinically_calibrated"});
            manifest["expected_enhanced_pet"] = expected;
            manifest["expected_visual_checks"] =
                json!({"pattern":"two_identical_pet_activity_frames_at_distinct_z_positions"});
            manifest["known_stressors"] = json!([
                "enhanced_pet_image_storage",
                "shared_per_frame_functional_groups",
                "native_multiframe_u16",
                "bqml_rwvm"
            ]);
        }
    }
    Ok(manifest)
}

fn enhanced_pet_contract(
    common: &crate::curated_execution::AdvancedCompatibilityCommon,
    frames: &[crate::recipes::EnhancedFrameGeometry],
    dimensions: &[u32],
    artifact: &crate::curated_execution::AdvancedCompatibilityArtifact,
    stored_values: &[i64],
    activity: &[f64],
    counts_source: &str,
    stack_id: &str,
    frame_hashes: &[String],
    pixels: &MaterializedContentEvidence,
) -> Value {
    let identity = json!({
        "image_type":common.image_type.split('\\').collect::<Vec<_>>(),
        "frame_type":common.frame_type.split('\\').collect::<Vec<_>>(),
        "pixel_presentation":"MONOCHROME","volumetric_properties":"VOLUME",
        "volume_based_calculation_technique":"NONE","content_qualification":"RESEARCH",
        "burned_in_annotation":"NO","lossy_image_compression":"00","presentation_lut_shape":"IDENTITY",
        "frame_count":frames.len()
    });
    let dimensions = json!({"shared_functional_groups_item_count":1,
        "per_frame_functional_groups_item_count":frames.len(),"dimension_organization_item_count":1,
        "dimension_index_item_count":1,"dimension_index_pointer":"0020,9057","functional_group_pointer":"0020,9111",
        "stack_ids":[stack_id,stack_id],"in_stack_position_numbers":artifact.in_stack_position_numbers,
        "dimension_index_values":dimensions,"temporal_position_indices":artifact.temporal_position_indices
    });
    let geometry = json!({
        "image_positions_patient_mm":[[0.0,0.0,0.0],[0.0,0.0,5.0]],"pixel_spacing_mm":[2.0,2.0],
        "slice_thickness_mm":5.0,"spacing_between_slices_mm":5.0,
        "image_orientation_patient":[1.0,0.0,0.0,0.0,1.0,0.0],"frame_laterality":"U",
        "anatomic_region":{"code_value":"69536005","coding_scheme_designator":"SCT","code_meaning":"Head"}
    });
    let quantitative = json!({
        "rescale_intercept":0.0,"rescale_slope":2.5,"rescale_type":"US","window_center":500.0,"window_width":1000.0,
        "real_world_value_mapping":{"first_value_mapped":0,"last_value_mapped":400,"intercept":0.0,"slope":2.5,
            "lut_label":"BQML","lut_explanation":"Activity concentration","measurement_units":{"code_value":"Bq/ml",
                "coding_scheme_designator":"UCUM","code_meaning":"Becquerels/milliliter"}},
        "radiopharmaceutical_information":{"item_count":1,"agent_number":1,
            "radionuclide":{"code_value":"77004003","coding_scheme_designator":"SCT","code_meaning":"^18^Fluorine"},
            "administration_route":{"code_value":"47625008","coding_scheme_designator":"SCT","code_meaning":"Intravenous route"},
            "start_datetime":"20260101000000","total_dose_present_empty":true,"half_life_seconds":6586.2,"positron_fraction":0.967,
            "radiopharmaceutical":{"code_value":"35321007","coding_scheme_designator":"SCT","code_meaning":"Fluorodeoxyglucose F^18^"}},
        "radiopharmaceutical_usage_agent_number":1
    });
    let acquisition = json!({"table_motion":"STATIC","time_of_flight_information_used":"FALSE",
        "view_code":{"code_value":"24422004","coding_scheme_designator":"SCT","code_meaning":"Axial"},
        "view_modifier_item_count":0,"slice_progression_direction_present":false,"counts_source":counts_source,
        "corrections":{"decay":"NO","attenuation":"NO","scatter":"NO","dead_time":"NO","gantry_motion":"NO",
            "patient_motion":"NO","count_loss_normalization":"NO","randoms":"NO","non_uniform_radial_sampling":"NO",
            "sensitivity_calibration":"NO","detector_normalization":"NO"},
        "derivation_image_item_count":0,"acquisition_context_item_count":0,
        "stored_values_by_frame":[stored_values.get(0..4).unwrap_or(stored_values),stored_values.get(4..8).unwrap_or(stored_values)],
        "activity_values_bqml_by_frame":[activity.get(0..4).unwrap_or(activity),activity.get(4..8).unwrap_or(activity)],
        "frame_sha256":frame_hashes,"pixel_data_sha256":pixels.sha256,
        "nonclaims":{"suv":false,"body_weight_normalization":false,"body_surface_area_normalization":false,
            "decay_corrected":false,"clinically_calibrated":false,"acquisition_counts":false,"actual_clinical_dose":false,
            "gating":false,"detector_motion":false,"time_of_flight_processing":false,"reconstruction":false}
    });
    let mut result = serde_json::Map::new();
    for fragment in [identity, dimensions, geometry, quantitative, acquisition] {
        result.extend(
            fragment
                .as_object()
                .expect("PET fragment is an object")
                .clone(),
        );
    }
    Value::Object(result)
}

fn enhanced_case_pixel_range(
    ctx: &CuratedArtifactProjectionContext,
) -> Result<(i64, i64), CuratedManifestError> {
    let artifacts = &ctx
        .case_recipe
        .dicom
        .as_ref()
        .ok_or_else(|| err("enhanced recipe has no DICOM artifacts"))?
        .artifacts;
    let mut minimum = None;
    let mut maximum = None;
    for artifact in artifacts {
        let parameters: crate::curated_execution::AdvancedCompatibilityArtifact =
            serde_json::from_value(Value::Object(artifact.parameters.clone()))
                .map_err(|error| err(format!("invalid enhanced artifact parameters: {error}")))?;
        let (_, item_minimum, item_maximum) =
            parameters.pixels.values(0).map_err(CuratedManifestError)?;
        minimum = Some(minimum.map_or(item_minimum, |value: i64| value.min(item_minimum)));
        maximum = Some(maximum.map_or(item_maximum, |value: i64| value.max(item_maximum)));
    }
    minimum
        .zip(maximum)
        .ok_or_else(|| err("enhanced recipe has no pixel range"))
}

fn project_wsi_manifest(
    ctx: &CuratedArtifactProjectionContext,
    planned: &PlannedDicomArtifact,
    common: &AdvancedManifestCommon<'_>,
) -> Result<Value, CuratedManifestError> {
    let item = wsi_artifact_parameters(ctx).map_err(|error| err(error.to_string()))?;
    let params = &item.parameters;
    let frame_of_reference = common
        .frame_of_reference
        .ok_or_else(|| err("missing WSI Frame of Reference UID"))?;
    let dimension = common
        .dimension
        .ok_or_else(|| err("missing WSI Dimension Organization UID"))?;
    let specimen = planned
        .instance
        .identities
        .get(
            &CompositionUidRole::TemplateDefined("specimen_uid".into()),
            0,
        )
        .ok_or_else(|| err("missing WSI specimen UID"))?;
    let mut manifest = advanced_base(ctx, planned, common)?;
    manifest["references"] = json!([]);
    manifest["uids"] = json!({"study_instance_uid":common.study,"series_instance_uid":common.series,
        "sop_instance_uid":common.sop,"frame_of_reference_uid":frame_of_reference,
        "dimension_organization_uid":dimension,"implementation_class_uid":planned.encoding.implementation.class_uid,
        "implementation_version_name":planned.encoding.implementation.version_name});
    match item.pixel_algorithm {
        WsiPixelAlgorithm::TiledColorQuadrants if !params.pyramid_membership => {
            let expected = crate::wsi_tiled_full_locked_contract(frame_of_reference, specimen);
            manifest["recipe"] = json!({"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,
                "recipe_parameters":{"dimension_organization_type":params.dimension_type,"tile_rows":params.rows,
                    "tile_columns":params.columns,"total_pixel_matrix_rows":params.matrix_rows,
                    "total_pixel_matrix_columns":params.matrix_columns,
                    "frame_order":"row_then_column_then_depth_then_optical_path_then_segment",
                    "tile_colors":["red","green","blue","white"],"icc_profile_sha256":expected["optical_path"]["icc_profile"]["sha256"]}});
            manifest["expected_capabilities"] = json!([
                "open_file",
                "read_metadata",
                "render_native_pixels",
                "navigate_multiframe",
                "reconstruct_total_pixel_matrix"
            ]);
            manifest["expected_semantics"] = json!({"synthetic_data":"YES","image_type":["ORIGINAL","PRIMARY","VOLUME","NONE"],
                "lossy_image_compression":"00","one_specimen":true,"one_optical_path":true,"one_focal_plane":true,
                "slide_label_present":true,"per_frame_functional_groups_absent":true,"dimension_index_sequence_absent":true});
            manifest["expected_wsi_tiled_full"] = expected;
            manifest["expected_visual_checks"] =
                json!({"pattern":"4x4_tiled_full_red_green_blue_white_quadrants"});
            manifest["known_stressors"] = json!([
                "vl_whole_slide_microscopy_image_storage",
                "tiled_full_implicit_frame_order",
                "total_pixel_matrix_reconstruction",
                "specimen_and_optical_path_metadata",
                "nested_icc_profile",
                "absent_per_frame_functional_groups"
            ]);
        }
        WsiPixelAlgorithm::SparseDiagonalTiles if !params.pyramid_membership => {
            let expected =
                crate::wsi_tiled_sparse_locked_contract(frame_of_reference, specimen, dimension);
            manifest["recipe"] = json!({"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,
                "recipe_parameters":{"dimension_organization_type":params.dimension_type,"tile_rows":params.rows,
                    "tile_columns":params.columns,"total_pixel_matrix_rows":params.matrix_rows,
                    "total_pixel_matrix_columns":params.matrix_columns,"stored_tile_positions":[[1,1],[3,3]],
                    "tile_colors":["red","white"],"occupancy_mask":["present","absent","absent","present"],
                    "icc_profile_sha256":expected["optical_path"]["icc_profile"]["sha256"]}});
            manifest["expected_capabilities"] = json!([
                "open_file",
                "read_metadata",
                "render_native_pixels",
                "navigate_multiframe",
                "reconstruct_sparse_total_pixel_matrix"
            ]);
            manifest["expected_semantics"] = json!({"synthetic_data":"YES","image_type":["ORIGINAL","PRIMARY","VOLUME","NONE"],
                "lossy_image_compression":"00","one_specimen":true,"one_optical_path":true,"one_focal_plane":true,
                "slide_label_present":true,"per_frame_functional_groups_present":true,"dimension_index_sequence_present":true,
                "absent_tiles_are_not_black_frames":true});
            manifest["expected_wsi_tiled_sparse"] = expected;
            manifest["expected_visual_checks"] =
                json!({"pattern":"4x4_tiled_sparse_red_and_white_diagonal_with_two_absent_tiles"});
            manifest["known_stressors"] = json!([
                "vl_whole_slide_microscopy_image_storage",
                "tiled_sparse_explicit_frame_positions",
                "dimension_index_values",
                "sparse_occupancy_reconstruction",
                "specimen_and_optical_path_metadata",
                "nested_icc_profile"
            ]);
        }
        WsiPixelAlgorithm::MultipleOpticalPaths if !params.pyramid_membership => {
            let expected = crate::wsi_multiple_optical_paths_locked_contract(
                frame_of_reference,
                specimen,
                dimension,
            );
            manifest["recipe"] = json!({"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,
                "recipe_parameters":{"dimension_organization_type":params.dimension_type,
                    "frame_order":"row_then_column_then_focal_plane_then_optical_path",
                    "optical_path_identifiers":params.optical_paths.iter().map(|path|path.identifier.as_str()).collect::<Vec<_>>(),
                    "optical_path_descriptions":params.optical_paths.iter().filter_map(|path|path.description.as_deref()).collect::<Vec<_>>(),
                    "illumination_wavelengths_nm":params.optical_paths.iter().map(|path|path.wavelength).collect::<Vec<_>>(),
                    "icc_profile_sha256":expected["optical_paths"][0]["icc_profile"]["sha256"]}});
            manifest["expected_capabilities"] = json!([
                "open_file",
                "read_metadata",
                "render_native_pixels",
                "navigate_multiframe",
                "reconstruct_optical_path_matrices"
            ]);
            manifest["expected_semantics"] = json!({"synthetic_data":"YES","two_ordered_optical_paths":true,"one_focal_plane":true,
                "path_major_implicit_frame_order":true,"nested_icc_profiles":true,"top_level_icc_profile_absent":true,
                "per_frame_functional_groups_absent":true,"dimension_index_sequence_absent":true});
            manifest["expected_wsi_multiple_optical_paths"] = expected;
            manifest["expected_visual_checks"] =
                json!({"pattern":"two_distinct_4x4_rgb_optical_path_matrices"});
            manifest["known_stressors"] = json!([
                "vl_whole_slide_microscopy_image_storage",
                "tiled_full_implicit_optical_path_order",
                "multiple_nested_icc_profiles",
                "separate_optical_path_matrix_reconstruction"
            ]);
        }
        WsiPixelAlgorithm::ReducedStress { level_index, .. } => {
            let pyramid = planned
                .instance
                .identities
                .get(
                    &CompositionUidRole::TemplateDefined("pyramid_uid".into()),
                    0,
                )
                .ok_or_else(|| err("missing reduced-stress WSI Pyramid UID"))?;
            manifest["recipe"] = json!({
                "recipe_id":ctx.case_recipe.recipe_id,
                "recipe_version":ctx.case_recipe.recipe_version,
                "recipe_parameters":{
                    "scale":"reduced",
                    "level_index":level_index,
                    "level_number":level_index + 1,
                    "pyramid_levels":ctx.case_recipe.dicom.as_ref().map_or(0, |dicom| dicom.artifacts.len()),
                    "total_pixel_matrix_rows":params.matrix_rows,
                    "total_pixel_matrix_columns":params.matrix_columns,
                    "tile_rows":params.rows,
                    "tile_columns":params.columns,
                    "frames":params.frames,
                    "pixel_spacing":params.spacing,
                    "native_payload_bytes":common.pixels.size_bytes
                }
            });
            manifest["expected_capabilities"] = json!([
                "open_file",
                "read_metadata",
                "render_native_pixels",
                "navigate_multiframe",
                "reconstruct_wsi_pyramid"
            ]);
            let spacing = params
                .spacing
                .split('\\')
                .map(|value| value.parse::<f64>().map_err(|error| err(error.to_string())))
                .collect::<Result<Vec<_>, _>>()?;
            if spacing.len() != 2 {
                return fail("reduced-stress WSI Pixel Spacing must contain two values");
            }
            manifest["expected_semantics"] = json!({
                "synthetic_data":"YES",
                "image_type":params.image_type.split('\\').collect::<Vec<_>>(),
                "shared_study_series_frame_of_reference":true,
                "shared_pyramid_uid":pyramid,
                "tiled_full":true,
                "ordered_level":level_index + 1,
                "level_count":ctx.case_recipe.dicom.as_ref().map_or(0, |dicom| dicom.artifacts.len()),
                "physical_extent_mm":[
                    spacing[1] * f64::from(params.matrix_columns),
                    spacing[0] * f64::from(params.matrix_rows)
                ]
            });
            manifest["expected_visual_checks"] =
                json!({"pattern":"rgb_xy_ramps_with_64_pixel_checkerboard_edges"});
            manifest["known_stressors"] = json!([
                "reduced_stress_scale",
                "vl_whole_slide_microscopy_image_storage",
                "three_level_pyramid",
                "1024_square_total_pixel_matrix",
                "256_square_tiles",
                "tiled_full_frame_inference"
            ]);
        }
        _ => {
            manifest["recipe"] = json!({"recipe_id":ctx.case_recipe.recipe_id,"recipe_version":ctx.case_recipe.recipe_version,
                "recipe_parameters":{"provider":ctx.case_recipe.provider_parameters,"artifact":ctx.artifact_recipe.parameters}});
            manifest["expected_capabilities"] = json!(ctx.registry_case.compatibility_axes);
            manifest["expected_semantics"] = json!({"synthetic_data":"YES","image_type":params.image_type.split('\\').collect::<Vec<_>>()});
            manifest["expected_visual_checks"] =
                json!({"pattern":ctx.artifact_recipe.content.provider_id});
            manifest["known_stressors"] = json!(ctx.registry_case.compatibility_axes);
        }
    }
    Ok(manifest)
}

fn planned_attribute<'a>(
    planned: &'a PlannedDicomArtifact,
    tag: &str,
) -> Result<&'a crate::composition::AttributeValue, CuratedManifestError> {
    planned
        .instance
        .attributes
        .iter()
        .find(|attribute| attribute.address.normalized_tag() == tag)
        .and_then(|attribute| attribute.value.as_ref())
        .ok_or_else(|| err(format!("advanced plan is missing attribute {tag}")))
}

fn planned_unsigned(
    planned: &PlannedDicomArtifact,
    tag: &str,
) -> Result<u64, CuratedManifestError> {
    planned_optional_unsigned(planned, tag)?
        .ok_or_else(|| err(format!("advanced plan is missing unsigned attribute {tag}")))
}

fn planned_optional_unsigned(
    planned: &PlannedDicomArtifact,
    tag: &str,
) -> Result<Option<u64>, CuratedManifestError> {
    let Some(attribute) = planned
        .instance
        .attributes
        .iter()
        .find(|attribute| attribute.address.normalized_tag() == tag)
    else {
        return Ok(None);
    };
    match attribute.value.as_ref() {
        Some(crate::composition::AttributeValue::Primitive(
            crate::composition::PrimitiveValue::Unsigned(value),
        )) => Ok(Some(*value)),
        Some(crate::composition::AttributeValue::Primitive(
            crate::composition::PrimitiveValue::String(value),
        )) => value
            .parse()
            .map(Some)
            .map_err(|error| err(format!("invalid unsigned attribute {tag}: {error}"))),
        _ => fail(format!(
            "advanced attribute {tag} is not an unsigned primitive"
        )),
    }
}

fn planned_string(
    planned: &PlannedDicomArtifact,
    tag: &str,
) -> Result<String, CuratedManifestError> {
    match planned_attribute(planned, tag)? {
        crate::composition::AttributeValue::Primitive(
            crate::composition::PrimitiveValue::String(value),
        ) => Ok(value.clone()),
        crate::composition::AttributeValue::Multi(values) => values
            .iter()
            .map(|value| match value {
                crate::composition::PrimitiveValue::String(value) => Ok(value.as_str()),
                _ => fail(format!("advanced attribute {tag} has a non-string value")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| values.join("\\")),
        _ => fail(format!("advanced attribute {tag} is not a string")),
    }
}

pub(super) fn pixel_data(
    planned: &PlannedDicomArtifact,
    execution: &ArtifactExecutionEvidence,
    pixels: &MaterializedContentEvidence,
    sc: &crate::recipes::SecondaryCaptureParameters,
) -> Result<Value, CuratedManifestError> {
    let vr = sc.pixel_data_vr.as_str();
    if planned.encoding.transfer_syntax_uid != RLE {
        if !execution.codecs.is_empty() || pixels.fragment_count != 0 {
            return fail("native artifact has codec/fragment evidence");
        }
        let expected_value_length = pixels
            .size_bytes
            .checked_add(
                sc.bit_packing
                    .as_ref()
                    .map_or(0, |facts| facts.value_field_padding_bytes),
            )
            .ok_or_else(|| err("native Value Field length overflow"))?;
        let value_length = pixels
            .native_value_field_size_bytes
            .ok_or_else(|| err("missing native Value Field size evidence"))?;
        if value_length != expected_value_length {
            return fail("native Value Field size differs from recipe");
        }
        return Ok(
            json!({"vr":vr,"native_or_encapsulated":"native","value_length":value_length,
            "frame_count":pixels.decoded_frame_sha256.len(),"frame_hashes":pixels.decoded_frame_sha256}),
        );
    }
    let codec = only(&execution.codecs, "codec evidence")?;
    if codec.status != ResultStatus::Passed
        || codec.transfer_syntax_uid != RLE
        || codec.slot != "pixels"
        || codec.encoded_frame_sha256 != pixels.compressed_frame_sha256
        || (!pixels.decoded_frame_sha256.is_empty()
            && codec.decoded_frame_sha256 != pixels.decoded_frame_sha256)
    {
        return fail("codec evidence differs from materialization");
    }
    let expect_bot = matches!(
        planned.encoding.offset_table,
        OffsetTablePolicy::PopulatedBasic
    );
    let expect_eot = matches!(planned.encoding.offset_table, OffsetTablePolicy::Extended);
    if pixels.fragment_count != pixels.fragments.len() as u64
        || pixels.fragment_count != pixels.compressed_lengths.len() as u64
        || pixels.fragment_count != pixels.padded_fragment_lengths.len() as u64
        || pixels.fragments_per_frame.iter().sum::<u64>() != pixels.fragment_count
        || pixels.fragments_per_frame.len() != codec.decoded_frame_sha256.len()
        || pixels.fragments_per_frame.iter().any(|count| *count != 1)
    {
        return fail("fragment evidence cardinality differs from curated encoding policy");
    }
    if expect_bot != !pixels.basic_offset_table.is_empty()
        || expect_eot != !pixels.extended_offset_table.is_empty()
    {
        return fail("offset table evidence differs from encoding policy");
    }
    Ok(json!({
        "vr":vr,"native_or_encapsulated":"encapsulated","value_length":Value::Null,
        "frame_count":codec.decoded_frame_sha256.len(),"frame_hashes":codec.decoded_frame_sha256,
        "codec":{"backend_id":codec.backend_id,"backend_kind":codec.backend_kind,"display_name":codec.display_name,
            "version":codec.backend_version,"transfer_syntax_uid":codec.transfer_syntax_uid,"feature_gate":codec.feature_gate,
            "determinism":codec.determinism},
        "encapsulated_pixel_data":{
            "basic_offset_table":{"present":true,"populated":!pixels.basic_offset_table.is_empty(),
                "offset_count":pixels.basic_offset_table.len(),"offsets":pixels.basic_offset_table},
            "fragments_per_frame":pixels.fragments_per_frame,"fragments":pixels.fragments,
            "extended_offset_table": if expect_eot { json!({"present":true,"lengths_present":true,
                "offset_count":pixels.extended_offset_table.len(),"length_count":pixels.extended_offset_table_lengths.len(),
                "offsets":pixels.extended_offset_table,"lengths":pixels.extended_offset_table_lengths}) }
                else { json!({"present":false,"lengths_present":false,"offset_count":0,"length_count":0}) },
            "compressed_frame_hashes":pixels.compressed_frame_sha256
        }
    }))
}

pub(super) fn validation_checks(
    execution: &ArtifactExecutionEvidence,
) -> Result<Vec<TypedValidationCheck>, CuratedManifestError> {
    let validation = execution
        .validation
        .first()
        .ok_or_else(|| err("missing typed validation"))?;
    if execution.validation.iter().any(|item| {
        item.status != ResultStatus::Passed
            || item.details.get("checks") != validation.details.get("checks")
    }) {
        return fail("typed validation results are failed or inconsistent");
    }
    serde_json::from_value(
        validation
            .details
            .get("checks")
            .cloned()
            .ok_or_else(|| err("missing validation checks"))?,
    )
    .map_err(|error| err(format!("invalid typed validation checks: {error}")))
}
fn metadata_observation(
    execution: &ArtifactExecutionEvidence,
) -> Result<Option<MetadataObservation>, CuratedManifestError> {
    let Some(validation) = execution.validation.first() else {
        return Ok(None);
    };
    validation
        .details
        .get("metadata_observation")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| err(format!("invalid metadata observation: {error}")))
}
pub(super) fn legacy_validation(checks: &[TypedValidationCheck]) -> Value {
    let layer = |wanted| {
        checks
            .iter()
            .filter(|check| check.layer == wanted)
            .map(TypedValidationCheck::legacy_json)
            .collect::<Vec<_>>()
    };
    json!({"status":if checks.iter().all(TypedValidationCheck::passed){"passed"}else{"failed"},
        "internal":layer(CheckLayer::Internal),"standards":layer(CheckLayer::Standards),"external":layer(CheckLayer::External)})
}

fn add_special(
    manifest: &mut Value,
    ctx: &CuratedArtifactProjectionContext,
    planned: &PlannedDicomArtifact,
    pixels: &MaterializedContentEvidence,
    observation: Option<&MetadataObservation>,
) -> Result<(), CuratedManifestError> {
    let sc = ctx.artifact_recipe.secondary_capture.as_ref().unwrap();
    if let Some(bits) = &sc.bit_packing {
        let actual = pixels
            .native_bit_packing
            .as_ref()
            .ok_or_else(|| err("missing native bit-packing evidence"))?;
        if actual.total_stored_values != bits.significant_bits
            || actual.packed_size_bytes != bits.significant_packed_bytes
            || actual.unused_trailing_bits != bits.unused_high_bits
        {
            return fail("bit-packing evidence differs from recipe");
        }
        let value_field_sha256 = pixels
            .native_value_field_sha256
            .as_ref()
            .ok_or_else(|| err("missing native Value Field hash evidence"))?;
        manifest["expected_u1_pixels"] = json!({"packing_order":bits.bit_order,"frame_boundary_policy":bits.frame_boundary_policy,
            "stored_values":sc.stored_values,"decoded_frame_sha256":pixels.decoded_frame_sha256,"pixel_data_sha256":value_field_sha256,
            "significant_bits":bits.significant_bits,"significant_packed_bytes":bits.significant_packed_bytes,
            "unused_high_bits":bits.unused_high_bits,"value_field_padding_bytes":bits.value_field_padding_bytes,
            "frame_two_bit_offset":bits.frame_start_bit_offsets.get(1).ok_or_else(|| err("missing second frame bit offset"))?});
    }
    if let Some(word) = &sc.integer_word {
        manifest["expected_u32_pixels"] = json!({"stored_values":sc.stored_values,"pixel_data_sha256":pixels.sha256,
            "word_byte_order":word.byte_order,"full_unsigned_range":word.covers_full_unsigned_range});
    }
    if let Some(eot) = &sc.encapsulation_projection {
        if pixels.extended_offset_table.is_empty() {
            return fail("missing EOT execution evidence");
        }
        let MetadataObservation::ExtendedOffsetTable {
            offsets,
            lengths,
            padded_fragment_lengths,
            basic_offset_table_entries,
            page_numbers,
        } = observation.ok_or_else(|| err("missing EOT validation observation"))?
        else {
            return fail("wrong EOT observation kind");
        };
        if offsets != &pixels.extended_offset_table
            || lengths != &pixels.extended_offset_table_lengths
            || padded_fragment_lengths != &pixels.padded_fragment_lengths
            || *basic_offset_table_entries != 0
            || page_numbers != &(1..=sc.frames as i32).collect::<Vec<_>>()
        {
            return fail("EOT validation observation differs from materialization/recipe");
        }
        manifest["expected_eot"] = json!({"origin":eot.offset_origin,"item_header_bytes":eot.item_header_bytes,
            "frame_encoded_lengths":pixels.extended_offset_table_lengths,"offsets":pixels.extended_offset_table,
            "lengths":pixels.extended_offset_table_lengths});
    }
    if let Some(metadata) = &ctx.artifact_recipe.metadata_sc {
        manifest["expected_metadata"] = expected_metadata(
            metadata,
            observation.ok_or_else(|| err("missing metadata observation"))?,
        )?;
        add_metadata_recipe_parameters(manifest, metadata);
    }
    if let Some(geometry) = &ctx.artifact_recipe.nonsquare_geometry {
        let MetadataObservation::NonsquareGeometry {
            variant_id,
            pixel_spacing,
            nominal_scanned_pixel_spacing,
            pixel_aspect_ratio,
            patient_space_geometry_present,
        } = observation.ok_or_else(|| err("missing nonsquare observation"))?
        else {
            return fail("wrong nonsquare observation kind");
        };
        if variant_id != &geometry.variant_id
            || *patient_space_geometry_present != geometry.patient_space_geometry_present
        {
            return fail("nonsquare observation differs from recipe");
        }
        let spacing = geometry
            .pixel_spacing
            .as_ref()
            .map(|v| spacing_value("0028,0030", "PixelSpacing", v))
            .transpose()?;
        let nominal = geometry
            .nominal_scanned_pixel_spacing
            .as_ref()
            .map(|v| spacing_value("0018,2010", "NominalScannedPixelSpacing", v))
            .transpose()?;
        let aspect = geometry.pixel_aspect_ratio.map(|v| json!({"tag":"0028,0034","keyword":"PixelAspectRatio","vr":"IS","vm":2,
            "lexical_value":format!("{}\\{}",v[0],v[1]),"vertical_extent":v[0],"horizontal_extent":v[1]}));
        if pixel_spacing.as_deref() != geometry.pixel_spacing.as_ref().map(|v| v.as_slice())
            || nominal_scanned_pixel_spacing.as_deref()
                != geometry
                    .nominal_scanned_pixel_spacing
                    .as_ref()
                    .map(|v| v.as_slice())
            || pixel_aspect_ratio.as_ref().map(|v| v.join("\\"))
                != geometry
                    .pixel_aspect_ratio
                    .map(|v| format!("{}\\{}", v[0], v[1]))
        {
            return fail("nonsquare raw observation differs from recipe");
        }
        manifest["expected_nonsquare_spacing"] = json!({"variant_id":geometry.variant_id,"pixel_spacing":spacing,
            "nominal_scanned_pixel_spacing":nominal,"pixel_aspect_ratio":aspect,"uncalibrated":!geometry.calibrated,
            "patient_space_geometry_present":geometry.patient_space_geometry_present,"pixel_data_sha256":pixels.sha256});
        manifest["recipe"]["recipe_parameters"]["nonsquare_variant"] = json!(geometry.variant_id);
        manifest["recipe"]["recipe_parameters"]["row_to_column_ratio"] =
            json!(geometry.row_to_column_ratio);
    }
    let _ = planned;
    Ok(())
}

fn expected_metadata(
    metadata: &MetadataScParameters,
    observation: &MetadataObservation,
) -> Result<Value, CuratedManifestError> {
    match metadata {
        MetadataScParameters::PersonName(value) => {
            let attributes = observed_attributes(observation)?;
            let pn = find_observed(attributes, "0010,0010")?;
            if pn.raw_value_hex != value.patient_name_raw_hex
                || pn.raw_value_sha256 != value.patient_name_raw_sha256
            {
                return fail("PN observation differs from recipe");
            }
            Ok(
                json!({"specific_character_sets":value.specific_character_sets,"person_names":[{"tag":"0010,0010","keyword":"PatientName","vr":"PN",
                "decoded_value":value.patient_name_decoded,"raw_value_hex":pn.raw_value_hex,"raw_value_sha256":pn.raw_value_sha256,
                "raw_value_byte_length":pn.raw_value_byte_length,"component_groups":value.component_groups.iter().enumerate().map(|(i,g)| json!({
                    "position":i+1,"kind":g.kind,"decoded_value":g.decoded_value,"components":g.components.iter().enumerate().map(|(j,c)|json!({"position":j+1,"decoded_value":c})).collect::<Vec<_>>()
                })).collect::<Vec<_>>()}]}),
            )
        }
        MetadataScParameters::TimezoneBoundary(value) => {
            let attrs = observed_attributes(observation)?;
            let encoded = |tag: &str,
                           keyword: &str,
                           vr: &str,
                           decoded: &str|
             -> Result<Value, CuratedManifestError> {
                let item = find_observed(attrs, tag)?;
                Ok(
                    json!({"tag":tag,"keyword":keyword,"vr":vr,"decoded_value":decoded,
                    "raw_value_hex":item.raw_value_hex,"raw_value_sha256":item.raw_value_sha256,"raw_value_byte_length":item.raw_value_byte_length}),
                )
            };
            let mut tz = encoded(
                "0008,0201",
                "TimezoneOffsetFromUTC",
                "SH",
                &value.timezone_offset,
            )?;
            tz["offset_minutes"] = json!(value.offset_minutes);
            let mut dt = encoded(
                "0008,002A",
                "AcquisitionDateTime",
                "DT",
                &value.acquisition_date_time,
            )?;
            dt["embedded_offset_minutes"] = json!(value.offset_minutes);
            dt["normalized_utc"] = json!(value.normalized_utc);
            Ok(
                json!({"temporal":{"boundary_id":value.boundary_id,"timezone_offset_from_utc":tz,
                "date_values":[encoded("0008,0020","StudyDate","DA",&value.study_date)?],
                "time_values":[encoded("0008,0030","StudyTime","TM",&value.study_time)?],"date_time_values":[dt],"combined_da_tm_utc":value.normalized_utc}}),
            )
        }
        MetadataScParameters::EmptyType2 { attributes } => Ok(
            json!({"empty_type2_attributes":attributes.iter().map(|a|json!({"tag":a.tag,"keyword":a.keyword,"vr":a.vr,"value_length":0})).collect::<Vec<_>>() }),
        ),
        MetadataScParameters::StringBoundaries { elements } => {
            let attrs = observed_attributes(observation)?;
            Ok(
                json!({"string_elements":elements.iter().map(|e| -> Result<Value,CuratedManifestError> { let a=find_observed(attrs,&e.tag)?;
                if a.raw_value_sha256!=e.raw_value_sha256 || a.raw_value_byte_length!=u64::from(e.raw_value_byte_length){return fail("string observation differs from recipe");}
                let decoded=match &e.source{StringValueSource::Repeated{pattern,repetitions}=>vec![pattern.repeat(*repetitions as usize)],StringValueSource::Literal{values}=>values.clone()};
                Ok(json!({"tag":e.tag,"keyword":e.keyword,"vr":e.vr,"value_multiplicity":decoded.len(),"decoded_values":decoded,
                    "decoded_value_lengths":decoded.iter().map(String::len).collect::<Vec<_>>(),"padding":e.padding,
                    "raw_value_byte_length":a.raw_value_byte_length,"raw_value_sha256":a.raw_value_sha256})) }).collect::<Result<Vec<_>,_>>()?}),
            )
        }
        MetadataScParameters::PrivateCreators { blocks } => {
            let attrs = observed_attributes(observation)?;
            Ok(
                json!({"private_creator_blocks":blocks.iter().map(|b| -> Result<Value,CuratedManifestError> { let creator=find_observed(attrs,&b.creator_tag)?;
                Ok(json!({"creator_tag":b.creator_tag,"creator_id":b.creator_id,"block_start_tag":b.block_start_tag,"block_end_tag":b.block_end_tag,
                    "vr":creator.vr,"raw_value_hex":creator.raw_value_hex,"raw_value_sha256":creator.raw_value_sha256,
                    "raw_value_byte_length":creator.raw_value_byte_length,"elements":b.elements.iter().map(|e| -> Result<Value,CuratedManifestError>{let a=find_observed(attrs,&e.tag)?;
                        let (vr,decoded)=match &e.value{PrivateElementValue::Lo{text}=>("LO",json!(text)),PrivateElementValue::Us{number}=>("US",json!(number))};
                        Ok(json!({"tag":e.tag,"vr":vr,"decoded_value":decoded,"raw_value_hex":a.raw_value_hex,"raw_value_sha256":a.raw_value_sha256,"raw_value_byte_length":a.raw_value_byte_length}))}).collect::<Result<Vec<_>,_>>()?}))}).collect::<Result<Vec<_>,_>>()?}),
            )
        }
        MetadataScParameters::SequenceLengths(value) => {
            let MetadataObservation::SequenceLengths {
                sequence_tag,
                raw_length,
                item_header_matches,
                item_delimiter_present,
                sequence_delimiter_present,
                decoded_item_count,
            } = observation
            else {
                return fail("wrong sequence observation kind");
            };
            if sequence_tag != &value.sequence_tag
                || !item_header_matches
                || *item_delimiter_present != value.item_delimitation_present
                || *sequence_delimiter_present != value.sequence_delimitation_present
                || *decoded_item_count != 1
            {
                return fail("sequence observation differs from recipe");
            }
            Ok(
                json!({"sequence_length_encoding":{"variant_id":value.variant_id,"sequence_tag":value.sequence_tag,"keyword":"AnatomicRegionSequence","vr":value.sequence_vr,
                "sequence_value_length":if value.variant_id=="defined"{json!(raw_length)}else{Value::Null},"sequence_length_field_hex":value.sequence_length_field_hex,
                "sequence_delimitation_present":value.sequence_delimitation_present,"item_count":1,"item_length_encoding":"undefined",
                "item_length_field_hex":value.item_length_field_hex,"item_delimitation_present":value.item_delimitation_present,
                "decoded_items":[{"code_value":value.code_value,"coding_scheme_designator":value.coding_scheme_designator,"code_meaning":value.code_meaning}]}}),
            )
        }
    }
}

fn add_metadata_recipe_parameters(manifest: &mut Value, metadata: &MetadataScParameters) {
    match metadata {
        MetadataScParameters::PersonName(v) => {
            manifest["recipe"]["recipe_parameters"]["specific_character_sets"] =
                json!(v.specific_character_sets);
            manifest["recipe"]["recipe_parameters"]["patient_name"] = json!(v.patient_name_decoded);
        }
        MetadataScParameters::TimezoneBoundary(v) => {
            manifest["recipe"]["recipe_parameters"]["temporal_boundary_id"] = json!(v.boundary_id);
            manifest["recipe"]["recipe_parameters"]["timezone_offset_from_utc"] =
                json!(v.timezone_offset);
        }
        MetadataScParameters::EmptyType2 { attributes } => {
            manifest["recipe"]["recipe_parameters"]["empty_type2_attribute_count"] =
                json!(attributes.len())
        }
        MetadataScParameters::StringBoundaries { elements } => {
            manifest["recipe"]["recipe_parameters"]["string_boundary_element_count"] =
                json!(elements.len())
        }
        MetadataScParameters::PrivateCreators { blocks } => {
            manifest["recipe"]["recipe_parameters"]["private_creator_block_count"] =
                json!(blocks.len())
        }
        MetadataScParameters::SequenceLengths(v) => {
            manifest["recipe"]["recipe_parameters"]["sequence_length_variant"] = json!(v.variant_id)
        }
    }
}

fn capabilities(
    sc: &crate::recipes::SecondaryCaptureParameters,
    ts: &str,
    geometry: bool,
) -> Vec<&'static str> {
    if geometry {
        return vec![
            "open_file",
            "read_metadata",
            "interpret_pixel_geometry",
            "render_native_pixels",
        ];
    }
    if sc.bits_allocated == 1 {
        return vec![
            "open_file",
            "read_metadata",
            "unpack_native_bit_packed_pixels",
            "render_native_pixels",
        ];
    }
    if ts == RLE {
        let render = if sc.palette.is_some() {
            "render_palette_color"
        } else if sc.samples_per_pixel == 3 {
            "render_color"
        } else {
            "render_grayscale"
        };
        vec![
            "open_file",
            "read_metadata",
            "decode_rle_lossless_pixels",
            render,
        ]
    } else {
        vec!["open_file", "read_metadata", "render_native_pixels"]
    }
}

fn standards(
    ctx: &CuratedArtifactProjectionContext,
    sc: &crate::recipes::SecondaryCaptureParameters,
    ts: &str,
) -> Vec<Value> {
    let mut values = ctx.registry_case.standards_evidence.clone();
    let records = if sc.bit_packing.is_some() {
        vec![
            std_record(
                "lookup_sop_class Multi-frame Single Bit Secondary Capture Image Storage",
                "PS3.4",
                "table_B.5-1",
            ),
            std_record(
                "lookup_iod Multi-frame Single Bit Secondary Capture Image",
                "PS3.3",
                "table_A.8-2",
            ),
            std_record(
                "retrieve_standard_text sect_A.8.2.4",
                "PS3.3",
                "sect_A.8.2.4",
            ),
            local_record(
                "standards/source-notes/phase-2-u1-native-pixels.md",
                "PS3.5",
                "sect_8.1.1",
            ),
        ]
    } else {
        vec![
            std_record(
                "lookup_sop_class SecondaryCaptureImageStorage",
                "PS3.3",
                "table_A.8-1",
            ),
            std_record("lookup_data_element SyntheticData", "PS3.6", "table_6-1"),
            std_record(
                "search_standard_text Image Pixel Description Macro",
                "PS3.3",
                "table_C.7-11c",
            ),
            std_record(
                "retrieve_standard_text sect_C.7.6.3.1.2",
                "PS3.3",
                "sect_C.7.6.3.1.2",
            ),
        ]
    };
    values.extend(records);
    if sc
        .color
        .as_ref()
        .and_then(|c| c.planar_configuration)
        .is_some()
    {
        values.extend([
            std_record(
                "lookup_data_element PlanarConfiguration",
                "PS3.6",
                "table_6-1",
            ),
            std_record(
                "retrieve_standard_text sect_C.7.6.3.1.3",
                "PS3.3",
                "sect_C.7.6.3.1.3",
            ),
        ]);
    }
    if sc.palette.is_some() {
        for (q, p, a) in [
            (
                "retrieve_standard_text sect_C.7.6.3.1.5",
                "PS3.3",
                "sect_C.7.6.3.1.5",
            ),
            (
                "lookup_data_element RedPaletteColorLookupTableDescriptor",
                "PS3.6",
                "table_6-1",
            ),
            (
                "lookup_data_element GreenPaletteColorLookupTableDescriptor",
                "PS3.6",
                "table_6-1",
            ),
            (
                "lookup_data_element BluePaletteColorLookupTableDescriptor",
                "PS3.6",
                "table_6-1",
            ),
            (
                "lookup_data_element RedPaletteColorLookupTableData",
                "PS3.6",
                "table_6-1",
            ),
            (
                "lookup_data_element GreenPaletteColorLookupTableData",
                "PS3.6",
                "table_6-1",
            ),
            (
                "lookup_data_element BluePaletteColorLookupTableData",
                "PS3.6",
                "table_6-1",
            ),
        ] {
            values.push(std_record(q, p, a));
        }
    }
    if sc.padding.is_some() {
        values.extend([
            std_record(
                "lookup_data_element PixelPaddingValue",
                "PS3.6",
                "table_6-1",
            ),
            std_record(
                "lookup_data_element PixelPaddingRangeLimit",
                "PS3.6",
                "table_6-1",
            ),
            std_record(
                "retrieve_standard_text sect_C.7.5.1.1.2",
                "PS3.3",
                "sect_C.7.5.1.1.2",
            ),
        ]);
    }
    if ts == RLE {
        values.extend([std_record("lookup_uid RLELossless","PS3.6","table_A-1"),std_record("search_standard_text RLE Lossless Transfer Syntax encapsulated Pixel Data","PS3.5","sect_8.2.2"),std_record("search_standard_text Basic Offset Table encapsulated Pixel Data Item padding Extended Offset Table","PS3.5","sect_A.4")]);
    }
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| {
            seen.insert((
                value
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                value
                    .get("query")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ))
        })
        .collect()
}
fn std_record(query: &str, part: &str, anchor: &str) -> Value {
    json!({"source":"dicom-standard-kb","edition":"2026b","query":query,"covered":true,"part":part,"anchor":anchor})
}
fn local_record(query: &str, part: &str, anchor: &str) -> Value {
    json!({"source":"local-source-note","edition":"2026b","query":query,"covered":true,"part":part,"anchor":anchor})
}
fn spacing_value(tag: &str, keyword: &str, v: &[String; 2]) -> Result<Value, CuratedManifestError> {
    let row_spacing = v[0]
        .parse::<f64>()
        .map_err(|error| err(format!("invalid row spacing {:?}: {error}", v[0])))?;
    let column_spacing = v[1]
        .parse::<f64>()
        .map_err(|error| err(format!("invalid column spacing {:?}: {error}", v[1])))?;
    Ok(
        json!({"tag":tag,"keyword":keyword,"vr":"DS","vm":2,"lexical_value":format!("{}\\{}",v[0],v[1]),"row_spacing_mm":row_spacing,"column_spacing_mm":column_spacing}),
    )
}
fn observed_attributes(
    observation: &MetadataObservation,
) -> Result<&[crate::curated_validation::ObservedAttribute], CuratedManifestError> {
    match observation {
        MetadataObservation::Attributes { attributes } => Ok(attributes),
        _ => fail("wrong metadata observation kind"),
    }
}
fn find_observed<'a>(
    attrs: &'a [crate::curated_validation::ObservedAttribute],
    tag: &str,
) -> Result<&'a crate::curated_validation::ObservedAttribute, CuratedManifestError> {
    attrs
        .iter()
        .find(|a| a.tag == tag)
        .ok_or_else(|| err(format!("missing observed attribute {tag}")))
}
pub(super) fn uid(
    planned: &PlannedDicomArtifact,
    role: CompositionUidRole,
) -> Result<&str, CuratedManifestError> {
    planned
        .instance
        .identities
        .get(&role, 0)
        .ok_or_else(|| err(format!("missing {role:?} UID")))
}
pub(super) fn transfer_syntax_name(uid: &str) -> Result<&'static str, CuratedManifestError> {
    match uid {
        "1.2.840.10008.1.2" => Ok("Implicit VR Little Endian"),
        "1.2.840.10008.1.2.1" => Ok("Explicit VR Little Endian"),
        "1.2.840.10008.1.2.2" => Ok("Explicit VR Big Endian"),
        RLE => Ok("RLE Lossless"),
        _ => fail(format!("unsupported curated transfer syntax {uid}")),
    }
}
pub(super) fn required<'a>(
    value: &'a Option<String>,
    label: &str,
) -> Result<&'a str, CuratedManifestError> {
    value
        .as_deref()
        .ok_or_else(|| err(format!("missing {label}")))
}
pub(super) fn only<'a, T>(values: &'a [T], label: &str) -> Result<&'a T, CuratedManifestError> {
    if values.len() == 1 {
        Ok(&values[0])
    } else {
        fail(format!("expected exactly one {label}"))
    }
}
pub(super) fn err(message: impl Into<String>) -> CuratedManifestError {
    CuratedManifestError(message.into())
}
pub(super) fn fail<T>(message: impl Into<String>) -> Result<T, CuratedManifestError> {
    Err(err(message))
}
