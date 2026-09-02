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

    generate_embedded_product_resources();
}

fn generate_embedded_product_resources() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("build output directory"))
        .join("embedded_product_resources.rs");
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
        resources.push((path.to_string(), root.join(path)));
    }
    resources.push((
        "assets/dcmtk_srgb_input_profile.hex".to_string(),
        root.join("src/assets/dcmtk_srgb_input_profile.hex"),
    ));
    resources.sort_by(|left, right| left.0.cmp(&right.0));
    resources.dedup_by(|left, right| left.0 == right.0);

    let mut generated =
        String::from("pub(crate) static EMBEDDED_PRODUCT_RESOURCES: &[(&str, &[u8])] = &[\n");
    for (logical_path, source_path) in resources {
        println!("cargo:rerun-if-changed={}", source_path.display());
        generated.push_str(&format!(
            "    ({logical_path:?}, include_bytes!({source_path:?})),\n",
        ));
    }
    generated.push_str("];\n");
    fs::write(output, generated).expect("write embedded product resource table");
}

fn collect_json_files(root: &Path, relative: &Path, resources: &mut Vec<(String, PathBuf)>) {
    let directory = root.join(relative);
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut entries = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("resource directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            let child = path.strip_prefix(root).expect("resource beneath root");
            collect_json_files(root, child, resources);
        } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
            let logical = path
                .strip_prefix(root)
                .expect("resource beneath root")
                .to_string_lossy()
                .replace('\\', "/");
            resources.push((logical, path));
        }
    }
}
