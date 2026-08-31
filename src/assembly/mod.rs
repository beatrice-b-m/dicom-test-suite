//! Typed structural-assembly request, planning, execution, and evidence models.

mod planning;
mod request;
mod run;
mod validation;

pub use planning::{AssemblyPlan, plan_assembly};

pub use request::{
    ASSEMBLY_REQUEST_SCHEMA_VERSION, AssemblyAddress, AssemblyBulk, AssemblyElement, AssemblyError,
    AssemblyIdentity, AssemblyInstance, AssemblyLimits, AssemblyReference, AssemblyRequest,
    AssemblyValue, BulkSource, ReferenceRole, SequenceItem,
};
pub use run::{
    ASSEMBLY_MANIFEST_SCHEMA_VERSION, AssembleOptions, AssembleSummary, AssemblyRunError, assemble,
};
pub use validation::{assembly_report, validate_assembly_root};
