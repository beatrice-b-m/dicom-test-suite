use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let rustc_version = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|version| version.trim().to_string())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "rustc unknown".to_string());

    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_string());

    println!("cargo:rustc-env=SYNTH_DICOM_GEN_RUSTC_VERSION={rustc_version}");
    println!("cargo:rustc-env=SYNTH_DICOM_GEN_TARGET={target}");

    generate_embedded_engine_resources();
}

fn generate_embedded_engine_resources() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("build output directory"))
        .join("embedded_engine_resources.rs");
    let mut resources = Vec::new();

    for directory in [
        "cases/recipes",
        "templates",
        "schemas",
        "transfer-syntax",
        "conformance",
    ] {
        collect_json_files(&root, Path::new(directory), &mut resources);
    }
    for path in [
        "cases/registry.json",
        "Cargo.lock",
        "standards.lock.json",
        "generation-backends.lock.json",
        "security/fixtures/fixtures.lock.json",
        "product/cli-error-codes.json",
        "generation-backends/highdicom-pydicom/uv.lock",
        "generation-backends/highdicom-pydicom/pyproject.toml",
        "generation-backends/highdicom-pydicom/src/dts_highdicom_backend/__init__.py",
        "generation-backends/highdicom-pydicom/src/dts_highdicom_backend/__main__.py",
        "generation-backends/highdicom-pydicom/src/dts_highdicom_backend/parametric_map.py",
        "generation-backends/highdicom-pydicom/src/dts_highdicom_backend/protocol.py",
        "generation-backends/highdicom-pydicom/src/dts_highdicom_backend/scoord3d.py",
        "generation-backends/highdicom-pydicom/src/dts_highdicom_backend/tid1500.py",
        "generation-backends/highdicom-pydicom/src/dts_highdicom_backend/wsi_tile_segmentation.py",
    ] {
        let source = root.join(path);
        require_regular_engine_resource(&source);
        resources.push((path.to_string(), source));
    }
    let color_profile = root.join("src/assets/dcmtk_srgb_input_profile.hex");
    require_regular_engine_resource(&color_profile);
    resources.push((
        "assets/dcmtk_srgb_input_profile.hex".to_string(),
        color_profile,
    ));
    resources.sort_by(|left, right| left.0.cmp(&right.0));
    resources.dedup_by(|left, right| left.0 == right.0);

    let mut generated =
        String::from("pub(crate) static EMBEDDED_ENGINE_RESOURCES: &[(&str, &[u8])] = &[\n");
    for (logical_path, source_path) in resources {
        println!("cargo:rerun-if-changed={}", source_path.display());
        generated.push_str(&format!(
            "    ({logical_path:?}, include_bytes!({source_path:?})),\n",
        ));
    }
    generated.push_str("];\n");
    fs::write(output, generated).expect("write embedded engine resource table");
}

fn collect_json_files(root: &Path, relative: &Path, resources: &mut Vec<(String, PathBuf)>) {
    let directory = root.join(relative);
    require_engine_resource_path(&directory, EngineResourcePathKind::Directory);
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut entries = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("resource directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path)
            .unwrap_or_else(|error| panic!("inspect engine resource {}: {error}", path.display()));
        assert!(
            !metadata.file_type().is_symlink(),
            "engine resource tree contains a symbolic link: {}",
            path.display()
        );
        if metadata.is_dir() {
            let child = path.strip_prefix(root).expect("resource beneath root");
            collect_json_files(root, child, resources);
        } else if metadata.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("json")
        {
            let logical = path
                .strip_prefix(root)
                .expect("resource beneath root")
                .to_string_lossy()
                .replace('\\', "/");
            if is_transitional_engine_resource(&logical) {
                resources.push((logical, path));
            }
        }
    }
}

/// Newly versioned identity-domain schemas are embedded directly by their
/// owning modules without perturbing the locked transitional v1 engine
/// inventory before R4.4 removes that compatibility oracle.
pub(crate) fn is_transitional_engine_resource(logical_path: &str) -> bool {
    !matches!(
        logical_path,
        "schemas/corpus-definition-bundle.schema.json"
            | "schemas/version-result-v2.schema.json"
            | "schemas/capabilities-result-v2.schema.json"
            | "schemas/generation-result-v2.schema.json"
            | "schemas/manifest-v1.schema.json"
    )
}

fn require_regular_engine_resource(path: &Path) {
    require_engine_resource_path(path, EngineResourcePathKind::File);
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum EngineResourcePathKind {
    Directory,
    File,
}

pub(crate) fn validate_engine_resource_path(
    path: &Path,
    expected: EngineResourcePathKind,
) -> Result<(), String> {
    let mut current = PathBuf::new();
    let components = path.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err("engine resource path is empty".to_string());
    }
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("inspect engine resource {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "engine resource path contains a symbolic link: {}",
                current.display()
            ));
        }
        let is_target = index + 1 == components.len();
        if !is_target && !metadata.is_dir() {
            return Err(format!(
                "engine resource ancestor is not a directory: {}",
                current.display()
            ));
        }
        if is_target {
            let correct_kind = match expected {
                EngineResourcePathKind::Directory => metadata.is_dir(),
                EngineResourcePathKind::File => metadata.is_file(),
            };
            if !correct_kind {
                return Err(format!(
                    "engine resource has the wrong file type: {}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn require_engine_resource_path(path: &Path, expected: EngineResourcePathKind) {
    if let Err(error) = validate_engine_resource_path(path, expected) {
        panic!("{error}");
    }
}
