//! Shared, registry-independent DICOM composition primitives.
//!
//! This module is intentionally not wired to a public CLI until the P2 gate.

pub mod attribute;
pub mod identity;
pub mod plan;
pub mod reference;
pub mod template;

pub use attribute::{
    AttributeAddress, AttributeError, AttributeItem, AttributeOperation, AttributeValue, DicomVr,
    PrimitiveValue,
};
pub use identity::{CompositionUidRole, IdentityAllocator, IdentityError, IdentityPlan};
pub use plan::{
    AttributeLayer, AttributePolicy, AttributeResolver, CanonicalContent, Condition,
    ResolveContext, ResolveError, ResolvedAttribute, ResolvedInstancePlan, ValueOrigin,
};
pub use reference::{
    CyclePolicy, LogicalReference, MaterializedReference, ReferenceError, ReferenceGraph,
    ReferenceNode,
};
pub use template::{
    CapabilitySet, RequirementGap, TemplateCatalog, TemplateDescriptor, TemplateError, TemplateId,
    TemplateStatus, TemplateVersion, TransferSyntaxDescriptor,
};
