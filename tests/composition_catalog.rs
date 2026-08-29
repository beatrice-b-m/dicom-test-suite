use std::fs;

use dicom_test_suite::composition::{TemplateCatalog, TemplateId, TemplateStatus};

#[test]
fn committed_sc_catalog_is_locked_and_qualified() {
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    let lock_hash = dicom_test_suite::sha256_hex(&fs::read("standards.lock.json").unwrap());
    assert_eq!(catalog.standards_lock_sha256, lock_hash);
    assert_eq!(catalog.templates.len(), 2);

    for id in [
        "classic/secondary-capture/monochrome",
        "classic/secondary-capture/rgb",
    ] {
        let template = catalog
            .resolve_qualified(&TemplateId(id.into()), Some("1.0.0".parse().unwrap()))
            .unwrap();
        assert_eq!(template.status, TemplateStatus::Qualified);
        assert_eq!(template.sop_class_uid, "1.2.840.10008.5.1.4.1.1.7");
        assert_eq!(
            catalog.default_transfer_syntax(template).uid,
            "1.2.840.10008.1.2.1"
        );
        assert!(template.attributes.iter().any(|attribute| {
            attribute["tag"] == "0028,0010" && attribute["behavior"] == "protected"
        }));
    }
}

#[test]
fn sc_catalog_separates_monochrome_and_rgb_pixel_contracts() {
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    let monochrome = catalog
        .resolve_qualified(
            &TemplateId("classic/secondary-capture/monochrome".into()),
            None,
        )
        .unwrap();
    let rgb = catalog
        .resolve_qualified(&TemplateId("classic/secondary-capture/rgb".into()), None)
        .unwrap();
    assert_eq!(
        monochrome.content_slots[0]["constraints"]["samples_per_pixel"][0],
        1
    );
    assert_eq!(
        rgb.content_slots[0]["constraints"]["photometric_interpretations"][0],
        "RGB"
    );
}
