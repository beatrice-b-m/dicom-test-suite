use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use super::{CompositionUidRole, IdentityPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CyclePolicy {
    Forbidden,
    AllowedWithinBundle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceNode {
    pub instance_id: String,
    pub bundle_id: String,
    pub sop_class_uid: String,
    pub frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalReference {
    pub source_instance_id: String,
    pub target_instance_id: String,
    pub role: String,
    pub frame_role: Option<String>,
    pub frames: Vec<u32>,
    pub cycle_policy: CyclePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterializedReference {
    pub source_instance_id: String,
    pub target_instance_id: String,
    pub role: String,
    pub frame_role: Option<String>,
    pub referenced_sop_class_uid: String,
    pub referenced_sop_instance_uid: String,
    pub referenced_frames: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceGraph {
    nodes: BTreeMap<String, ReferenceNode>,
    edges: Vec<LogicalReference>,
}

impl ReferenceGraph {
    pub fn new(
        nodes: impl IntoIterator<Item = ReferenceNode>,
        mut edges: Vec<LogicalReference>,
    ) -> Result<Self, ReferenceError> {
        let mut indexed_nodes = BTreeMap::new();
        for node in nodes {
            if node.frames == 0 {
                return Err(ReferenceError::ZeroFrames(node.instance_id));
            }
            let node_id = node.instance_id.clone();
            if indexed_nodes.insert(node_id.clone(), node).is_some() {
                return Err(ReferenceError::DuplicateNode(node_id));
            }
        }
        edges.sort_by(|left, right| {
            (
                &left.source_instance_id,
                &left.role,
                &left.target_instance_id,
                &left.frames,
            )
                .cmp(&(
                    &right.source_instance_id,
                    &right.role,
                    &right.target_instance_id,
                    &right.frames,
                ))
        });
        let mut identities = BTreeSet::new();
        for edge in &edges {
            let Some(source) = indexed_nodes.get(&edge.source_instance_id) else {
                return Err(ReferenceError::UnknownNode(edge.source_instance_id.clone()));
            };
            let Some(target) = indexed_nodes.get(&edge.target_instance_id) else {
                return Err(ReferenceError::UnknownNode(edge.target_instance_id.clone()));
            };
            if edge.role.is_empty() || edge.frame_role.as_deref() == Some("") {
                return Err(ReferenceError::InvalidRole(edge.role.clone()));
            }
            let identity = (
                &edge.source_instance_id,
                &edge.target_instance_id,
                &edge.role,
                &edge.frame_role,
                &edge.frames,
            );
            if !identities.insert(identity) {
                return Err(ReferenceError::DuplicateReference {
                    source: edge.source_instance_id.clone(),
                    target: edge.target_instance_id.clone(),
                    role: edge.role.clone(),
                });
            }
            let mut unique_frames = BTreeSet::new();
            for frame in &edge.frames {
                if *frame == 0 || *frame > target.frames {
                    return Err(ReferenceError::FrameOutOfRange {
                        target: target.instance_id.clone(),
                        frame: *frame,
                        frames: target.frames,
                    });
                }
                if !unique_frames.insert(*frame) {
                    return Err(ReferenceError::DuplicateFrame {
                        target: target.instance_id.clone(),
                        frame: *frame,
                    });
                }
            }
            if edge.source_instance_id == edge.target_instance_id
                && (edge.cycle_policy == CyclePolicy::Forbidden
                    || source.bundle_id != target.bundle_id)
            {
                return Err(ReferenceError::ForbiddenCycle(vec![
                    edge.source_instance_id.clone(),
                    edge.target_instance_id.clone(),
                ]));
            }
        }
        let graph = Self {
            nodes: indexed_nodes,
            edges,
        };
        graph.validate_cycles()?;
        Ok(graph)
    }

    pub fn dependency_closure(
        &self,
        roots: impl IntoIterator<Item = String>,
    ) -> Result<Vec<String>, ReferenceError> {
        let mut pending = roots.into_iter().collect::<BTreeSet<_>>();
        for root in &pending {
            if !self.nodes.contains_key(root) {
                return Err(ReferenceError::UnknownNode(root.clone()));
            }
        }
        let mut closure = BTreeSet::new();
        while let Some(next) = pending.pop_first() {
            if !closure.insert(next.clone()) {
                continue;
            }
            for edge in self
                .edges
                .iter()
                .filter(|edge| edge.source_instance_id == next)
            {
                if !closure.contains(&edge.target_instance_id) {
                    pending.insert(edge.target_instance_id.clone());
                }
            }
        }
        Ok(closure.into_iter().collect())
    }

    pub fn materialize(
        &self,
        identities: &BTreeMap<String, IdentityPlan>,
    ) -> Result<Vec<MaterializedReference>, ReferenceError> {
        let mut materialized = Vec::with_capacity(self.edges.len());
        for edge in &self.edges {
            let target = self
                .nodes
                .get(&edge.target_instance_id)
                .expect("constructor checked target");
            let target_plan = identities.get(&edge.target_instance_id).ok_or_else(|| {
                ReferenceError::MissingIdentityPlan(edge.target_instance_id.clone())
            })?;
            let referenced_sop_instance_uid = target_plan
                .get(&CompositionUidRole::SopInstance, 0)
                .ok_or_else(|| {
                    ReferenceError::MissingSopIdentity(edge.target_instance_id.clone())
                })?;
            materialized.push(MaterializedReference {
                source_instance_id: edge.source_instance_id.clone(),
                target_instance_id: edge.target_instance_id.clone(),
                role: edge.role.clone(),
                frame_role: edge.frame_role.clone(),
                referenced_sop_class_uid: target.sop_class_uid.clone(),
                referenced_sop_instance_uid: referenced_sop_instance_uid.to_string(),
                referenced_frames: edge.frames.clone(),
            });
        }
        Ok(materialized)
    }

    fn validate_cycles(&self) -> Result<(), ReferenceError> {
        let mut visited = BTreeSet::new();
        let mut stack = Vec::new();
        let mut in_stack = BTreeSet::new();
        for node in self.nodes.keys() {
            self.visit(node, &mut visited, &mut stack, &mut in_stack)?;
        }
        Ok(())
    }

    fn visit(
        &self,
        node: &str,
        visited: &mut BTreeSet<String>,
        stack: &mut Vec<String>,
        in_stack: &mut BTreeSet<String>,
    ) -> Result<(), ReferenceError> {
        if visited.contains(node) {
            return Ok(());
        }
        stack.push(node.to_string());
        in_stack.insert(node.to_string());
        for edge in self
            .edges
            .iter()
            .filter(|edge| edge.source_instance_id == node)
        {
            if in_stack.contains(&edge.target_instance_id) {
                let start = stack
                    .iter()
                    .position(|entry| entry == &edge.target_instance_id)
                    .expect("target in stack");
                let mut cycle = stack[start..].to_vec();
                cycle.push(edge.target_instance_id.clone());
                if !self.cycle_is_allowed(&cycle) {
                    return Err(ReferenceError::ForbiddenCycle(cycle));
                }
            } else if !visited.contains(&edge.target_instance_id) {
                self.visit(&edge.target_instance_id, visited, stack, in_stack)?;
            }
        }
        stack.pop();
        in_stack.remove(node);
        visited.insert(node.to_string());
        Ok(())
    }

    fn cycle_is_allowed(&self, cycle: &[String]) -> bool {
        let Some(first) = cycle.first().and_then(|id| self.nodes.get(id)) else {
            return false;
        };
        cycle.windows(2).all(|pair| {
            let same_bundle = self
                .nodes
                .get(&pair[1])
                .is_some_and(|node| node.bundle_id == first.bundle_id);
            same_bundle
                && self.edges.iter().any(|edge| {
                    edge.source_instance_id == pair[0]
                        && edge.target_instance_id == pair[1]
                        && edge.cycle_policy == CyclePolicy::AllowedWithinBundle
                })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceError {
    DuplicateNode(String),
    UnknownNode(String),
    ZeroFrames(String),
    InvalidRole(String),
    DuplicateReference {
        source: String,
        target: String,
        role: String,
    },
    FrameOutOfRange {
        target: String,
        frame: u32,
        frames: u32,
    },
    DuplicateFrame {
        target: String,
        frame: u32,
    },
    ForbiddenCycle(Vec<String>),
    MissingIdentityPlan(String),
    MissingSopIdentity(String),
}

impl fmt::Display for ReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNode(node) => write!(formatter, "duplicate reference node {node}"),
            Self::UnknownNode(node) => write!(formatter, "unknown reference node {node}"),
            Self::ZeroFrames(node) => {
                write!(formatter, "reference node {node} declares zero frames")
            }
            Self::InvalidRole(role) => write!(formatter, "invalid reference role {role:?}"),
            Self::DuplicateReference {
                source,
                target,
                role,
            } => {
                write!(
                    formatter,
                    "duplicate {role} reference from {source} to {target}"
                )
            }
            Self::FrameOutOfRange {
                target,
                frame,
                frames,
            } => write!(
                formatter,
                "reference frame {frame} exceeds {target} frame count {frames}"
            ),
            Self::DuplicateFrame { target, frame } => {
                write!(formatter, "reference to {target} repeats frame {frame}")
            }
            Self::ForbiddenCycle(cycle) => write!(
                formatter,
                "forbidden reference cycle: {}",
                cycle.join(" -> ")
            ),
            Self::MissingIdentityPlan(node) => {
                write!(formatter, "missing identity plan for {node}")
            }
            Self::MissingSopIdentity(node) => {
                write!(formatter, "missing SOP Instance UID for {node}")
            }
        }
    }
}

impl std::error::Error for ReferenceError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{IdentityAllocator, TemplateId};

    const LOCK_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn node(id: &str, bundle: &str, frames: u32) -> ReferenceNode {
        ReferenceNode {
            instance_id: id.into(),
            bundle_id: bundle.into(),
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".into(),
            frames,
        }
    }

    fn edge(source: &str, target: &str, role: &str) -> LogicalReference {
        LogicalReference {
            source_instance_id: source.into(),
            target_instance_id: target.into(),
            role: role.into(),
            frame_role: None,
            frames: vec![],
            cycle_policy: CyclePolicy::Forbidden,
        }
    }

    fn identities(ids: &[&str]) -> BTreeMap<String, IdentityPlan> {
        let allocator = IdentityAllocator::new(
            LOCK_HASH,
            TemplateId("classic/secondary-capture/monochrome".into()),
            "1.0.0".parse().unwrap(),
            1,
        )
        .unwrap();
        ids.iter()
            .map(|id| {
                (
                    (*id).to_string(),
                    allocator
                        .allocate_plan(*id, [(CompositionUidRole::SopInstance, 0)])
                        .unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn closure_is_sorted_and_follows_dependencies() {
        let graph = ReferenceGraph::new(
            [
                node("derived", "bundle", 1),
                node("source", "bundle", 2),
                node("other", "other", 1),
            ],
            vec![edge("derived", "source", "source_image")],
        )
        .unwrap();
        assert_eq!(
            graph.dependency_closure(["derived".into()]).unwrap(),
            vec!["derived", "source"]
        );
    }

    #[test]
    fn frames_are_one_based_unique_and_bounded() {
        let mut reference = edge("derived", "source", "source_image");
        reference.frames = vec![1, 3];
        assert!(matches!(
            ReferenceGraph::new(
                [node("derived", "bundle", 1), node("source", "bundle", 2)],
                vec![reference]
            ),
            Err(ReferenceError::FrameOutOfRange { .. })
        ));
    }

    #[test]
    fn cycles_require_explicit_policy_on_every_same_bundle_edge() {
        let mut first = edge("a", "b", "peer");
        let mut second = edge("b", "a", "peer");
        assert!(matches!(
            ReferenceGraph::new(
                [node("a", "bundle", 1), node("b", "bundle", 1)],
                vec![first.clone(), second.clone()]
            ),
            Err(ReferenceError::ForbiddenCycle(_))
        ));
        first.cycle_policy = CyclePolicy::AllowedWithinBundle;
        second.cycle_policy = CyclePolicy::AllowedWithinBundle;
        assert!(
            ReferenceGraph::new(
                [node("a", "bundle", 1), node("b", "bundle", 1)],
                vec![first, second]
            )
            .is_ok()
        );
    }

    #[test]
    fn materialization_closes_logical_and_uid_identity() {
        let mut reference = edge("derived", "source", "source_image");
        reference.frame_role = Some("referenced_frame_number".into());
        reference.frames = vec![2];
        let graph = ReferenceGraph::new(
            [node("derived", "bundle", 1), node("source", "bundle", 2)],
            vec![reference],
        )
        .unwrap();
        let materialized = graph
            .materialize(&identities(&["derived", "source"]))
            .unwrap();
        assert_eq!(materialized.len(), 1);
        assert_eq!(materialized[0].target_instance_id, "source");
        assert_eq!(materialized[0].referenced_frames, vec![2]);
        assert!(
            materialized[0]
                .referenced_sop_instance_uid
                .starts_with("2.25.")
        );
    }
}
