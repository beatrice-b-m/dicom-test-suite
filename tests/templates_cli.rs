use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_dicom-test-suite")
}

fn compile_schema(path: &str) -> jsonschema::Validator {
    let schema = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap()
}

#[test]
fn templates_list_has_human_and_machine_readable_output() {
    let table = Command::new(binary())
        .args(["templates", "list"])
        .output()
        .unwrap();
    assert!(table.status.success());
    let table = String::from_utf8(table.stdout).unwrap();
    assert!(table.contains("classic/secondary-capture/monochrome\t1.0.0\tQualified"));
    assert!(table.contains("classic/secondary-capture/rgb\t1.0.0\tQualified"));

    let json = Command::new(binary())
        .args(["templates", "list", "--format", "json"])
        .output()
        .unwrap();
    assert!(json.status.success());
    assert!(json.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert!(compile_schema("schemas/cli-success-envelope.schema.json").is_valid(&value));
    assert!(compile_schema("schemas/templates-result.schema.json").is_valid(&value["result"]));
    assert_eq!(value["command"], "templates list");
    let catalog =
        dicom_test_suite::composition::TemplateCatalog::load("templates/catalog.json").unwrap();
    assert_eq!(
        value["result"]["templates"].as_array().unwrap().len(),
        catalog.templates.len()
    );
}

#[test]
fn templates_reference_renders_catalog_markdown_and_json() {
    let markdown = Command::new(binary())
        .args(["templates", "reference", "--format", "markdown"])
        .output()
        .unwrap();
    assert!(markdown.status.success());
    assert_eq!(
        String::from_utf8(markdown.stdout).unwrap(),
        std::fs::read_to_string("docs/composition-template-reference.md").unwrap()
    );

    let json = Command::new(binary())
        .args(["templates", "reference", "--format", "json"])
        .output()
        .unwrap();
    assert!(json.status.success());
    let descriptors: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(descriptors["command"], "templates reference");
    assert!(
        descriptors["result"]["templates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|descriptor| {
                descriptor["template_id"] == "classic/xa"
                    && descriptor["transfer_syntaxes"].as_array().unwrap().len() > 1
            })
    );
}

#[test]
fn templates_describe_returns_the_complete_versioned_descriptor() {
    let output = Command::new(binary())
        .args([
            "templates",
            "describe",
            "classic/secondary-capture/rgb",
            "--version",
            "1.0.0",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let descriptor: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(descriptor["command"], "templates describe");
    let descriptor = &descriptor["result"]["templates"][0];
    assert_eq!(descriptor["status"], "qualified");
    assert_eq!(descriptor["content_slots"][0]["slot"], "pixels");
    assert!(descriptor["standards_evidence"].as_array().unwrap().len() >= 2);
}

#[test]
fn templates_describe_rejects_unknown_or_unqualified_identity() {
    let output = Command::new(binary())
        .args(["templates", "describe", "classic/unknown"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["command"], "templates describe");
    assert_eq!(error["error"]["code"], "capability.template.unavailable");
}
