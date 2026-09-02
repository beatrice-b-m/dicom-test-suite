use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use synth_dicom_gen::composition::{
    CompositionUidRole, IdentityPlan, ResolvedInstancePlan, TemplateId, TemplateVersion,
};
use synth_dicom_gen::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, EncodingPlan, EvidencePlan, FileMetaPolicy,
    FragmentationPolicy, ImplementationIdentityPlan, ItemLengthPolicy, OffsetTablePolicy,
    OutputPlan, OutputRelativePath, PlannedArtifact, PlannedDicomArtifact, PreamblePolicy,
    SequenceLengthPolicy, ValidationPlan, ValidationRequirement, ValidationRule,
};
use synth_dicom_gen::executor::materialization::{
    AuxiliaryMaterializationHandler, AuxiliaryPayload, MaterializationDispatcher,
    MaterializationError,
};
use synth_dicom_gen::executor::services::{
    ArtifactExecutionBindings, MaterializationRequest, StagedAssetRegistry,
};
use synth_dicom_gen::planning::RecipeIdentity;
use synth_dicom_gen::recipes::{
    AdvancedArtifactPlanningContext, AdvancedBlendingPresentationParameters, AdvancedPlanProvider,
    AdvancedPlanProviderRequest, AdvancedProviderFamily, AdvancedProviderLimits,
    AdvancedSourceRole, BlendingPresentationParameters, ColorPresentationParameters,
    DisplayedAreaParameters, GrayscalePresentationParameters, PRESENTATION_ADVANCED_PROVIDER_ID,
    PresentationKind, PresentationPlanInput, PresentationPlanProvider, PresentationRecipe,
    PresentationSourceInput,
};
use synth_dicom_gen::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};

const SEED: u64 = 1;
const CT_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.2";
const ENHANCED_CT_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.2.1";
const SC_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.7";
const TRANSFER_SYNTAX: &str = "1.2.840.10008.1.2.1";
const LOCKED_STUDY: &str = "2.25.199901363808168357571011363759126514571";
const LOCKED_FOR: &str = "2.25.270148652620481937038343652600736357210";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn absent(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "dicom-test-suite-presentation-direct-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

struct NoAuxiliary;

impl AuxiliaryMaterializationHandler for NoAuxiliary {
    fn render(
        &self,
        _: &synth_dicom_gen::corpus_plan::PlannedAuxiliaryArtifact,
        _: &ArtifactExecutionBindings,
        _: &StagedAssetRegistry,
    ) -> Result<AuxiliaryPayload, MaterializationError> {
        panic!("presentation providers contain no auxiliary artifacts")
    }
}

#[derive(Clone, Copy)]
struct SourceSpec {
    logical_id: &'static str,
    path: &'static str,
    study: &'static str,
    series: &'static str,
    sop_class: &'static str,
    sop: &'static str,
    frame_of_reference: Option<&'static str>,
}

const GRAYSCALE_SOURCE: SourceSpec = SourceSpec {
    logical_id: "enhanced_ct_source",
    path: "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
    study: "2.25.269033570553049102093664871375122165084",
    series: "2.25.115285365513962680770954006188334713275",
    sop_class: ENHANCED_CT_SOP_CLASS,
    sop: "2.25.55404081588209817437957528114155141547",
    frame_of_reference: None,
};

const COLOR_SOURCE: SourceSpec = SourceSpec {
    logical_id: "classic_sc_rgb_source",
    path: "classic/sc/rgb_planar0_explicit_le/instance.dcm",
    study: "2.25.157442252679385108441424001200466571605",
    series: "2.25.310329571028498074578263222893941807283",
    sop_class: SC_SOP_CLASS,
    sop: "2.25.35845167800952987451667441293412972481",
    frame_of_reference: None,
};

const CT_SOURCES: [SourceSpec; 4] = [
    SourceSpec {
        logical_id: "ct_series_1_slice_1",
        path: "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-001.dcm",
        study: LOCKED_STUDY,
        series: "2.25.284999589232098302040634352048564370471",
        sop_class: CT_SOP_CLASS,
        sop: "2.25.261713180064855901870754768338121538365",
        frame_of_reference: Some(LOCKED_FOR),
    },
    SourceSpec {
        logical_id: "ct_series_1_slice_2",
        path: "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-002.dcm",
        study: LOCKED_STUDY,
        series: "2.25.284999589232098302040634352048564370471",
        sop_class: CT_SOP_CLASS,
        sop: "2.25.165411065504186926471604528785872486361",
        frame_of_reference: Some(LOCKED_FOR),
    },
    SourceSpec {
        logical_id: "ct_series_2_slice_1",
        path: "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-001.dcm",
        study: LOCKED_STUDY,
        series: "2.25.323929840837500415854324526015341006991",
        sop_class: CT_SOP_CLASS,
        sop: "2.25.275589758537517033398530724551215909869",
        frame_of_reference: Some(LOCKED_FOR),
    },
    SourceSpec {
        logical_id: "ct_series_2_slice_2",
        path: "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-002.dcm",
        study: LOCKED_STUDY,
        series: "2.25.323929840837500415854324526015341006991",
        sop_class: CT_SOP_CLASS,
        sop: "2.25.19909340604262048306826313084792435943",
        frame_of_reference: Some(LOCKED_FOR),
    },
];

fn displayed_area() -> DisplayedAreaParameters {
    DisplayedAreaParameters {
        top_left: [1, 1],
        bottom_right: [2, 2],
        size_mode: "SCALE TO FIT".into(),
        pixel_aspect_ratio: [1, 1],
    }
}

fn recipes() -> Vec<(PresentationRecipe, Vec<SourceSpec>, &'static str)> {
    vec![
        (
            PresentationRecipe {
                case_id: "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le".into(),
                recipe_id: "gsps_grayscale_softcopy_ct_window".into(),
                recipe_version: "0.1.0".into(),
                output_relative_path: "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le/instance.dcm".into(),
                logical_id: "grayscale_presentation_state".into(),
                uid_reference_index: Some(0),
                kind: PresentationKind::Grayscale(GrayscalePresentationParameters {
                    expected_source_sop_class_uid: ENHANCED_CT_SOP_CLASS.into(),
                    content_label: "DTSGSPS".into(),
                    content_description: "Synthetic CT window presentation state".into(),
                    displayed_area: displayed_area(),
                    window_center: "350".into(),
                    window_width: "1400".into(),
                    window_explanation: "DTS CT softcopy window".into(),
                    presentation_lut_shape: "IDENTITY".into(),
                }),
            },
            vec![GRAYSCALE_SOURCE],
            "7a6bf77ddbb37c389ec7873264a35b8473983d00f06a7cfceedb55bb27dc75f7",
        ),
        (
            PresentationRecipe {
                case_id: "derived/presentation-state/color_softcopy".into(),
                recipe_id: "derived_presentation_state_color_softcopy".into(),
                recipe_version: "0.1.0".into(),
                output_relative_path: "derived/presentation-state/color_softcopy/instance.dcm".into(),
                logical_id: "color_presentation_state".into(),
                uid_reference_index: None,
                kind: PresentationKind::Color(ColorPresentationParameters {
                    expected_source_sop_class_uid: SC_SOP_CLASS.into(),
                    content_label: "DTSCOLORPR".into(),
                    content_description: "Synthetic RGB color presentation state".into(),
                    displayed_area: displayed_area(),
                }),
            },
            vec![COLOR_SOURCE],
            "25c55239968c6f0ce64c509b1bb8aa961c5d0c62238e04ce4a99ae7c25ebf270",
        ),
        (
            PresentationRecipe {
                case_id: "derived/presentation-state/blending".into(),
                recipe_id: "derived_presentation_state_blending".into(),
                recipe_version: "0.1.0".into(),
                output_relative_path: "derived/presentation-state/blending/instance.dcm".into(),
                logical_id: "blending_presentation_state".into(),
                uid_reference_index: None,
                kind: PresentationKind::Blending(BlendingPresentationParameters {
                    expected_source_sop_class_uid: CT_SOP_CLASS.into(),
                    content_label: "DTSBLEND".into(),
                    content_description: "Synthetic DTSBLEND presentation state".into(),
                    displayed_area: displayed_area(),
                    positions: ["UNDERLYING".into(), "SUPERIMPOSED".into()],
                    relative_opacity: 0.5,
                    rescale_intercept: "-1024".into(),
                    rescale_slope: "1".into(),
                    rescale_type: "HU".into(),
                }),
            },
            CT_SOURCES.to_vec(),
            "f16e8258b7d3b1b7e327895dd96972dc84594d474775d58d948b2a738c3dc505",
        ),
        (
            PresentationRecipe {
                case_id: "derived/presentation-state/advanced_blending".into(),
                recipe_id: "derived_presentation_state_advanced_blending".into(),
                recipe_version: "0.1.0".into(),
                output_relative_path: "derived/presentation-state/advanced_blending/instance.dcm".into(),
                logical_id: "advanced_blending_presentation_state".into(),
                uid_reference_index: None,
                kind: PresentationKind::AdvancedBlending(AdvancedBlendingPresentationParameters {
                    expected_source_sop_class_uid: CT_SOP_CLASS.into(),
                    content_label: "DTSADVBLEND".into(),
                    content_description: "Synthetic DTSADVBLEND presentation state".into(),
                    input_numbers: [1, 2],
                    geometry_input_number: 1,
                    blending_mode: "EQUAL".into(),
                    pixel_presentation: "TRUE_COLOR".into(),
                }),
            },
            CT_SOURCES.to_vec(),
            "d6a74ee71fe2206c2d11be6124313a8058a1ac71123c19e697578289a8627ebf",
        ),
    ]
}

fn source_artifact(spec: SourceSpec) -> PlannedDicomArtifact {
    let mut identities = vec![
        (CompositionUidRole::StudyInstance, 0, spec.study.into()),
        (CompositionUidRole::SeriesInstance, 0, spec.series.into()),
        (CompositionUidRole::SopInstance, 0, spec.sop.into()),
        (CompositionUidRole::ImplementationClass, 0, "2.25.1".into()),
    ];
    if let Some(value) = spec.frame_of_reference {
        identities.push((CompositionUidRole::FrameOfReference, 0, value.into()));
    }
    PlannedDicomArtifact {
        logical_id: spec.logical_id.into(),
        order: 0,
        provenance: ArtifactProvenance::Requested,
        case_binding: None,
        instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: spec.logical_id.into(),
            template_id: TemplateId("test/source".into()),
            template_version: TemplateVersion::from_str("1.0.0").unwrap(),
            sop_class_uid: spec.sop_class.into(),
            transfer_syntax_uid: TRANSFER_SYNTAX.into(),
            identities: IdentityPlan::from_exact_values(spec.logical_id, identities).unwrap(),
            attributes: vec![],
            content: vec![],
            references: vec![],
        },
        output: OutputPlan {
            relative_path: OutputRelativePath::new(spec.path).unwrap(),
            role: "source".into(),
            publish: true,
        },
        encoding: EncodingPlan {
            transfer_syntax_uid: TRANSFER_SYNTAX.into(),
            sequence_length: SequenceLengthPolicy::WriterDefault,
            item_length: ItemLengthPolicy::WriterDefault,
            fragmentation: FragmentationPolicy::Native,
            offset_table: OffsetTablePolicy::NotApplicable,
            preamble: PreamblePolicy::ZeroFilled,
            file_meta: FileMetaPolicy::Standard,
            implementation: ImplementationIdentityPlan {
                class_uid: "2.25.1".into(),
                version_name: Some("TEST".into()),
            },
            backend_id: "dicom-rs.part10".into(),
        },
        validation: ValidationPlan {
            rules: vec![ValidationRule {
                rule_id: "validation.shared".into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            }],
        },
        evidence: EvidencePlan {
            obligations: vec![],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: 1,
            peak_working_bytes: 1,
        },
    }
}

fn input(recipe: PresentationRecipe, specs: Vec<SourceSpec>) -> PresentationPlanInput {
    let blending = matches!(
        &recipe.kind,
        PresentationKind::Blending(_) | PresentationKind::AdvancedBlending(_)
    );
    let grayscale = matches!(&recipe.kind, PresentationKind::Grayscale(_));
    PresentationPlanInput {
        recipe,
        sources: specs
            .into_iter()
            .enumerate()
            .map(|(index, spec)| {
                let mut artifact = source_artifact(spec);
                artifact.order = index as u64;
                PresentationSourceInput {
                    ordinal: index as u32 + 1,
                    role: if blending {
                        AdvancedSourceRole::PresentationBlendingInput {
                            input_number: if index < 2 { 1 } else { 2 },
                        }
                    } else {
                        AdvancedSourceRole::PresentationSourceImage
                    },
                    referenced_frames: if grayscale { vec![1, 2] } else { Vec::new() },
                    binding: ArtifactExecutionBindings {
                        artifact_id: artifact.logical_id.clone(),
                        slots: BTreeMap::new(),
                    },
                    artifact,
                }
            })
            .collect(),
    }
}

fn request(input: &PresentationPlanInput) -> AdvancedPlanProviderRequest {
    AdvancedPlanProviderRequest {
        provider_id: PRESENTATION_ADVANCED_PROVIDER_ID.into(),
        family: AdvancedProviderFamily::PresentationState,
        case_id: input.recipe.case_id.clone(),
        recipe: RecipeIdentity {
            recipe_id: input.recipe.recipe_id.clone(),
            recipe_version: input.recipe.recipe_version.clone(),
        },
        seed: SEED,
        artifact_contexts: PresentationPlanProvider::new(lock_hash())
            .recipe_default_contexts(input, SEED)
            .unwrap_or_else(|_| {
                let target = input.recipe.logical_id.clone();
                vec![AdvancedArtifactPlanningContext {
                    recipe_artifact_logical_id: target.clone(),
                    target_instance_id: target.clone(),
                    order: input.sources.len() as u64,
                    output: OutputPlan {
                        relative_path: OutputRelativePath::new(&input.recipe.output_relative_path)
                            .unwrap(),
                        role: "presentation_state".into(),
                        publish: true,
                    },
                    identities: IdentityPlan::from_exact_values(
                        target,
                        [
                            (CompositionUidRole::StudyInstance, 0, "2.25.1".into()),
                            (CompositionUidRole::SeriesInstance, 0, "2.25.2".into()),
                            (CompositionUidRole::SopInstance, 0, "2.25.3".into()),
                            (CompositionUidRole::FrameOfReference, 0, "2.25.4".into()),
                            (CompositionUidRole::ImplementationClass, 0, "2.25.5".into()),
                        ],
                    )
                    .unwrap(),
                }]
            }),
        limits: AdvancedProviderLimits {
            max_artifacts: 8,
            max_references: 8,
            max_binding_slots: 1,
            max_total_output_bytes: 128 * 1024,
            max_peak_working_bytes: 128 * 1024,
            max_parallelism: 2,
        },
    }
}

fn lock_hash() -> String {
    sha256_hex(&fs::read("standards.lock.json").unwrap())
}

fn direct_bytes(
    provider: &PresentationPlanProvider,
    input: &PresentationPlanInput,
    root: &PathBuf,
) -> Vec<u8> {
    let request = request(input);
    let output = provider.plan(&request, input).unwrap();
    output.validate(&request).unwrap();
    let presentation = output.artifacts.last().unwrap();
    let binding = output.bindings.last().unwrap().clone();
    let dispatcher = MaterializationDispatcher::new(root, Arc::new(NoAuxiliary)).unwrap();
    dispatcher
        .dispatch(
            &MaterializationRequest {
                artifact: PlannedArtifact::Dicom(presentation.planned.clone()),
                bindings: binding,
            },
            &StagedAssetRegistry::default(),
        )
        .unwrap();
    fs::read(root.join(presentation.planned.output.relative_path.as_str())).unwrap()
}

fn generated_all() -> (TempRoot, Value) {
    let root = TempRoot::absent("legacy");
    let run = prepare_generation_run(GenerateOptions {
        profile: "all".into(),
        out_dir: root.0.clone(),
        seed: SEED,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    let manifest =
        serde_json::from_slice(&fs::read(root.0.join("manifest.json")).unwrap()).unwrap();
    (root, manifest)
}

#[test]
fn direct_plans_are_output_free_and_encode_closed_reference_graphs() {
    let absent = TempRoot::absent("planning");
    let provider = PresentationPlanProvider::new(lock_hash());
    for (recipe, sources, _) in recipes() {
        let input = input(recipe, sources);
        let request = request(&input);
        let output = provider.plan(&request, &input).unwrap();
        assert_eq!(output.artifacts.len(), input.sources.len() + 1);
        assert_eq!(output.dependencies.len(), input.sources.len());
        assert_eq!(output.references.len(), input.sources.len());
        assert!(
            output
                .artifacts
                .windows(2)
                .all(|pair| pair[0].planned.order < pair[1].planned.order)
        );
        let source_ids = input
            .sources
            .iter()
            .map(|source| source.artifact.logical_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            output
                .dependencies
                .iter()
                .map(|edge| edge.depends_on.as_str())
                .collect::<BTreeSet<_>>(),
            source_ids
        );
        output.validate(&request).unwrap();
    }
    assert!(!absent.0.exists(), "planning created an output root");
}

#[test]
fn direct_plans_match_frozen_seed_one_bytes_identities_and_references() {
    let (legacy_root, manifest) = generated_all();
    let files = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| (file["path"].as_str().unwrap(), file))
        .collect::<BTreeMap<_, _>>();
    let direct_root = TempRoot::absent("direct");
    fs::create_dir(&direct_root.0).unwrap();
    let provider = PresentationPlanProvider::new(lock_hash());
    for (recipe, sources, locked_sha) in recipes() {
        let input = input(recipe, sources.clone());
        let bytes = direct_bytes(&provider, &input, &direct_root.0);
        let path = input.recipe.output_relative_path.as_str();
        assert_eq!(bytes, fs::read(legacy_root.0.join(path)).unwrap(), "{path}");
        assert_eq!(sha256_hex(&bytes), locked_sha, "{path}");
        let file = files[path];
        assert_eq!(file["sha256"], locked_sha);
        let output = provider.plan(&request(&input), &input).unwrap();
        let planned = &output.artifacts.last().unwrap().planned.instance;
        assert_eq!(
            file["uids"]["study_instance_uid"],
            planned
                .identities
                .get(&CompositionUidRole::StudyInstance, 0)
                .unwrap()
        );
        assert_eq!(
            file["uids"]["series_instance_uid"],
            planned
                .identities
                .get(&CompositionUidRole::SeriesInstance, 0)
                .unwrap()
        );
        assert_eq!(
            file["uids"]["sop_instance_uid"],
            planned
                .identities
                .get(&CompositionUidRole::SopInstance, 0)
                .unwrap()
        );
        assert_eq!(
            planned
                .references
                .iter()
                .map(|reference| reference.referenced_sop_instance_uid.as_str())
                .collect::<Vec<_>>(),
            sources.iter().map(|source| source.sop).collect::<Vec<_>>()
        );
        let legacy_references = file["references"].as_array().unwrap();
        assert_eq!(legacy_references.len(), sources.len());
        for (index, ((reference, legacy), source)) in planned
            .references
            .iter()
            .zip(legacy_references)
            .zip(&sources)
            .enumerate()
        {
            assert_eq!(legacy["source_path"], source.path);
            assert_eq!(
                output.artifacts[index]
                    .planned
                    .output
                    .relative_path
                    .as_str(),
                source.path
            );
            assert_eq!(legacy["series_instance_uid"], source.series);
            assert_eq!(legacy["sop_class_uid"], reference.referenced_sop_class_uid);
            assert_eq!(
                legacy["sop_instance_uid"],
                reference.referenced_sop_instance_uid
            );
            assert_eq!(
                legacy
                    .get("frame_numbers")
                    .and_then(Value::as_array)
                    .map(|frames| {
                        frames
                            .iter()
                            .map(|frame| frame.as_u64().unwrap() as u32)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                reference.referenced_frames
            );
            assert_eq!(legacy["relationship"], reference.role);
            assert_eq!(reference.target_instance_id, source.logical_id);
        }
    }
}

#[test]
fn malformed_source_sets_fail_closed_before_staging() {
    let provider = PresentationPlanProvider::new(lock_hash());
    let (recipe, sources, _) = recipes()
        .into_iter()
        .find(|(recipe, _, _)| matches!(recipe.kind, PresentationKind::Blending(_)))
        .unwrap();
    let valid = input(recipe, sources);

    let mut missing = valid.clone();
    missing.sources.pop();
    assert!(provider.plan(&request(&missing), &missing).is_err());

    let mut duplicate = valid.clone();
    duplicate.sources[1].artifact = duplicate.sources[0].artifact.clone();
    duplicate.sources[1].binding.artifact_id = duplicate.sources[0].artifact.logical_id.clone();
    assert!(provider.plan(&request(&duplicate), &duplicate).is_err());

    let mut reordered = valid.clone();
    reordered.sources.swap(0, 1);
    assert!(provider.plan(&request(&reordered), &reordered).is_err());

    let mut wrong_sop = valid.clone();
    wrong_sop.sources[0].artifact.instance.sop_class_uid = SC_SOP_CLASS.into();
    assert!(provider.plan(&request(&wrong_sop), &wrong_sop).is_err());

    let mut wrong_series = valid.clone();
    wrong_series.sources[1].artifact.instance.identities = IdentityPlan::from_exact_values(
        wrong_series.sources[1].artifact.logical_id.clone(),
        vec![
            (CompositionUidRole::StudyInstance, 0, LOCKED_STUDY.into()),
            (CompositionUidRole::SeriesInstance, 0, "2.25.999".into()),
            (CompositionUidRole::SopInstance, 0, CT_SOURCES[1].sop.into()),
            (CompositionUidRole::ImplementationClass, 0, "2.25.1".into()),
            (CompositionUidRole::FrameOfReference, 0, LOCKED_FOR.into()),
        ],
    )
    .unwrap();
    assert!(
        provider
            .plan(&request(&wrong_series), &wrong_series)
            .is_err()
    );

    let mut wrong_role = valid.clone();
    wrong_role.sources[0].role = AdvancedSourceRole::PresentationSourceImage;
    assert!(provider.plan(&request(&wrong_role), &wrong_role).is_err());
}

#[test]
fn provider_preserves_caller_owned_target_context() {
    let provider = PresentationPlanProvider::new(lock_hash());
    let (recipe, sources, _) = recipes().remove(0);
    let mut input = input(recipe, sources);
    for (index, source) in input.sources.iter_mut().enumerate() {
        source.artifact.order = 10 + index as u64;
    }
    let mut request = request(&input);
    let context = &mut request.artifact_contexts[0];
    context.target_instance_id = "caller_presentation_target".into();
    context.identities.logical_instance_id = context.target_instance_id.clone();
    context.order = 0;
    context.output.relative_path =
        OutputRelativePath::new("composition/presentation/custom.dcm").unwrap();
    let expected = context.clone();

    let output = provider.plan(&request, &input).unwrap();
    let planned = &output.artifacts.first().unwrap().planned;
    assert_eq!(planned.logical_id, expected.target_instance_id);
    assert_eq!(planned.order, expected.order);
    assert_eq!(planned.output, expected.output);
    assert_eq!(planned.instance.identities, expected.identities);
    assert!(output.references.iter().all(|reference| {
        reference.owner_artifact_id == expected.target_instance_id
            && reference.reference.source_instance_id == expected.target_instance_id
    }));
    assert_eq!(
        output
            .artifacts
            .iter()
            .map(|artifact| artifact.planned.order)
            .collect::<Vec<_>>(),
        (0..=input.sources.len())
            .map(|index| if index == 0 { 0 } else { 9 + index as u64 })
            .collect::<Vec<_>>()
    );
}
