#!/bin/sh
set -eu

[ "$#" -eq 1 ] || { echo "usage: $0 <release-manifest.json>" >&2; exit 2; }
manifest=$1
[ -f "$manifest" ] || { echo "release manifest is missing: $manifest" >&2; exit 3; }

manifest_version=$(jq -er '.release_manifest_schema_version' "$manifest")
jq -e '.source | type == "object" and
       (.revision | type == "string" and test("^[0-9a-f]{40}$")) and
       (.dirty | type == "boolean")' "$manifest" >/dev/null || {
    echo "release manifest source provenance is invalid" >&2
    exit 4
}
case "$manifest_version" in
    1.0.0)
        jq -e '.product.name == "synth-dicom-gen" and
               .version_result.product.name == "synth-dicom-gen"' "$manifest" >/dev/null || {
            echo "release manifest product identity must be synth-dicom-gen" >&2
            exit 4
        }
        ;;
    2.0.0|3.0.0)
        jq -e '.product.name == "synth-dicom-gen" and
               .version_result.product.name == "synth-dicom-gen"' "$manifest" >/dev/null || {
            echo "release manifest product identity must be synth-dicom-gen" >&2
            exit 4
        }
        jq -e --arg expected_capabilities "$manifest_version" '
          def sha256: type == "string" and test("^[0-9a-f]{64}$");
          def runtime_valid:
            type == "object" and
            (.runtime_id | type == "string" and length > 0) and
            (.runtime_kind | type == "string" and length > 0) and
            (.executable_sha256 | sha256) and
            (.version | type == "string" and length > 0) and
            (.invocation_sha256 | sha256);
          .version_result.version_result_schema_version == "2.0.0" and
          .capabilities_result.capabilities_result_schema_version == $expected_capabilities and
          (.identity_domains | type == "object") and
          .identity_domains.identity_domains_schema_version == "1.0.0" and
          (.identity_domains.engine.engine_sha256 | sha256) and
          (.identity_domains.schema_set.schema_set_sha256 | sha256) and
          (.identity_domains.template_catalog.template_catalog_sha256 | sha256) and
          (.identity_domains.provider_catalog.provider_catalog_sha256 | sha256) and
          (.identity_domains.toolchain.cargo_lock_sha256 | sha256) and
          (.identity_domains.toolchain.toolchain_sha256 | sha256) and
          (.identity_domains.standards.standards_lock_sha256 | sha256) and
          (.identity_domains.execution.execution_sha256 | sha256) and
          (.identity_domains.external_runtime | type == "array") and
          all(.identity_domains.external_runtime[]; runtime_valid) and
          ([.identity_domains.external_runtime[].runtime_id] | length == (unique | length)) and
          .identity_domains == .version_result.identity_domains and
          .identity_domains == .capabilities_result.identity_domains and
          .product.version == .version_result.product.version and
          .product.version == .capabilities_result.product_version and
          .target == .version_result.target and
          (.enabled_features | sort) == (.version_result.enabled_features | sort) and
          (.enabled_features | sort) == (.capabilities_result.enabled_features | sort) and
          (has("legacy_product_resources") == (.version_result | has("product_resources"))) and
          (has("legacy_product_resources") == (.capabilities_result | has("product_resources"))) and
          ((has("legacy_product_resources") | not) or
            (.legacy_product_resources == .version_result.product_resources and
             .legacy_product_resources == .capabilities_result.product_resources))
        ' "$manifest" >/dev/null || {
            echo "release manifest current identity or embedded discovery contract is invalid" >&2
            exit 4
        }
        ;;
    *) echo "unsupported release manifest schema version: $manifest_version" >&2; exit 4 ;;
esac

jq -e '.files | type == "array" and length > 0 and
       ([.[].path] | length == (unique | length))' "$manifest" >/dev/null || {
    echo "release manifest file inventory is invalid or has duplicate paths" >&2
    exit 4
}
