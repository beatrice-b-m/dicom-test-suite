use std::process::ExitCode;
mod external_corpus_cli;

fn main() -> ExitCode {
    let raw_arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let command = command_context(&raw_arguments);
    let machine = requests_machine_json(&raw_arguments);
    let result = external_corpus_cli::try_run(&raw_arguments).unwrap_or_else(|| {
        run().map_err(|message| {
            synth_dicom_gen::cli_protocol::CliFailure::classify(command, message)
        })
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            if machine {
                match serde_json::to_string(&failure.envelope()) {
                    Ok(envelope) => eprintln!("{envelope}"),
                    Err(_) => eprintln!(
                        "{{\"cli_api_version\":\"1.0.0\",\"command\":\"internal\",\"status\":\"error\",\"error\":{{\"code\":\"internal.serialization.failed\",\"message\":\"serialize CLI error envelope\",\"context\":{{}},\"retryable\":false}}}}"
                    ),
                }
            } else {
                eprintln!("{}", failure.human_message);
            }
            ExitCode::from(failure.exit)
        }
    }
}

fn command_context(arguments: &[String]) -> String {
    let mut index = 0;
    if arguments.first().map(String::as_str) == Some("--resource-root") {
        index = 2;
    }
    let Some(command) = arguments.get(index) else {
        return "command".to_string();
    };
    if matches!(
        command.as_str(),
        "templates" | "report" | "standards" | "conformance" | "interoperate"
    ) {
        if let Some(subcommand) = arguments.get(index + 1) {
            return format!("{command} {subcommand}");
        }
    }
    command.clone()
}

fn requests_machine_json(arguments: &[String]) -> bool {
    if external_corpus_cli::recognizes(arguments) && arguments.iter().any(|arg| arg == "--cli-api")
    {
        return true;
    }
    let json = arguments
        .windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json");
    let offset = usize::from(arguments.first().map(String::as_str) == Some("--resource-root")) * 2;
    if arguments.get(offset).map(String::as_str) == Some("report") {
        return json && arguments.iter().any(|argument| argument == "--cli-api");
    }
    if json {
        return true;
    }
    arguments.get(offset).map(String::as_str) == Some("templates")
        && arguments.get(offset + 1).map(String::as_str) == Some("describe")
        && !arguments.iter().any(|argument| argument == "--format")
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1).peekable();
    let explicit_resource_root = if args.peek().map(String::as_str) == Some("--resource-root") {
        args.next();
        Some(
            args.next()
                .ok_or_else(|| "--resource-root requires a path".to_string())?,
        )
    } else {
        None
    };
    let resources = if let Some(root) = explicit_resource_root.as_deref() {
        synth_dicom_gen::engine_resources::EngineResources::explicit(root)
            .map_err(|error| error.to_string())?
    } else {
        synth_dicom_gen::engine_resources::EngineResources::embedded()
    };
    let Some(command) = args.next() else {
        println!("{}", synth_dicom_gen::version_banner());
        return Ok(());
    };
    let mut resource_snapshot = None;
    let mut resource_path = |logical_path: &str| -> Result<String, String> {
        let snapshot = match resource_snapshot.as_ref() {
            Some(snapshot) => snapshot,
            None => {
                resource_snapshot = Some(resources.snapshot().map_err(|error| error.to_string())?);
                resource_snapshot
                    .as_ref()
                    .expect("snapshot was initialized")
            }
        };
        Ok(snapshot
            .root()
            .join(logical_path)
            .to_string_lossy()
            .into_owned())
    };

    match command.as_str() {
        "version" => {
            let mut format = None;
            while let Some(argument) = args.next() {
                match argument.as_str() {
                    "--format" => format = Some(required_value(&mut args, "--format")?),
                    "--help" | "-h" => {
                        println!("Usage: synth-dicom-gen version [--format json]");
                        return Ok(());
                    }
                    unknown => return Err(format!("unknown version argument: {unknown}")),
                }
            }
            match format.as_deref() {
                None => println!("{}", synth_dicom_gen::version_banner()),
                Some("json") => {
                    let result = synth_dicom_gen::discovery::version_result(&resources)
                        .map_err(|error| error.to_string())?;
                    let envelope =
                        synth_dicom_gen::cli_protocol::SuccessEnvelope::new("version", result);
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&envelope)
                            .map_err(|error| error.to_string())?
                    );
                }
                Some(other) => return Err(format!("unsupported version format: {other}")),
            }
            Ok(())
        }
        "capabilities" => {
            let mut format = None;
            while let Some(argument) = args.next() {
                match argument.as_str() {
                    "--format" => format = Some(required_value(&mut args, "--format")?),
                    "--help" | "-h" => {
                        println!("Usage: synth-dicom-gen capabilities --format json");
                        return Ok(());
                    }
                    unknown => return Err(format!("unknown capabilities argument: {unknown}")),
                }
            }
            match format.as_deref() {
                Some("json") => {
                    let result = synth_dicom_gen::discovery::capabilities_result(&resources)
                        .map_err(|error| error.to_string())?;
                    let envelope =
                        synth_dicom_gen::cli_protocol::SuccessEnvelope::new("capabilities", result);
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&envelope)
                            .map_err(|error| error.to_string())?
                    );
                    Ok(())
                }
                Some(other) => Err(format!("unsupported capabilities format: {other}")),
                None => Err("capabilities requires --format json".to_string()),
            }
        }
        "conformance" => {
            let subcommand = args
                .next()
                .ok_or_else(|| "conformance requires a subcommand".to_string())?;
            match subcommand.as_str() {
                "check-tools" => {
                    let mut config =
                        resource_path(synth_dicom_gen::conformance::DEFAULT_VALIDATOR_CONFIG)?;
                    let mut format = None;
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--config" => {
                                config = args
                                    .next()
                                    .ok_or_else(|| "--config requires a path".to_string())?;
                            }
                            "--format" => format = Some(required_value(&mut args, "--format")?),
                            "--help" | "-h" => {
                                println!(
                                    "Usage: synth-dicom-gen conformance check-tools [--config PATH] [--format json]"
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
                    let report = synth_dicom_gen::conformance::check_tools_path(config)?;
                    match format.as_deref() {
                        None => println!(
                            "{}",
                            serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
                        ),
                        Some("json") => write_machine_success(
                            "conformance check-tools",
                            synth_dicom_gen::cli_protocol::ConformanceResult::new(
                                "check_tools",
                                report,
                            ),
                        )?,
                        Some(other) => {
                            return Err(format!("unsupported conformance format: {other}"));
                        }
                    }
                    Ok(())
                }
                "run" => {
                    let generated_root = args.next().ok_or_else(|| {
                        "conformance run requires a generated root path".to_string()
                    })?;
                    let mut out = None;
                    let mut config =
                        resource_path(synth_dicom_gen::conformance::DEFAULT_VALIDATOR_CONFIG)?;
                    let mut format = None;
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
                            "--format" => format = Some(required_value(&mut args, "--format")?),
                            "--help" | "-h" => {
                                println!(
                                    "Usage: synth-dicom-gen conformance run GENERATED_ROOT --out EVIDENCE_ROOT [--config PATH] [--format json]"
                                );
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!("unknown conformance run argument: {unknown}"));
                            }
                        }
                    }
                    let out = out.ok_or_else(|| "conformance run requires --out".to_string())?;
                    let evidence = synth_dicom_gen::conformance::run_conformance(
                        generated_root,
                        &out,
                        config,
                    )?;
                    let outcome = serde_json::json!({
                        "evidence_root": out,
                        "run_id": evidence["run_id"],
                        "instance_count": evidence["instances"].as_array().map(Vec::len).unwrap_or(0),
                        "evidence_path": format!("{out}/conformance-run.json")
                    });
                    match format.as_deref() {
                        None => {
                            println!("evidence_root\t{out}");
                            println!("run_id\t{}", evidence["run_id"].as_str().unwrap_or(""));
                            println!(
                                "instances\t{}",
                                evidence["instances"].as_array().map(Vec::len).unwrap_or(0)
                            );
                        }
                        Some("json") => write_machine_success(
                            "conformance run",
                            synth_dicom_gen::cli_protocol::ConformanceResult::new("run", outcome),
                        )?,
                        Some(other) => {
                            return Err(format!("unsupported conformance format: {other}"));
                        }
                    }
                    Ok(())
                }
                "verify" => {
                    let evidence_root = args.next().ok_or_else(|| {
                        "conformance verify requires an evidence root path".to_string()
                    })?;
                    let mut allowlist =
                        resource_path(synth_dicom_gen::conformance::DEFAULT_ACCEPTED_FINDINGS)?;
                    let mut format = None;
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--allowlist" => {
                                allowlist = args
                                    .next()
                                    .ok_or_else(|| "--allowlist requires a path".to_string())?;
                            }
                            "--format" => format = Some(required_value(&mut args, "--format")?),
                            "--help" | "-h" => {
                                println!(
                                    "Usage: synth-dicom-gen conformance verify EVIDENCE_ROOT [--allowlist PATH] [--format json]"
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
                    let result = synth_dicom_gen::conformance::verify_conformance_with_resources(
                        evidence_root,
                        allowlist,
                        &resources,
                    )?;
                    let failures = result["failures"].as_array().cloned().unwrap_or_default();
                    match format.as_deref() {
                        None => {
                            println!(
                                "accepted_findings\t{}",
                                result["accepted_findings"].as_u64().unwrap_or(0)
                            );
                            println!("verification_failures\t{}", failures.len());
                            for failure in &failures {
                                println!("failure\t{}", failure.as_str().unwrap_or("unknown"));
                            }
                        }
                        Some("json") if failures.is_empty() => write_machine_success(
                            "conformance verify",
                            synth_dicom_gen::cli_protocol::ConformanceResult::new("verify", result),
                        )?,
                        Some("json") => {}
                        Some(other) => {
                            return Err(format!("unsupported conformance format: {other}"));
                        }
                    }
                    if failures.is_empty() {
                        Ok(())
                    } else {
                        Err("conformance verification failed".to_string())
                    }
                }
                "--help" | "-h" => {
                    println!("Usage: synth-dicom-gen conformance <check-tools|run|verify>");
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
            let mut format = None;
            let mut case_ids = Vec::new();

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
                    "--case-id" => case_ids.push(required_value(&mut args, "--case-id")?),
                    "--format" => format = Some(required_value(&mut args, "--format")?),
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
                synth_dicom_gen::prepare_generation_run(synth_dicom_gen::GenerateOptions {
                    profile,
                    out_dir: out_dir.into(),
                    seed,
                    include_stress,
                })
                .map_err(|err| err.to_string())?;
            let summary = if case_ids.is_empty() {
                synth_dicom_gen::write_generation_run_with_resources(&prepared, &resources)
            } else {
                synth_dicom_gen::write_selected_generation_run_with_resources(
                    &prepared, case_ids, &resources,
                )
            }
            .map_err(|err| err.to_string())?;

            match format.as_deref() {
                None => {
                    println!("profile\t{}", prepared.profile);
                    println!("seed\t{}", prepared.seed);
                    println!("include_stress\t{}", prepared.include_stress);
                    println!("out\t{}", prepared.out_dir.display());
                    println!("manifest\t{}", prepared.manifest_path.display());
                    println!("files_written\t{}", summary.files_written);
                    println!("manifest_written\t{}", summary.manifest_written);
                }
                Some("json") => {
                    write_machine_success("generate", generation_result(&prepared, &summary)?)?
                }
                Some(other) => return Err(format!("unsupported generate format: {other}")),
            }
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
                        synth_dicom_gen::media_sources::load_mixed_media_sources(&generated_root)
                            .map_err(|error| error.to_string())?;
                    let qualification = synth_dicom_gen::media_runner::run_dicomdir_qualification(
                        &synth_dicom_gen::media_runner::DicomDirRunRequest {
                            tools: synth_dicom_gen::media_runner::MediaToolPaths {
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
                        "json" => write_machine_success(
                            "interoperate media-dicomdir",
                            synth_dicom_gen::cli_protocol::InteroperabilityResult::new(
                                "media_dicomdir",
                                qualification,
                            ),
                        )?,
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
                    let mut fixtures = resource_path("security/fixtures/fixtures.lock.json")?;
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
                        synth_dicom_gen::media_sources::load_mixed_media_sources(&generated_root)
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
                            Ok(synth_dicom_gen::protocol::SourceCaseLink {
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
                        synth_dicom_gen::protocol_baseline::build_unavailable_protocol_baseline(
                            synth_dicom_gen::protocol_baseline::ProtocolBaselineInput {
                                run_seed: seed,
                                harness: synth_dicom_gen::protocol::ToolFingerprint {
                                    id: "synth-dicom-gen-protocol-baseline".to_string(),
                                    version: env!("CARGO_PKG_VERSION").to_string(),
                                    executable_sha256: synth_dicom_gen::sha256_hex(
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
                        "json" => write_machine_success(
                            "interoperate protocol-baseline",
                            synth_dicom_gen::cli_protocol::InteroperabilityResult::new(
                                "protocol_baseline",
                                report,
                            ),
                        )?,
                        "markdown" => print!(
                            "{}",
                            synth_dicom_gen::protocol_baseline::protocol_report_markdown(&report)
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
        "compose" => {
            let mut spec_path = None;
            let mut out_dir = None;
            let mut seed = 1_u64;
            let mut catalog_path = "templates/catalog.json".to_string();
            let mut dry_run = false;
            let mut format = None;
            while let Some(argument) = args.next() {
                match argument.as_str() {
                    "--spec" => spec_path = Some(required_value(&mut args, "--spec")?),
                    "--out" => out_dir = Some(required_value(&mut args, "--out")?),
                    "--seed" => seed = parse_seed(required_value(&mut args, "--seed")?)?,
                    "--catalog" => catalog_path = required_value(&mut args, "--catalog")?,
                    "--dry-run" => dry_run = true,
                    "--format" => format = Some(required_value(&mut args, "--format")?),
                    "--help" | "-h" => {
                        print_compose_usage();
                        return Ok(());
                    }
                    unknown => return Err(format!("unknown compose argument: {unknown}")),
                }
            }
            let spec_path = spec_path.ok_or_else(|| "compose requires --spec".to_string())?;
            let out_dir = out_dir.ok_or_else(|| "compose requires --out".to_string())?;
            let (summary, output) = synth_dicom_gen::composition::compose_with_resources(
                &synth_dicom_gen::composition::ComposeOptions {
                    spec_path: spec_path.clone().into(),
                    out_dir: out_dir.into(),
                    seed,
                    catalog_path: catalog_path.into(),
                    dry_run,
                },
                &resources,
            )
            .map_err(|error| error.to_string())?;
            match format.as_deref() {
                Some("json") => {
                    write_machine_success("compose", composition_result(&summary, &output, seed)?)?
                }
                Some(other) => return Err(format!("unsupported compose format: {other}")),
                None if dry_run => println!(
                    "{}",
                    serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?
                ),
                None => {
                    println!("spec\t{spec_path}");
                    println!("seed\t{seed}");
                    println!("out\t{}", summary.out_dir.display());
                    println!("manifest\t{}", summary.manifest_path.display());
                    println!("instances_written\t{}", summary.instances_written);
                    println!("output_bytes\t{}", summary.output_bytes);
                }
            }
            Ok(())
        }
        "assemble" => {
            let mut request_path = None;
            let mut asset_root = None;
            let mut out_dir = None;
            let mut seed = 1_u64;
            let mut parallelism = 1_u32;
            let mut dry_run = false;
            let mut format = None;
            while let Some(argument) = args.next() {
                match argument.as_str() {
                    "--request" => request_path = Some(required_value(&mut args, "--request")?),
                    "--asset-root" => asset_root = Some(required_value(&mut args, "--asset-root")?),
                    "--out" => out_dir = Some(required_value(&mut args, "--out")?),
                    "--seed" => seed = parse_seed(required_value(&mut args, "--seed")?)?,
                    "--parallelism" => {
                        parallelism = required_value(&mut args, "--parallelism")?
                            .parse::<u32>()
                            .map_err(|_| "--parallelism must be a positive integer".to_string())?;
                        if parallelism == 0 {
                            return Err("--parallelism must be a positive integer".into());
                        }
                    }
                    "--dry-run" => dry_run = true,
                    "--format" => format = Some(required_value(&mut args, "--format")?),
                    "--help" | "-h" => {
                        print_assemble_usage();
                        return Ok(());
                    }
                    unknown => return Err(format!("unknown assemble argument: {unknown}")),
                }
            }
            let request_path = std::path::PathBuf::from(
                request_path.ok_or_else(|| "assemble requires --request".to_string())?,
            );
            let out_dir = std::path::PathBuf::from(
                out_dir.ok_or_else(|| "assemble requires --out".to_string())?,
            );
            let request_bytes = std::fs::read(&request_path).map_err(|error| {
                format!(
                    "assembly input read failed at {}: {error}",
                    request_path.display()
                )
            })?;
            let caller_asset_root = asset_root.map(std::path::PathBuf::from).unwrap_or_else(|| {
                request_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .to_path_buf()
            });
            let summary = synth_dicom_gen::assembly::assemble(
                &synth_dicom_gen::assembly::AssembleOptions {
                    request_bytes,
                    caller_asset_root,
                    output_root: out_dir,
                    seed,
                    parallelism,
                    dry_run,
                },
                &synth_dicom_gen::executor::cancellation::CancellationToken::new(),
                &resources,
            )
            .map_err(|error| error.to_string())?;
            match format.as_deref() {
                Some("json") => write_machine_success("assemble", assembly_result(&summary, seed))?,
                Some(other) => return Err(format!("unsupported assemble format: {other}")),
                None => {
                    println!("request\t{}", request_path.display());
                    println!("seed\t{seed}");
                    println!("out\t{}", summary.output_root.display());
                    println!("published\t{}", summary.published);
                    println!("artifacts_written\t{}", summary.artifacts_written);
                    println!("corpus_plan_sha256\t{}", summary.corpus_plan_sha256);
                }
            }
            Ok(())
        }
        "templates" => {
            let subcommand = args
                .next()
                .ok_or_else(|| "templates requires a subcommand".to_string())?;
            let mut catalog_path = resource_path("templates/catalog.json")?;
            match subcommand.as_str() {
                "list" => {
                    let mut format = String::from("table");
                    while let Some(argument) = args.next() {
                        match argument.as_str() {
                            "--catalog" => catalog_path = required_value(&mut args, "--catalog")?,
                            "--format" => format = required_value(&mut args, "--format")?,
                            "--help" | "-h" => {
                                print_templates_usage();
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!("unknown templates list argument: {unknown}"));
                            }
                        }
                    }
                    let catalog = synth_dicom_gen::composition::TemplateCatalog::load(catalog_path)
                        .map_err(|error| error.to_string())?;
                    match format.as_str() {
                        "json" => write_machine_success(
                            "templates list",
                            synth_dicom_gen::cli_protocol::TemplatesResult::new(
                                "list",
                                catalog.templates,
                            ),
                        )?,
                        "table" => {
                            println!("template_id\tversion\tstatus\tsop_class_uid\tdeterminism");
                            for template in &catalog.templates {
                                println!(
                                    "{}\t{}\t{:?}\t{}\t{}",
                                    template.template_id,
                                    template.template_version,
                                    template.status,
                                    template.sop_class_uid,
                                    template.determinism
                                );
                            }
                        }
                        other => return Err(format!("unsupported templates format: {other}")),
                    }
                    Ok(())
                }
                "describe" => {
                    let id = args
                        .next()
                        .ok_or_else(|| "templates describe requires a template ID".to_string())?;
                    let mut version = None;
                    let mut format = String::from("json");
                    while let Some(argument) = args.next() {
                        match argument.as_str() {
                            "--catalog" => catalog_path = required_value(&mut args, "--catalog")?,
                            "--version" => {
                                version =
                                    Some(required_value(&mut args, "--version")?.parse().map_err(
                                        |error: synth_dicom_gen::composition::TemplateError| {
                                            error.to_string()
                                        },
                                    )?)
                            }
                            "--format" => format = required_value(&mut args, "--format")?,
                            "--help" | "-h" => {
                                print_templates_usage();
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!(
                                    "unknown templates describe argument: {unknown}"
                                ));
                            }
                        }
                    }
                    let catalog = synth_dicom_gen::composition::TemplateCatalog::load(catalog_path)
                        .map_err(|error| error.to_string())?;
                    let descriptor = catalog
                        .resolve_qualified(&synth_dicom_gen::composition::TemplateId(id), version)
                        .map_err(|error| error.to_string())?;
                    match format.as_str() {
                        "json" => write_machine_success(
                            "templates describe",
                            synth_dicom_gen::cli_protocol::TemplatesResult::new(
                                "describe",
                                vec![descriptor.clone()],
                            ),
                        )?,
                        "text" => {
                            println!(
                                "template\t{}@{}",
                                descriptor.template_id, descriptor.template_version
                            );
                            println!("iod\t{}", descriptor.iod_name);
                            println!("sop_class_uid\t{}", descriptor.sop_class_uid);
                            println!("content_slots\t{}", descriptor.content_slots.len());
                        }
                        other => return Err(format!("unsupported templates format: {other}")),
                    }
                    Ok(())
                }
                "reference" => {
                    let mut format = String::from("markdown");
                    while let Some(argument) = args.next() {
                        match argument.as_str() {
                            "--catalog" => catalog_path = required_value(&mut args, "--catalog")?,
                            "--format" => format = required_value(&mut args, "--format")?,
                            "--help" | "-h" => {
                                print_templates_usage();
                                return Ok(());
                            }
                            unknown => {
                                return Err(format!(
                                    "unknown templates reference argument: {unknown}"
                                ));
                            }
                        }
                    }
                    let catalog = synth_dicom_gen::composition::TemplateCatalog::load(catalog_path)
                        .map_err(|error| error.to_string())?;
                    match format.as_str() {
                        "markdown" => print!("{}", catalog.render_reference_markdown()),
                        "json" => write_machine_success(
                            "templates reference",
                            synth_dicom_gen::cli_protocol::TemplatesResult::new(
                                "reference",
                                catalog.templates,
                            ),
                        )?,
                        other => {
                            return Err(format!("unsupported templates reference format: {other}"));
                        }
                    }
                    Ok(())
                }
                "--help" | "-h" => {
                    print_templates_usage();
                    Ok(())
                }
                unknown => Err(format!("unknown templates subcommand: {unknown}")),
            }
        }
        "list-cases" => {
            let mut registry_path = resource_path("cases/registry.json")?;
            let mut profile_filter = None;
            let mut status_filter = None;
            let mut format = None;

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
                    "--format" => format = Some(required_value(&mut args, "--format")?),
                    "--help" | "-h" => {
                        print_list_cases_usage();
                        return Ok(());
                    }
                    unknown => {
                        return Err(format!("unknown list-cases argument: {unknown}"));
                    }
                }
            }

            match format.as_deref() {
                None => {
                    let output = synth_dicom_gen::list_cases_from_registry_path(
                        registry_path,
                        profile_filter.as_deref(),
                        status_filter.as_deref(),
                    )
                    .map_err(|err| err.to_string())?;
                    print!("{output}");
                }
                Some("json") => {
                    let cases = synth_dicom_gen::case_list_entries_from_registry_path(
                        registry_path,
                        profile_filter.as_deref(),
                        status_filter.as_deref(),
                    )
                    .map_err(|err| err.to_string())?;
                    write_machine_success(
                        "list-cases",
                        synth_dicom_gen::cli_protocol::CaseListResult {
                            case_list_result_schema_version:
                                synth_dicom_gen::cli_protocol::CASE_LIST_RESULT_SCHEMA_VERSION,
                            profile_filter,
                            status_filter,
                            case_count: cases.len(),
                            cases,
                        },
                    )?;
                }
                Some(other) => return Err(format!("unsupported list-cases format: {other}")),
            }
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
            let mut format = None;
            while let Some(argument) = args.next() {
                match argument.as_str() {
                    "--format" => format = Some(required_value(&mut args, "--format")?),
                    unknown => return Err(format!("unknown validate argument: {unknown}")),
                }
            }

            let product = if let Some(resource_root) = explicit_resource_root.as_deref() {
                synth_dicom_gen::sdk::DicomTestSuite::explicit_resource_root(resource_root)
            } else {
                synth_dicom_gen::sdk::DicomTestSuite::embedded()
            }
            .map_err(|error| error.to_string())?;
            let outcome = product
                .validate(synth_dicom_gen::sdk::ValidateRequest::new(&root))
                .map_err(|error| error.to_string())?;
            match format.as_deref() {
                None => {
                    println!("generated_root\t{root}");
                    println!("manifest\t{}", outcome.manifest().path().display());
                    println!("files_checked\t{}", outcome.files_checked());
                    println!("validation_failures\t{}", outcome.failures().len());
                    for failure in outcome.failures() {
                        println!("failure\t{failure}");
                    }
                }
                Some("json") if outcome.is_valid() => write_machine_success(
                    "validate",
                    synth_dicom_gen::cli_protocol::ValidationResult {
                        validation_result_schema_version:
                            synth_dicom_gen::cli_protocol::VALIDATION_RESULT_SCHEMA_VERSION,
                        generated_root: root.clone(),
                        manifest_path: outcome.manifest().path().display().to_string(),
                        files_checked: outcome.files_checked(),
                        valid: true,
                        failures: Vec::new(),
                    },
                )?,
                Some("json") => {}
                Some(other) => return Err(format!("unsupported validate format: {other}")),
            }
            if outcome.is_valid() {
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
            let mut registry_path = None;
            let mut standards_lock_path = None;
            let mut cli_api = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--format" => {
                        format = Some(
                            args.next()
                                .ok_or_else(|| "--format requires a value".to_string())?,
                        );
                    }
                    "--registry" if gap_report => {
                        registry_path = Some(required_value(&mut args, "--registry")?);
                    }
                    "--standards-lock" if gap_report => {
                        standards_lock_path = Some(required_value(&mut args, "--standards-lock")?);
                    }
                    "--cli-api" => cli_api = Some(required_value(&mut args, "--cli-api")?),
                    unknown => {
                        return Err(format!("unknown report argument: {unknown}"));
                    }
                }
            }
            let format = format.ok_or_else(|| "report requires --format".to_string())?;
            if let Some(version) = cli_api.as_deref() {
                if version != synth_dicom_gen::cli_protocol::CLI_API_VERSION {
                    return Err(format!("unsupported CLI API version: {version}"));
                }
                if format != "json" {
                    return Err("--cli-api requires --format json".to_string());
                }
            }
            if gap_report {
                let registry_path = match registry_path {
                    Some(path) => path,
                    None => resource_path("cases/registry.json")?,
                };
                let standards_lock_path = match standards_lock_path {
                    Some(path) => path,
                    None => resource_path("standards.lock.json")?,
                };
                let report =
                    synth_dicom_gen::build_coverage_gap_report(registry_path, standards_lock_path)
                        .map_err(|err| err.to_string())?;
                return match format.as_str() {
                    "json" => {
                        if cli_api.is_some() {
                            write_machine_success(
                                "report gaps",
                                report_result("coverage_gaps", &report)?,
                            )?;
                        } else {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&report)
                                    .map_err(|err| err.to_string())?
                            );
                        }
                        Ok(())
                    }
                    "markdown" => {
                        print!(
                            "{}",
                            synth_dicom_gen::render_coverage_gap_report_markdown(&report)
                        );
                        Ok(())
                    }
                    other => Err(format!("unsupported report format: {other}")),
                };
            }
            match format.as_str() {
                "json" => {
                    let report =
                        synth_dicom_gen::build_coverage_report_with_resources(&root, &resources)
                            .map_err(|err| err.to_string())?;
                    if cli_api.is_some() {
                        write_machine_success("report", report_result("coverage", &report)?)?;
                    } else {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
                        );
                    }
                    Ok(())
                }
                "markdown" => {
                    let report =
                        synth_dicom_gen::build_coverage_report_with_resources(&root, &resources)
                            .map_err(|err| err.to_string())?;
                    print!(
                        "{}",
                        synth_dicom_gen::render_coverage_report_markdown(&report)
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
                    let mut lock_path = resource_path("standards.lock.json")?;
                    let mut format = None;
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--lock" => {
                                lock_path = args
                                    .next()
                                    .ok_or_else(|| "--lock requires a path".to_string())?;
                            }
                            "--format" => format = Some(required_value(&mut args, "--format")?),
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
                    let summary = synth_dicom_gen::check_standards_lock_path(&lock_path)
                        .map_err(|err| err.to_string())?;
                    match format.as_deref() {
                        None => print!(
                            "{}",
                            synth_dicom_gen::format_standards_lock_summary(&summary)
                        ),
                        Some("json") => write_machine_success(
                            "standards check-lock",
                            synth_dicom_gen::cli_protocol::StandardsResult::new(
                                "check_lock",
                                vec![summary],
                            ),
                        )?,
                        Some(other) => {
                            return Err(format!("unsupported standards format: {other}"));
                        }
                    }
                    Ok(())
                }
                "gaps" => {
                    let mut registry_path = resource_path("cases/registry.json")?;
                    let mut profile = None;
                    let mut format = None;
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
                            "--format" => format = Some(required_value(&mut args, "--format")?),
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
                    match format.as_deref() {
                        None => {
                            let output = synth_dicom_gen::standards_gaps_from_registry_path(
                                &registry_path,
                                &profile,
                            )
                            .map_err(|err| err.to_string())?;
                            print!("{output}");
                        }
                        Some("json") => {
                            let gaps = synth_dicom_gen::standards_gap_entries_from_registry_path(
                                &registry_path,
                                &profile,
                            )
                            .map_err(|err| err.to_string())?;
                            write_machine_success(
                                "standards gaps",
                                synth_dicom_gen::cli_protocol::StandardsResult::new("gaps", gaps),
                            )?;
                        }
                        Some(other) => {
                            return Err(format!("unsupported standards format: {other}"));
                        }
                    }
                    Ok(())
                }
                "verify-kb" => {
                    let mut edition = None;
                    let mut format = None;
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--edition" => {
                                edition = Some(
                                    args.next()
                                        .ok_or_else(|| "--edition requires a value".to_string())?,
                                );
                            }
                            "--format" => format = Some(required_value(&mut args, "--format")?),
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
                    if let Some(value) = format.as_deref() {
                        if value != "json" {
                            return Err(format!("unsupported standards format: {value}"));
                        }
                        return Err(format!(
                            "standards knowledge base unavailable for edition {edition}"
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

fn write_machine_success<T: serde::Serialize>(
    command: &'static str,
    result: T,
) -> Result<(), String> {
    let envelope = synth_dicom_gen::cli_protocol::SuccessEnvelope::new(command, result);
    println!(
        "{}",
        serde_json::to_string_pretty(&envelope).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn generation_result(
    prepared: &synth_dicom_gen::PreparedGenerationRun,
    summary: &synth_dicom_gen::GenerationSummary,
) -> Result<synth_dicom_gen::cli_protocol::GenerationResult, String> {
    let manifest_bytes = std::fs::read(&prepared.manifest_path)
        .map_err(|error| format!("read {}: {error}", prepared.manifest_path.display()))?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    let unavailable = unavailable_summaries(&manifest["skipped_cases"], "generation");
    Ok(synth_dicom_gen::cli_protocol::GenerationResult {
        generation_result_schema_version:
            synth_dicom_gen::cli_protocol::GENERATION_RESULT_SCHEMA_VERSION,
        outcome: synth_dicom_gen::cli_protocol::FileProducingOutcome {
            requested_output_root: prepared.out_dir.display().to_string(),
            manifest_path: Some(prepared.manifest_path.display().to_string()),
            run_kind: "curated_generation",
            seed: prepared.seed,
            request_schema_version: "1.0.0".to_string(),
            manifest_schema_version: manifest["manifest_schema_version"]
                .as_str()
                .ok_or_else(|| "generation manifest has no schema version".to_string())?
                .to_string(),
            product_version: synth_dicom_gen::PACKAGE_VERSION,
            emitted_artifact_count: summary.files_written,
            output_bytes: summary.output_bytes,
            unavailable_capability_count: unavailable.len(),
            unavailable_capabilities: unavailable,
            corpus_plan_sha256: summary.corpus_plan_sha256.clone(),
            published: true,
            publication_status: "published",
            validation_status: "passed",
            plan_preview: None,
        },
    })
}

fn composition_result(
    summary: &synth_dicom_gen::composition::ComposeSummary,
    output: &serde_json::Value,
    seed: u64,
) -> Result<synth_dicom_gen::cli_protocol::CompositionResult, String> {
    let request_schema_version = if summary.dry_run {
        output["composition_spec_schema_version"].as_str()
    } else {
        output
            .pointer("/run/composition_spec_schema_version")
            .and_then(serde_json::Value::as_str)
    }
    .ok_or_else(|| "composition result has no request schema version".to_string())?;
    let unavailable = if summary.dry_run {
        Vec::new()
    } else {
        unavailable_summaries(
            &output["composition"]["unavailable_capabilities"],
            "composition",
        )
    };
    let plan_preview = if summary.dry_run {
        let artifact_ids = output["plans"]
            .as_array()
            .ok_or_else(|| "composition dry-run has no plan array".to_string())?
            .iter()
            .map(|plan| {
                plan["instance_id"]
                    .as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| "composition dry-run plan has no instance ID".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Some(synth_dicom_gen::cli_protocol::CanonicalPlanPreview {
            artifact_count: artifact_ids.len(),
            artifact_ids,
        })
    } else {
        None
    };
    Ok(synth_dicom_gen::cli_protocol::CompositionResult {
        composition_result_schema_version:
            synth_dicom_gen::cli_protocol::COMPOSITION_RESULT_SCHEMA_VERSION,
        outcome: synth_dicom_gen::cli_protocol::FileProducingOutcome {
            requested_output_root: summary.out_dir.display().to_string(),
            manifest_path: (!summary.dry_run).then(|| summary.manifest_path.display().to_string()),
            run_kind: "qualified_composition",
            seed,
            request_schema_version: request_schema_version.to_string(),
            manifest_schema_version: if summary.dry_run {
                "1.0.0".to_string()
            } else {
                output["manifest_schema_version"]
                    .as_str()
                    .ok_or_else(|| "composition manifest has no schema version".to_string())?
                    .to_string()
            },
            product_version: synth_dicom_gen::PACKAGE_VERSION,
            emitted_artifact_count: summary.instances_written,
            output_bytes: summary.output_bytes,
            unavailable_capability_count: unavailable.len(),
            unavailable_capabilities: unavailable,
            corpus_plan_sha256: summary.corpus_plan_sha256.clone(),
            published: !summary.dry_run,
            publication_status: if summary.dry_run {
                "not_requested"
            } else {
                "published"
            },
            validation_status: if summary.dry_run { "not_run" } else { "passed" },
            plan_preview,
        },
    })
}

fn assembly_result(
    summary: &synth_dicom_gen::assembly::AssembleSummary,
    seed: u64,
) -> synth_dicom_gen::cli_protocol::AssemblyResult {
    synth_dicom_gen::cli_protocol::AssemblyResult {
        assembly_result_schema_version:
            synth_dicom_gen::cli_protocol::ASSEMBLY_RESULT_SCHEMA_VERSION,
        outcome: synth_dicom_gen::cli_protocol::FileProducingOutcome {
            requested_output_root: summary.output_root.display().to_string(),
            manifest_path: summary
                .manifest_path
                .as_ref()
                .map(|path| path.display().to_string()),
            run_kind: "structural_assembly",
            seed,
            request_schema_version: synth_dicom_gen::assembly::ASSEMBLY_REQUEST_SCHEMA_VERSION
                .into(),
            manifest_schema_version: synth_dicom_gen::assembly::ASSEMBLY_MANIFEST_SCHEMA_VERSION
                .into(),
            product_version: synth_dicom_gen::PACKAGE_VERSION,
            emitted_artifact_count: summary.artifacts_written,
            output_bytes: summary.output_bytes,
            unavailable_capability_count: 0,
            unavailable_capabilities: vec![],
            corpus_plan_sha256: summary.corpus_plan_sha256.clone(),
            published: summary.published,
            publication_status: if summary.published {
                "published"
            } else {
                "not_requested"
            },
            validation_status: if summary.published {
                "passed"
            } else {
                "not_run"
            },
            plan_preview: (!summary.published).then(|| {
                synth_dicom_gen::cli_protocol::CanonicalPlanPreview {
                    artifact_count: summary.artifact_ids.len(),
                    artifact_ids: summary.artifact_ids.clone(),
                }
            }),
        },
    }
}

fn unavailable_summaries(
    value: &serde_json::Value,
    namespace: &str,
) -> Vec<synth_dicom_gen::cli_protocol::UnavailableCapabilitySummary> {
    let mut summaries = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let capability_id = entry
                .get("capability_id")
                .or_else(|| entry.get("case_id"))?
                .as_str()?;
            let reason = entry.get("reason_code")?.as_str()?;
            let reason_code = if reason.contains('.') {
                reason.to_string()
            } else {
                format!("{namespace}.{reason}")
            };
            Some(
                synth_dicom_gen::cli_protocol::UnavailableCapabilitySummary {
                    capability_id: capability_id.to_string(),
                    reason_code,
                },
            )
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        (&left.capability_id, &left.reason_code).cmp(&(&right.capability_id, &right.reason_code))
    });
    summaries.dedup();
    summaries
}

fn report_result(
    fallback_kind: &str,
    report: &serde_json::Value,
) -> Result<synth_dicom_gen::cli_protocol::ReportResult<serde_json::Value>, String> {
    let report_kind = report
        .get("report_kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(fallback_kind);
    let report_schema_version = [
        "coverage_report_schema_version",
        "coverage_gap_report_schema_version",
        "composition_report_schema_version",
        "structural_assembly_report_schema_version",
    ]
    .into_iter()
    .find_map(|field| report.get(field).and_then(serde_json::Value::as_str))
    .ok_or_else(|| "report has no supported schema version".to_string())?;
    let mut result = synth_dicom_gen::cli_protocol::ReportResult::new(
        report_kind,
        report_schema_version,
        report.clone(),
    );
    if report_kind == "external_corpus" {
        result.report_result_schema_version =
            synth_dicom_gen::cli_protocol::EXTERNAL_REPORT_RESULT_SCHEMA_VERSION;
    }
    Ok(result)
}

fn print_usage() {
    println!("{}", synth_dicom_gen::version_banner());
    println!("usage:");
    println!("  synth-dicom-gen version [--format json]");
    println!("  synth-dicom-gen capabilities --format json");
    println!(
        "  synth-dicom-gen generate --profile PROFILE --out PATH [--seed SEED] [--include-stress] [--format json]"
    );
    println!(
        "  synth-dicom-gen compose --spec PATH --out PATH [--seed SEED] [--dry-run] [--format json]"
    );
    println!(
        "  synth-dicom-gen assemble --request PATH --out PATH [--asset-root PATH] [--seed SEED] [--parallelism N] [--dry-run] [--format json]"
    );
    println!(
        "  synth-dicom-gen list-cases [--profile PROFILE] [--status STATUS] [--registry PATH] [--format json]"
    );
    println!("  synth-dicom-gen templates <list|describe|reference> ...");
    println!("  synth-dicom-gen interoperate <media-dicomdir|protocol-baseline> ...");
    println!("  synth-dicom-gen validate GENERATED_ROOT [--format json]");
    println!("  synth-dicom-gen report GENERATED_ROOT --format json|markdown [--cli-api 1.0.0]");
    println!("  synth-dicom-gen standards check-lock [--lock PATH] [--format json]");
    println!(
        "  synth-dicom-gen standards gaps --profile PROFILE [--registry PATH] [--format json]"
    );
    println!("  synth-dicom-gen standards verify-kb --edition 2026b [--format json]");
}

fn print_interoperate_usage() {
    println!("usage:");
    println!(
        "  synth-dicom-gen interoperate media-dicomdir GENERATED_ROOT --dcmmkdir PATH --dcmdump PATH --dciodvfy PATH [--dcentvfy PATH] --format json|markdown [--timeout-seconds N]"
    );
    println!(
        "  synth-dicom-gen interoperate protocol-baseline GENERATED_ROOT --format json|markdown [--seed SEED] [--fixtures PATH]"
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
    qualification: &synth_dicom_gen::media::DicomDirQualification,
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
        "usage: synth-dicom-gen generate --corpus PATH --asset-root ROOT --profile PROFILE --out PATH [--case-id ID ...] [--seed SEED] [--parallelism N] [--dry-run] [--include-stress] [--format json]"
    );
    println!(
        "usage: synth-dicom-gen generate --profile PROFILE --out PATH [--seed SEED] [--include-stress] [--case-id CASE_ID ...] [--format json]"
    );
}

fn print_compose_usage() {
    println!(
        "usage: synth-dicom-gen compose --spec PATH --out PATH [--seed SEED] [--dry-run] [--catalog PATH] [--format json]"
    );
}

fn print_assemble_usage() {
    println!(
        "usage: synth-dicom-gen assemble --request PATH --out PATH [--asset-root PATH] [--seed SEED] [--parallelism N] [--dry-run] [--format json]"
    );
}

fn print_list_cases_usage() {
    println!(
        "usage: synth-dicom-gen list-cases [--profile PROFILE] [--status STATUS] [--registry PATH] [--format json]"
    );
}

fn print_templates_usage() {
    println!("usage:");
    println!("  synth-dicom-gen templates list [--format table|json] [--catalog PATH]");
    println!(
        "  synth-dicom-gen templates describe TEMPLATE_ID [--version VERSION] [--format json|text] [--catalog PATH]"
    );
    println!("  synth-dicom-gen templates reference [--format markdown|json] [--catalog PATH]");
}

fn print_validate_usage() {
    println!("usage: synth-dicom-gen validate GENERATED_ROOT [--format json]");
}

fn print_report_usage() {
    println!("usage:");
    println!("  synth-dicom-gen report GENERATED_ROOT --format json|markdown [--cli-api 1.0.0]");
    println!(
        "  synth-dicom-gen report gaps --format json|markdown [--registry PATH] [--standards-lock PATH] [--cli-api 1.0.0]"
    );
}

fn print_standards_usage() {
    println!("usage:");
    println!("  synth-dicom-gen standards check-lock [--lock PATH]");
    println!("  synth-dicom-gen standards gaps --profile PROFILE [--registry PATH]");
    println!("  synth-dicom-gen standards verify-kb --edition 2026b");
}

fn print_standards_check_lock_usage() {
    println!("usage: synth-dicom-gen standards check-lock [--lock PATH] [--format json]");
}

fn print_standards_gaps_usage() {
    println!(
        "usage: synth-dicom-gen standards gaps --profile PROFILE [--registry PATH] [--format json]"
    );
}

fn print_standards_verify_kb_usage() {
    println!("usage: synth-dicom-gen standards verify-kb --edition 2026b [--format json]");
}

fn parse_seed(seed: String) -> Result<u64, String> {
    seed.parse()
        .map_err(|_| format!("--seed must be a non-negative integer: {seed}"))
}
