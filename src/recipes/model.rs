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
    pub non_template_encoding_provider_id: Option<String>,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedArtifactRecipe {
    pub logical_id: String,
    pub order: u32,
    #[serde(default)]
    pub template: Option<TemplateReference>,
    pub output: OutputBinding,
    pub encoding: EncodingPolicy,
    pub parameters: Parameters,
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
