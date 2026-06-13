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

    println!("cargo:rustc-env=DICOM_TEST_SUITE_RUSTC_VERSION={rustc_version}");
    println!("cargo:rustc-env=DICOM_TEST_SUITE_TARGET={target}");
}
