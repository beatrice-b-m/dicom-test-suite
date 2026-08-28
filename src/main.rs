use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        println!("{}", dicom_test_suite::version_banner());
        return Ok(());
    };

    match command.as_str() {
        "conformance" => {
            let subcommand = args
                .next()
                .ok_or_else(|| "conformance requires a subcommand".to_string())?;
            match subcommand.as_str() {
                "check-tools" => {
                    let mut config =
                        String::from(dicom_test_suite::conformance::DEFAULT_VALIDATOR_CONFIG);
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--config" => {
                                config = args
                                    .next()
                                    .ok_or_else(|| "--config requires a path".to_string())?;
                            }
                            "--help" | "-h" => {
                                println!(
                                    "Usage: dicom-test-suite conformance check-tools [--config PATH]"
                                );
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!(
                                    "unknown conformance check-tools argument: {unknown}"
                                ));
                            }
                        }
                    }
                    let report = dicom_test_suite::conformance::check_tools_path(config)?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
                    );
                    Ok(())
                }
                "run" => {
                    let generated_root = args.next().ok_or_else(|| {
                        "conformance run requires a generated root path".to_string()
                    })?;
                    let mut out = None;
                    let mut config =
                        String::from(dicom_test_suite::conformance::DEFAULT_VALIDATOR_CONFIG);
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--out" => {
                                out = Some(
                                    args.next()
                                        .ok_or_else(|| "--out requires a path".to_string())?,
                                );
                            }
                            "--config" => {
                                config = args
                                    .next()
                                    .ok_or_else(|| "--config requires a path".to_string())?;
                            }
                            "--help" | "-h" => {
                                println!(
                                    "Usage: dicom-test-suite conformance run GENERATED_ROOT --out EVIDENCE_ROOT [--config PATH]"
                                );
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!("unknown conformance run argument: {unknown}"));
                            }
                        }
                    }
                    let out = out.ok_or_else(|| "conformance run requires --out".to_string())?;
                    let evidence = dicom_test_suite::conformance::run_conformance(
                        generated_root,
                        &out,
                        config,
                    )?;
                    println!("evidence_root\t{out}");
                    println!("run_id\t{}", evidence["run_id"].as_str().unwrap_or(""));
                    println!(
                        "instances\t{}",
                        evidence["instances"].as_array().map(Vec::len).unwrap_or(0)
                    );
                    Ok(())
                }
                "verify" => {
                    let evidence_root = args.next().ok_or_else(|| {
                        "conformance verify requires an evidence root path".to_string()
                    })?;
                    let mut allowlist =
                        String::from(dicom_test_suite::conformance::DEFAULT_ACCEPTED_FINDINGS);
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--allowlist" => {
                                allowlist = args
                                    .next()
                                    .ok_or_else(|| "--allowlist requires a path".to_string())?;
                            }
                            "--help" | "-h" => {
                                println!(
                                    "Usage: dicom-test-suite conformance verify EVIDENCE_ROOT [--allowlist PATH]"
                                );
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!(
                                    "unknown conformance verify argument: {unknown}"
                                ));
                            }
                        }
                    }
                    let result = dicom_test_suite::conformance::verify_conformance(
                        evidence_root,
                        allowlist,
                    )?;
                    println!(
                        "accepted_findings\t{}",
                        result["accepted_findings"].as_u64().unwrap_or(0)
                    );
                    let failures = result["failures"].as_array().cloned().unwrap_or_default();
                    println!("verification_failures\t{}", failures.len());
                    for failure in &failures {
                        println!("failure\t{}", failure.as_str().unwrap_or("unknown"));
                    }
                    if failures.is_empty() {
                        Ok(())
                    } else {
                        Err("conformance verification failed".to_string())
                    }
                }
                "--help" | "-h" => {
                    println!("Usage: dicom-test-suite conformance <check-tools|run|verify>");
                    Ok(())
                }
                unknown => Err(format!("unknown conformance subcommand: {unknown}")),
            }
        }
        "generate" => {
            let mut profile = None;
            let mut out_dir = None;
            let mut seed = 1;
            let mut include_stress = false;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--profile" => {
                        profile = Some(
                            args.next()
                                .ok_or_else(|| "--profile requires a value".to_string())?,
                        );
                    }
                    "--out" => {
                        out_dir = Some(
                            args.next()
                                .ok_or_else(|| "--out requires a path".to_string())?,
                        );
                    }
                    "--seed" => {
                        seed = parse_seed(
                            args.next()
                                .ok_or_else(|| "--seed requires a value".to_string())?,
                        )?;
                    }
                    "--include-stress" => {
                        include_stress = true;
                    }
                    "--help" | "-h" => {
                        print_generate_usage();
                        return Ok(());
                    }
                    unknown => {
                        return Err(format!("unknown generate argument: {unknown}"));
                    }
                }
            }

            let profile = profile.ok_or_else(|| "generate requires --profile".to_string())?;
            let out_dir = out_dir.ok_or_else(|| "generate requires --out".to_string())?;
            let prepared =
                dicom_test_suite::prepare_generation_run(dicom_test_suite::GenerateOptions {
                    profile,
                    out_dir: out_dir.into(),
                    seed,
                    include_stress,
                })
                .map_err(|err| err.to_string())?;
            let summary =
                dicom_test_suite::write_generation_run(&prepared).map_err(|err| err.to_string())?;

            println!("profile\t{}", prepared.profile);
            println!("seed\t{}", prepared.seed);
            println!("include_stress\t{}", prepared.include_stress);
            println!("out\t{}", prepared.out_dir.display());
            println!("manifest\t{}", prepared.manifest_path.display());
            println!("files_written\t{}", summary.files_written);
            println!("manifest_written\t{}", summary.manifest_written);
            Ok(())
        }
        "interoperate" => {
            let subcommand = args
                .next()
                .ok_or_else(|| "interoperate requires a subcommand".to_string())?;
            match subcommand.as_str() {
                "media-dicomdir" => {
                    let generated_root = args.next().ok_or_else(|| {
                        "interoperate media-dicomdir requires a generated root path".to_string()
                    })?;
                    let mut dcmmkdir = None;
                    let mut dcmdump = None;
                    let mut dciodvfy = None;
                    let mut dcentvfy = None;
                    let mut format = None;
                    let mut timeout_seconds = 30_u64;
                    while let Some(argument) = args.next() {
                        match argument.as_str() {
                            "--dcmmkdir" => {
                                dcmmkdir = Some(required_value(&mut args, "--dcmmkdir")?)
                            }
                            "--dcmdump" => dcmdump = Some(required_value(&mut args, "--dcmdump")?),
                            "--dciodvfy" => {
                                dciodvfy = Some(required_value(&mut args, "--dciodvfy")?)
                            }
                            "--dcentvfy" => {
                                dcentvfy = Some(required_value(&mut args, "--dcentvfy")?)
                            }
                            "--format" => format = Some(required_value(&mut args, "--format")?),
                            "--timeout-seconds" => {
                                timeout_seconds = required_value(&mut args, "--timeout-seconds")?
                                    .parse()
                                    .map_err(|_| {
                                        "--timeout-seconds requires an integer".to_string()
                                    })?;
                            }
                            "--help" | "-h" => {
                                print_interoperate_usage();
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!("unknown media-dicomdir argument: {unknown}"));
                            }
                        }
                    }
                    if timeout_seconds == 0 {
                        return Err("--timeout-seconds must be non-zero".to_string());
                    }
                    let sources =
                        dicom_test_suite::media_sources::load_mixed_media_sources(&generated_root)
                            .map_err(|error| error.to_string())?;
                    let qualification = dicom_test_suite::media_runner::run_dicomdir_qualification(
                        &dicom_test_suite::media_runner::DicomDirRunRequest {
                            tools: dicom_test_suite::media_runner::MediaToolPaths {
                                dcmmkdir: required_path(dcmmkdir, "--dcmmkdir")?,
                                dcmdump: required_path(dcmdump, "--dcmdump")?,
                                dciodvfy: required_path(dciodvfy, "--dciodvfy")?,
                                dcentvfy: dcentvfy.map(Into::into),
                            },
                            sources,
                            timeout: std::time::Duration::from_secs(timeout_seconds),
                            staging_parent: None,
                        },
                    )
                    .map_err(|error| error.to_string())?;
                    let format = required_format(format)?;
                    match format.as_str() {
                        "json" => println!(
                            "{}",
                            serde_json::to_string_pretty(&qualification)
                                .map_err(|error| error.to_string())?
                        ),
                        "markdown" => print_media_qualification_markdown(&qualification),
                        other => return Err(format!("unsupported interoperate format: {other}")),
                    }
                    Ok(())
                }
                "protocol-baseline" => {
                    let generated_root = args.next().ok_or_else(|| {
                        "interoperate protocol-baseline requires a generated root path".to_string()
                    })?;
                    let mut format = None;
                    let mut seed = 1_u64;
                    let mut fixtures = String::from("security/fixtures/fixtures.lock.json");
                    while let Some(argument) = args.next() {
                        match argument.as_str() {
                            "--format" => format = Some(required_value(&mut args, "--format")?),
                            "--seed" => seed = parse_seed(required_value(&mut args, "--seed")?)?,
                            "--fixtures" => fixtures = required_value(&mut args, "--fixtures")?,
                            "--help" | "-h" => {
                                print_interoperate_usage();
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!(
                                    "unknown protocol-baseline argument: {unknown}"
                                ));
                            }
                        }
                    }
                    let selected =
                        dicom_test_suite::media_sources::load_mixed_media_sources(&generated_root)
                            .map_err(|error| error.to_string())?;
                    let source_links = selected
                        .into_iter()
                        .map(|source| {
                            let path =
                                source
                                    .source_path
                                    .strip_prefix(&generated_root)
                                    .map_err(|_| {
                                        "selected protocol source escaped generated root"
                                            .to_string()
                                    })?;
                            Ok(dicom_test_suite::protocol::SourceCaseLink {
                                case_id: source.member.case_id,
                                path: path.to_string_lossy().replace('\\', "/"),
                                sha256: source.member.sha256,
                                sop_instance_uid: source.member.sop_instance_uid,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    let executable = std::env::current_exe()
                        .map_err(|error| format!("resolve current executable: {error}"))?;
                    let executable_bytes = std::fs::read(&executable)
                        .map_err(|error| format!("read {}: {error}", executable.display()))?;
                    let report =
                        dicom_test_suite::protocol_baseline::build_unavailable_protocol_baseline(
                            dicom_test_suite::protocol_baseline::ProtocolBaselineInput {
                                run_seed: seed,
                                harness: dicom_test_suite::protocol::ToolFingerprint {
                                    id: "dicom-test-suite-protocol-baseline".to_string(),
                                    version: env!("CARGO_PKG_VERSION").to_string(),
                                    executable_sha256: dicom_test_suite::sha256_hex(
                                        &executable_bytes,
                                    ),
                                },
                                sources: source_links,
                            },
                            std::path::Path::new(&fixtures),
                        )
                        .map_err(|error| error.to_string())?;
                    let format = required_format(format)?;
                    match format.as_str() {
                        "json" => println!(
                            "{}",
                            serde_json::to_string_pretty(&report)
                                .map_err(|error| error.to_string())?
                        ),
                        "markdown" => print!(
                            "{}",
                            dicom_test_suite::protocol_baseline::protocol_report_markdown(&report)
                        ),
                        other => return Err(format!("unsupported interoperate format: {other}")),
                    }
                    Ok(())
                }
                "--help" | "-h" => {
                    print_interoperate_usage();
                    Ok(())
                }
                unknown => Err(format!("unknown interoperate subcommand: {unknown}")),
            }
        }
        "list-cases" => {
            let mut registry_path = String::from("cases/registry.json");
            let mut profile_filter = None;
            let mut status_filter = None;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--registry" => {
                        registry_path = args
                            .next()
                            .ok_or_else(|| "--registry requires a path".to_string())?;
                    }
                    "--profile" => {
                        profile_filter = Some(
                            args.next()
                                .ok_or_else(|| "--profile requires a value".to_string())?,
                        );
                    }
                    "--status" => {
                        status_filter = Some(
                            args.next()
                                .ok_or_else(|| "--status requires a value".to_string())?,
                        );
                    }
                    "--help" | "-h" => {
                        print_list_cases_usage();
                        return Ok(());
                    }
                    unknown => {
                        return Err(format!("unknown list-cases argument: {unknown}"));
                    }
                }
            }

            let output = dicom_test_suite::list_cases_from_registry_path(
                registry_path,
                profile_filter.as_deref(),
                status_filter.as_deref(),
            )
            .map_err(|err| err.to_string())?;
            print!("{output}");
            Ok(())
        }
        "validate" => {
            let root = args
                .next()
                .ok_or_else(|| "validate requires a generated root path".to_string())?;
            if root == "--help" || root == "-h" {
                print_validate_usage();
                return Ok(());
            }
            if let Some(extra) = args.next() {
                return Err(format!("unknown validate argument: {extra}"));
            }

            let summary =
                dicom_test_suite::validate_generated_root(&root).map_err(|err| err.to_string())?;
            println!("generated_root\t{root}");
            println!("manifest\t{}", summary.manifest_path.display());
            println!("files_checked\t{}", summary.files_checked);
            println!("validation_failures\t{}", summary.failures.len());
            for failure in &summary.failures {
                println!("failure\t{failure}");
            }
            if summary.failures.is_empty() {
                Ok(())
            } else {
                Err("validation failed".to_string())
            }
        }
        "report" => {
            let root = args
                .next()
                .ok_or_else(|| "report requires a generated root path".to_string())?;
            if root == "--help" || root == "-h" {
                print_report_usage();
                return Ok(());
            }
            let gap_report = root == "gaps";
            let mut format = None;
            let mut registry_path = String::from("cases/registry.json");
            let mut standards_lock_path = String::from("standards.lock.json");
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--format" => {
                        format = Some(
                            args.next()
                                .ok_or_else(|| "--format requires a value".to_string())?,
                        );
                    }
                    "--registry" if gap_report => {
                        registry_path = args
                            .next()
                            .ok_or_else(|| "--registry requires a value".to_string())?;
                    }
                    "--standards-lock" if gap_report => {
                        standards_lock_path = args
                            .next()
                            .ok_or_else(|| "--standards-lock requires a value".to_string())?;
                    }
                    unknown => {
                        return Err(format!("unknown report argument: {unknown}"));
                    }
                }
            }
            let format = format.ok_or_else(|| "report requires --format".to_string())?;
            if gap_report {
                let report =
                    dicom_test_suite::build_coverage_gap_report(registry_path, standards_lock_path)
                        .map_err(|err| err.to_string())?;
                return match format.as_str() {
                    "json" => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
                        );
                        Ok(())
                    }
                    "markdown" => {
                        print!(
                            "{}",
                            dicom_test_suite::render_coverage_gap_report_markdown(&report)
                        );
                        Ok(())
                    }
                    other => Err(format!("unsupported report format: {other}")),
                };
            }
            match format.as_str() {
                "json" => {
                    let report = dicom_test_suite::build_coverage_report(&root)
                        .map_err(|err| err.to_string())?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
                    );
                    Ok(())
                }
                "markdown" => {
                    let report = dicom_test_suite::build_coverage_report(&root)
                        .map_err(|err| err.to_string())?;
                    print!(
                        "{}",
                        dicom_test_suite::render_coverage_report_markdown(&report)
                    );
                    Ok(())
                }
                other => Err(format!("unsupported report format: {other}")),
            }
        }
        "standards" => {
            let subcommand = args
                .next()
                .ok_or_else(|| "standards requires a subcommand".to_string())?;
            match subcommand.as_str() {
                "check-lock" => {
                    let mut lock_path = String::from("standards.lock.json");
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--lock" => {
                                lock_path = args
                                    .next()
                                    .ok_or_else(|| "--lock requires a path".to_string())?;
                            }
                            "--help" | "-h" => {
                                print_standards_check_lock_usage();
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!(
                                    "unknown standards check-lock argument: {unknown}"
                                ));
                            }
                        }
                    }
                    let summary = dicom_test_suite::check_standards_lock_path(&lock_path)
                        .map_err(|err| err.to_string())?;
                    print!(
                        "{}",
                        dicom_test_suite::format_standards_lock_summary(&summary)
                    );
                    Ok(())
                }
                "gaps" => {
                    let mut registry_path = String::from("cases/registry.json");
                    let mut profile = None;
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--profile" => {
                                profile = Some(
                                    args.next()
                                        .ok_or_else(|| "--profile requires a value".to_string())?,
                                );
                            }
                            "--registry" => {
                                registry_path = args
                                    .next()
                                    .ok_or_else(|| "--registry requires a path".to_string())?;
                            }
                            "--help" | "-h" => {
                                print_standards_gaps_usage();
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!("unknown standards gaps argument: {unknown}"));
                            }
                        }
                    }
                    let profile =
                        profile.ok_or_else(|| "standards gaps requires --profile".to_string())?;
                    let output = dicom_test_suite::standards_gaps_from_registry_path(
                        &registry_path,
                        &profile,
                    )
                    .map_err(|err| err.to_string())?;
                    print!("{output}");
                    Ok(())
                }
                "verify-kb" => {
                    let mut edition = None;
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--edition" => {
                                edition = Some(
                                    args.next()
                                        .ok_or_else(|| "--edition requires a value".to_string())?,
                                );
                            }
                            "--help" | "-h" => {
                                print_standards_verify_kb_usage();
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!(
                                    "unknown standards verify-kb argument: {unknown}"
                                ));
                            }
                        }
                    }
                    let edition = edition
                        .ok_or_else(|| "standards verify-kb requires --edition".to_string())?;
                    if edition != "2026b" {
                        return Err(format!(
                            "unsupported standards edition {edition}; expected 2026b"
                        ));
                    }
                    println!("status\tunavailable");
                    println!("edition\t{edition}");
                    println!(
                        "reason\tstandalone CLI cannot access the dicom-standard-kb MCP server or local KB database metadata"
                    );
                    println!(
                        "recommended_action\tverify through the configured dicom-standard-kb MCP tools and update standards.lock.json when commit and DB hash metadata are exposed"
                    );
                    Ok(())
                }
                "--help" | "-h" => {
                    print_standards_usage();
                    Ok(())
                }
                unknown => Err(format!("unknown standards subcommand: {unknown}")),
            }
        }
        "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        unknown => Err(format!("unknown command: {unknown}")),
    }
}

fn print_usage() {
    println!("{}", dicom_test_suite::version_banner());
    println!("usage:");
    println!(
        "  dicom-test-suite generate --profile PROFILE --out PATH [--seed SEED] [--include-stress]"
    );
    println!(
        "  dicom-test-suite list-cases [--profile PROFILE] [--status STATUS] [--registry PATH]"
    );
    println!("  dicom-test-suite interoperate <media-dicomdir|protocol-baseline> ...");
    println!("  dicom-test-suite validate GENERATED_ROOT");
    println!("  dicom-test-suite report GENERATED_ROOT --format json|markdown");
    println!("  dicom-test-suite standards check-lock [--lock PATH]");
    println!("  dicom-test-suite standards gaps --profile PROFILE [--registry PATH]");
    println!("  dicom-test-suite standards verify-kb --edition 2026b");
}

fn print_interoperate_usage() {
    println!("usage:");
    println!(
        "  dicom-test-suite interoperate media-dicomdir GENERATED_ROOT --dcmmkdir PATH --dcmdump PATH --dciodvfy PATH [--dcentvfy PATH] --format json|markdown [--timeout-seconds N]"
    );
    println!(
        "  dicom-test-suite interoperate protocol-baseline GENERATED_ROOT --format json|markdown [--seed SEED] [--fixtures PATH]"
    );
}

fn required_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn required_path(value: Option<String>, option: &str) -> Result<std::path::PathBuf, String> {
    value
        .map(Into::into)
        .ok_or_else(|| format!("media-dicomdir requires {option}"))
}

fn required_format(value: Option<String>) -> Result<String, String> {
    value.ok_or_else(|| "interoperate requires --format".to_string())
}

fn print_media_qualification_markdown(
    qualification: &dicom_test_suite::media::DicomDirQualification,
) {
    println!("# DICOMDIR interoperability qualification\n");
    println!("- File-set ID: `{}`", qualification.file_set_id);
    println!("- Members: {}", qualification.member_count);
    println!(
        "- Provider: `{}` `{}`",
        qualification.provider.provider_id, qualification.provider.version
    );
    println!(
        "- Independent interoperability proven: {}",
        qualification.independent_interoperability_proven
    );
    println!(
        "- Independent dcm4che peer: `{:?}`",
        qualification.evidence.dcm4che_independent_peer
    );
    println!(
        "\nA same-provider DCMTK parser pass is baseline evidence, not independent promotion."
    );
}

fn print_generate_usage() {
    println!(
        "usage: dicom-test-suite generate --profile PROFILE --out PATH [--seed SEED] [--include-stress]"
    );
}

fn print_list_cases_usage() {
    println!(
        "usage: dicom-test-suite list-cases [--profile PROFILE] [--status STATUS] [--registry PATH]"
    );
}

fn print_validate_usage() {
    println!("usage: dicom-test-suite validate GENERATED_ROOT");
}

fn print_report_usage() {
    println!("usage:");
    println!("  dicom-test-suite report GENERATED_ROOT --format json|markdown");
    println!(
        "  dicom-test-suite report gaps --format json|markdown [--registry PATH] [--standards-lock PATH]"
    );
}

fn print_standards_usage() {
    println!("usage:");
    println!("  dicom-test-suite standards check-lock [--lock PATH]");
    println!("  dicom-test-suite standards gaps --profile PROFILE [--registry PATH]");
    println!("  dicom-test-suite standards verify-kb --edition 2026b");
}

fn print_standards_check_lock_usage() {
    println!("usage: dicom-test-suite standards check-lock [--lock PATH]");
}

fn print_standards_gaps_usage() {
    println!("usage: dicom-test-suite standards gaps --profile PROFILE [--registry PATH]");
}

fn print_standards_verify_kb_usage() {
    println!("usage: dicom-test-suite standards verify-kb --edition 2026b");
}

fn parse_seed(seed: String) -> Result<u64, String> {
    seed.parse()
        .map_err(|_| format!("--seed must be a non-negative integer: {seed}"))
}
