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
        "not currently available",
    ] {
        assert!(
            guide.contains(contract),
            "missing composition boundary {contract}"
        );
    }
}
