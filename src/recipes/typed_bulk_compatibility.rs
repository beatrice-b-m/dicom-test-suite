//! Compatibility projection and validation for typed non-image bulk providers.
//!
//! This module is deliberately frontend-neutral. The curated integration layer
//! supplies execution observations and merges the returned family fragment with
//! common manifest fields.

#[path = "typed_bulk_compatibility/projection.rs"]
mod projection;
#[path = "typed_bulk_compatibility/validation.rs"]
mod validation;

pub use projection::{
    EncapsulatedPayloadManifestProjection, SpecializedManifestProjection,
    WaveformManifestProjection, project_encapsulated_payload, project_waveform,
};
pub use validation::{
    ObservedSpecializedContent, SpecializedValidationError, SpecializedValidationObservation,
    validate_encapsulated_payload, validate_waveform,
};
