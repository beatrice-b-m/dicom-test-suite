use std::fs;

use synth_dicom_gen::composition::{CompositionUidRole, IdentityPlan, Part10Materializer};
use synth_dicom_gen::corpus_plan::{OutputPlan, OutputRelativePath};
use synth_dicom_gen::recipes::{
    ContentProviderLimits, EncapsulatedPayload, EncapsulatedPayloadPlanProvider, RecipeIdentity,
    TypedBulkPlanningContext, WaveformPlanProvider, encapsulated_payload_input_from_recipe,
    waveform_input_from_recipe,
};
use synth_dicom_gen::{
    DeterministicUidInput, PACKAGE_VERSION, UidRole, deterministic_uid, sha256_hex,
};

const LOCK: &str = "823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559";

fn recipe(path: &str) -> synth_dicom_gen::recipes::CaseRecipe {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn context(
    case_id: &str,
    recipe: &RecipeIdentity,
    artifact_id: &str,
    output: &str,
    frame_of_reference: bool,
) -> TypedBulkPlanningContext {
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256: LOCK,
            case_id,
            recipe_version: &recipe.recipe_version,
            run_seed: 1,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let mut values = vec![
        (
            CompositionUidRole::StudyInstance,
            0,
            uid(UidRole::StudyInstance),
        ),
        (
            CompositionUidRole::SeriesInstance,
            0,
            uid(UidRole::SeriesInstance),
        ),
        (
            CompositionUidRole::SopInstance,
            0,
            uid(UidRole::SopInstance),
        ),
        (
            CompositionUidRole::ImplementationClass,
            0,
            deterministic_uid(&DeterministicUidInput {
                standards_lock_sha256: LOCK,
                case_id: "dicom-test-suite/implementation",
                recipe_version: PACKAGE_VERSION,
                run_seed: 0,
                file_index: 0,
                frame_index: None,
                referenced_object_index: None,
                role: UidRole::ImplementationClass,
            }),
        ),
    ];
    if frame_of_reference {
        values.push((
            CompositionUidRole::FrameOfReference,
            0,
            uid(UidRole::FrameOfReference),
        ));
    }
    TypedBulkPlanningContext {
        recipe_artifact_logical_id: artifact_id.into(),
        target_instance_id: artifact_id.into(),
        order: 0,
        output: OutputPlan {
            relative_path: OutputRelativePath::new(output).unwrap(),
            role: "dicom_instance".into(),
            publish: true,
        },
        identities: IdentityPlan::from_exact_values(artifact_id, values).unwrap(),
    }
}

#[test]
fn waveform_recipes_materialize_exact_historical_part10_bytes() {
    let fixtures = [
        (
            "cases/recipes/non-image/waveform/non_image_waveform_twelve_lead_ecg.json",
            "453e37be78321e72876c373ded94471753745cb5f335908e76960d2e92e7315b",
        ),
        (
            "cases/recipes/non-image/waveform/non_image_waveform_general_ecg.json",
            "6b1c0d95ec9330cebc9e2ee824486ce7fde4aa0c7e0f7a28564bddd4d90843f5",
        ),
    ];
    let root =
        std::env::temp_dir().join(format!("dts-waveform-direct-plan-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for (index, (path, expected)) in fixtures.into_iter().enumerate() {
        let document = recipe(path);
        let input = waveform_input_from_recipe(&document).unwrap().unwrap();
        let context = context(
            &input.case_id,
            &input.recipe,
            &input.artifact_logical_id,
            &input.output_path,
            false,
        );
        let output = WaveformPlanProvider
            .plan(&input, &context, ContentProviderLimits::default())
            .unwrap();
        assert_eq!(output.artifact.instance.content.len(), input.groups.len());
        assert_eq!(output.bindings.slots.len(), input.groups.len());
        let file = root.join(format!("{index}.dcm"));
        Part10Materializer
            .materialize(&output.artifact.instance, &file)
            .unwrap();
        assert_eq!(sha256_hex(&fs::read(file).unwrap()), expected);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pdf_and_stl_recipes_materialize_exact_historical_part10_bytes() {
    let fixtures = [
        (
            "cases/recipes/non-image/encapsulated-document/encapsulated_pdf_minimal.json",
            "485c9228033059a578b809004fae19436e1cf1accb5c1fd6f4f106e6a14ff091",
            false,
        ),
        (
            "cases/recipes/derived/mesh/derived_mesh_encapsulated_stl.json",
            "4f5d7e4018ad9a545656e63cbd3bf45ed02bb00da3c841e401a60f26481553ef",
            true,
        ),
    ];
    let root = std::env::temp_dir().join(format!(
        "dts-encapsulated-direct-plan-{}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    for (index, (path, expected, frame_of_reference)) in fixtures.into_iter().enumerate() {
        let document = recipe(path);
        let input = encapsulated_payload_input_from_recipe(&document)
            .unwrap()
            .unwrap();
        let context = context(
            &input.case_id,
            &input.recipe,
            &input.artifact_logical_id,
            &input.output_path,
            frame_of_reference,
        );
        let output = EncapsulatedPayloadPlanProvider
            .plan(&input, &context, ContentProviderLimits::default())
            .unwrap();
        let content = &output.artifact.instance.content[0];
        assert!(matches!(
            content.materialization,
            Some(synth_dicom_gen::composition::ContentMaterialization::Inline(_))
        ));
        let file = root.join(format!("{index}.dcm"));
        Part10Materializer
            .materialize(&output.artifact.instance, &file)
            .unwrap();
        assert_eq!(sha256_hex(&fs::read(file).unwrap()), expected);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn providers_are_output_free_bounded_and_reject_declared_hash_drift() {
    let document =
        recipe("cases/recipes/non-image/encapsulated-document/encapsulated_pdf_minimal.json");
    let mut input = encapsulated_payload_input_from_recipe(&document)
        .unwrap()
        .unwrap();
    let document_context = context(
        &input.case_id,
        &input.recipe,
        &input.artifact_logical_id,
        &input.output_path,
        false,
    );
    let sentinel = std::env::temp_dir().join(format!(
        "dts-typed-bulk-plan-sentinel-{}",
        std::process::id()
    ));
    assert!(!sentinel.exists());
    EncapsulatedPayloadPlanProvider
        .plan(&input, &document_context, ContentProviderLimits::default())
        .unwrap();
    assert!(!sentinel.exists());

    let EncapsulatedPayload::MinimalPdf {
        declared_sha256, ..
    } = &mut input.payload
    else {
        unreachable!()
    };
    *declared_sha256 = "0".repeat(64);
    assert!(
        EncapsulatedPayloadPlanProvider
            .plan(&input, &document_context, ContentProviderLimits::default())
            .is_err()
    );

    let waveform =
        recipe("cases/recipes/non-image/waveform/non_image_waveform_twelve_lead_ecg.json");
    let mut waveform = waveform_input_from_recipe(&waveform).unwrap().unwrap();
    waveform.groups[0].samples_per_channel = u32::MAX;
    let context = context(
        &waveform.case_id,
        &waveform.recipe,
        &waveform.artifact_logical_id,
        &waveform.output_path,
        false,
    );
    assert!(
        WaveformPlanProvider
            .plan(&waveform, &context, ContentProviderLimits::default())
            .is_err()
    );
}

#[test]
fn provider_sources_have_no_frontend_writer_or_filesystem_dependency() {
    for source in [
        include_str!("../src/recipes/typed_bulk.rs"),
        include_str!("../src/recipes/waveform.rs"),
        include_str!("../src/recipes/encapsulated_payload.rs"),
    ] {
        for forbidden in [
            "crate::generator",
            "Part10Materializer",
            "open_file",
            "std::fs",
            "output_root",
        ] {
            assert!(
                !source.contains(forbidden),
                "forbidden boundary {forbidden}"
            );
        }
    }
}
