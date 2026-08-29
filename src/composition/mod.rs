//! Shared, registry-independent DICOM composition primitives.
//!
//! This module is intentionally not wired to a public CLI until the P2 gate.

pub mod attribute;

pub use attribute::{
    AttributeAddress, AttributeError, AttributeItem, AttributeOperation, AttributeValue, DicomVr,
    PrimitiveValue,
};
