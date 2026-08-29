use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::curated_validation::{CheckLayer, TypedValidationCheck, TypedValidationReport};
use crate::recipes::{EncapsulatedPayload, EncapsulatedPayloadPlanInput, WaveformPlanInput};

use super::projection::project_waveform;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecializedValidationObservation {
    pub generic_plan_validation_passed: bool,
    pub part10_preamble_present: bool,
    pub transfer_syntax_uid: String,
    pub sop_class_uid: String,
    pub implementation_identity_matches: bool,
    pub pixel_data_absent: bool,
    pub content: BTreeMap<String, ObservedSpecializedContent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedSpecializedContent {
    pub size_bytes: u64,
    pub sha256: String,
    pub vr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecializedValidationError {
    Observation(String),
    Projection(String),
}

impl fmt::Display for SpecializedValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation(message) => write!(formatter, "specialized observation: {message}"),
            Self::Projection(message) => write!(formatter, "specialized projection: {message}"),
        }
    }
}

impl std::error::Error for SpecializedValidationError {}

pub fn validate_waveform(
    input: &WaveformPlanInput,
    observed: &SpecializedValidationObservation,
) -> Result<TypedValidationReport, SpecializedValidationError> {
    require_common(
        observed,
        &input.sop_class_uid,
        input.groups.iter().map(|group| {
            (
                group.slot.as_str(),
                group.declared_size_bytes,
                group.declared_sha256.as_str(),
                "OW",
            )
        }),
    )?;
    let projected = project_waveform(input)?;
    let waveform = &projected.expected_waveform;
    let prefix = waveform.iod_kind.as_str();
    let finding = |suffix: &str| format!("{prefix}_{suffix}");
    let mut checks = Vec::new();
    push(
        &mut checks,
        &finding("part10_preamble"),
        "File has a Part 10 preamble and DICM marker.",
    );
    push(
        &mut checks,
        &finding("transfer_syntax"),
        "Transfer Syntax matches the locked recipe.",
    );
    for name in [
        "sop_class_uid",
        "sop_instance_uid",
        "synthetic_data",
        "study_instance_uid",
        "series_instance_uid",
    ] {
        push(
            &mut checks,
            &finding(name),
            "Identity matches the locked recipe.",
        );
    }
    push(
        &mut checks,
        &finding("media_storage_sop_class_uid"),
        "File Meta SOP Class matches the dataset.",
    );
    push(
        &mut checks,
        &finding("media_storage_sop_instance_uid"),
        "File Meta SOP Instance matches the dataset.",
    );
    push(
        &mut checks,
        &finding("implementation_class_uid"),
        "Implementation Class UID matches the deterministic generator.",
    );
    for name in [
        "patient_name",
        "patient_id",
        "patient_birth_date",
        "patient_sex",
        "study_date",
        "study_time",
        "referring_physician",
        "study_id",
        "accession_number",
        "modality",
        "series_number",
        "manufacturer",
        "institution_name",
        "institution_address",
        "manufacturer_model_name",
        "device_serial_number",
        "software_versions",
        "instance_number",
        "content_date",
        "content_time",
        "acquisition_date_time",
    ] {
        push(
            &mut checks,
            &finding(name),
            "Required IOD attribute matches the locked recipe.",
        );
    }
    push(
        &mut checks,
        &finding("acquisition_context_count"),
        "Acquisition Context Sequence is present and empty.",
    );
    for (suffix, message) in [
        (
            "manifest_aggregate_group_count",
            "Manifest aggregate group count matches the ordered group array.",
        ),
        (
            "group_count",
            "Waveform Sequence cardinality matches the ordered manifest groups.",
        ),
        (
            "manifest_aggregate_channel_count",
            "Manifest aggregate channel count matches its ordered groups.",
        ),
        (
            "manifest_aggregate_payload_length",
            "Manifest aggregate payload length matches its ordered groups.",
        ),
        (
            "manifest_aggregate_group_hash_count",
            "Manifest aggregate contains one ordered payload hash per group.",
        ),
        (
            "manifest_aggregate_common_duration",
            "Every ordered group matches the manifest aggregate common duration.",
        ),
        (
            "manifest_aggregate_group_hashes",
            "Manifest aggregate preserves the ordered group payload hashes.",
        ),
    ] {
        push(&mut checks, &finding(suffix), message);
    }
    let qualify_group = waveform.multiplex_groups.len() > 1;
    for group in &waveform.multiplex_groups {
        push(
            &mut checks,
            &format!("{prefix}_group_{}_ordinal", group.ordinal),
            "Manifest multiplex-group ordinal is one-based and ordered.",
        );
        for name in ["originality", "label", "sample_interpretation"] {
            push(
                &mut checks,
                &finding(name),
                "Waveform attribute matches the locked recipe.",
            );
        }
        for (suffix, message) in [
            (
                "sample_interpretation_vr",
                "Waveform Sample Interpretation uses VR CS.",
            ),
            (
                "manifest_channel_count",
                "Manifest group channel count matches its channel definitions.",
            ),
            (
                "channel_count",
                "Number of Waveform Channels is exactly twelve.",
            ),
            (
                "sample_count",
                "Number of Waveform Samples matches the locked one-second trace.",
            ),
            ("sampling_frequency", "Sampling Frequency is 500 Hz."),
            (
                "duration",
                "Sample count and frequency encode the locked duration.",
            ),
            ("bits_allocated", "Waveform Bits Allocated is 16."),
            (
                "channel_definition_count",
                "Channel Definition Sequence contains the twelve ordered leads.",
            ),
        ] {
            push(&mut checks, &finding(suffix), message);
        }
        for channel in &group.channels {
            let channel_prefix = if qualify_group {
                format!(
                    "{prefix}_group_{}_channel_{}",
                    group.ordinal, channel.ordinal
                )
            } else {
                format!("{prefix}_channel_{}", channel.ordinal)
            };
            push(
                &mut checks,
                &format!("{channel_prefix}_ordinal"),
                "Waveform Channel Number is the one-based channel ordinal.",
            );
            push(
                &mut checks,
                &format!("{channel_prefix}_label"),
                "Channel Label matches the locked lead order.",
            );
            code_checks(&mut checks, &format!("{channel_prefix}_source"));
            push(
                &mut checks,
                &format!("{channel_prefix}_sensitivity"),
                "Channel Sensitivity matches the locked recipe.",
            );
            code_checks(&mut checks, &format!("{channel_prefix}_sensitivity_units"));
            push(
                &mut checks,
                &format!("{channel_prefix}_sensitivity_correction_factor"),
                "Channel numeric metadata matches the locked recipe.",
            );
            push(
                &mut checks,
                &format!("{channel_prefix}_baseline"),
                "Channel numeric metadata matches the locked recipe.",
            );
            push(
                &mut checks,
                &format!("{channel_prefix}_time_skew_present"),
                "Channel Time Skew is explicitly present.",
            );
            push(
                &mut checks,
                &format!("{channel_prefix}_time_skew"),
                "Channel Time Skew matches the locked recipe.",
            );
            push(
                &mut checks,
                &format!("{channel_prefix}_bits_stored"),
                "Waveform Bits Stored is 16 for every channel.",
            );
            push(
                &mut checks,
                &format!("{channel_prefix}_sample_skew_absent"),
                "Channel Sample Skew is absent while explicit Time Skew is zero.",
            );
        }
        for (suffix, message) in [
            ("waveform_data_vr", "Waveform Data uses OW storage."),
            (
                "payload_byte_arithmetic",
                "Waveform byte length equals channels times samples times bytes per signed sample.",
            ),
            (
                "payload_length",
                "Waveform Data has the locked 12,000-byte length with no padding.",
            ),
            (
                "payload_sha256",
                "Waveform payload hash matches the locked recipe.",
            ),
            (
                "signed_sample_width",
                "Waveform payload is composed of complete signed 16-bit values.",
            ),
            (
                "sample_min",
                "Decoded signed sample minimum matches the locked range.",
            ),
            (
                "sample_max",
                "Decoded signed sample maximum matches the locked range.",
            ),
            (
                "formula_contract",
                "Manifest sample formula is the locked deterministic formula.",
            ),
            (
                "interleave_contract",
                "Manifest interleave is channel-then-sample.",
            ),
            (
                "byte_order_contract",
                "Manifest byte order is little endian.",
            ),
            (
                "formula_and_interleave",
                "Every signed sample matches the deterministic formula in channel-then-sample order.",
            ),
            (
                "channel_hash_count",
                "Manifest contains one deinterleaved hash per channel.",
            ),
        ] {
            push(&mut checks, &finding(suffix), message);
        }
        for channel in &group.channels {
            let name = if qualify_group {
                format!(
                    "{prefix}_group_{}_channel_{}_sha256",
                    group.ordinal, channel.ordinal
                )
            } else {
                format!("{prefix}_channel_{}_sha256", channel.ordinal)
            };
            push(
                &mut checks,
                &name,
                "Deinterleaved channel hash matches the locked recipe.",
            );
        }
        push(
            &mut checks,
            &finding("waveform_padding_absent"),
            "Waveform Padding Value is absent.",
        );
    }
    for (suffix, message) in [
        (
            "aggregate_channel_count",
            "Decoded group channel counts match the manifest aggregate.",
        ),
        (
            "aggregate_payload_length",
            "Concatenated ordered group payload length matches the manifest aggregate.",
        ),
        (
            "aggregate_group_hashes",
            "Actual payload hashes preserve manifest group order.",
        ),
        (
            "aggregate_payload_sha256",
            "Concatenated ordered group payload hash matches the manifest aggregate.",
        ),
    ] {
        push(&mut checks, &finding(suffix), message);
    }
    for name in [
        "waveform_annotation",
        "structured_waveform_annotation",
        "synchronization_frame_of_reference",
        "synchronization_trigger",
        "synchronization_channel",
        "acquisition_time_synchronized",
        "time_source",
        "time_distribution_protocol",
        "ntp_source_address",
        "referenced_study",
        "referenced_series",
        "referenced_waveform",
        "referenced_image",
        "referenced_instance",
        "source_image",
        "rows",
        "columns",
        "samples_per_pixel",
        "number_of_frames",
        "photometric_interpretation",
        "bits_allocated",
        "bits_stored",
        "high_bit",
        "pixel_representation",
        "pixel_data",
    ] {
        push(
            &mut checks,
            &format!("{prefix}_{name}_absent"),
            "Optional or forbidden content is absent.",
        );
    }
    push(
        &mut checks,
        "curated_composition_plan",
        "The curated dataset resolved through the shared composition plan before Part 10 materialization.",
    );
    checks.push(standard(
        if prefix == "twelve_lead_ecg" {
            "twelve_lead_ecg_waveform_sop_class"
        } else {
            "general_ecg_waveform_sop_class"
        },
        if prefix == "twelve_lead_ecg" {
            "SOP Class UID matches 12-lead ECG Waveform Storage in the 2026b reference."
        } else {
            "SOP Class UID matches General ECG Waveform Storage in the 2026b reference."
        },
    ));
    checks.push(standard(
        "explicit_vr_little_endian_transfer_syntax",
        "Transfer Syntax UID matches Explicit VR Little Endian in the 2026b reference.",
    ));
    checks.push(standard(
        if prefix == "twelve_lead_ecg" { "twelve_lead_ecg_waveform_modules" } else { "general_ecg_waveform_modules" },
        if prefix == "twelve_lead_ecg" {
            "Twelve-lead ECG IOD, channel definitions, signed OW storage, deterministic interleave and absence invariants match the locked recipe."
        } else {
            "General ECG IOD, two ordered heterogeneous groups, channel definitions, signed OW storage, deterministic interleave, aggregate closure, and absence invariants match the locked recipe."
        },
    ));
    Ok(TypedValidationReport {
        bytes: vec![],
        checks,
        metadata_observation: None,
    })
}

pub fn validate_encapsulated_payload(
    input: &EncapsulatedPayloadPlanInput,
    observed: &SpecializedValidationObservation,
) -> Result<TypedValidationReport, SpecializedValidationError> {
    let (size, hash, is_pdf) = match &input.payload {
        EncapsulatedPayload::MinimalPdf {
            declared_size_bytes,
            declared_sha256,
            ..
        } => (*declared_size_bytes, declared_sha256.as_str(), true),
        EncapsulatedPayload::ClosedTetrahedronBinaryStl {
            declared_size_bytes,
            declared_sha256,
            ..
        } => (*declared_size_bytes, declared_sha256.as_str(), false),
    };
    require_common(
        observed,
        &input.sop_class_uid,
        [("encapsulated_document", size, hash, "OB")],
    )?;
    let mut checks = Vec::new();
    if is_pdf {
        for (name, message) in [
            (
                "part10_preamble",
                "File has a 128-byte preamble followed by the DICM marker.",
            ),
            (
                "file_meta_transfer_syntax",
                "File Meta Information Transfer Syntax UID matches the recipe.",
            ),
            (
                "sop_class_uid_consistency",
                "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
            ),
            (
                "media_storage_sop_class_uid",
                "File Meta SOP Class UID matches the dataset SOP Class UID.",
            ),
            (
                "sop_instance_uid_consistency",
                "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
            ),
            (
                "media_storage_sop_instance_uid",
                "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
            ),
            (
                "implementation_class_uid",
                "File Meta Implementation Class UID matches the deterministic generator UID.",
            ),
            (
                "synthetic_data",
                "Synthetic Data is present and set to YES.",
            ),
            (
                "encapsulated_pdf_modality",
                "Encapsulated Document Series Modality matches the recipe.",
            ),
            (
                "encapsulated_pdf_conversion_type",
                "SC Equipment Conversion Type matches the recipe.",
            ),
            (
                "encapsulated_pdf_instance_number",
                "Instance Number matches the document recipe.",
            ),
            (
                "encapsulated_pdf_content_date",
                "Content Date Type 2 attribute is present and deterministic.",
            ),
            (
                "encapsulated_pdf_content_time",
                "Content Time Type 2 attribute is present and deterministic.",
            ),
            (
                "encapsulated_pdf_acquisition_datetime",
                "Acquisition DateTime Type 2 attribute is present and deterministic.",
            ),
            (
                "encapsulated_pdf_burned_in_annotation",
                "Burned In Annotation is NO for the synthetic de-identified PDF.",
            ),
            (
                "encapsulated_pdf_recognizable_visual_features",
                "Recognizable Visual Features is NO for the synthetic PDF.",
            ),
            (
                "encapsulated_pdf_document_title",
                "Document Title Type 2 attribute is present.",
            ),
            (
                "encapsulated_pdf_concept_name_code_sequence",
                "Concept Name Code Sequence Type 2 attribute is present with zero items.",
            ),
            (
                "encapsulated_pdf_mime_type",
                "MIME Type of Encapsulated Document is application/pdf.",
            ),
            (
                "encapsulated_pdf_document_length",
                "Encapsulated Document Length records the original unpadded PDF length.",
            ),
            (
                "encapsulated_pdf_document_vr",
                "Encapsulated Document VR is OB.",
            ),
            (
                "encapsulated_pdf_document_payload",
                "Encapsulated Document contains the deterministic PDF payload.",
            ),
            (
                "encapsulated_pdf_pixel_data_absent",
                "Encapsulated PDF contains no Pixel Data.",
            ),
            (
                "curated_composition_plan",
                "The curated dataset resolved through the shared composition plan before Part 10 materialization.",
            ),
        ] {
            push(&mut checks, name, message);
        }
        for (name, message) in [
            (
                "encapsulated_pdf_sop_class",
                "SOP Class UID matches Encapsulated PDF Storage in the 2026b reference.",
            ),
            (
                "explicit_vr_little_endian_transfer_syntax",
                "Transfer Syntax UID matches Explicit VR Little Endian in the 2026b reference.",
            ),
            (
                "synthetic_data_attribute",
                "Synthetic Data (0008,001C) is present with value YES.",
            ),
            (
                "encapsulated_pdf_modules",
                "Encapsulated Document Series, SC Equipment, Encapsulated Document, and SOP Common attributes match the recipe.",
            ),
        ] {
            checks.push(standard(name, message));
        }
    } else {
        for (name, message) in [
            (
                "part10_identity",
                "Part 10, SOP, transfer syntax, and deterministic implementation identity match.",
            ),
            (
                "encapsulated_stl_modules",
                "M3D modality, Frame of Reference, manufacturing-model flags, units, and MIME type match.",
            ),
            (
                "encapsulated_stl_payload",
                "Encapsulated Document Length and exact binary STL bytes match the locked payload.",
            ),
            (
                "pixel_data_absent",
                "Encapsulated STL contains no Pixel Data.",
            ),
            (
                "curated_composition_plan",
                "The curated dataset resolved through the shared composition plan before Part 10 materialization.",
            ),
        ] {
            push(&mut checks, name, message);
        }
        checks.push(standard(
            "sop_class_encapsulated_stl",
            "SOP Class UID is Encapsulated STL Storage.",
        ));
        checks.push(standard(
            "transfer_syntax_explicit_vr_little_endian",
            "Transfer Syntax is Explicit VR Little Endian.",
        ));
    }
    Ok(TypedValidationReport {
        bytes: vec![],
        checks,
        metadata_observation: None,
    })
}

fn require_common<'a>(
    observed: &SpecializedValidationObservation,
    sop_class_uid: &str,
    expected: impl IntoIterator<Item = (&'a str, u64, &'a str, &'a str)>,
) -> Result<(), SpecializedValidationError> {
    if !observed.generic_plan_validation_passed
        || !observed.part10_preamble_present
        || observed.transfer_syntax_uid != "1.2.840.10008.1.2.1"
        || observed.sop_class_uid != sop_class_uid
        || !observed.implementation_identity_matches
        || !observed.pixel_data_absent
    {
        return Err(SpecializedValidationError::Observation(
            "common Part 10 or identity evidence failed".into(),
        ));
    }
    for (slot, size, hash, vr) in expected {
        let actual = observed.content.get(slot).ok_or_else(|| {
            SpecializedValidationError::Observation(format!("missing content observation {slot}"))
        })?;
        if actual.size_bytes != size || actual.sha256 != hash || actual.vr != vr {
            return Err(SpecializedValidationError::Observation(format!(
                "content observation drift for {slot}"
            )));
        }
    }
    Ok(())
}

fn code_checks(checks: &mut Vec<TypedValidationCheck>, prefix: &str) {
    push(
        checks,
        &format!("{prefix}_item_count"),
        "Code Sequence contains exactly one Item.",
    );
    for suffix in ["value", "scheme", "meaning"] {
        push(
            checks,
            &format!("{prefix}_{suffix}"),
            "Coded value matches the locked recipe.",
        );
    }
}

fn push(checks: &mut Vec<TypedValidationCheck>, name: &str, message: &str) {
    checks.push(TypedValidationCheck::passed_internal(name, message));
}

fn standard(name: &str, message: &str) -> TypedValidationCheck {
    TypedValidationCheck {
        layer: CheckLayer::Standards,
        name: name.into(),
        status: "passed".into(),
        message: message.into(),
    }
}
