//! Shared, registry-independent DICOM composition primitives.
//!
//! This module is intentionally not wired to a public CLI until the P2 gate.

pub mod attribute;
pub mod content;
pub mod defaults;
pub mod identity;
pub mod manifest;
pub mod materializer;
pub mod pixel;
pub mod plan;
pub mod reference;
pub mod template;

pub use attribute::{
    AttributeAddress, AttributeError, AttributeItem, AttributeOperation, AttributeValue, DicomVr,
    PrimitiveValue,
};
pub use content::{ContentError, ContentLimits, LocalContentResolver, StagedAsset};
pub use defaults::{
    DefaultError, DefaultPixelOutput, canonical_native_pixels, resolved_sc_plan, sc_default_pixels,
    sc_derived_layer, sc_template_default_layer,
};
pub use identity::{CompositionUidRole, IdentityAllocator, IdentityError, IdentityPlan};
pub use manifest::{
    CompositionManifestAssembler, CompositionManifestInputs, GenericPlanValidator,
    ManifestEntryInput, ManifestError, ValidationCheck,
};
pub use materializer::{MaterializeError, Part10Materializer};
pub use pixel::{
    ByteOrder, FrameSpan, NativePixelPlan, PhotometricInterpretation, PixelElement, PixelError,
    PixelShape, PlanarConfiguration, SampleType,
};
pub use plan::{
    AttributeLayer, AttributePolicy, AttributeResolver, CanonicalContent, Condition,
    ContentMaterialization, ResolveContext, ResolveError, ResolvedAttribute, ResolvedInstancePlan,
    ValueOrigin,
};
pub use reference::{
    CyclePolicy, LogicalReference, MaterializedReference, ReferenceError, ReferenceGraph,
    ReferenceNode,
};
pub use template::{
    CapabilitySet, RequirementGap, TemplateCatalog, TemplateDescriptor, TemplateError, TemplateId,
    TemplateStatus, TemplateVersion, TransferSyntaxDescriptor,
};
