use crate::planning::RecipeIdentity;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub type Parameters = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseBinding {
    pub case_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeReference {
    pub recipe_id: String,
    pub recipe_version: String,
}
impl RecipeReference {
    pub fn identity(&self) -> RecipeIdentity {
        RecipeIdentity {
            recipe_id: self.recipe_id.clone(),
            recipe_version: self.recipe_version.clone(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyBinding {
    pub recipe: RecipeReference,
    pub role: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateReference {
    pub template_id: String,
    pub template_version: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputBinding {
    pub role: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub provider_derived: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncodingPolicy {
    pub transfer_syntax_uid: String,
    pub sequence_length_policy: String,
    pub item_length_policy: String,
    pub offset_table_policy: String,
    pub fragmentation_policy: String,
    #[serde(default)]
    pub preamble_policy: Option<String>,
    #[serde(default)]
    pub file_meta_policy: Option<String>,
    #[serde(default)]
    pub non_template_encoding_provider_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PixelPaddingParameters {
    pub value: i64,
    #[serde(default)]
    pub range_limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteParameters {
    pub descriptor: [u32; 3],
    pub red: Vec<u16>,
    pub green: Vec<u16>,
    pub blue: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorParameters {
    #[serde(default)]
    pub planar_configuration: Option<u8>,
    pub chroma_subsampling: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BitPackingParameters {
    pub bit_order: String,
    pub frame_boundary_policy: String,
    pub significant_bits: u64,
    pub significant_packed_bytes: u64,
    pub unused_high_bits: u8,
    pub value_field_padding_bytes: u64,
    pub frame_start_bit_offsets: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegerWordParameters {
    pub byte_order: String,
    pub covers_full_unsigned_range: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncapsulationProjectionParameters {
    pub offset_origin: String,
    pub item_header_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonsquareGeometryParameters {
    pub variant_id: String,
    #[serde(default)]
    pub pixel_spacing: Option<[String; 2]>,
    #[serde(default)]
    pub nominal_scanned_pixel_spacing: Option<[String; 2]>,
    #[serde(default)]
    pub pixel_aspect_ratio: Option<[u32; 2]>,
    pub row_to_column_ratio: f64,
    pub calibrated: bool,
    pub patient_space_geometry_present: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecondaryCaptureParameters {
    pub rows: u32,
    pub columns: u32,
    pub frames: u32,
    pub samples_per_pixel: u16,
    pub photometric_interpretation: String,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    pub pixel_representation: u16,
    pub pixel_data_vr: String,
    pub stored_value_type: String,
    pub stored_values: Vec<i64>,
    pub frame_sha256: Vec<String>,
    pub visual_pattern: String,
    pub semantic_note: String,
    pub pixel_min: i64,
    pub pixel_max: i64,
    #[serde(default)]
    pub padding: Option<PixelPaddingParameters>,
    #[serde(default)]
    pub palette: Option<PaletteParameters>,
    #[serde(default)]
    pub color: Option<ColorParameters>,
    #[serde(default)]
    pub bit_packing: Option<BitPackingParameters>,
    #[serde(default)]
    pub integer_word: Option<IntegerWordParameters>,
    #[serde(default)]
    pub encapsulation_projection: Option<EncapsulationProjectionParameters>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonNameComponentGroup {
    pub kind: String,
    pub decoded_value: String,
    pub components: [String; 5],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonNameMetadata {
    pub specific_character_sets: Vec<String>,
    pub patient_name_decoded: String,
    pub patient_name_raw_hex: String,
    pub patient_name_raw_sha256: String,
    pub native_unicode_round_trip: bool,
    pub component_groups: Vec<PersonNameComponentGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimezoneBoundaryMetadata {
    pub boundary_id: String,
    pub study_date: String,
    pub study_time: String,
    pub acquisition_date_time: String,
    pub timezone_offset: String,
    pub offset_minutes: i16,
    pub normalized_utc: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyType2AttributeMetadata {
    pub tag: String,
    pub keyword: String,
    pub vr: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source_kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StringValueSource {
    Repeated { pattern: String, repetitions: u32 },
    Literal { values: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StringBoundaryElementMetadata {
    pub tag: String,
    pub keyword: String,
    pub vr: String,
    pub source: StringValueSource,
    pub padding: String,
    pub raw_value_byte_length: u32,
    pub raw_value_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "value_kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrivateElementValue {
    Lo { text: String },
    Us { number: u16 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateElementMetadata {
    pub tag: String,
    pub value: PrivateElementValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateCreatorBlockMetadata {
    pub creator_tag: String,
    pub creator_id: String,
    pub block_start_tag: String,
    pub block_end_tag: String,
    pub elements: Vec<PrivateElementMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceLengthMetadata {
    pub variant_id: String,
    pub sequence_tag: String,
    pub sequence_vr: String,
    pub code_value: String,
    pub coding_scheme_designator: String,
    pub code_meaning: String,
    pub item_dataset_encoded_length: u32,
    pub undefined_item_encoded_length: u32,
    pub sequence_length_field_hex: String,
    pub item_length_field_hex: String,
    pub item_delimitation_present: bool,
    pub sequence_delimitation_present: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MetadataScParameters {
    PersonName(PersonNameMetadata),
    TimezoneBoundary(TimezoneBoundaryMetadata),
    EmptyType2 {
        attributes: Vec<EmptyType2AttributeMetadata>,
    },
    StringBoundaries {
        elements: Vec<StringBoundaryElementMetadata>,
    },
    PrivateCreators {
        blocks: Vec<PrivateCreatorBlockMetadata>,
    },
    SequenceLengths(SequenceLengthMetadata),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeOperation {
    pub operation: String,
    pub tag: String,
    #[serde(default)]
    pub vr: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBinding {
    pub provider_id: String,
    pub parameters: Parameters,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassicProjectionFamily {
    Ct,
    DxMg,
    MrCr,
    Nuclear,
    VlProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicSemanticLabels {
    #[serde(default)]
    pub photometric_semantics: Option<String>,
    #[serde(default)]
    pub overlay_pattern: Option<String>,
    #[serde(default)]
    pub modality_lut: Option<String>,
    #[serde(default)]
    pub voi_lut: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicProjection {
    pub family: ClassicProjectionFamily,
    pub expected_capabilities: Vec<String>,
    pub visual_pattern: String,
    pub include_implementation_version_name: bool,
    #[serde(default)]
    pub semantic_labels: Option<ClassicSemanticLabels>,
    #[serde(default)]
    pub standards_evidence_append: Vec<ClassicStandardEvidence>,
    #[serde(default)]
    pub mr: Option<ClassicMrProjection>,
    #[serde(default)]
    pub icc: Option<ClassicIccProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicIccProjection {
    pub tag: String,
    pub vr: String,
    pub profile_signature: String,
    pub device_class: String,
    pub data_color_space: String,
    pub profile_connection_space: String,
    pub profile_version: String,
    pub rendering_intent: String,
    pub rendering_intent_code: u32,
    pub profile_description: String,
    pub copyright: String,
    pub tag_count: u32,
    pub source_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicMrProjection {
    pub scanning_sequence: String,
    pub sequence_variant: String,
    pub scan_options: String,
    pub mr_acquisition_type: String,
    pub repetition_time: String,
    pub echo_time: String,
    pub echo_train_length: String,
    pub magnetic_field_strength: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicStandardEvidence {
    pub source: String,
    pub edition: String,
    pub query: String,
    pub covered: bool,
    pub part: String,
    pub anchor: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedArtifactRecipe {
    pub logical_id: String,
    pub order: u32,
    /// Artifact-level public manifest membership. When absent, projection
    /// inherits the authoritative registry case profiles unchanged.
    #[serde(default)]
    pub public_profile_membership: Option<Vec<String>>,
    #[serde(default)]
    pub template: Option<TemplateReference>,
    pub output: OutputBinding,
    pub encoding: EncodingPolicy,
    pub parameters: Parameters,
    #[serde(default)]
    pub secondary_capture: Option<SecondaryCaptureParameters>,
    #[serde(default)]
    pub metadata_sc: Option<MetadataScParameters>,
    #[serde(default)]
    pub nonsquare_geometry: Option<NonsquareGeometryParameters>,
    #[serde(default)]
    pub classic_projection: Option<ClassicProjection>,
    pub attribute_operations: Vec<AttributeOperation>,
    pub content: ContentBinding,
    pub validation_rule_ids: Vec<String>,
    pub projection_rule_ids: Vec<String>,
    pub determinism: String,
    pub stressors: Vec<String>,
    #[serde(default)]
    pub algorithm_provider_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DicomRecipe {
    pub artifacts: Vec<PlannedArtifactRecipe>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationEdit {
    pub edit_id: String,
    pub mutation_id: String,
    pub parameters: Parameters,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationRecipe {
    pub source: RecipeReference,
    pub source_logical_role: String,
    pub edits: Vec<MutationEdit>,
    pub failure_layers: Vec<String>,
    pub acceptable_outcomes: Vec<String>,
    pub output: OutputBinding,
    pub retention: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePolicy {
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub max_operations: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualificationRecipe {
    pub parameters: Parameters,
    pub resource_policy: ResourcePolicy,
    pub retention: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeKind {
    Dicom,
    Mutation,
    Qualification,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseRecipe {
    pub case_recipe_schema_version: String,
    pub recipe_id: String,
    pub recipe_version: String,
    pub binding: CaseBinding,
    pub kind: RecipeKind,
    pub plan_provider_id: String,
    #[serde(default)]
    pub planning_order: Option<u32>,
    pub provider_parameters: Parameters,
    pub dependencies: Vec<DependencyBinding>,
    pub validation_rule_ids: Vec<String>,
    pub projection_rule_ids: Vec<String>,
    #[serde(default)]
    pub dicom: Option<DicomRecipe>,
    #[serde(default)]
    pub mutation: Option<MutationRecipe>,
    #[serde(default)]
    pub qualification: Option<QualificationRecipe>,
}
impl CaseRecipe {
    pub fn identity(&self) -> RecipeIdentity {
        RecipeIdentity {
            recipe_id: self.recipe_id.clone(),
            recipe_version: self.recipe_version.clone(),
        }
    }
}
