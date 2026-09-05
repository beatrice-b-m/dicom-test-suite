use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::sdk::CorpusSelector;

pub struct GenericTimezoneScBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
}

impl GenericTimezoneScBundle {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "generic-timezone-sc-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-timezone-sc-corpus");
        for relative in [
            "definition.json",
            "members/cases/registry.json",
            "members/cases/recipes/caller_offset_pair.json",
        ] {
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(source.join(relative), destination).unwrap();
        }
        Self {
            members: root.join("members"),
            descriptor: root.join("definition.json"),
            root,
        }
    }

    pub fn selector() -> CorpusSelector {
        CorpusSelector::CaseIds {
            profile: "core".into(),
            include_stress: false,
            case_ids: vec!["caller/temporal/offset-extrema".into()],
        }
    }

    pub fn args(&self, command: &str, out: Option<&str>) -> Vec<String> {
        let mut args = vec![
            command.into(),
            "--corpus".into(),
            "./definition.json".into(),
            "--asset-root".into(),
            "members".into(),
            "--profile".into(),
            "core".into(),
            "--case-id".into(),
            "caller/temporal/offset-extrema".into(),
            "--seed".into(),
            "41".into(),
            "--format".into(),
            "json".into(),
        ];
        if let Some(out) = out {
            args.extend(["--out".into(), out.into()]);
        }
        args
    }
}

impl Drop for GenericTimezoneScBundle {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
