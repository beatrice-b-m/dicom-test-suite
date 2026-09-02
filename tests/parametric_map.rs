use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::{Tag, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "derived/parametric-map/float32_ct_derived_explicit_le";
const RELATIVE_PATH: &str =
    "derived/parametric-map/float32_ct_derived_explicit_le/parametric-map.dcm";
const FLOAT64_CASE_ID: &str = "derived/parametric-map/float64_ct_derived_explicit_le";
const FLOAT64_RELATIVE_PATH: &str =
    "derived/parametric-map/float64_ct_derived_explicit_le/parametric-map-float64.dcm";
const PARAMETRIC_MAP_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.30";
const TAG_DERIVATION_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x9124);
const TAG_SOURCE_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x2112);
const TAG_REFERENCED_SOP_CLASS_UID: Tag = Tag(0x0008, 0x1150);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

#[test]
fn promoted_float32_parametric_map_satisfies_external_proof_contract() {
    let first_root = unique_temp_dir("parametric-map-a");
    let second_root = unique_temp_dir("parametric-map-b");
    let first = generate_extended(&first_root);
    let second = generate_extended(&second_root);

    let first_file = file_for_case(&first, CASE_ID);
    let second_file = file_for_case(&second, CASE_ID);
    match (first_file, second_file) {
        (None, None) => {
            assert_explicitly_unavailable(&first, &first_root);
            assert_explicitly_unavailable(&second, &second_root);
            assert_eq!(skipped_for_case(&first), skipped_for_case(&second));
        }
        (Some(first_file), Some(second_file)) => {
            assert_generated_contract(&first_root, first_file);
            assert_generated_contract(&second_root, second_file);

            let mut first_semantics = first_file.clone();
            let mut second_semantics = second_file.clone();
            // semantic_stable deliberately makes no claim about container bytes.
            for entry in [&mut first_semantics, &mut second_semantics] {
                let object = entry.as_object_mut().expect("file entry must be an object");
                object.remove("sha256");
                object.remove("size_bytes");
            }
            assert_eq!(
                first_semantics, second_semantics,
                "all non-byte-level manifest semantics must be stable for seed 7"
            );

            let first_float = float_bytes(&first_root.join(RELATIVE_PATH));
            let second_float = float_bytes(&second_root.join(RELATIVE_PATH));
            assert_eq!(
                first_float, second_float,
                "the derived float payload itself must remain semantically stable"
            );

            let first_float64 = file_for_case(&first, FLOAT64_CASE_ID)
                .expect("implemented float64 Parametric Map must be generated");
            let second_float64 = file_for_case(&second, FLOAT64_CASE_ID)
                .expect("implemented float64 Parametric Map must be generated twice");
            assert_generated_float64_contract(&first_root, first_float64);
            assert_generated_float64_contract(&second_root, second_float64);
            assert_eq!(
                double_float_bytes(&first_root.join(FLOAT64_RELATIVE_PATH)),
                double_float_bytes(&second_root.join(FLOAT64_RELATIVE_PATH)),
                "the derived binary64 payload must remain semantically stable"
            );

            for root in [&first_root, &second_root] {
                let validation = synth_dicom_gen::validate_generated_root(root)
                    .expect("generated extended root should validate");
                assert!(
                    validation.failures.is_empty(),
                    "generated root validation failed: {:?}",
                    validation.failures
                );
            }
        }
        _ => panic!("the same prepared backend must not change availability between two runs"),
    }

    fs::remove_dir_all(first_root).expect("first temporary root should be removable");
    fs::remove_dir_all(second_root).expect("second temporary root should be removable");
}

fn assert_generated_float64_contract(root: &Path, file: &Value) {
    assert_eq!(file["path"].as_str(), Some(FLOAT64_RELATIVE_PATH));
    assert_eq!(
        file.pointer("/image/sample_type").and_then(Value::as_str),
        Some("float64")
    );
    assert_eq!(
        file.pointer("/image/bits_allocated")
            .and_then(Value::as_u64),
        Some(64)
    );
    assert_eq!(
        file.pointer("/pixel_data/vr").and_then(Value::as_str),
        Some("OD")
    );
    assert_eq!(
        file.pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(96)
    );

    let path = root.join(FLOAT64_RELATIVE_PATH);
    let object = open_file(&path).expect("float64 Parametric Map should reopen");
    let element = object
        .element(tags::DOUBLE_FLOAT_PIXEL_DATA)
        .expect("Double Float Pixel Data must exist");
    assert_eq!(element.vr(), VR::OD);
    for forbidden in [
        tags::PIXEL_DATA,
        tags::FLOAT_PIXEL_DATA,
        tags::BITS_STORED,
        tags::HIGH_BIT,
        tags::PIXEL_REPRESENTATION,
    ] {
        assert!(object.element_opt(forbidden).unwrap().is_none());
    }

    let bytes = element.value().to_bytes().expect("native OD bytes");
    assert_eq!(bytes.len(), 96);
    let bits = bits64_by_frame(&bytes, 4);
    assert_eq!(
        bits,
        nested_u64_frames(&file["expected_semantics"]["little_endian_float64_bits"])
    );
    assert_eq!(
        bits,
        nested_u64_frames(&file["recipe"]["recipe_parameters"]["little_endian_float64_bits"])
    );
    let hashes = bytes
        .chunks_exact(32)
        .map(synth_dicom_gen::sha256_hex)
        .collect::<Vec<_>>();
    let expected_hashes = file["pixel_data"]["frame_hashes"]
        .as_array()
        .expect("frame hashes")
        .iter()
        .map(|value| value.as_str().expect("frame hash").to_string())
        .collect::<Vec<_>>();
    assert_eq!(hashes, expected_hashes);
    assert_functional_groups_and_references(root, file, &object);
}

fn assert_explicitly_unavailable(manifest: &Value, root: &Path) {
    let skipped = skipped_for_case(manifest);
    assert_eq!(skipped["status"].as_str(), Some("unavailable"));
    assert_eq!(
        skipped["reason_code"].as_str(),
        Some("external_backend_unavailable")
    );
    assert_eq!(skipped["recheck_phase"].as_str(), Some("phase-1"));
    assert!(
        !skipped["message"].as_str().unwrap_or_default().is_empty(),
        "unavailability must retain the backend discovery reason"
    );
    assert!(!root.join(RELATIVE_PATH).exists());
}

fn assert_generated_contract(root: &Path, file: &Value) {
    assert_eq!(file["path"].as_str(), Some(RELATIVE_PATH));
    assert_eq!(file["determinism"].as_str(), Some("semantic_stable"));
    assert_eq!(
        file.pointer("/dicom/sop_class_uid").and_then(Value::as_str),
        Some(PARAMETRIC_MAP_STORAGE)
    );
    assert_eq!(
        file.pointer("/dicom/transfer_syntax_uid")
            .and_then(Value::as_str),
        Some(uids::EXPLICIT_VR_LITTLE_ENDIAN)
    );

    let backend = file["generation_backend"]
        .as_object()
        .expect("generated proof must record backend provenance");
    assert_eq!(backend["backend_id"].as_str(), Some("highdicom_pydicom"));
    assert_eq!(backend["protocol_version"].as_str(), Some("0.1.0"));
    assert_eq!(backend["determinism"].as_str(), Some("semantic_stable"));
    for field in [
        "dependency_lock_sha256",
        "executable_fingerprint",
        "entrypoint_fingerprint",
        "environment_fingerprint",
    ] {
        let fingerprint = backend[field]
            .as_str()
            .unwrap_or_else(|| panic!("backend {field} must be a string"));
        assert_eq!(fingerprint.len(), 64, "backend {field} must be SHA-256");
        assert!(fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    assert!(
        backend["name"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        backend["version"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(
        backend["runtime_identity"]["backend_id"].as_str(),
        Some("highdicom_pydicom")
    );
    assert_eq!(
        backend["runtime_identity"]["protocol_version"].as_str(),
        Some("0.1.0")
    );
    assert!(
        backend["runtime_identity"]["distributions"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );

    let references = file["references"]
        .as_array()
        .expect("proof references must be an array");
    assert_eq!(references.len(), 3);
    assert!(references.iter().all(|reference| {
        reference["relationship"].as_str() == Some("source_image")
            && reference["sop_class_uid"].as_str() == Some(uids::CT_IMAGE_STORAGE)
    }));

    assert_eq!(
        file.pointer("/image/sample_type").and_then(Value::as_str),
        Some("float32")
    );
    assert_eq!(
        file.pointer("/image/bits_allocated")
            .and_then(Value::as_u64),
        Some(32)
    );
    assert_eq!(
        file.pointer("/image/frames").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        file.pointer("/pixel_data/vr").and_then(Value::as_str),
        Some("OF")
    );
    assert_eq!(
        file.pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(48)
    );

    let path = root.join(RELATIVE_PATH);
    let object = open_file(&path).expect("promoted Parametric Map should reopen");
    assert_eq!(
        object.meta().media_storage_sop_class_uid(),
        PARAMETRIC_MAP_STORAGE
    );
    assert_eq!(
        object.meta().transfer_syntax(),
        uids::EXPLICIT_VR_LITTLE_ENDIAN
    );
    let float_element = object
        .element(tags::FLOAT_PIXEL_DATA)
        .expect("Parametric Map must contain Float Pixel Data");
    assert_eq!(float_element.vr(), VR::OF);
    for forbidden in [
        tags::PIXEL_DATA,
        tags::DOUBLE_FLOAT_PIXEL_DATA,
        tags::BITS_STORED,
        tags::HIGH_BIT,
        tags::PIXEL_REPRESENTATION,
    ] {
        assert!(
            object
                .element_opt(forbidden)
                .expect("tag lookup should work")
                .is_none()
        );
    }
    let float_bytes = float_element
        .value()
        .to_bytes()
        .expect("Float Pixel Data should be native bytes");
    assert_eq!(float_bytes.len(), 48);
    assert_eq!(
        file["sha256"].as_str(),
        Some(synth_dicom_gen::sha256_hex(&fs::read(&path).expect("PM bytes should read")).as_str())
    );

    let manifest_bits =
        nested_u32_frames(&file["expected_semantics"]["little_endian_float32_bits"]);
    let actual_bits = bits_by_frame(&float_bytes, 4);
    assert_eq!(actual_bits, manifest_bits);
    assert_eq!(
        actual_bits,
        nested_u32_frames(&file["recipe"]["recipe_parameters"]["little_endian_float32_bits"])
    );
    let expected_from_sources = recompute_from_sources(root, references, file, &object);
    assert_eq!(actual_bits, expected_from_sources);

    let manifest_hashes = file["pixel_data"]["frame_hashes"]
        .as_array()
        .expect("frame hashes must be an array")
        .iter()
        .map(|value| value.as_str().expect("frame hash must be text"))
        .collect::<Vec<_>>();
    let actual_hashes = float_bytes
        .chunks_exact(16)
        .map(synth_dicom_gen::sha256_hex)
        .collect::<Vec<_>>();
    assert_eq!(actual_hashes, manifest_hashes);

    assert_functional_groups_and_references(root, file, &object);
}

fn assert_functional_groups_and_references(
    root: &Path,
    file: &Value,
    object: &dicom_object::DefaultDicomObject,
) {
    let dimension_uid = file["expected_semantics"]["dimension_organization_uid"]
        .as_str()
        .expect("dimension UID must be text");
    let organizations = sequence(object, tags::DIMENSION_ORGANIZATION_SEQUENCE);
    assert_eq!(organizations.len(), 1);
    assert_eq!(
        text(&organizations[0], tags::DIMENSION_ORGANIZATION_UID),
        dimension_uid
    );
    let indexes = sequence(object, tags::DIMENSION_INDEX_SEQUENCE);
    assert_eq!(indexes.len(), 1);
    assert_eq!(
        text(&indexes[0], tags::DIMENSION_ORGANIZATION_UID),
        dimension_uid
    );

    let shared = sequence(object, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE);
    assert_eq!(shared.len(), 1);
    let mappings = shared[0]
        .element(tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE)
        .expect("shared group must contain RWVM")
        .items()
        .expect("RWVM must be a sequence");
    assert_eq!(mappings.len(), 1);
    let mapping = &mappings[0];
    assert_eq!(
        text(mapping, tags::LUT_LABEL),
        file["expected_semantics"]["real_world_value_mapping"]["lut_label"]
            .as_str()
            .expect("manifest LUT label")
    );
    assert_eq!(number(mapping, tags::REAL_WORLD_VALUE_SLOPE), 1.0);
    assert_eq!(number(mapping, tags::REAL_WORLD_VALUE_INTERCEPT), 0.0);
    assert_eq!(
        number(
            mapping,
            tags::DOUBLE_FLOAT_REAL_WORLD_VALUE_FIRST_VALUE_MAPPED
        ),
        file["expected_semantics"]["pixel_min"]
            .as_f64()
            .expect("pixel minimum")
    );
    assert_eq!(
        number(
            mapping,
            tags::DOUBLE_FLOAT_REAL_WORLD_VALUE_LAST_VALUE_MAPPED
        ),
        file["expected_semantics"]["pixel_max"]
            .as_f64()
            .expect("pixel maximum")
    );
    assert_code(
        mapping,
        tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
        "1",
        "UCUM",
        "no units",
    );
    let quantity = mapping
        .element(tags::QUANTITY_DEFINITION_SEQUENCE)
        .expect("RWVM must contain quantity definition")
        .items()
        .expect("quantity definition must be a sequence");
    assert_eq!(quantity.len(), 1);
    assert_code(
        &quantity[0],
        tags::CONCEPT_CODE_SEQUENCE,
        "110850",
        "DCM",
        "X-Ray Attenuation",
    );

    let references = file["references"].as_array().expect("references array");
    let mut source_positions = BTreeMap::new();
    for reference in references {
        let source = open_file(root.join(reference["source_path"].as_str().expect("source path")))
            .expect("referenced source should reopen");
        assert_eq!(
            text(&source, tags::SOP_INSTANCE_UID),
            reference["sop_instance_uid"].as_str().expect("source UID")
        );
        source_positions.insert(
            text(&source, tags::SOP_INSTANCE_UID),
            text(&source, tags::IMAGE_POSITION_PATIENT),
        );
    }
    let expected_uids = source_positions.keys().cloned().collect::<BTreeSet<_>>();

    let frames = sequence(object, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE);
    assert_eq!(frames.len(), 3);
    let mut frame_uids = BTreeSet::new();
    for (index, frame) in frames.iter().enumerate() {
        let frame_content = frame
            .element(tags::FRAME_CONTENT_SEQUENCE)
            .expect("frame content")
            .items()
            .expect("frame content sequence");
        assert_eq!(
            frame_content[0]
                .element(tags::DIMENSION_INDEX_VALUES)
                .expect("dimension index")
                .to_int::<u32>()
                .expect("UL value"),
            (index + 1) as u32
        );
        let derivation = frame
            .element(TAG_DERIVATION_IMAGE_SEQUENCE)
            .expect("derivation sequence")
            .items()
            .expect("derivation items");
        let sources = derivation[0]
            .element(TAG_SOURCE_IMAGE_SEQUENCE)
            .expect("source image sequence")
            .items()
            .expect("source image items");
        assert_eq!(sources.len(), 1);
        assert_eq!(
            text(&sources[0], TAG_REFERENCED_SOP_CLASS_UID),
            uids::CT_IMAGE_STORAGE
        );
        let source_uid = text(&sources[0], TAG_REFERENCED_SOP_INSTANCE_UID);
        frame_uids.insert(source_uid.clone());
        let positions = frame
            .element(tags::PLANE_POSITION_SEQUENCE)
            .expect("plane position sequence")
            .items()
            .expect("plane position items");
        assert_eq!(
            text(&positions[0], tags::IMAGE_POSITION_PATIENT),
            source_positions[&source_uid]
        );
    }
    assert_eq!(frame_uids, expected_uids);

    let series = sequence(object, tags::REFERENCED_SERIES_SEQUENCE);
    assert_eq!(series.len(), 1);
    let instances = series[0]
        .element(tags::REFERENCED_INSTANCE_SEQUENCE)
        .expect("referenced instances")
        .items()
        .expect("referenced instance items");
    assert_eq!(instances.len(), 3);
    let common_uids = instances
        .iter()
        .map(|item| text(item, TAG_REFERENCED_SOP_INSTANCE_UID))
        .collect::<BTreeSet<_>>();
    assert_eq!(common_uids, expected_uids);
}

fn recompute_from_sources(
    root: &Path,
    references: &[Value],
    file: &Value,
    object: &dicom_object::DefaultDicomObject,
) -> Vec<Vec<u32>> {
    let scale = file["recipe"]["recipe_parameters"]["stored_value_scale"]
        .as_f64()
        .expect("scale") as f32;
    let increment = file["recipe"]["recipe_parameters"]["spatial_rank_increment"]
        .as_f64()
        .expect("increment") as f32;
    let frames_by_source = references
        .iter()
        .enumerate()
        .map(|(rank, reference)| {
            let source =
                open_file(root.join(reference["source_path"].as_str().expect("source path")))
                    .expect("source should reopen");
            let signed = source
                .element(tags::PIXEL_REPRESENTATION)
                .expect("pixel representation")
                .to_int::<u16>()
                .expect("US");
            let bits_stored = source
                .element(tags::BITS_STORED)
                .expect("bits stored")
                .to_int::<u16>()
                .expect("US");
            let mask = if bits_stored == 16 {
                u16::MAX
            } else {
                (1_u16 << bits_stored) - 1
            };
            let sign = 1_u16 << (bits_stored - 1);
            let bits = source
                .element(tags::PIXEL_DATA)
                .expect("source Pixel Data")
                .value()
                .to_bytes()
                .expect("native pixels")
                .chunks_exact(2)
                .map(|pair| {
                    let raw = u16::from_le_bytes([pair[0], pair[1]]) & mask;
                    let stored = if signed == 1 && raw & sign != 0 {
                        (i32::from(raw) - (1_i32 << bits_stored)) as f32
                    } else {
                        f32::from(raw)
                    };
                    (stored * scale + rank as f32 * increment).to_bits()
                })
                .collect::<Vec<_>>();
            (
                reference["sop_instance_uid"]
                    .as_str()
                    .expect("source SOP Instance UID")
                    .to_string(),
                bits,
            )
        })
        .collect::<BTreeMap<_, _>>();
    sequence(object, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
        .iter()
        .map(|frame| {
            let derivation = frame
                .element(TAG_DERIVATION_IMAGE_SEQUENCE)
                .expect("derivation sequence")
                .items()
                .expect("derivation items");
            let source = derivation[0]
                .element(TAG_SOURCE_IMAGE_SEQUENCE)
                .expect("source image sequence")
                .items()
                .expect("source image items");
            frames_by_source[&text(&source[0], TAG_REFERENCED_SOP_INSTANCE_UID)].clone()
        })
        .collect()
}

fn assert_code(
    object: &dicom_object::mem::InMemDicomObject,
    tag: Tag,
    value: &str,
    scheme: &str,
    meaning: &str,
) {
    let items = object
        .element(tag)
        .expect("code sequence")
        .items()
        .expect("code items");
    assert_eq!(items.len(), 1);
    assert_eq!(text(&items[0], tags::CODE_VALUE), value);
    assert_eq!(text(&items[0], tags::CODING_SCHEME_DESIGNATOR), scheme);
    assert_eq!(text(&items[0], tags::CODE_MEANING), meaning);
}

fn sequence<'a>(
    object: &'a dicom_object::DefaultDicomObject,
    tag: Tag,
) -> &'a [dicom_object::mem::InMemDicomObject] {
    object
        .element(tag)
        .expect("required sequence")
        .items()
        .expect("sequence value")
}

fn text(object: &dicom_object::mem::InMemDicomObject, tag: Tag) -> String {
    object
        .element(tag)
        .expect("required text element")
        .to_str()
        .expect("text value")
        .trim_end_matches(['\0', ' '])
        .to_string()
}

fn number(object: &dicom_object::mem::InMemDicomObject, tag: Tag) -> f64 {
    object
        .element(tag)
        .expect("required numeric element")
        .to_float64()
        .expect("numeric value")
}

fn nested_u32_frames(value: &Value) -> Vec<Vec<u32>> {
    value
        .as_array()
        .expect("frames array")
        .iter()
        .map(|frame| {
            frame
                .as_array()
                .expect("frame array")
                .iter()
                .map(|bit| {
                    u32::try_from(bit.as_u64().expect("u32 bit pattern")).expect("u32 range")
                })
                .collect()
        })
        .collect()
}

fn nested_u64_frames(value: &Value) -> Vec<Vec<u64>> {
    value
        .as_array()
        .expect("frames array")
        .iter()
        .map(|frame| {
            frame
                .as_array()
                .expect("frame array")
                .iter()
                .map(|bit| bit.as_u64().expect("u64 bit pattern"))
                .collect()
        })
        .collect()
}

fn bits_by_frame(bytes: &[u8], values_per_frame: usize) -> Vec<Vec<u32>> {
    bytes
        .chunks_exact(values_per_frame * 4)
        .map(|frame| {
            frame
                .chunks_exact(4)
                .map(|word| u32::from_le_bytes(word.try_into().expect("four bytes")))
                .collect()
        })
        .collect()
}

fn bits64_by_frame(bytes: &[u8], values_per_frame: usize) -> Vec<Vec<u64>> {
    bytes
        .chunks_exact(values_per_frame * 8)
        .map(|frame| {
            frame
                .chunks_exact(8)
                .map(|word| u64::from_le_bytes(word.try_into().expect("eight bytes")))
                .collect()
        })
        .collect()
}

fn float_bytes(path: &Path) -> Vec<u8> {
    open_file(path)
        .expect("Parametric Map should reopen")
        .element(tags::FLOAT_PIXEL_DATA)
        .expect("Float Pixel Data")
        .value()
        .to_bytes()
        .expect("native float bytes")
        .into_owned()
}

fn double_float_bytes(path: &Path) -> Vec<u8> {
    open_file(path)
        .expect("Parametric Map should reopen")
        .element(tags::DOUBLE_FLOAT_PIXEL_DATA)
        .expect("Double Float Pixel Data")
        .value()
        .to_bytes()
        .expect("native double float bytes")
        .into_owned()
}

fn generate_extended(root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args([
            "generate",
            "--profile",
            "extended",
            "--out",
            root.to_str().expect("UTF-8 path"),
            "--seed",
            "7",
        ])
        .output()
        .expect("extended generation should run");
    assert!(
        output.status.success(),
        "extended generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("manifest should read"))
        .expect("manifest should parse")
}

fn file_for_case<'a>(manifest: &'a Value, case_id: &str) -> Option<&'a Value> {
    manifest["files"]
        .as_array()?
        .iter()
        .find(|file| file["case_id"].as_str() == Some(case_id))
}

fn skipped_for_case(manifest: &Value) -> &Value {
    manifest["skipped_cases"]
        .as_array()
        .expect("skipped cases array")
        .iter()
        .find(|row| row["case_id"].as_str() == Some(CASE_ID))
        .expect("explicit Parametric Map unavailable row")
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "synth-dicom-gen-{label}-{}-{nonce}",
        std::process::id()
    ))
}
