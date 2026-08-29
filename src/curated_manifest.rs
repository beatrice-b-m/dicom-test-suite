//! Pure compatibility projection for plan-first curated generation.

mod classic;

use std::collections::BTreeSet;
use std::fmt;

use serde_json::{Value, json};

use crate::composition::CompositionUidRole;
use crate::corpus_plan::{OffsetTablePolicy, PlannedArtifact, PlannedDicomArtifact};
use crate::curated_plan::{CuratedArtifactProjectionContext, CuratedScProjectionContext};
use crate::curated_validation::{CheckLayer, MetadataObservation, TypedValidationCheck};
use crate::executor::adapters::{ManifestProjectionArtifact, ManifestProjectionCompatibilityInput};
use crate::executor::evidence::{
    ArtifactExecutionEvidence, MaterializedContentEvidence, ResultStatus,
};
use crate::recipes::{MetadataScParameters, PrivateElementValue, StringValueSource};

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
    context
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
            project_one(ctx, artifact)
        })
        .collect()
}

fn project_one(
    ctx: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
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
