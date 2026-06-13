use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RUSTC_VERSION: &str = env!("DICOM_TEST_SUITE_RUSTC_VERSION");
pub const TARGET_TRIPLE: &str = env!("DICOM_TEST_SUITE_TARGET");

pub fn version_banner() -> String {
    format!("{PACKAGE_NAME} {PACKAGE_VERSION}")
}

pub const SUPPORTED_PROFILES: &[&str] = &[
    "smoke", "core", "extended", "legacy", "stress", "all", "negative", "fuzz",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateOptions {
    pub profile: String,
    pub out_dir: PathBuf,
    pub seed: u64,
    pub include_stress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedGenerationRun {
    pub profile: String,
    pub out_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub seed: u64,
    pub include_stress: bool,
}

#[derive(Debug)]
pub enum GenerateError {
    InvalidProfile(String),
    CreateOutputDir {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseMetadata {
        path: PathBuf,
        source: serde_json::Error,
    },
    MetadataShape {
        path: PathBuf,
        message: &'static str,
    },
    SerializeManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    WriteManifest {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile(profile) => write!(
                f,
                "unsupported profile {profile}; expected one of {}",
                SUPPORTED_PROFILES.join(", ")
            ),
            Self::CreateOutputDir { path, source } => {
                write!(
                    f,
                    "failed to create output directory {}: {source}",
                    path.display()
                )
            }
            Self::ReadMetadata { path, source } => {
                write!(
                    f,
                    "failed to read metadata file {}: {source}",
                    path.display()
                )
            }
            Self::ParseMetadata { path, source } => {
                write!(
                    f,
                    "failed to parse metadata file {}: {source}",
                    path.display()
                )
            }
            Self::MetadataShape { path, message } => {
                write!(f, "invalid metadata shape in {}: {message}", path.display())
            }
            Self::SerializeManifest { path, source } => {
                write!(
                    f,
                    "failed to serialize manifest {}: {source}",
                    path.display()
                )
            }
            Self::WriteManifest { path, source } => {
                write!(f, "failed to write manifest {}: {source}", path.display())
            }
        }
    }
}

impl Error for GenerateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidProfile(_) => None,
            Self::CreateOutputDir { source, .. } => Some(source),
            Self::ReadMetadata { source, .. } => Some(source),
            Self::ParseMetadata { source, .. } => Some(source),
            Self::MetadataShape { .. } => None,
            Self::SerializeManifest { source, .. } => Some(source),
            Self::WriteManifest { source, .. } => Some(source),
        }
    }
}

pub fn prepare_generation_run(
    options: GenerateOptions,
) -> Result<PreparedGenerationRun, GenerateError> {
    if !SUPPORTED_PROFILES.contains(&options.profile.as_str()) {
        return Err(GenerateError::InvalidProfile(options.profile));
    }

    fs::create_dir_all(&options.out_dir).map_err(|source| GenerateError::CreateOutputDir {
        path: options.out_dir.clone(),
        source,
    })?;

    Ok(PreparedGenerationRun {
        manifest_path: options.out_dir.join("manifest.json"),
        profile: options.profile,
        out_dir: options.out_dir,
        seed: options.seed,
        include_stress: options.include_stress,
    })
}

pub fn write_initial_manifest(run: &PreparedGenerationRun) -> Result<(), GenerateError> {
    let standards_lock_path = Path::new("standards.lock.json");
    let cargo_lock_path = Path::new("Cargo.lock");
    let registry_path = Path::new("cases/registry.json");

    let standards_lock = read_json_metadata(standards_lock_path)?;
    let cargo_lock = read_bytes_metadata(cargo_lock_path)?;
    let registry = read_json_metadata(registry_path)?;

    let manifest = build_initial_manifest(run, &standards_lock, &cargo_lock, &registry)?;
    let mut contents = serde_json::to_string_pretty(&manifest).map_err(|source| {
        GenerateError::SerializeManifest {
            path: run.manifest_path.clone(),
            source,
        }
    })?;
    contents.push('\n');

    fs::write(&run.manifest_path, contents).map_err(|source| GenerateError::WriteManifest {
        path: run.manifest_path.clone(),
        source,
    })
}

fn build_initial_manifest(
    run: &PreparedGenerationRun,
    standards_lock: &Value,
    cargo_lock: &[u8],
    registry: &Value,
) -> Result<Value, GenerateError> {
    let standards_lock_bytes = read_bytes_metadata("standards.lock.json")?;
    let skipped_cases = skipped_cases_for_run(registry, run)?;
    let dicom_standard_kb = standards_lock
        .get("dicom_standard_kb")
        .cloned()
        .unwrap_or(Value::Null);

    Ok(serde_json::json!({
        "manifest_schema_version": "0.1.0",
        "generated_at": "19700101000000.000000+0000",
        "generator": {
            "name": PACKAGE_NAME,
            "version": PACKAGE_VERSION,
            "git_sha": Value::Null,
            "rustc_version": RUSTC_VERSION,
            "target_triple": TARGET_TRIPLE,
            "cargo_lock_sha256": sha256_hex(cargo_lock),
            "feature_flags": []
        },
        "standards": {
            "dicom_base_edition": standards_lock.get("dicom_base_edition").and_then(Value::as_str).unwrap_or("2026b"),
            "include_final_text_after_base": standards_lock.get("include_final_text_after_base").and_then(Value::as_bool).unwrap_or(false),
            "standards_lock_sha256": sha256_hex(&standards_lock_bytes),
            "dicom_standard_kb": {
                "commit": dicom_standard_kb.get("commit").cloned().unwrap_or(Value::Null),
                "db_edition": dicom_standard_kb.get("db_edition").and_then(Value::as_str).unwrap_or("2026b"),
                "db_sha256": dicom_standard_kb.get("db_sha256").cloned().unwrap_or(Value::Null),
                "source_manifest_sha256": dicom_standard_kb.get("source_manifest_sha256").cloned().unwrap_or(Value::Null)
            }
        },
        "dependencies": {
            "dicom_rs_versions": {
                "dicom-core": "0.9.1",
                "dicom-dictionary-std": "0.9.0",
                "dicom-object": "0.9.1",
                "dicom-transfer-syntax-registry": "0.9.1"
            },
            "codec_versions": {}
        },
        "run": {
            "profile": run.profile,
            "seed": run.seed,
            "include_stress": run.include_stress
        },
        "files": [],
        "skipped_cases": skipped_cases
    }))
}

fn skipped_cases_for_run(
    registry: &Value,
    run: &PreparedGenerationRun,
) -> Result<Vec<Value>, GenerateError> {
    let cases =
        registry
            .get("cases")
            .and_then(Value::as_array)
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "missing cases array",
            })?;

    let mut skipped = Vec::new();
    for case in cases {
        let profiles =
            string_array(case.get("profiles")).map_err(|_| GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "case profiles must be a string array",
            })?;
        if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
            continue;
        }

        skipped.push(serde_json::json!({
            "case_id": required_str(case, "case_id").map_err(|_| GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "case_id must be a string",
            })?,
            "status": "unavailable",
            "reason_code": "generator_not_implemented",
            "message": "The Phase 1 manifest skeleton records planned cases before DICOM instance writing is implemented.",
            "recheck_phase": "phase-1",
            "standards_evidence": case.get("standards_evidence").cloned().unwrap_or_else(|| serde_json::json!([]))
        }));
    }

    Ok(skipped)
}

fn case_matches_profile(profiles: &[String], requested: &str, include_stress: bool) -> bool {
    match requested {
        "all" => profiles.iter().any(|profile| {
            matches!(profile.as_str(), "smoke" | "core" | "extended" | "legacy")
                || (include_stress && profile == "stress")
        }),
        profile => profiles.iter().any(|case_profile| case_profile == profile),
    }
}

fn read_json_metadata(path: impl AsRef<Path>) -> Result<Value, GenerateError> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path).map_err(|source| GenerateError::ReadMetadata {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| GenerateError::ParseMetadata {
        path: path.to_path_buf(),
        source,
    })
}

fn read_bytes_metadata(path: impl AsRef<Path>) -> Result<Vec<u8>, GenerateError> {
    let path = path.as_ref();
    fs::read(path).map_err(|source| GenerateError::ReadMetadata {
        path: path.to_path_buf(),
        source,
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut data = bytes.to_vec();
    let bit_len = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());

    let mut h = H0;
    for chunk in data.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let start = i * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    h.iter().map(|word| format!("{word:08x}")).collect()
}

#[derive(Debug)]
pub enum CaseRegistryError {
    Read {
        path: String,
        source: std::io::Error,
    },
    Parse {
        path: String,
        source: serde_json::Error,
    },
    Shape(&'static str),
}

impl fmt::Display for CaseRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read case registry {path}: {source}")
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse case registry {path}: {source}")
            }
            Self::Shape(message) => write!(f, "invalid case registry shape: {message}"),
        }
    }
}

impl Error for CaseRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Shape(_) => None,
        }
    }
}

pub fn list_cases_from_registry_path(
    registry_path: impl AsRef<Path>,
    profile_filter: Option<&str>,
) -> Result<String, CaseRegistryError> {
    let registry_path = registry_path.as_ref();
    let path_display = registry_path.display().to_string();
    let contents = fs::read_to_string(registry_path).map_err(|source| CaseRegistryError::Read {
        path: path_display.clone(),
        source,
    })?;
    let registry: Value =
        serde_json::from_str(&contents).map_err(|source| CaseRegistryError::Parse {
            path: path_display,
            source,
        })?;

    list_cases_from_registry_value(&registry, profile_filter)
}

pub fn list_cases_from_registry_value(
    registry: &Value,
    profile_filter: Option<&str>,
) -> Result<String, CaseRegistryError> {
    let cases = registry
        .get("cases")
        .and_then(Value::as_array)
        .ok_or(CaseRegistryError::Shape("missing cases array"))?;

    let mut output = String::from(
        "case_id\tstatus\tprofiles\tsop_class_uid\ttransfer_syntax_uid\tstandards_evidence\n",
    );

    for case in cases {
        let profiles = string_array(case.get("profiles"))?;
        if let Some(profile_filter) = profile_filter {
            if !profiles.iter().any(|profile| profile == profile_filter) {
                continue;
            }
        }

        let case_id = required_str(case, "case_id")?;
        let status = required_str(case, "status")?;
        let sop_class_uid = required_str(case, "sop_class_uid")?;
        let transfer_syntax_uid = required_str(case, "transfer_syntax_uid")?;
        let evidence = case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .ok_or(CaseRegistryError::Shape("missing standards_evidence array"))?;
        let covered = evidence
            .iter()
            .filter(|entry| entry.get("covered").and_then(Value::as_bool) == Some(true))
            .count();

        output.push_str(&format!(
            "{case_id}\t{status}\t{}\t{sop_class_uid}\t{transfer_syntax_uid}\t{covered}/{} covered\n",
            profiles.join(","),
            evidence.len()
        ));
    }

    Ok(output)
}

fn required_str<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, CaseRegistryError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(CaseRegistryError::Shape(field))
}

fn string_array(value: Option<&Value>) -> Result<Vec<String>, CaseRegistryError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or(CaseRegistryError::Shape("missing string array"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or(CaseRegistryError::Shape("array item is not a string"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_dictionary_std::uids;
    use dicom_object::InMemDicomObject;
    use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};

    #[test]
    fn version_banner_uses_package_metadata() {
        assert_eq!(version_banner(), "dicom-test-suite 0.1.0");
    }

    #[test]
    fn pinned_dicom_rs_crates_expose_phase_one_primitives() {
        let _obj = InMemDicomObject::new_empty();

        let explicit_vr_le = TransferSyntaxRegistry
            .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
            .expect("Explicit VR Little Endian must be available for Part 10 smoke cases");

        assert_eq!(explicit_vr_le.uid(), uids::EXPLICIT_VR_LITTLE_ENDIAN);
        assert_eq!(uids::VERIFICATION, "1.2.840.10008.1.1");
    }

    #[test]
    fn list_cases_shows_committed_smoke_case_status_and_evidence() {
        let output = list_cases_from_registry_path("cases/registry.json", Some("smoke"))
            .expect("smoke case registry should list");

        assert!(
            output.contains(
                "classic/sc/mono2_u8_explicit_le\tplanned\tsmoke\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t2/2 covered"
            ),
            "list-cases output must show smoke status and standards evidence coverage"
        );
    }

    #[test]
    fn list_cases_shows_committed_core_case_status_and_evidence() {
        let output = list_cases_from_registry_path("cases/registry.json", Some("core"))
            .expect("core case registry should list");

        assert!(
            output.contains(
                "classic/ct/mono2_i16_rescale_12bit_explicit_le\tplanned\tcore\t1.2.840.10008.5.1.4.1.1.2\t1.2.840.10008.1.2.1\t2/2 covered"
            ),
            "list-cases output must show core status and standards evidence coverage"
        );
    }

    #[test]
    fn prepare_generation_run_creates_output_root_and_manifest_path() {
        let out_dir = unique_temp_dir("prepare_generation_run");
        let prepared = prepare_generation_run(GenerateOptions {
            profile: "smoke".to_string(),
            out_dir: out_dir.clone(),
            seed: 1,
            include_stress: false,
        })
        .expect("generation run should prepare");

        assert!(out_dir.is_dir(), "prepare must create the output root");
        assert_eq!(prepared.profile, "smoke");
        assert_eq!(prepared.seed, 1);
        assert!(!prepared.include_stress);
        assert_eq!(prepared.manifest_path, out_dir.join("manifest.json"));
        assert!(
            !prepared.manifest_path.exists(),
            "preparing a run must not write a manifest before manifest construction"
        );

        fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
    }

    #[test]
    fn prepare_generation_run_rejects_unknown_profile() {
        let err = prepare_generation_run(GenerateOptions {
            profile: "unknown".to_string(),
            out_dir: unique_temp_dir("reject_unknown_profile"),
            seed: 1,
            include_stress: false,
        })
        .expect_err("unknown profile should be rejected");

        assert!(
            err.to_string().contains("unsupported profile unknown"),
            "error should name the rejected profile"
        );
    }

    #[test]
    fn write_initial_manifest_records_empty_smoke_run_metadata() {
        let out_dir = unique_temp_dir("write_initial_manifest");
        let prepared = prepare_generation_run(GenerateOptions {
            profile: "smoke".to_string(),
            out_dir: out_dir.clone(),
            seed: 7,
            include_stress: false,
        })
        .expect("generation run should prepare");

        write_initial_manifest(&prepared).expect("manifest should write");

        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(&prepared.manifest_path).expect("manifest should be readable"),
        )
        .expect("manifest should parse");

        assert_eq!(
            manifest
                .get("manifest_schema_version")
                .and_then(Value::as_str),
            Some("0.1.0")
        );
        assert_eq!(
            manifest.pointer("/run/profile").and_then(Value::as_str),
            Some("smoke")
        );
        assert_eq!(
            manifest.pointer("/run/seed").and_then(Value::as_u64),
            Some(7)
        );
        assert_eq!(
            manifest
                .pointer("/files")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert!(
            manifest
                .pointer("/skipped_cases")
                .and_then(Value::as_array)
                .is_some_and(|cases| {
                    cases.iter().any(|case| {
                        case.get("case_id").and_then(Value::as_str)
                            == Some("classic/sc/mono2_u8_explicit_le")
                            && case.get("status").and_then(Value::as_str) == Some("unavailable")
                    })
                }),
            "manifest should record planned smoke cases as unavailable before DICOM writing"
        );

        fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
    }

    #[test]
    fn sha256_hex_matches_known_digest() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "dicom-test-suite-{name}-{}-{nonce}",
            std::process::id()
        ))
    }
}
