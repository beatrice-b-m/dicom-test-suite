//! Shared caller-owned planning context for U6 typed-bulk providers.

use serde::{Deserialize, Serialize};

use crate::composition::IdentityPlan;
use crate::corpus_plan::{OutputPlan, PlannedDicomArtifact};
use crate::executor::services::ArtifactExecutionBindings;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedBulkPlanningContext {
    pub recipe_artifact_logical_id: String,
    pub target_instance_id: String,
    pub order: u64,
    pub output: OutputPlan,
    pub identities: IdentityPlan,
}

impl TypedBulkPlanningContext {
    pub fn validate(&self, expected_recipe_artifact: &str) -> Result<(), String> {
        if self.recipe_artifact_logical_id != expected_recipe_artifact {
            return Err("typed-bulk context targets the wrong recipe artifact".into());
        }
        if self.target_instance_id.is_empty()
            || self.identities.logical_instance_id != self.target_instance_id
        {
            return Err("typed-bulk context identity ownership is invalid".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedBulkPlanProviderOutput {
    pub artifact: PlannedDicomArtifact,
    pub bindings: ArtifactExecutionBindings,
}
