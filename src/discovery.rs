use serde::Serialize;

use crate::product_resources::{ProductResourceError, ProductResourceIdentity, ProductResources};

pub const VERSION_RESULT_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProductIdentity {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionResult {
    pub version_result_schema_version: &'static str,
    pub product: ProductIdentity,
    pub cli_api_version: &'static str,
    pub target: &'static str,
    pub rust_toolchain: &'static str,
    pub enabled_features: Vec<&'static str>,
    pub product_resources: ProductResourceIdentity,
}

pub fn version_result(resources: &ProductResources) -> Result<VersionResult, ProductResourceError> {
    Ok(VersionResult {
        version_result_schema_version: VERSION_RESULT_SCHEMA_VERSION,
        product: ProductIdentity {
            name: crate::PACKAGE_NAME,
            version: crate::PACKAGE_VERSION,
        },
        cli_api_version: crate::cli_protocol::CLI_API_VERSION,
        target: crate::TARGET_TRIPLE,
        rust_toolchain: crate::RUSTC_VERSION,
        enabled_features: crate::ACTIVE_FEATURE_FLAGS.to_vec(),
        product_resources: resources.verify_integrity()?,
    })
}
