use std::fs;

#[test]
fn public_docs_preserve_composition_and_curated_evidence_boundaries() {
    let readme = fs::read_to_string("README.md").unwrap();
    let generation = fs::read_to_string("docs/generation-guide.md").unwrap();
    let composition = fs::read_to_string("docs/composition-guide.md").unwrap();
    let all = format!("{readme}\n{generation}\n{composition}");

    for command in [
        "templates list",
        "compose \\",
        "--spec tests/fixtures/composition/valid/template-only.json",
        "validate generated/composition-sc",
        "report generated/composition-sc --format",
    ] {
        assert!(
            all.contains(command),
            "missing documented command {command}"
        );
    }
    assert!(composition.contains("do not claim registry coverage"));
    assert!(composition.contains("same-project checks"));
    assert!(composition.contains("pinned `dicom3tools-dciodvfy`"));
    assert!(composition.contains("The output path must not exist"));
    assert!(composition.contains("never to the process"));
}

#[test]
fn docs_limit_public_templates_to_the_qualified_native_pixel_domains() {
    let guide = fs::read_to_string("docs/composition-guide.md").unwrap();
    for contract in [
        "classic/secondary-capture/monochrome@1.0.0",
        "classic/secondary-capture/rgb@1.0.0",
        "classic/secondary-capture/multiframe-single-bit@1.0.0",
        "classic/secondary-capture/multiframe-grayscale-byte@1.0.0",
        "unsigned 8- or 16-bit",
        "unsigned 8-bit RGB",
        "classic/cr@1.0.0",
        "classic/ct@1.0.0",
        "classic/mr@1.0.0",
        "classic/dx/for-presentation@1.0.0",
        "classic/mammography/for-presentation@1.0.0",
        "classic/mammography/for-processing@1.0.0",
        "classic/ultrasound/single-frame@1.0.0",
        "classic/ultrasound/multiframe@1.0.0",
        "classic/nuclear-medicine@1.0.0",
        "classic/pet@1.0.0",
        "vl/endoscopic@1.0.0",
        "vl/microscopic@1.0.0",
        "vl/photographic@1.0.0",
        "classic/xa@1.0.0",
        "classic/xrf@1.0.0",
        "Network content is not supported",
    ] {
        assert!(
            guide.contains(contract),
            "missing composition boundary {contract}"
        );
    }
}

#[test]
fn p7_integration_guide_documents_every_external_boundary() {
    let guide = fs::read_to_string("docs/composition-integration-guide.md").unwrap();
    for contract in [
        "compose_from_bytes",
        "ComposeCancellationToken",
        "composition-provider-request.schema.json",
        "DTS_COMPOSITION_PROVIDER_NETWORK=disabled",
        "argument_sha256",
        "portable OS-level socket sandbox",
        "same-project evidence",
        "never a registry `case_id`",
        "parallelism",
        "reproducibility",
    ] {
        assert!(guide.contains(contract), "missing P7 contract {contract}");
    }
}

#[test]
fn p8_docs_record_promotion_and_remaining_scope_without_coverage_inflation() {
    let status = fs::read_to_string("docs/arbitrary-dicom-composition-status.md").unwrap();
    let guide = fs::read_to_string("docs/composition-guide.md").unwrap();
    let consumption = fs::read_to_string("docs/corpus-consumption.md").unwrap();
    for contract in [
        "Phase P8 program completion gate",
        "unqualified_unknown_sop",
        "lossless_image_container_not_qualified",
        "template_transfer_syntax_not_qualified",
        "provider_os_socket_sandbox_external",
        "full_scale_resource_behavior_unproven",
        "There are no schema-only content-source shims left",
    ] {
        assert!(status.contains(contract), "missing P8 status contract {contract}");
    }
    for source in [
        "inline_small_fixture",
        "encoded_frames",
        "templates/qualification-evidence.json",
    ] {
        assert!(guide.contains(source), "missing P8 guide contract {source}");
    }
    assert!(consumption.contains("Do not translate template IDs into"));
    assert!(consumption.contains("curated `case_id`, profile, or coverage claims"));
}
