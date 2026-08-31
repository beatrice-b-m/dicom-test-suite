use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn installed_artifact_runs_every_resource_backed_workflow_from_three_directories() {
    let base = unique_root("relocation");
    let install_root = base.join("installed-product/bin");
    let isolated_home = base.join("isolated-home");
    let isolated_tmp = base.join("isolated-tmp");
    fs::create_dir_all(&install_root).unwrap();
    fs::create_dir_all(&isolated_home).unwrap();
    fs::create_dir_all(&isolated_tmp).unwrap();

    let installed_binary = install_root.join("dicom-test-suite");
    fs::copy(env!("CARGO_BIN_EXE_dicom-test-suite"), &installed_binary).unwrap();
    make_executable(&installed_binary);

    let spec_bytes = fs::read("tests/fixtures/composition/valid/template-only.json").unwrap();
    let mut resource_identity = None;
    for index in 0..3 {
        let working = base.join(format!("unrelated-{index}/nested/working-directory"));
        let caller_root = base.join(format!("caller-{index}"));
        let output_parent = base.join(format!("outputs-{index}"));
        fs::create_dir_all(&working).unwrap();
        fs::create_dir_all(&caller_root).unwrap();
        fs::create_dir_all(&output_parent).unwrap();
        let spec_path = caller_root.join("request.json");
        fs::write(&spec_path, &spec_bytes).unwrap();

        assert_success(run_installed(
            &installed_binary,
            &working,
            &isolated_home,
            &isolated_tmp,
            &["templates", "list", "--format", "json"],
        ));

        let composition_root = output_parent.join("composition");
        assert_success(run_installed_owned(
            &installed_binary,
            &working,
            &isolated_home,
            &isolated_tmp,
            vec![
                "compose".into(),
                "--spec".into(),
                spec_path.display().to_string(),
                "--out".into(),
                composition_root.display().to_string(),
                "--seed".into(),
                "1".into(),
            ],
        ));

        let curated_root = output_parent.join("curated");
        assert_success(run_installed_owned(
            &installed_binary,
            &working,
            &isolated_home,
            &isolated_tmp,
            vec![
                "generate".into(),
                "--profile".into(),
                "smoke".into(),
                "--out".into(),
                curated_root.display().to_string(),
                "--seed".into(),
                "1".into(),
            ],
        ));
        for arguments in [
            vec!["validate".into(), curated_root.display().to_string()],
            vec![
                "report".into(),
                curated_root.display().to_string(),
                "--format".into(),
                "json".into(),
            ],
        ] {
            assert_success(run_installed_owned(
                &installed_binary,
                &working,
                &isolated_home,
                &isolated_tmp,
                arguments,
            ));
        }

        let curated_manifest: Value =
            serde_json::from_slice(&fs::read(curated_root.join("manifest.json")).unwrap()).unwrap();
        let composition_manifest: Value =
            serde_json::from_slice(&fs::read(composition_root.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(curated_manifest["product_resources"]["origin"], "embedded");
        assert_eq!(
            curated_manifest["product_resources"],
            composition_manifest["product_resources"]
        );
        let identity = curated_manifest["product_resources"]["resource_set_sha256"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(resource_identity.get_or_insert(identity.clone()), &identity);
    }

    fs::remove_dir_all(base).unwrap();
}

fn run_installed(
    binary: &Path,
    working: &Path,
    isolated_home: &Path,
    isolated_tmp: &Path,
    arguments: &[&str],
) -> Output {
    run_installed_owned(
        binary,
        working,
        isolated_home,
        isolated_tmp,
        arguments.iter().map(|value| (*value).to_string()).collect(),
    )
}

fn run_installed_owned(
    binary: &Path,
    working: &Path,
    isolated_home: &Path,
    isolated_tmp: &Path,
    arguments: Vec<String>,
) -> Output {
    Command::new(binary)
        .current_dir(working)
        .env_clear()
        .env("HOME", isolated_home)
        .env("CARGO_HOME", isolated_home.join("absent-cargo-cache"))
        .env("CARGO_MANIFEST_DIR", isolated_home.join("absent-checkout"))
        .env("TMPDIR", isolated_tmp)
        .env("PATH", "/usr/bin:/bin")
        .args(arguments)
        .output()
        .unwrap()
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "installed command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn unique_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_: &Path) {}
