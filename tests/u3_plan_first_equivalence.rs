use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::value::Value as DicomValue;
use dicom_dictionary_std::tags;
use dicom_object::open_file;
use synth_dicom_gen::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use synth_dicom_gen::corpus_plan::{FragmentationPolicy, PlannedArtifact};
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlan, CuratedScCorpusPlanProvider, CuratedScPlanRequest,
    CuratedScSelection,
};
use synth_dicom_gen::executor::materialization::{
    AuxiliaryMaterializationHandler, AuxiliaryPayload, MaterializationDispatcher,
    MaterializationError,
};
use synth_dicom_gen::executor::services::{
    ArtifactExecutionBindings, ByteBinding, CodecRequest, EncodedFrameResult,
    MaterializationRequest, SlotExecutionBinding, StagedAssetRegistry,
};
use synth_dicom_gen::recipes::RecipeCatalog;
use synth_dicom_gen::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};
use serde_json::{Value, json};

const SEED: u64 = 7;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoots(Vec<PathBuf>);

impl TempRoots {
    fn absent(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dicom-test-suite-u3-equivalence-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }
}

impl Drop for TempRoots {
    fn drop(&mut self) {
        for root in &self.0 {
            let _ = fs::remove_dir_all(root);
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
        panic!("the SC equivalence slice contains no auxiliary artifacts")
    }
}

fn registry() -> Value {
    serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap()
}

fn is_feature_free_byte_stable_native(case: &Value) -> bool {
    case["status"] == "implemented"
        && case["determinism"] == "byte_stable"
        && case["provider"]["kind"] == "rust_native"
        && case["provider"]["id"] == "rust_native"
        && ["features", "external_codecs", "external_validators"]
            .iter()
            .all(|field| {
                case["requirements"][field]
                    .as_array()
                    .is_some_and(Vec::is_empty)
            })
}

fn migrated_provider(provider: &str) -> bool {
    matches!(provider, "native.sc_plan" | "native.metadata_sc_plan")
}

fn expected_inventory(registry: &Value, recipes: &RecipeCatalog) -> Vec<(String, String)> {
    let mut expected = Vec::new();
    for case in registry["cases"].as_array().unwrap() {
        if !is_feature_free_byte_stable_native(case) {
            continue;
        }
        let case_id = case["case_id"].as_str().unwrap();
        let Some(identity) = recipes.binding_for_case(case_id) else {
            continue;
        };
        let recipe = &recipes.recipes()[identity];
        if !migrated_provider(&recipe.plan_provider_id) {
            continue;
        }
        let mut artifacts = recipe
            .dicom
            .as_ref()
            .unwrap()
            .artifacts
            .iter()
            .collect::<Vec<_>>();
        artifacts.sort_by_key(|artifact| artifact.order);
        for artifact in artifacts {
            expected.push((
                case_id.into(),
                artifact.output.path.as_ref().unwrap().clone(),
            ));
        }
    }
    expected
}

fn plan_cases(
    provider: &CuratedScCorpusPlanProvider,
    case_ids: impl IntoIterator<Item = String>,
) -> CuratedScCorpusPlan {
    provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(case_ids.into_iter().collect()),
            seed: SEED,
            max_parallelism: 4,
        })
        .unwrap()
}

fn generated_root(profile: &str, root: &Path) -> Value {
    let run = prepare_generation_run(GenerateOptions {
        profile: profile.into(),
        out_dir: root.to_path_buf(),
        seed: SEED,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap()
}

fn manifest_entries(manifest: &Value) -> &[Value] {
    manifest["files"].as_array().unwrap()
}

fn bundle_inventory(bundle: &CuratedScCorpusPlan) -> Vec<(String, String)> {
    bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| {
            let PlannedArtifact::Dicom(artifact) = artifact else {
                panic!("curated SC provider emitted a non-DICOM artifact")
            };
            (
                artifact.case_binding.as_ref().unwrap().case_id.clone(),
                artifact.output.relative_path.as_str().into(),
            )
        })
        .collect()
}

fn resolve_codec_request(request: &CodecRequest) -> Vec<EncodedFrameResult> {
    assert_eq!(request.backend_id, NativeRleLosslessEncoder::BACKEND_ID);
    let bits_stored = request.parameters["bits_stored"].as_u64().unwrap() as u16;
    let encoder = NativeRleLosslessEncoder::new();
    request
        .frames
        .iter()
        .map(|frame| {
            let ByteBinding::Inline {
                bytes: native_frame,
                sha256,
            } = &frame.bytes
            else {
                panic!("curated native codec input is not inline")
            };
            assert_eq!(sha256_hex(native_frame), *sha256);
            let bytes = encoder
                .encode_frame(FrameEncodeInput {
                    native_frame,
                    rows: u16::try_from(frame.rows).unwrap(),
                    columns: u16::try_from(frame.columns).unwrap(),
                    samples_per_pixel: frame.samples_per_pixel,
                    bits_allocated: frame.bits_allocated,
                    bits_stored,
                    photometric_interpretation: &frame.photometric_interpretation,
                })
                .unwrap()
                .bytes;
            let encoded_sha256 = sha256_hex(&bytes);
            EncodedFrameResult {
                frame_number: frame.frame_number,
                encoded_size_bytes: bytes.len() as u64,
                bytes: ByteBinding::Inline {
                    bytes,
                    sha256: encoded_sha256.clone(),
                },
                encoded_sha256,
            }
        })
        .collect()
}

fn executable_bindings(bindings: &ArtifactExecutionBindings) -> ArtifactExecutionBindings {
    let mut resolved = bindings.clone();
    for binding in resolved.slots.values_mut() {
        if let SlotExecutionBinding::CodecRequest { request } = binding {
            *binding = SlotExecutionBinding::EncodedFrames {
                frames: resolve_codec_request(request),
            };
        }
    }
    resolved
}

fn identity_facts(path: &Path) -> Value {
    let object = open_file(path).unwrap();
    let string = |tag| {
        object
            .element(tag)
            .unwrap()
            .to_str()
            .unwrap()
            .trim_end_matches([' ', '\0'])
            .to_owned()
    };
    json!({
        "media_storage_sop_class_uid": object.meta().media_storage_sop_class_uid().trim_end_matches('\0'),
        "media_storage_sop_instance_uid": object.meta().media_storage_sop_instance_uid().trim_end_matches('\0'),
        "transfer_syntax_uid": object.meta().transfer_syntax().trim_end_matches('\0'),
        "implementation_class_uid": object.meta().implementation_class_uid().trim_end_matches('\0'),
        "study_instance_uid": string(tags::STUDY_INSTANCE_UID),
        "series_instance_uid": string(tags::SERIES_INSTANCE_UID),
        "sop_instance_uid": string(tags::SOP_INSTANCE_UID),
    })
}

fn pixel_encoding_facts(path: &Path) -> Value {
    let object = open_file(path).unwrap();
    let pixel = object.element(tags::PIXEL_DATA).unwrap();
    match pixel.value() {
        DicomValue::PixelSequence(sequence) => json!({
            "kind": "encapsulated",
            "vr": pixel.vr().to_string(),
            "basic_offset_table": sequence.offset_table(),
            "fragment_lengths": sequence.fragments().iter().map(Vec::len).collect::<Vec<_>>(),
            "fragment_sha256": sequence.fragments().iter().map(|bytes| sha256_hex(bytes)).collect::<Vec<_>>(),
            "extended_offset_table": object.element(tags::EXTENDED_OFFSET_TABLE).ok().map(|element| element.value().to_bytes().unwrap().into_owned()),
            "extended_offset_table_lengths": object.element(tags::EXTENDED_OFFSET_TABLE_LENGTHS).ok().map(|element| element.value().to_bytes().unwrap().into_owned()),
        }),
        value => json!({
            "kind": "native",
            "vr": pixel.vr().to_string(),
            "value_length": value.to_bytes().unwrap().len(),
            "value_sha256": sha256_hex(value.to_bytes().unwrap().as_ref()),
        }),
    }
}

fn qualify_bundle(
    bundle: &CuratedScCorpusPlan,
    manifest: &Value,
    current_root: &Path,
    direct_root: &Path,
    dispatcher: &MaterializationDispatcher,
) {
    bundle.plan.validate().unwrap();
    assert_eq!(
        bundle.plan.canonical_sha256().unwrap(),
        bundle.plan.canonical_sha256().unwrap()
    );
    let selected_paths = bundle_inventory(bundle)
        .into_iter()
        .map(|(_, path)| path)
        .collect::<BTreeSet<_>>();
    let generated_order = manifest_entries(manifest)
        .iter()
        .filter_map(|entry| {
            let path = entry["path"].as_str()?;
            selected_paths.contains(path).then(|| path.to_owned())
        })
        .collect::<Vec<_>>();
    let planned_order = bundle_inventory(bundle)
        .into_iter()
        .map(|(_, path)| path)
        .collect::<Vec<_>>();
    let order_matches = generated_order == planned_order;

    let assets = StagedAssetRegistry::default();
    for artifact in &bundle.plan.artifacts {
        let PlannedArtifact::Dicom(dicom) = artifact else {
            unreachable!()
        };
        let actual_binding = &bundle.bindings[&dicom.logical_id];
        actual_binding.validate(&assets).unwrap();
        assert!(actual_binding.slots.contains_key("pixels"));
        let request = MaterializationRequest {
            artifact: artifact.clone(),
            bindings: executable_bindings(actual_binding),
        };
        let result = dispatcher
            .dispatch(&request, &assets)
            .unwrap_or_else(|error| {
                panic!(
                    "{} failed direct materialization: {error}",
                    dicom.logical_id
                )
            });
        let path = dicom.output.relative_path.as_str();
        let direct_path = direct_root.join(path);
        let current_path = current_root.join(path);
        assert_eq!(
            fs::read(&direct_path).unwrap(),
            fs::read(&current_path).unwrap(),
            "{} changed bytes",
            dicom.logical_id
        );
        assert_eq!(identity_facts(&direct_path), identity_facts(&current_path));
        assert_eq!(
            pixel_encoding_facts(&direct_path),
            pixel_encoding_facts(&current_path),
            "{} changed native/fragment/BOT/EOT facts",
            dicom.logical_id
        );
        let evidence = result
            .evidence
            .iter()
            .find(|evidence| evidence.evidence_kind == "materialized_instance_plan")
            .unwrap();
        let materialized_plan_sha256 = evidence.claims["materialized_instance_plan_sha256"]
            .as_str()
            .unwrap();
        if dicom.encoding.fragmentation == FragmentationPolicy::Native {
            assert_eq!(materialized_plan_sha256, dicom.instance.canonical_sha256());
        }
        assert_eq!(materialized_plan_sha256.len(), 64);
        let output_sha256 = sha256_hex(&fs::read(&direct_path).unwrap());
        assert_eq!(
            evidence.claims["materialized_artifact_sha256"],
            output_sha256
        );
        let manifest_entry = manifest_entries(manifest)
            .iter()
            .find(|entry| entry["path"] == path)
            .unwrap();
        assert_eq!(manifest_entry["sha256"], output_sha256);
        if let Some(plan_sha256) = manifest_entry["resolved_plan_sha256"].as_str() {
            assert_eq!(plan_sha256, dicom.instance.canonical_sha256());
        }
    }
    assert!(order_matches, "migrated path order drifted");
}

#[test]
fn curated_provider_slice_is_plan_first_and_exactly_equivalent_to_current_generation() {
    let current_all_root = TempRoots::absent("current-all");
    let current_legacy_root = TempRoots::absent("current-legacy");
    let direct_root = TempRoots::absent("direct");
    let _cleanup = TempRoots(vec![
        current_all_root.clone(),
        current_legacy_root.clone(),
        direct_root.clone(),
    ]);
    let all_manifest = generated_root("all", &current_all_root);
    let legacy_manifest = generated_root("legacy", &current_legacy_root);
    let registry = registry();
    let recipes = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let expected = expected_inventory(&registry, &recipes);
    assert!(!expected.is_empty());

    assert!(!direct_root.exists());
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let legacy_case_ids = manifest_entries(&legacy_manifest)
        .iter()
        .filter_map(|entry| entry["case_id"].as_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let expected_case_ids = expected
        .iter()
        .map(|(case_id, _)| case_id.clone())
        .collect::<BTreeSet<_>>();
    let legacy = plan_cases(
        &provider,
        expected_case_ids.intersection(&legacy_case_ids).cloned(),
    );
    let all = plan_cases(
        &provider,
        expected_case_ids.difference(&legacy_case_ids).cloned(),
    );
    assert!(
        !direct_root.exists(),
        "production planning created the output root"
    );

    let actual = bundle_inventory(&all)
        .into_iter()
        .chain(bundle_inventory(&legacy))
        .collect::<Vec<_>>();
    let expected_counts = expected.iter().fold(BTreeMap::new(), |mut counts, item| {
        *counts.entry(item.clone()).or_insert(0usize) += 1;
        counts
    });
    let actual_counts = actual.iter().fold(BTreeMap::new(), |mut counts, item| {
        *counts.entry(item.clone()).or_insert(0usize) += 1;
        counts
    });
    assert_eq!(actual_counts, expected_counts);
    assert!(actual_counts.values().all(|count| *count == 1));
    for bundle in [&all, &legacy] {
        assert_eq!(bundle.bindings.len(), bundle.plan.artifacts.len());
        assert_eq!(
            bundle.native_content_requests.len(),
            bundle.plan.artifacts.len()
        );
        assert!(bundle.pending.is_empty());
    }

    fs::create_dir(&direct_root).unwrap();
    let dispatcher = MaterializationDispatcher::new(&direct_root, Arc::new(NoAuxiliary)).unwrap();
    qualify_bundle(
        &all,
        &all_manifest,
        &current_all_root,
        &direct_root,
        &dispatcher,
    );
    qualify_bundle(
        &legacy,
        &legacy_manifest,
        &current_legacy_root,
        &direct_root,
        &dispatcher,
    );
}

#[test]
fn native_u3_planners_have_no_read_back_or_materialization_escape_hatch() {
    for path in [
        "src/recipes/sc.rs",
        "src/recipes/metadata_sc.rs",
        "src/recipes/encoding.rs",
    ] {
        let source = fs::read_to_string(path).unwrap();
        for forbidden in [
            "std::fs",
            "std::path",
            "open_file",
            "Part10Materializer",
            "MaterializationDispatcher",
            "resolved_plan_from_curated_dataset",
            "crate::generator",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} contains forbidden planner dependency {forbidden}"
            );
        }
    }
    let source = fs::read_to_string("src/curated_plan.rs").unwrap();
    for forbidden in [
        "open_file",
        "Part10Materializer",
        "MaterializationDispatcher",
        "resolved_plan_from_curated_dataset",
        "crate::generator",
    ] {
        assert!(
            !source.contains(forbidden),
            "curated planner contains {forbidden}"
        );
    }
}
