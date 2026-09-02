use std::collections::BTreeSet;
use std::fs;

use synth_dicom_gen::composition::{TemplateCatalog, TemplateId, TemplateStatus};

#[test]
fn every_p3_inventory_mapping_is_a_qualified_evidenced_descriptor() {
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read("templates/inventory.json").unwrap()).unwrap();
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    for mapping in inventory["mappings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|mapping| {
            mapping["phase_task"]
                .as_str()
                .is_some_and(|task| task.starts_with("P3"))
        })
    {
        let template_id = TemplateId(mapping["template_family"].as_str().unwrap().into());
        let descriptor = catalog.resolve_qualified(&template_id, None).unwrap();
        assert_eq!(descriptor.status, TemplateStatus::Qualified);
        assert_eq!(descriptor.sop_class_uid, mapping["sop_class_uid"]);
        let behaviors = descriptor
            .attributes
            .iter()
            .filter_map(|attribute| attribute["behavior"].as_str())
            .collect::<BTreeSet<_>>();
        assert!(behaviors.contains("protected"), "{template_id}");
        assert!(behaviors.contains("derived"), "{template_id}");
        assert!(behaviors.contains("caller_settable"), "{template_id}");
        assert!(
            descriptor.attributes.iter().any(|attribute| {
                matches!(attribute["requirement"].as_str(), Some("1C" | "2C"))
                    && !attribute["condition"].is_null()
            }),
            "{template_id}"
        );
        assert!(
            descriptor.validation["independent_routes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|route| {
                    route["adapter_id"] == "dicom_validator"
                        && route["required_for_qualification"] == true
                }),
            "{template_id}"
        );
        assert!(!descriptor.standards_evidence.is_empty(), "{template_id}");
    }
}

#[test]
fn committed_cross_family_reference_matches_catalog_renderer() {
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    assert_eq!(
        fs::read_to_string("docs/composition-template-reference.md").unwrap(),
        catalog.render_reference_markdown()
    );
}
