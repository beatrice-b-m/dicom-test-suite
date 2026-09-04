//! External corpus CLI: execution imports only the supported SDK facade.
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::Write;
use synth_dicom_gen::cli_protocol::{
    CliFailure, EXTERNAL_GENERATION_RESULT_SCHEMA_VERSION, ExternalGenerationResult,
    SuccessEnvelope,
};
use synth_dicom_gen::sdk::{
    CorpusSelector, DicomTestSuite, GenerateCorpusOutcome, GenerateCorpusRequest,
    InspectCorpusRequest,
};

fn failure(code: &'static str, detail: impl Into<String>) -> CliFailure {
    CliFailure::from_code("generate", code, detail)
}
fn sdk(error: synth_dicom_gen::sdk::SdkError) -> CliFailure {
    CliFailure::from_sdk("generate", error)
}

pub(super) fn try_inspect(arguments: &[String]) -> Option<Result<(), CliFailure>> {
    let offset = usize::from(arguments.first().map(String::as_str) == Some("--resource-root")) * 2;
    if arguments.get(offset).map(String::as_str) != Some("capabilities") {
        return None;
    }
    Some(inspect(
        &arguments[offset + 1..],
        if offset == 2 {
            arguments.get(1).map(String::as_str)
        } else {
            None
        },
    ))
}

fn inspect(arguments: &[String], resource_root: Option<&str>) -> Result<(), CliFailure> {
    let fail = |code, detail: &str| CliFailure::from_code("capabilities", code, detail);
    let mut corpus = None;
    let mut assets = None;
    let mut profile = None;
    let mut cases = Vec::new();
    let mut seed = 1u64;
    let mut parallelism = 1u32;
    let mut stress = false;
    let mut format = None;
    let mut seen = BTreeSet::new();
    let mut args = arguments.iter();
    while let Some(option) = args.next() {
        if option != "--case-id" && !seen.insert(option.as_str()) {
            return Err(fail(
                "command.syntax.invalid",
                "duplicate capabilities option",
            ));
        }
        let mut value = || {
            args.next().filter(|v| !v.starts_with("--")).ok_or_else(|| {
                fail(
                    "command.argument.missing",
                    "capabilities option requires a value",
                )
            })
        };
        match option.as_str() {
            "--corpus" => corpus = Some(value()?.clone()),
            "--asset-root" => assets = Some(value()?.clone()),
            "--profile" => profile = Some(value()?.clone()),
            "--case-id" => cases.push(value()?.clone()),
            "--seed" => {
                seed = value()?
                    .parse()
                    .map_err(|_| fail("command.syntax.invalid", "invalid seed"))?
            }
            "--parallelism" => {
                parallelism = value()?
                    .parse()
                    .map_err(|_| fail("command.syntax.invalid", "invalid parallelism"))?
            }
            "--include-stress" => stress = true,
            "--format" => format = Some(value()?.clone()),
            "--cli-api" => {
                if value()? != "1.0.0" {
                    return Err(fail(
                        "request.version.unsupported",
                        "unsupported CLI API version",
                    ));
                }
            }
            "--help" | "-h" => {
                println!(
                    "Usage: synth-dicom-gen capabilities --format json [--corpus ./definition.json --asset-root ROOT [--profile PROFILE [--case-id ID] [--seed N] [--parallelism N] [--include-stress]]]"
                );
                return Ok(());
            }
            _ => {
                return Err(fail(
                    "command.syntax.invalid",
                    "unknown capabilities option",
                ));
            }
        }
    }
    if format.as_deref() != Some("json") {
        return Err(fail(
            "command.syntax.invalid",
            "capabilities requires --format json",
        ));
    }
    if parallelism == 0 {
        return Err(fail(
            "request.schema.invalid",
            "parallelism must be positive",
        ));
    }
    if profile.is_none()
        && (seen.contains("--seed")
            || seen.contains("--parallelism")
            || stress
            || !cases.is_empty())
    {
        return Err(fail(
            "command.argument.missing",
            "selection options require --profile",
        ));
    }
    if corpus.is_none() && (assets.is_some() || profile.is_some()) {
        return Err(fail(
            "command.argument.missing",
            "loaded corpus options require --corpus",
        ));
    }
    if corpus.is_some() && assets.is_none() {
        return Err(fail(
            "command.argument.missing",
            "--corpus requires explicit --asset-root",
        ));
    }
    let sdk_error = |error| CliFailure::from_sdk("capabilities", error);
    let product = match resource_root {
        Some(root) => DicomTestSuite::explicit_resource_root(root),
        None => DicomTestSuite::embedded(),
    }
    .map_err(sdk_error)?;
    let result = if let Some(descriptor) = corpus {
        let mut request = InspectCorpusRequest::from_file(descriptor, assets.unwrap())
            .with_seed(seed)
            .with_parallelism(parallelism);
        if let Some(profile) = profile {
            request = request.with_selection(if cases.is_empty() {
                CorpusSelector::Profile {
                    profile,
                    include_stress: stress,
                }
            } else {
                CorpusSelector::CaseIds {
                    profile,
                    include_stress: stress,
                    case_ids: cases,
                }
            });
        }
        product
            .capabilities_with_corpus(request)
            .map_err(sdk_error)?
    } else {
        product.capabilities().map_err(sdk_error)?
    };
    let bytes = serde_json::to_string_pretty(&SuccessEnvelope::new("capabilities", result))
        .map_err(|_| fail("internal.serialization.failed", "serialize capabilities"))?;
    writeln!(std::io::stdout().lock(), "{bytes}")
        .map_err(|_| fail("io.write.failed", "write capabilities"))
}

pub(super) fn recognizes(arguments: &[String]) -> bool {
    let offset = if arguments.first().map(String::as_str) == Some("--resource-root") {
        2
    } else {
        0
    };
    if arguments.get(offset).map(String::as_str) != Some("generate") {
        return false;
    }
    let values = &arguments[offset + 1..];
    // Do not interpret an option-looking value (e.g. --out --corpus) as a flag.
    let mut index = 0;
    let mut external = false;
    while index < values.len() {
        match values[index].as_str() {
            "--corpus" => {
                external = true;
                break;
            }
            "--out" | "--profile" | "--seed" | "--case-id" | "--format" | "--asset-root"
            | "--parallelism" | "--cli-api" => index += 2,
            _ => index += 1,
        }
    }
    external
}

pub(super) fn try_run(arguments: &[String]) -> Option<Result<(), CliFailure>> {
    if !recognizes(arguments) {
        return None;
    }
    let offset = if arguments.first().map(String::as_str) == Some("--resource-root") {
        2
    } else {
        0
    };
    Some(run(
        &arguments[offset + 1..],
        if offset == 2 {
            arguments.get(1).map(String::as_str)
        } else {
            None
        },
    ))
}

fn run(arguments: &[String], resource_root: Option<&str>) -> Result<(), CliFailure> {
    let mut corpus = None;
    let mut assets = None;
    let mut output = None;
    let mut profile = None;
    let mut seed = 1u64;
    let mut parallelism = 1u32;
    let mut stress = false;
    let mut dry = false;
    let mut format = None;
    let mut cli_api = false;
    let mut cases = Vec::new();
    let mut seen = BTreeSet::new();
    let mut help = false;
    let mut args = arguments.iter();
    while let Some(option) = args.next() {
        if option != "--case-id" && !seen.insert(option.as_str()) {
            return Err(failure(
                "command.syntax.invalid",
                format!("duplicate option {option}"),
            ));
        }
        let mut value = || {
            args.next().filter(|v| !v.starts_with("--")).ok_or_else(|| {
                failure(
                    "command.argument.missing",
                    format!("{option} requires a value"),
                )
            })
        };
        match option.as_str() {
            "--corpus" => corpus = Some(value()?.clone()),
            "--asset-root" => assets = Some(value()?.clone()),
            "--out" => output = Some(value()?.clone()),
            "--profile" => profile = Some(value()?.clone()),
            "--case-id" => cases.push(value()?.clone()),
            "--seed" => {
                seed = value()?.parse().map_err(|_| {
                    failure("command.syntax.invalid", "seed must be an unsigned integer")
                })?
            }
            "--parallelism" => {
                parallelism = value()?.parse().map_err(|_| {
                    failure(
                        "command.syntax.invalid",
                        "parallelism must be an unsigned integer",
                    )
                })?
            }
            "--format" => format = Some(value()?.clone()),
            "--cli-api" => {
                if value()? != "1.0.0" {
                    return Err(failure(
                        "request.version.unsupported",
                        "unsupported CLI API version",
                    ));
                }
                cli_api = true;
            }
            "--include-stress" => stress = true,
            "--dry-run" => dry = true,
            "--help" | "-h" => help = true,
            other => {
                return Err(failure(
                    "command.syntax.invalid",
                    format!("unknown generate option {other}"),
                ));
            }
        }
    }
    if format.as_deref().is_some_and(|f| f != "json") {
        return Err(failure(
            "command.syntax.invalid",
            "external corpus generation supports only --format json",
        ));
    }
    if cli_api && format.is_none() {
        format = Some("json".into());
    }
    if parallelism == 0 {
        return Err(failure(
            "request.schema.invalid",
            "parallelism must be positive",
        ));
    }
    if help {
        super::print_generate_usage();
        return Ok(());
    }
    let required = |value: Option<String>, flag: &str| {
        value.filter(|v| !v.is_empty()).ok_or_else(|| {
            failure(
                "command.argument.missing",
                format!("generate requires {flag}"),
            )
        })
    };
    let corpus = required(corpus, "--corpus")?;
    let assets = required(assets, "--asset-root")?;
    let output = required(output, "--out")?;
    let profile = required(profile, "--profile")?;
    let selector = if cases.is_empty() {
        CorpusSelector::Profile {
            profile: profile.clone(),
            include_stress: stress,
        }
    } else {
        CorpusSelector::CaseIds {
            profile: profile.clone(),
            include_stress: stress,
            case_ids: cases,
        }
    };
    // All syntax, formats and caller numeric options are checked before resources/capture.
    let product = match resource_root {
        Some(root) => DicomTestSuite::explicit_resource_root(root),
        None => DicomTestSuite::embedded(),
    }
    .map_err(sdk)?;
    let product_version = product.version().map_err(sdk)?.product.version.to_owned();
    let outcome = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(corpus, assets, &output, selector.clone())
                .with_seed(seed)
                .with_parallelism(parallelism)
                .dry_run(dry),
        )
        .map_err(sdk)?;
    let result = project(
        outcome,
        &output,
        profile,
        stress,
        selector,
        seed,
        product_version,
    )?;
    let text = if format.as_deref() == Some("json") {
        serde_json::to_string(&SuccessEnvelope::new("generate", result))
            .map_err(|e| failure("internal.serialization.failed", e.to_string()))?
    } else {
        format!(
            "outcome\t{}\nprofile\t{}\nseed\t{}\nselected_cases\t{}\nfiles_written\t{}\npublication\t{}\nvalidation\t{}\nmanifest\t{}",
            result.outcome,
            result.profile,
            result.seed,
            result.selected_case_count,
            result.emitted_file_count,
            result.publication_status,
            result.validation_status,
            result.manifest_path.as_deref().unwrap_or("not_run")
        )
    };
    writeln!(std::io::stdout().lock(), "{text}")
        .map_err(|e| failure("io.write.failed", e.to_string()))
}

fn project(
    outcome: GenerateCorpusOutcome,
    output: &str,
    profile: String,
    stress: bool,
    selector: CorpusSelector,
    seed: u64,
    product_version: String,
) -> Result<ExternalGenerationResult, CliFailure> {
    let no_executable = matches!(&outcome, GenerateCorpusOutcome::NoExecutableCases(_));
    let mut result = ExternalGenerationResult {
        generation_result_schema_version: EXTERNAL_GENERATION_RESULT_SCHEMA_VERSION,
        outcome: "planned",
        requested_output_root: output.into(),
        manifest_path: None,
        run_kind: "external_corpus",
        seed,
        profile,
        include_stress: stress,
        selector: match selector {
            CorpusSelector::Profile { .. } => json!({"kind":"profile"}),
            CorpusSelector::CaseIds { mut case_ids, .. } => {
                case_ids.sort();
                json!({"kind":"case_ids","case_ids":case_ids})
            }
        },
        request_schema_version: "1.0.0",
        manifest_schema_version: "2.0.0",
        product_version,
        identity_projection: Value::Null,
        selection_ledger: Vec::new(),
        corpus_plan_sha256: String::new(),
        emitted_file_count: 0,
        output_bytes: 0,
        selected_case_count: 0,
        direct_case_count: 0,
        dependency_case_count: 0,
        published: false,
        publication_status: "not_run",
        validation_status: "not_run",
        preview_artifact_ids: None,
    };
    match outcome {
        GenerateCorpusOutcome::Published(run) => {
            let manifest: Value = run.manifest().deserialize().map_err(sdk)?;
            result.outcome = "published";
            result.published = true;
            result.publication_status = "published";
            result.validation_status = "passed";
            result.manifest_path = Some(run.manifest().path().display().to_string());
            result.corpus_plan_sha256 = run.corpus_plan_sha256().into();
            result.emitted_file_count = run.emitted_file_count();
            result.output_bytes = run.output_bytes();
            result.identity_projection = manifest["identity_projection"].clone();
            result.selector = manifest["run"]["selector"].clone();
            result.selection_ledger = manifest["selection_ledger"]
                .as_array()
                .expect("schema-bound ledger")
                .clone();
        }
        GenerateCorpusOutcome::Planned(preview)
        | GenerateCorpusOutcome::NoExecutableCases(preview) => {
            result.outcome = if no_executable {
                "no_executable_cases"
            } else {
                "planned"
            };
            result.identity_projection = preview.identity_projection().clone();
            result.selection_ledger = preview
                .cases()
                .iter()
                .map(|case| case.evidence().clone())
                .collect();
            result.corpus_plan_sha256 = preview.corpus_plan_sha256().into();
            result.preview_artifact_ids = Some(preview.artifact_ids().to_vec());
        }
        _ => {
            return Err(failure(
                "internal.invariant.failed",
                "unsupported SDK corpus outcome",
            ));
        }
    }
    result.selected_case_count = result.selection_ledger.len();
    result.direct_case_count = result
        .selection_ledger
        .iter()
        .filter(|row| row["selection"] == "direct")
        .count();
    result.dependency_case_count = result.selected_case_count - result.direct_case_count;
    Ok(result)
}
