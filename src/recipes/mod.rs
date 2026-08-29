mod encoding;
mod error;
mod loader;
mod metadata_sc;
mod model;
mod sc;

pub use crate::planning::RecipeIdentity;
pub use encoding::{RecipeEncodingError, encoding_plan_from_recipe};
pub use error::RecipeCatalogError;
pub use loader::RecipeCatalog;
pub use metadata_sc::{MetadataScPlanInput, MetadataScPlannerError, resolved_metadata_sc_plan};
pub use model::{
    AttributeOperation, BitPackingParameters, CaseBinding, CaseRecipe, ColorParameters,
    ContentBinding, DependencyBinding, DicomRecipe, EmptyType2AttributeMetadata,
    EncapsulationProjectionParameters, EncodingPolicy, IntegerWordParameters, MetadataScParameters,
    MutationEdit, MutationRecipe, NonsquareGeometryParameters, OutputBinding, PaletteParameters,
    PersonNameComponentGroup, PersonNameMetadata, PixelPaddingParameters, PlannedArtifactRecipe,
    PrivateCreatorBlockMetadata, PrivateElementMetadata, PrivateElementValue, QualificationRecipe,
    RecipeKind, RecipeReference, ResourcePolicy, SecondaryCaptureParameters,
    SequenceLengthMetadata, StringBoundaryElementMetadata, StringValueSource, TemplateReference,
    TimezoneBoundaryMetadata,
};
pub use sc::{
    ScPlanError, SecondaryCapturePlanInput, native_pixel_content_from_recipe,
    native_pixel_request_from_recipe, resolved_secondary_capture_plan,
};
