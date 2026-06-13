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
        "list-cases" => {
            let mut registry_path = String::from("cases/registry.json");
            let mut profile_filter = None;

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
            )
            .map_err(|err| err.to_string())?;
            print!("{output}");
            Ok(())
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
    println!("  dicom-test-suite list-cases [--profile PROFILE] [--registry PATH]");
}

fn print_list_cases_usage() {
    println!("usage: dicom-test-suite list-cases [--profile PROFILE] [--registry PATH]");
}
