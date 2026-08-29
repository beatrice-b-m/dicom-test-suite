use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use super::{AttributeAddress, AttributeOperation};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionalGroupItemPlan {
    pub operations: Vec<AttributeOperation>,
}

impl FunctionalGroupItemPlan {
    fn validate(&self, scope: &str) -> Result<(), EnhancedPlanError> {
        let mut addresses = BTreeSet::new();
        for operation in &self.operations {
            operation
                .validate()
                .map_err(|error| EnhancedPlanError::Attribute {
                    scope: scope.to_string(),
                    message: error.to_string(),
                })?;
            if !addresses.insert(operation.address().clone()) {
                return Err(EnhancedPlanError::DuplicateAttribute {
                    scope: scope.to_string(),
                    address: operation.address().normalized_tag(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DimensionOrganization {
    pub organization_uid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DimensionIndex {
    pub organization_uid: String,
    pub dimension_index_pointer: AttributeAddress,
    pub functional_group_pointer: AttributeAddress,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DimensionOrganizationPlan {
    pub organizations: Vec<DimensionOrganization>,
    pub indices: Vec<DimensionIndex>,
}

impl DimensionOrganizationPlan {
    fn validate(&self) -> Result<(), EnhancedPlanError> {
        if self.organizations.is_empty() || self.indices.is_empty() {
            return Err(EnhancedPlanError::EmptyDimensions);
        }
        let organizations = self
            .organizations
            .iter()
            .map(|organization| organization.organization_uid.as_str())
            .collect::<BTreeSet<_>>();
        if organizations.len() != self.organizations.len()
            || organizations.iter().any(|uid| uid.is_empty())
        {
            return Err(EnhancedPlanError::DuplicateOrEmptyOrganization);
        }
        let mut pointers = BTreeSet::new();
        for index in &self.indices {
            if !organizations.contains(index.organization_uid.as_str()) {
                return Err(EnhancedPlanError::UnknownOrganization(
                    index.organization_uid.clone(),
                ));
            }
            let identity = (
                index.organization_uid.as_str(),
                index.dimension_index_pointer.clone(),
                index.functional_group_pointer.clone(),
            );
            if !pointers.insert(identity) {
                return Err(EnhancedPlanError::DuplicateDimensionIndex);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemporalFramePlan {
    pub temporal_position_index: u32,
    pub acquisition_offset_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConcatenationPlan {
    pub concatenation_uid: String,
    pub in_concatenation_number: u32,
    pub concatenation_frame_offset_number: u32,
    pub total_number_of_instances: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerFrameFunctionalGroupPlan {
    pub frame_number: u32,
    pub content_frame_index: u32,
    pub dimension_index_values: Vec<u32>,
    pub temporal: Option<TemporalFramePlan>,
    pub functional_groups: FunctionalGroupItemPlan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedMultiframePlan {
    pub number_of_frames: u32,
    pub shared_functional_groups: FunctionalGroupItemPlan,
    pub per_frame_functional_groups: Vec<PerFrameFunctionalGroupPlan>,
    pub dimensions: DimensionOrganizationPlan,
    pub concatenation: Option<ConcatenationPlan>,
}

impl EnhancedMultiframePlan {
    pub fn validate(&self) -> Result<(), EnhancedPlanError> {
        if self.number_of_frames == 0 {
            return Err(EnhancedPlanError::ZeroFrames);
        }
        if self.per_frame_functional_groups.len() != self.number_of_frames as usize {
            return Err(EnhancedPlanError::PerFrameCardinality {
                expected: self.number_of_frames,
                actual: self.per_frame_functional_groups.len(),
            });
        }
        self.shared_functional_groups.validate("shared")?;
        self.dimensions.validate()?;

        let mut content_indices = BTreeSet::new();
        let mut temporal_offsets = BTreeMap::<u32, f64>::new();
        for (index, frame) in self.per_frame_functional_groups.iter().enumerate() {
            let expected_frame = index as u32 + 1;
            if frame.frame_number != expected_frame {
                return Err(EnhancedPlanError::FrameOrder {
                    expected: expected_frame,
                    actual: frame.frame_number,
                });
            }
            if frame.dimension_index_values.len() != self.dimensions.indices.len() {
                return Err(EnhancedPlanError::DimensionValueCardinality {
                    frame: frame.frame_number,
                    expected: self.dimensions.indices.len(),
                    actual: frame.dimension_index_values.len(),
                });
            }
            if frame.dimension_index_values.contains(&0) {
                return Err(EnhancedPlanError::ZeroDimensionValue(frame.frame_number));
            }
            if !content_indices.insert(frame.content_frame_index)
                || frame.content_frame_index == 0
                || frame.content_frame_index > self.number_of_frames
            {
                return Err(EnhancedPlanError::InvalidContentFrameIndex {
                    frame: frame.frame_number,
                    index: frame.content_frame_index,
                });
            }
            if let Some(temporal) = &frame.temporal {
                if temporal.temporal_position_index == 0
                    || !temporal.acquisition_offset_seconds.is_finite()
                {
                    return Err(EnhancedPlanError::InvalidTemporalPosition(
                        frame.frame_number,
                    ));
                }
                let previous = temporal_offsets.insert(
                    temporal.temporal_position_index,
                    temporal.acquisition_offset_seconds,
                );
                if let Some(previous) = previous {
                    if previous != temporal.acquisition_offset_seconds {
                        return Err(EnhancedPlanError::InconsistentTemporalOffset(
                            temporal.temporal_position_index,
                        ));
                    }
                }
            }
            frame
                .functional_groups
                .validate(&format!("per-frame {}", frame.frame_number))?;
        }
        if let Some(concatenation) = &self.concatenation {
            if concatenation.concatenation_uid.is_empty()
                || concatenation.in_concatenation_number == 0
            {
                return Err(EnhancedPlanError::InvalidConcatenationIdentity);
            }
            let end = concatenation
                .concatenation_frame_offset_number
                .checked_add(self.number_of_frames)
                .ok_or(EnhancedPlanError::ConcatenationFrameOverflow)?;
            if end == 0 {
                return Err(EnhancedPlanError::ConcatenationFrameOverflow);
            }
            if concatenation
                .total_number_of_instances
                .is_some_and(|total| total < concatenation.in_concatenation_number)
            {
                return Err(EnhancedPlanError::ConcatenationInstanceRange);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnhancedPlanError {
    ZeroFrames,
    PerFrameCardinality {
        expected: u32,
        actual: usize,
    },
    Attribute {
        scope: String,
        message: String,
    },
    DuplicateAttribute {
        scope: String,
        address: String,
    },
    EmptyDimensions,
    DuplicateOrEmptyOrganization,
    UnknownOrganization(String),
    DuplicateDimensionIndex,
    FrameOrder {
        expected: u32,
        actual: u32,
    },
    DimensionValueCardinality {
        frame: u32,
        expected: usize,
        actual: usize,
    },
    ZeroDimensionValue(u32),
    InvalidContentFrameIndex {
        frame: u32,
        index: u32,
    },
    InvalidTemporalPosition(u32),
    InconsistentTemporalOffset(u32),
    InvalidConcatenationIdentity,
    ConcatenationFrameOverflow,
    ConcatenationInstanceRange,
}

impl fmt::Display for EnhancedPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid enhanced multi-frame plan: {self:?}")
    }
}

impl std::error::Error for EnhancedPlanError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::AttributeAddress;

    fn plan() -> EnhancedMultiframePlan {
        EnhancedMultiframePlan {
            number_of_frames: 2,
            shared_functional_groups: FunctionalGroupItemPlan { operations: vec![] },
            per_frame_functional_groups: (1..=2)
                .map(|frame| PerFrameFunctionalGroupPlan {
                    frame_number: frame,
                    content_frame_index: frame,
                    dimension_index_values: vec![frame],
                    temporal: Some(TemporalFramePlan {
                        temporal_position_index: frame,
                        acquisition_offset_seconds: f64::from(frame - 1),
                    }),
                    functional_groups: FunctionalGroupItemPlan { operations: vec![] },
                })
                .collect(),
            dimensions: DimensionOrganizationPlan {
                organizations: vec![DimensionOrganization {
                    organization_uid: "2.25.1".into(),
                }],
                indices: vec![DimensionIndex {
                    organization_uid: "2.25.1".into(),
                    dimension_index_pointer: AttributeAddress::from_normalized_tag("0020,9057")
                        .unwrap(),
                    functional_group_pointer: AttributeAddress::from_normalized_tag("0020,9111")
                        .unwrap(),
                    label: Some("InStackPositionNumber".into()),
                }],
            },
            concatenation: None,
        }
    }

    #[test]
    fn validates_consistent_functional_groups_and_dimensions() {
        plan().validate().unwrap();
    }

    #[test]
    fn rejects_frame_and_dimension_cardinality_mismatches() {
        let mut invalid = plan();
        invalid.per_frame_functional_groups[1].frame_number = 3;
        assert!(matches!(
            invalid.validate(),
            Err(EnhancedPlanError::FrameOrder { .. })
        ));
        let mut invalid = plan();
        invalid.per_frame_functional_groups[0]
            .dimension_index_values
            .clear();
        assert!(matches!(
            invalid.validate(),
            Err(EnhancedPlanError::DimensionValueCardinality { .. })
        ));
    }
}
