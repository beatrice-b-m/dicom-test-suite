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
            let mut format = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--format" => {
                        format = Some(
                            args.next()
                                .ok_or_else(|| "--format requires a value".to_string())?,
                        );
                    }
                    unknown => {
                        return Err(format!("unknown report argument: {unknown}"));
                    }
                }
            }
            let format = format.ok_or_else(|| "report requires --format".to_string())?;
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
    println!("  dicom-test-suite validate GENERATED_ROOT");
    println!("  dicom-test-suite report GENERATED_ROOT --format json|markdown");
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
    println!("usage: dicom-test-suite report GENERATED_ROOT --format json|markdown");
}

fn parse_seed(seed: String) -> Result<u64, String> {
    seed.parse()
        .map_err(|_| format!("--seed must be a non-negative integer: {seed}"))
}
