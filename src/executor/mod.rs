//! Shared, frontend-neutral corpus execution contracts and engine.

pub mod adapters;
pub mod cancellation;
pub mod engine;
pub mod evidence;
pub mod frame_codec;
#[cfg(feature = "legacy_jpeg_dcmtk")]
pub mod locked_full_file;
pub mod materialization;
pub mod native_codec;
pub mod scheduler;
pub mod services;
pub mod stress_content;
pub mod transaction;
