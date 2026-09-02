use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use synth_dicom_gen::media::MemberRole;
use synth_dicom_gen::media_sources::{
    MEDIA_DERIVED_CASE_ID, MEDIA_IMAGE_CASE_ID, MEDIA_NON_IMAGE_CASE_ID, load_mixed_media_sources,
};

static NONCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn selects_locked_mixed_file_set_with_closed_reference() {
    let root = fixture_root();
    write_fixture(&root, false, false);

    let sources = load_mixed_media_sources(&root).expect("fixture should select");
    assert_eq!(sources.len(), 3);
    assert_eq!(sources[0].member.case_id, MEDIA_IMAGE_CASE_ID);
    assert_eq!(sources[0].member.role, MemberRole::Image);
    assert_eq!(sources[0].member.file_id.display(), "IMAGE\\IM000001");
    assert_eq!(sources[1].member.case_id, MEDIA_DERIVED_CASE_ID);
    assert_eq!(sources[1].member.role, MemberRole::Derived);
    assert_eq!(
        sources[1].member.referenced_sop_instance_uids,
        [sources[0].member.sop_instance_uid.clone()]
    );
    assert_eq!(sources[2].member.case_id, MEDIA_NON_IMAGE_CASE_ID);
    assert_eq!(sources[2].member.role, MemberRole::NonImage);
    assert!(sources[2].member.referenced_sop_instance_uids.is_empty());
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn rejects_traversal_and_broken_reference_closure() {
    let traversal = fixture_root();
    write_fixture(&traversal, true, false);
    assert!(
        load_mixed_media_sources(&traversal)
            .unwrap_err()
            .to_string()
            .contains("unsafe manifest path")
    );
    fs::remove_dir_all(traversal).expect("remove traversal fixture");

    let broken = fixture_root();
    write_fixture(&broken, false, true);
    assert!(
        load_mixed_media_sources(&broken)
            .unwrap_err()
            .to_string()
            .contains("SEG to Enhanced CT")
    );
    fs::remove_dir_all(broken).expect("remove broken fixture");
}

fn write_fixture(root: &Path, traversal: bool, broken_reference: bool) {
    fs::create_dir_all(root).expect("create root");
    let cases = [
        (MEDIA_IMAGE_CASE_ID, "1.2.3.1", "1.2.840.1", Vec::new()),
        (
            MEDIA_DERIVED_CASE_ID,
            "1.2.3.2",
            "1.2.840.2",
            vec![if broken_reference {
                "1.2.9.9"
            } else {
                "1.2.3.1"
            }],
        ),
        (MEDIA_NON_IMAGE_CASE_ID, "1.2.3.3", "1.2.840.3", Vec::new()),
    ];
    let files = cases
        .iter()
        .enumerate()
        .map(|(index, (case_id, sop_instance_uid, sop_class_uid, references))| {
            let path = if traversal && index == 0 {
                "../escape.dcm".to_owned()
            } else {
                format!("fixture/{index}.dcm")
            };
            if !path.starts_with("..") {
                let target = root.join(&path);
                fs::create_dir_all(target.parent().unwrap()).expect("create fixture directory");
                fs::write(target, format!("payload-{index}")).expect("write fixture payload");
            }
            json!({
                "case_id": case_id,
                "path": path,
                "sha256": "a".repeat(64),
                "dicom": { "sop_class_uid": sop_class_uid },
                "uids": { "sop_instance_uid": sop_instance_uid },
                "references": references.iter().map(|uid| json!({"sop_instance_uid": uid})).collect::<Vec<Value>>()
            })
        })
        .collect::<Vec<_>>();
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&json!({"files": files})).unwrap(),
    )
    .expect("write manifest");
}

fn fixture_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-media-sources-{}-{}",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ))
}
