use std::collections::BTreeMap;
use std::fs;

use dicom_test_suite::composition::{
    CompositionUidRole, IdentityPlan, MaterializedReference, Part10Materializer,
    ResolvedInstancePlan, TemplateId, TemplateVersion,
};
use dicom_test_suite::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan, EvidenceIndependence,
    EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
    ImplementationIdentityPlan, ItemLengthPolicy, OffsetTablePolicy, OutputPlan,
    OutputRelativePath, PlannedDicomArtifact, PreamblePolicy, PublicationPlan,
    PublicationTransaction, SequenceLengthPolicy, ValidationPlan, ValidationRequirement,
    ValidationRule,
};
use dicom_test_suite::executor::services::ArtifactExecutionBindings;
use dicom_test_suite::recipes::{
    AdvancedPlanProviderRequest, AdvancedProviderFamily, AdvancedProviderLimits,
    AdvancedSourceRole, DeformableRegistrationParameters, REGISTRATION_PLAN_PROVIDER_ID,
    RecipeIdentity, RegistrationCommonInput, RegistrationKindInput, RegistrationPlanProvider,
    RegistrationProviderInput, RegistrationSourceInput, SpatialRegistrationParameters,
};
use dicom_test_suite::sha256_hex;

const LOCK: &str = "823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559";
const IMPLEMENTATION: &str = "2.25.93442075376351194778596039619060852790";
const ENHANCED_CT: &str = "1.2.840.10008.5.1.4.1.1.2.1";
const CLASSIC_CT: &str = "1.2.840.10008.5.1.4.1.1.2";

#[derive(Clone, Copy)]
struct SourceFixture {
    logical_id: &'static str,
    order: u64,
    study: &'static str,
    series: &'static str,
    sop_class: &'static str,
    sop: &'static str,
    frame_of_reference: &'static str,
    path: &'static str,
}

const FIXED: SourceFixture = SourceFixture {
    logical_id: "registration_fixed",
    order: 0,
    study: "2.25.269033570553049102093664871375122165084",
    series: "2.25.115285365513962680770954006188334713275",
    sop_class: ENHANCED_CT,
    sop: "2.25.55404081588209817437957528114155141547",
    frame_of_reference: "2.25.226302238501659861638544378617423560010",
    path: "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
};

const MOVING: SourceFixture = SourceFixture {
    logical_id: "registration_moving",
    order: 1,
    study: "2.25.236314287686458728680218109532907111654",
    series: "2.25.248249117821178674275908569013742099373",
    sop_class: CLASSIC_CT,
    sop: "2.25.902198310294057015398484690740670376",
    frame_of_reference: "2.25.292834410588567853231298669075533151170",
    path: "classic/ct/mono2_i16_rescale_12bit_explicit_le/instance.dcm",
};

fn request(
    case_id: &str,
    recipe_id: &str,
    input: &RegistrationProviderInput,
) -> AdvancedPlanProviderRequest {
    let recipe = RecipeIdentity {
        recipe_id: recipe_id.into(),
        recipe_version: "0.1.0".into(),
    };
    AdvancedPlanProviderRequest {
        provider_id: REGISTRATION_PLAN_PROVIDER_ID.into(),
        family: AdvancedProviderFamily::Registration,
        case_id: case_id.into(),
        recipe: recipe.clone(),
        seed: 1,
        artifact_contexts: RegistrationPlanProvider::new(LOCK)
            .unwrap()
            .recipe_default_contexts(input, case_id, &recipe, 1)
            .unwrap(),
        limits: AdvancedProviderLimits {
            max_artifacts: 3,
            max_references: 2,
            max_binding_slots: 1,
            max_total_output_bytes: 4 * 1024 * 1024,
            max_peak_working_bytes: 4 * 1024 * 1024,
            max_parallelism: 2,
        },
    }
}

fn source(
    fixture: SourceFixture,
    owner: &str,
    role: AdvancedSourceRole,
) -> RegistrationSourceInput {
    let identities = IdentityPlan::from_exact_values(
        fixture.logical_id,
        [
            (CompositionUidRole::StudyInstance, 0, fixture.study.into()),
            (CompositionUidRole::SeriesInstance, 0, fixture.series.into()),
            (CompositionUidRole::SopInstance, 0, fixture.sop.into()),
            (
                CompositionUidRole::FrameOfReference,
                0,
                fixture.frame_of_reference.into(),
            ),
            (
                CompositionUidRole::ImplementationClass,
                0,
                IMPLEMENTATION.into(),
            ),
        ],
    )
    .unwrap();
    let artifact = PlannedDicomArtifact {
        logical_id: fixture.logical_id.into(),
        order: fixture.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: fixture.path.trim_end_matches("/instance.dcm").into(),
            recipe_id: format!("{}_recipe", fixture.logical_id),
            recipe_version: "0.1.0".into(),
        }),
        instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: fixture.logical_id.into(),
            template_id: TemplateId("source/ct".into()),
            template_version: "1.0.0".parse::<TemplateVersion>().unwrap(),
            sop_class_uid: fixture.sop_class.into(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            identities,
            attributes: vec![],
            content: vec![],
            references: vec![],
        },
        output: OutputPlan {
            relative_path: OutputRelativePath::new(fixture.path).unwrap(),
            role: "dicom_instance".into(),
            publish: true,
        },
        encoding: encoding(),
        validation: ValidationPlan {
            rules: vec![ValidationRule {
                rule_id: "source.identity".into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            }],
        },
        evidence: EvidencePlan {
            obligations: vec![EvidenceObligation {
                obligation_id: format!("source:{}", fixture.logical_id),
                route_id: "builtin.strict".into(),
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            }],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: 512 * 1024,
            peak_working_bytes: 1024 * 1024,
        },
    };
    RegistrationSourceInput {
        role,
        bindings: ArtifactExecutionBindings {
            artifact_id: fixture.logical_id.into(),
            slots: BTreeMap::new(),
        },
        artifact,
        reference: MaterializedReference {
            source_instance_id: owner.into(),
            target_instance_id: fixture.logical_id.into(),
            role: if fixture.sop_class == ENHANCED_CT {
                "registered_target".into()
            } else {
                "moving_source".into()
            },
            frame_role: None,
            referenced_sop_class_uid: fixture.sop_class.into(),
            referenced_sop_instance_uid: fixture.sop.into(),
            referenced_frames: vec![],
        },
    }
}

fn encoding() -> EncodingPlan {
    EncodingPlan {
        transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        sequence_length: SequenceLengthPolicy::WriterDefault,
        item_length: ItemLengthPolicy::WriterDefault,
        fragmentation: FragmentationPolicy::Native,
        offset_table: OffsetTablePolicy::NotApplicable,
        preamble: PreamblePolicy::ZeroFilled,
        file_meta: FileMetaPolicy::Standard,
        implementation: ImplementationIdentityPlan {
            class_uid: IMPLEMENTATION.into(),
            version_name: Some("DICOMTS010".into()),
        },
        backend_id: "dicom-rs.part10".into(),
    }
}

fn common(case_id: &str, deformable: bool) -> RegistrationCommonInput {
    RegistrationCommonInput {
        logical_id: "registration".into(),
        order: 2,
        output_path: OutputRelativePath::new(format!("{case_id}/instance.dcm")).unwrap(),
        template_id: if deformable {
            "derived/registration/deformable"
        } else {
            "derived/registration/spatial"
        }
        .into(),
        series_number: if deformable { "8004" } else { "8003" }.into(),
        study_id: "DTS-ECT".into(),
        laterality: "R".into(),
        manufacturer_model_name: if deformable {
            "Native Deformable Registration"
        } else {
            "Native Spatial Registration"
        }
        .into(),
        device_serial_number: if deformable {
            "DTS-DEFREG-001"
        } else {
            "DTS-REG-001"
        }
        .into(),
        content_label: if deformable {
            "DTS_DEFORM_REG"
        } else {
            "DTS_RIGID_REG"
        }
        .into(),
        content_description: if deformable {
            "Deformable CT pair registration"
        } else {
            "Rigid CT pair registration"
        }
        .into(),
    }
}

fn spatial() -> (AdvancedPlanProviderRequest, RegistrationProviderInput) {
    let case_id = "derived/registration/spatial_ct_pair";
    let input = RegistrationProviderInput {
        common: common(case_id, false),
        sources: vec![
            source(FIXED, "registration", AdvancedSourceRole::RegistrationFixed),
            source(
                MOVING,
                "registration",
                AdvancedSourceRole::RegistrationMoving,
            ),
        ],
        registration: RegistrationKindInput::Spatial(SpatialRegistrationParameters {
            fixed_matrix: identity_matrix(),
            fixed_comment: "Enhanced CT target identity".into(),
            moving_matrix: [
                "1", "0", "0", "0.625", "0", "1", "0", "0.625", "0", "0", "1", "2.5", "0", "0",
                "0", "1",
            ]
            .map(str::to_owned),
            moving_comment: "Classic CT first-pixel origin aligned to target frame 2".into(),
        }),
    };
    let request = request(case_id, "derived_registration_spatial_ct_pair", &input);
    (request, input)
}

fn deformable() -> (AdvancedPlanProviderRequest, RegistrationProviderInput) {
    let case_id = "derived/registration/deformable_ct_pair";
    let input = RegistrationProviderInput {
        common: common(case_id, true),
        sources: vec![
            source(FIXED, "registration", AdvancedSourceRole::RegistrationFixed),
            source(
                MOVING,
                "registration",
                AdvancedSourceRole::RegistrationMoving,
            ),
        ],
        registration: RegistrationKindInput::Deformable(DeformableRegistrationParameters {
            image_position_patient: ["0", "0", "2.5"].map(str::to_owned),
            image_orientation_patient: ["1", "0", "0", "0", "1", "0"].map(str::to_owned),
            grid_dimensions: [2, 2, 1],
            grid_resolution: [0.75, 0.75, 2.5],
            vector_grid_data: vec![
                -0.625, -0.625, -2.5, -0.75, -0.625, -2.5, -0.625, -0.75, -2.5, -0.75, -0.75, -2.5,
            ],
            pre_deformation_matrix: identity_matrix(),
            post_deformation_matrix: identity_matrix(),
        }),
    };
    let request = request(case_id, "derived_registration_deformable_ct_pair", &input);
    (request, input)
}

fn identity_matrix() -> [String; 16] {
    [
        "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1",
    ]
    .map(str::to_owned)
}

#[test]
fn direct_registration_plans_match_frozen_seed_one_part10_bytes() {
    let provider = RegistrationPlanProvider::new(LOCK).unwrap();
    let fixtures = [
        (
            spatial(),
            "992e3576cc379304516797ae7a739b8da36f5e18343cf82f07335020d62b657d",
        ),
        (
            deformable(),
            "ff5bce7c7c3acceda19b600429cb2eaa70be1df985505275c1c0e88428d9bb26",
        ),
    ];
    let root = std::env::temp_dir().join(format!("dts-registration-plan-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for ((request, input), expected_hash) in fixtures {
        let output = provider.plan_typed(&request, &input).unwrap();
        output.validate(&request).unwrap();
        let target = &output.artifacts[2].planned;
        let path = root.join(format!("{}.dcm", request.recipe.recipe_id));
        Part10Materializer
            .materialize(&target.instance, &path)
            .unwrap();
        assert_eq!(sha256_hex(&fs::read(path).unwrap()), expected_hash);
        assert_eq!(target.instance.references.len(), 2);
        assert_eq!(
            target
                .instance
                .identities
                .get(&CompositionUidRole::StudyInstance, 0),
            Some(FIXED.study)
        );
        assert_eq!(
            target
                .instance
                .identities
                .get(&CompositionUidRole::FrameOfReference, 0),
            Some(FIXED.frame_of_reference)
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn registration_sources_are_private_ordered_dependencies_and_planning_is_output_free() {
    let provider = RegistrationPlanProvider::new(LOCK).unwrap();
    let (request, input) = spatial();
    let sentinel =
        std::env::temp_dir().join(format!("dts-registration-no-output-{}", std::process::id()));
    assert!(!sentinel.exists());
    let output = provider.plan_typed(&request, &input).unwrap();
    assert!(!sentinel.exists());
    assert_eq!(
        output
            .artifacts
            .iter()
            .map(|value| value.planned.order)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(
        output.artifacts[..2]
            .iter()
            .all(|value| !value.planned.output.publish)
    );
    assert_eq!(output.dependencies.len(), 2);
    assert_eq!(output.dependencies[0].relationship, "registration_fixed");
    assert_eq!(output.dependencies[1].relationship, "registration_moving");
    let plan = output
        .to_corpus_plan(
            &request,
            PublicationPlan {
                manifest_path: OutputRelativePath::new("manifest.json").unwrap(),
                transaction: PublicationTransaction::AtomicNoReplace,
                private_staging: true,
                no_overwrite: true,
            },
        )
        .unwrap();
    assert_eq!(
        plan.topological_order().unwrap(),
        ["registration_fixed", "registration_moving", "registration"]
    );
}

#[test]
fn malformed_registration_sources_are_rejected_before_staging() {
    let provider = RegistrationPlanProvider::new(LOCK).unwrap();
    let (request, input) = spatial();
    let mut corruptions = Vec::new();
    let mut missing = input.clone();
    missing.sources.pop();
    corruptions.push(missing);
    let mut reordered = input.clone();
    reordered.sources.swap(0, 1);
    corruptions.push(reordered);
    let mut duplicate_role = input.clone();
    duplicate_role.sources[1].role = AdvancedSourceRole::RegistrationFixed;
    corruptions.push(duplicate_role);
    let mut wrong_sop = input.clone();
    wrong_sop.sources[0].artifact.instance.sop_class_uid = CLASSIC_CT.into();
    corruptions.push(wrong_sop);
    let mut wrong_role = input.clone();
    wrong_role.sources[0].role = AdvancedSourceRole::PresentationSourceImage;
    corruptions.push(wrong_role);
    let mut framed = input.clone();
    framed.sources[0].reference.referenced_frames = vec![1];
    corruptions.push(framed);
    let mut wrong_binding = input.clone();
    wrong_binding.sources[0].bindings.artifact_id = "other".into();
    corruptions.push(wrong_binding);
    for corrupt in corruptions {
        assert!(provider.plan_typed(&request, &corrupt).is_err());
    }
}

#[test]
fn registration_provider_source_has_no_writer_or_filesystem_boundary() {
    let source = include_str!("../src/recipes/registration.rs");
    for forbidden in [
        "crate::generator",
        "InMemDicomObject",
        "open_file",
        "resolved_plan_from_curated_dataset",
        "std::fs",
        "PathBuf",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden boundary {forbidden}"
        );
    }
}

#[test]
fn provider_preserves_caller_owned_target_context() {
    let provider = RegistrationPlanProvider::new(LOCK).unwrap();
    let (mut request, mut input) = spatial();
    let context = &mut request.artifact_contexts[0];
    context.target_instance_id = "caller_registration_target".into();
    context.identities.logical_instance_id = context.target_instance_id.clone();
    context.order = 91;
    context.output.relative_path =
        OutputRelativePath::new("composition/registration/custom.dcm").unwrap();
    for source in &mut input.sources {
        source.reference.source_instance_id = context.target_instance_id.clone();
    }
    let expected = context.clone();

    let output = provider.plan_typed(&request, &input).unwrap();
    let planned = &output.artifacts.last().unwrap().planned;
    assert_eq!(planned.logical_id, expected.target_instance_id);
    assert_eq!(planned.order, expected.order);
    assert_eq!(planned.output, expected.output);
    assert_eq!(planned.instance.identities, expected.identities);
}
