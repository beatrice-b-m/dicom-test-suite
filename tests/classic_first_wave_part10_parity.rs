use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use synth_dicom_gen::codecs::{NativeRleLosslessEncoder, RLE_LOSSLESS_TRANSFER_SYNTAX_UID};
use synth_dicom_gen::composition::{
    CompositionUidRole, ContentMaterialization, Part10Materializer, TemplateCatalog, TemplateId,
};
use synth_dicom_gen::corpus_plan::{ImplementationIdentityPlan, OffsetTablePolicy};
use synth_dicom_gen::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData};
use synth_dicom_gen::executor::cancellation::CancellationToken;
use synth_dicom_gen::executor::native_codec::execute_native_rle;
use synth_dicom_gen::executor::services::{ByteBinding, CodecRequest, NativeFrameBinding};
use synth_dicom_gen::native_pixel::PhotometricInterpretation;
use synth_dicom_gen::recipes::classic_ct::plan_ct_recipe;
use synth_dicom_gen::recipes::classic_dx_mg::plan_dx_mg_recipe;
use synth_dicom_gen::recipes::classic_mr_cr::plan_mr_cr_recipe;
use synth_dicom_gen::recipes::{
    CLASSIC_PIXEL_SLOT, CaseRecipe, ClassicInstanceRequest, ClassicResolvedPlanInput,
    OrderedSeriesProvider, RecipeCatalog, encoding_plan_from_recipe,
    resolved_classic_instance_plan,
};
use synth_dicom_gen::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};

const SEED: u64 = 7;
const EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1";
const IMPLEMENTATION_VERSION_NAME: &str = "DICOMTS010";
const FIRST_WAVE_ALGORITHMS: [&str; 3] = [
    "algorithm.classic_ct",
    "algorithm.classic_mr_cr",
    "algorithm.classic_dx_mg",
];
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoots(Vec<PathBuf>);

impl TempRoots {
    fn absent(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dicom-test-suite-classic-first-wave-{label}-{}-{}",
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

fn first_wave_recipes(catalog: &RecipeCatalog) -> Vec<CaseRecipe> {
    let mut recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            recipe.plan_provider_id == "native.classic_plan"
                && recipe.dicom.as_ref().is_some_and(|dicom| {
                    !dicom.artifacts.is_empty()
                        && dicom.artifacts.iter().all(|artifact| {
                            artifact
                                .algorithm_provider_id
                                .as_deref()
                                .is_some_and(|id| FIRST_WAVE_ALGORITHMS.contains(&id))
                        })
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    recipes.sort_by_key(|recipe| recipe.planning_order);
    recipes
}

fn lane_requests(recipe: &CaseRecipe, lock_sha256: &str) -> Vec<ClassicInstanceRequest> {
    let candidates = [
        plan_ct_recipe(recipe, lock_sha256, SEED)
            .unwrap()
            .map(|requests| ("ct", requests)),
        plan_mr_cr_recipe(recipe, lock_sha256, SEED)
            .unwrap()
            .map(|requests| ("mr_cr", requests)),
        plan_dx_mg_recipe(recipe, lock_sha256, SEED)
            .unwrap()
            .map(|requests| ("dx_mg", requests)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    assert_eq!(
        candidates.len(),
        1,
        "{} must be owned by exactly one first-wave provider, got {:?}",
        recipe.binding.case_id,
        candidates
            .iter()
            .map(|(provider, _)| *provider)
            .collect::<Vec<_>>()
    );
    candidates.into_iter().next().unwrap().1
}

fn photometric(value: PhotometricInterpretation) -> &'static str {
    match value {
        PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        PhotometricInterpretation::Rgb => "RGB",
        PhotometricInterpretation::YbrFull => "YBR_FULL",
        PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
    }
}

fn apply_registered_rle(
    plan: &mut synth_dicom_gen::composition::ResolvedInstancePlan,
    planned: &synth_dicom_gen::recipes::ClassicPlannedInstance,
    offset_table: OffsetTablePolicy,
) {
    let native = &planned.pixels.content;
    assert_eq!(native.plan.shape.frames, 1);
    let request = CodecRequest {
        request_id: "classic_first_wave_rle".into(),
        artifact_id: planned.logical_id.clone(),
        slot: planned.pixels.slot.clone(),
        backend_id: NativeRleLosslessEncoder::BACKEND_ID.into(),
        source_transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.into(),
        target_transfer_syntax_uid: RLE_LOSSLESS_TRANSFER_SYNTAX_UID.into(),
        frames: vec![NativeFrameBinding {
            frame_number: 1,
            bytes: ByteBinding::Inline {
                bytes: native.unpadded_bytes.clone(),
                sha256: sha256_hex(&native.unpadded_bytes),
            },
            rows: native.plan.shape.rows,
            columns: native.plan.shape.columns,
            samples_per_pixel: native.plan.shape.samples_per_pixel,
            bits_allocated: native.plan.shape.bits_allocated,
            photometric_interpretation: photometric(native.plan.shape.photometric_interpretation)
                .into(),
        }],
        parameters: BTreeMap::from([(
            "bits_stored".into(),
            Value::from(native.plan.shape.bits_stored),
        )]),
    };
    let outcome = execute_native_rle(&request, &CancellationToken::new(), |binding| {
        let ByteBinding::Inline { bytes, sha256 } = binding else {
            panic!("first-wave native content must be inline")
        };
        assert_eq!(&sha256_hex(bytes), sha256);
        Ok(bytes.clone())
    })
    .unwrap();
    assert_eq!(
        outcome.result.backend.backend_id,
        NativeRleLosslessEncoder::BACKEND_ID
    );
    assert_eq!(outcome.result.frames.len(), 1);
    assert_eq!(
        outcome.decoded_frame_sha256[&1],
        native.frames[0].decoded_sha256
    );
    let encoded_frames = outcome
        .result
        .frames
        .into_iter()
        .map(|frame| match frame.bytes {
            ByteBinding::Inline { bytes, sha256 } => {
                assert_eq!(sha256_hex(&bytes), sha256);
                bytes
            }
            _ => panic!("registered native RLE returned non-inline bytes"),
        })
        .collect::<Vec<_>>();
    let basic_policy = match offset_table {
        OffsetTablePolicy::PopulatedBasic => BasicOffsetTablePolicy::Populated,
        OffsetTablePolicy::EmptyBasic => BasicOffsetTablePolicy::Empty,
        other => panic!("unsupported first-wave RLE offset-table policy {other:?}"),
    };
    let encapsulated =
        EncapsulatedPixelData::one_fragment_per_frame(&encoded_frames, basic_policy).unwrap();
    let encoded = encoded_frames.concat();
    let content = &mut plan.content[0];
    content.kind = "encapsulated_pixels".into();
    content.vr = synth_dicom_gen::composition::DicomVr::OB;
    content.size_bytes = encoded.len() as u64;
    content.sha256 = sha256_hex(&encoded);
    content.materialization = Some(ContentMaterialization::Encapsulated {
        basic_offset_table: encapsulated.basic_offset_table.offsets,
        fragments: encapsulated.fragment_payloads,
    });
}

fn manifest_uid_facts(entry: &Value) -> Value {
    let uids = &entry["uids"];
    json!({
        "study": uids.get("study_instance_uid").cloned().unwrap_or(Value::Null),
        "series": uids.get("series_instance_uid").cloned().unwrap_or(Value::Null),
        "sop": uids.get("sop_instance_uid").cloned().unwrap_or(Value::Null),
        "implementation": uids.get("implementation_class_uid").cloned().unwrap_or(Value::Null),
        "frame_of_reference": uids.get("frame_of_reference_uid").cloned().unwrap_or(Value::Null),
    })
}

fn planned_uid_facts(plan: &synth_dicom_gen::composition::ResolvedInstancePlan) -> Value {
    let identity = |role| {
        plan.identities
            .get(&role, 0)
            .map(|value| Value::String(value.into()))
            .unwrap_or(Value::Null)
    };
    json!({
        "study": identity(CompositionUidRole::StudyInstance),
        "series": identity(CompositionUidRole::SeriesInstance),
        "sop": identity(CompositionUidRole::SopInstance),
        "implementation": identity(CompositionUidRole::ImplementationClass),
        "frame_of_reference": identity(CompositionUidRole::FrameOfReference),
    })
}

fn fresh_generator(root: &Path) -> Value {
    let run = prepare_generation_run(GenerateOptions {
        profile: "all".into(),
        out_dir: root.into(),
        seed: SEED,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap()
}

#[test]
fn first_wave_classic_plans_match_current_part10_bytes_and_order() {
    let generated_root = TempRoots::absent("current");
    let direct_root = TempRoots::absent("direct");
    let sentinel_root = TempRoots::absent("planning-sentinel");
    let _cleanup = TempRoots(vec![
        generated_root.clone(),
        direct_root.clone(),
        sentinel_root.clone(),
    ]);
    assert!(!sentinel_root.exists());

    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let templates = TemplateCatalog::load("templates/catalog.json").unwrap();
    let lock_sha256 = sha256_hex(&fs::read("standards.lock.json").unwrap());
    let recipes = first_wave_recipes(&catalog);
    assert!(!recipes.is_empty());

    // Exercise every lane before any direct output root exists.
    for recipe in &recipes {
        let requests = lane_requests(recipe, &lock_sha256);
        assert!(
            requests
                .iter()
                .all(|request| request.pixels.slot == CLASSIC_PIXEL_SLOT)
        );
        OrderedSeriesProvider.plan(requests).unwrap();
    }
    assert!(!sentinel_root.exists());
    assert!(!direct_root.exists());

    let manifest = fresh_generator(&generated_root);
    fs::create_dir(&direct_root).unwrap();
    let mut expected_inventory = Vec::new();
    let mut selected_cases = BTreeSet::new();

    for recipe in &recipes {
        selected_cases.insert(recipe.binding.case_id.clone());
        let artifacts = &recipe.dicom.as_ref().unwrap().artifacts;
        let requests = lane_requests(recipe, &lock_sha256);
        assert!(
            requests
                .iter()
                .all(|request| request.pixels.slot == CLASSIC_PIXEL_SLOT)
        );
        let planned = OrderedSeriesProvider.plan(requests).unwrap();
        assert_eq!(planned.len(), artifacts.len());
        for (artifact, planned) in artifacts.iter().zip(planned) {
            let path = artifact.output.path.as_ref().unwrap();
            assert_eq!(planned.output_relative_path.as_str(), path);
            expected_inventory.push((recipe.binding.case_id.clone(), path.clone()));

            let template_reference = artifact.template.as_ref().unwrap();
            let template = templates
                .resolve_qualified(
                    &TemplateId(template_reference.template_id.clone()),
                    Some(template_reference.template_version.parse().unwrap()),
                )
                .unwrap();
            let implementation_class_uid = planned
                .identities
                .get(&CompositionUidRole::ImplementationClass, 0)
                .unwrap()
                .to_owned();
            let encoding = encoding_plan_from_recipe(
                &artifact.encoding,
                ImplementationIdentityPlan {
                    class_uid: implementation_class_uid,
                    version_name: Some(IMPLEMENTATION_VERSION_NAME.into()),
                },
            )
            .unwrap();
            let planned_for_codec = planned.clone();
            let mut resolved = resolved_classic_instance_plan(ClassicResolvedPlanInput {
                planned,
                template,
                transfer_syntax_uid: &encoding.transfer_syntax_uid,
                encoding_backend_id: &encoding.backend_id,
            })
            .unwrap();
            if encoding.transfer_syntax_uid == RLE_LOSSLESS_TRANSFER_SYNTAX_UID {
                assert_eq!(encoding.backend_id, "encoding.native.rle_lossless");
                apply_registered_rle(&mut resolved, &planned_for_codec, encoding.offset_table);
            }
            assert!(resolved.references.is_empty());
            assert!(planned_for_codec.dependencies.is_empty());

            let direct_path = direct_root.join(path);
            Part10Materializer
                .materialize(&resolved, &direct_path)
                .unwrap();
            let current_path = generated_root.join(path);
            assert_eq!(
                fs::read(&direct_path).unwrap(),
                fs::read(&current_path).unwrap(),
                "{} / {} changed complete Part 10 bytes",
                recipe.binding.case_id,
                artifact.logical_id
            );
            let entry = manifest["files"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["path"].as_str() == Some(path))
                .unwrap();
            assert_eq!(entry["case_id"], recipe.binding.case_id);
            assert_eq!(entry["references"], json!([]));
            assert_eq!(manifest_uid_facts(entry), planned_uid_facts(&resolved));
        }
    }

    let current_inventory = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| {
            let case_id = entry["case_id"].as_str()?;
            selected_cases.contains(case_id).then(|| {
                (
                    case_id.to_owned(),
                    entry["path"].as_str().unwrap().to_owned(),
                )
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(current_inventory, expected_inventory);
    assert!(!sentinel_root.exists());
}
