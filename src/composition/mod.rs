//! Shared, registry-independent DICOM composition primitives.
//!
//! The file-backed CLI and public Rust entry points share this implementation.

pub mod advanced_family;
pub mod attribute;
pub mod bulk;
pub mod bundle;
pub mod classic;
pub mod content;
pub mod curated;
pub mod defaults;
pub mod enhanced;
pub mod family;
pub mod identity;
pub mod manifest;
pub mod materializer;
pub mod modules;
pub mod native_content;
pub mod pixel;
pub mod plan;
pub mod provider;
pub mod reference;
pub mod run;
pub mod spec;
pub mod template;
pub mod validation;

pub use advanced_family::{AdvancedFamilyError, AdvancedFamilyKind, AdvancedFamilyProfile};
pub use attribute::{
    AttributeAddress, AttributeError, AttributeItem, AttributeOperation, AttributeValue, DicomVr,
    PrimitiveValue,
};
pub use bulk::{
    BackendProducedBulkDataSlot, BulkDataBounds, BulkDataError, BulkDataKind, BulkDataPlan,
    BulkDataSource, DoubleFloatPixelDataSlot, EncapsulatedDocumentSlot, FloatPixelDataSlot,
    MeshSlot, PixelDataSlot, TypedBulkDataSlot, WaveformSamplesSlot,
};
pub use bundle::{
    BundleError, BundleMemberProvenance, BundleResolution, BundleResolver, DefaultBundleDependency,
    DefaultBundleDescriptor,
};
pub use classic::{
    AcquisitionPlan, ClassicImageModulePlans, ClassicPlanError, DetectorPlan, DisplayTransformPlan,
    GeometryPlan, PixelModulePlan,
};
pub use content::{ContentError, ContentLimits, LocalContentResolver, StagedAsset};
pub use curated::{CuratedPlanError, CuratedPlanInput, resolved_plan_from_curated_dataset};
pub use defaults::{
    DefaultError, DefaultPixelOutput, canonical_native_pixels, resolved_sc_plan, sc_default_pixels,
    sc_derived_layer, sc_template_default_layer,
};
pub use enhanced::{
    ConcatenationPlan, DimensionIndex, DimensionOrganization, DimensionOrganizationPlan,
    EnhancedMultiframePlan, EnhancedPlanError, FunctionalGroupItemPlan,
    PerFrameFunctionalGroupPlan, TemporalFramePlan,
};
pub use family::{
    ClassicFamilyKind, ClassicFamilyProfile, FamilyError, default_family_pixels,
    resolve_family_attributes,
};
pub use identity::{CompositionUidRole, IdentityAllocator, IdentityError, IdentityPlan};
pub use manifest::{
    CompositionManifestAssembler, CompositionManifestInputs, GenericPlanValidator,
    ManifestEntryInput, ManifestError, ValidationCheck,
};
pub use materializer::{MaterializeError, MaterializeOutcome, Part10Materializer};
pub use modules::{CommonModulePlans, ModuleError, ModulePlan, sop_common_operations};
pub use native_content::{RawContentError, RawNativePixelOutput, resolve_raw_native_pixels};
pub use pixel::{
    ByteOrder, FrameSpan, NativePixelPlan, PhotometricInterpretation, PixelElement, PixelError,
    PixelShape, PlanarConfiguration, SampleType,
};
pub use plan::{
    AttributeLayer, AttributePolicy, AttributeResolver, CanonicalContent, Condition,
    ContentMaterialization, ContentPlacement, ResolveContext, ResolveError, ResolvedAttribute,
    ResolvedInstancePlan, SequenceItemPlacement, ValueOrigin,
};
pub use provider::{
    CONTENT_PROVIDER_PROTOCOL_VERSION, ProviderError, ProviderInvocation, ProviderOutput,
    ProviderOutputDeclaration, ProviderRequest, ProviderResponse, invoke_content_provider,
};
pub use reference::{
    CyclePolicy, LogicalReference, MaterializedReference, ReferenceError, ReferenceGraph,
    ReferenceNode,
};
pub use run::{
    ComposeBytesOptions, ComposeError, ComposeOptions, ComposeSummary, compose, compose_from_bytes,
};
pub use spec::{
    AttributeScope, CompositionSpec, ContentAssignment, ContentSource, EncodedFrame,
    IdentityChoice, PixelDeclaration, ResourceLimits, SpecDefaults, SpecError, SpecInstance,
    SpecReference, SpecSampleType, TemplateSelector,
};
pub use template::{
    CapabilitySet, RequirementGap, TemplateCatalog, TemplateDescriptor, TemplateError, TemplateId,
    TemplateStatus, TemplateVersion, TransferSyntaxDescriptor,
};
pub use validation::{
    composition_report, render_composition_report_markdown, validate_composition_root,
};
