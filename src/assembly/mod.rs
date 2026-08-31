//! Typed structural-assembly request, planning, execution, and evidence models.

mod request;

pub use request::{
    ASSEMBLY_REQUEST_SCHEMA_VERSION, AssemblyAddress, AssemblyBulk, AssemblyElement, AssemblyError,
    AssemblyIdentity, AssemblyInstance, AssemblyLimits, AssemblyReference, AssemblyRequest,
    AssemblyValue, BulkSource, ReferenceRole, SequenceItem,
};
