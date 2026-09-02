use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

fn workflow(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("cannot read {path}: {error}"))
}

const PROVIDER_IGNORED_TESTS: [(&str, &str); 7] = [
    (
        "--lib",
        "generation_backends::process::tests::fake_backend_timeout_is_enforced",
    ),
    (
        "--lib",
        "generation_backends::process::tests::fake_backend_cancellation_interrupts_fingerprinting_promptly",
    ),
    (
        "--lib",
        "generation_backends::process::tests::fake_backend_cancellation_kills_and_reaps_a_spawned_process_tree_promptly",
    ),
    (
        "--lib",
        "generation_backends::process::tests::fake_backend_inherited_pipe_timeout_is_enforced",
    ),
    (
        "--test composition__subsystem",
        "composition_curated_migration::migrated_curated_recipes_record_shared_plan_materialization",
    ),
    (
        "--test composition__subsystem",
        "composition_quantitative::quantitative_default_bundles_are_closed_provenanced_and_reproducible",
    ),
    (
        "--test composition__subsystem",
        "composition_quantitative::caller_segmentation_and_parametric_values_round_trip_at_fixed_shape",
    ),
];

const EXPLICIT_HEAVY_TESTS: [(&str, &str, &str, &str); 6] = [
    (
        "byte-parity",
        "tests/case_recipe_catalog.rs",
        "corpus_generation__nightly",
        "case_recipe_catalog::data_first_sc_and_metadata_values_and_hashes_match_current_generator_bytes",
    ),
    (
        "all-profile",
        "tests/generate_cli.rs",
        "cli_sdk__nonfast",
        "generate_cli::generate_command_writes_all_profile_union_and_skips_planned_cases",
    ),
    (
        "wsi",
        "tests/wsi_direct_plan.rs",
        "engine__nightly",
        "wsi_direct_plan::ordinary_wsi_direct_plans_match_fresh_seed_one_bytes_and_manifest_facts",
    ),
    (
        "wsi",
        "tests/wsi_pyramid.rs",
        "corpus_generation__nightly",
        "wsi_pyramid::stress_profile_emits_complete_three_instance_wsi_pyramid",
    ),
    (
        "stress",
        "tests/curated_stress_manifest.rs",
        "corpus_generation__nightly",
        "curated_stress_manifest::typed_stress_projection_matches_frozen_file_values_and_resources",
    ),
    (
        "stress",
        "tests/curated_stress_sc_integration.rs",
        "corpus_generation__nightly",
        "curated_stress_sc_integration::all_stress_sc_cases_execute_through_private_streaming_services",
    ),
];

#[test]
fn fast_pr_cancels_superseded_runs_without_duplicate_pr_branch_ownership() {
    let fast = workflow(".github/workflows/ci.yml");
    let expected_header = r#"name: Fast PR

on:
  push:
    branches:
      - main
  pull_request:
  workflow_dispatch:

concurrency:
  group: ci-${{ github.event_name == 'pull_request' && format('pr-{0}', github.event.pull_request.number) || format('{0}-{1}', github.event_name, github.ref) }}
  cancel-in-progress: true

"#;

    assert!(
        fast.starts_with(expected_header),
        "Fast PR must retain R1.1 main-push, PR, manual, and superseded-run ownership"
    );
    assert_eq!(fast.matches("\n  push:").count(), 1);
    assert_eq!(fast.matches("\n  pull_request:").count(), 1);
    assert_eq!(fast.matches("\n  workflow_dispatch:").count(), 1);
}

#[test]
fn fast_pr_is_bounded_to_light_contracts_and_tiny_smoke() {
    let fast = workflow(".github/workflows/ci.yml");

    for required in [
        "timeout-minutes: 15",
        "cargo fmt --all -- --check",
        "jq empty cases/registry.json schemas/*.json",
        "RUSTFLAGS: -D warnings",
        "cargo check --locked --no-default-features --lib --bins",
        "--test schema_resources__fast --test release_ci__fast",
        "fetch-depth: 0",
        "python3 -m unittest tests/test_change_test_routing.py",
        "Run routed ordinary subsystem bundles",
        "PR_BASE_SHA: ${{ github.event.pull_request.base.sha }}",
        "PUSH_BEFORE_SHA: ${{ github.event.before }}",
        "workflow_dispatch) base=0000000000000000000000000000000000000000",
        "python3 scripts/route-changed-tests.py --diff \"$base\" \"$HEAD_SHA\"",
        "generate --profile smoke",
        "validate \"$root\"",
        "scripts/report-ci-cost.sh fast-pr",
        "output_artifact_count=",
    ] {
        assert!(fast.contains(required), "Fast PR omitted {required}");
    }

    for forbidden in [
        "setup-uv",
        "uv python",
        "highdicom",
        "--all-targets",
        "--all-features",
        "--profile core",
        "--profile extended",
        "--profile all",
        "--include-stress",
        "cargo package",
        "build-release-archive",
        "verify-release-archive",
        "upload-artifact",
        "--release",
        "in-process-codecs",
        "external-codec",
        "run-heavy-qualification",
        "__nightly",
        "cli_sdk__nonfast",
        "--ignored",
    ] {
        assert!(
            !fast.contains(forbidden),
            "heavy boundary leaked into Fast PR: {forbidden}"
        );
    }
    for (_, _, _, entry) in EXPLICIT_HEAVY_TESTS {
        assert!(
            !fast.contains(entry),
            "explicit heavy entry leaked into Fast PR: {entry}"
        );
    }
    assert_eq!(
        fast.matches("scripts/route-changed-tests.py --diff")
            .count(),
        1
    );
    assert!(
        fast.find("--test schema_resources__fast --test release_ci__fast")
            .unwrap()
            < fast.find("scripts/route-changed-tests.py --diff").unwrap(),
        "unconditional Fast ownership must run before routed ordinary bundles"
    );
}

#[test]
fn heavy_workflow_retains_nightly_matrix_and_immutable_release_gate() {
    let heavy = workflow(".github/workflows/qualification.yml");
    let dispatcher = workflow("scripts/run-heavy-qualification.sh");
    let provider = heavy
        .split("\n  native-provider-contract:\n")
        .nth(1)
        .unwrap()
        .split("\n  default:\n")
        .next()
        .unwrap();
    let default = heavy
        .split("\n  default:\n")
        .nth(1)
        .unwrap()
        .split("\n  standalone-release:\n")
        .next()
        .unwrap();
    let release = heavy
        .split("\n  standalone-release:\n")
        .nth(1)
        .unwrap()
        .split("\n  in-process-codecs:\n")
        .next()
        .unwrap();
    let codecs = heavy
        .split("\n  in-process-codecs:\n")
        .nth(1)
        .unwrap()
        .split("\n  external-codec-compile:\n")
        .next()
        .unwrap();
    let external = heavy.split("\n  external-codec-compile:\n").nth(1).unwrap();

    for required in [
        "schedule:",
        "workflow_dispatch:",
        "qualification_class:",
        "release-candidate",
        "candidate_revision:",
        "^[0-9a-f]{40}$",
        "immutable 40-hex commit",
        "cancel-in-progress: true",
        "Prove selected revision",
    ] {
        assert!(
            heavy.contains(required),
            "heavy selection omitted {required}"
        );
    }
    assert!(!heavy.contains("\n  push:"));
    assert!(!heavy.contains("\n  pull_request:"));
    assert_eq!(
        heavy.matches("--compile-bytecode").count(),
        3,
        "every retained heavy backend environment must precompile the locked runtime"
    );

    assert!(provider.contains("Native provider contract"));
    assert!(provider.contains("RUST_TEST_THREADS: \"1\""));
    assert_eq!(
        provider.matches("-- --ignored --exact").count(),
        PROVIDER_IGNORED_TESTS.len()
    );
    let normalized_provider = provider.split_whitespace().collect::<Vec<_>>().join(" ");
    for (target, test) in PROVIDER_IGNORED_TESTS {
        let separator = if target == "--lib" { " \\" } else { "" };
        let command = format!(
            "cargo test --locked --no-default-features {target} \\ {test}{separator} -- --ignored --exact"
        );
        assert!(
            normalized_provider.contains(&command),
            "serial provider job omitted exact ignored test {test}"
        );
    }

    assert!(default.contains("timeout-minutes: 120"));
    assert!(!default.contains("RUST_TEST_THREADS"));
    assert!(default.contains("cargo test --locked --all-targets --no-default-features"));
    assert_eq!(
        heavy
            .matches("scripts/run-heavy-qualification.sh all")
            .count(),
        1,
        "Nightly/default must dispatch all explicit heavy entries exactly once"
    );
    assert!(default.contains("scripts/run-heavy-qualification.sh all"));
    assert!(!release.contains("scripts/run-heavy-qualification.sh"));
    assert!(
        default
            .find("cargo test --locked --all-targets --no-default-features")
            .unwrap()
            < default
                .find("scripts/run-heavy-qualification.sh all")
                .unwrap(),
        "ordinary broad evidence must precede the explicit heavy dispatcher"
    );
    assert!(default.contains("--profile core"));
    assert!(default.contains("--profile extended"));
    assert!(default.contains("Verify smoke reproducibility"));

    let normalized_dispatcher = dispatcher.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        dispatcher.matches("run_exact ").count(),
        EXPLICIT_HEAVY_TESTS.len()
    );
    for fail_closed_marker in [
        "--ignored --exact --list",
        "grep -Fxc -- \"$entry: test\"",
        "heavy entry selection must resolve exactly once",
        "exit 3",
    ] {
        assert!(
            dispatcher.contains(fail_closed_marker),
            "heavy dispatcher lacks exact-selection preflight {fail_closed_marker}"
        );
    }
    let expected_command = |harness: &str, entry: &str| {
        format!(
            "cargo test --locked --no-default-features --test {harness} {entry} -- --ignored --exact"
        )
    };
    let mut expected_by_class = std::collections::BTreeMap::<&str, BTreeSet<String>>::new();
    for (class, source_path, harness, entry) in EXPLICIT_HEAVY_TESTS {
        let source = workflow(source_path);
        let function = format!("fn {}()", entry.rsplit("::").next().unwrap());
        let offset = source
            .find(&function)
            .unwrap_or_else(|| panic!("heavy inventory names undiscoverable test {entry}"));
        let prefix = &source[offset.saturating_sub(220)..offset];
        assert!(
            prefix.contains(
                "#[ignore = \"R2.3 explicit heavy qualification; run through scripts/run-heavy-qualification.sh\"]"
            ),
            "heavy entry {entry} is not excluded from ordinary broad execution"
        );
        assert!(
            normalized_dispatcher.contains(&format!("run_exact {harness} \\ {entry}")),
            "heavy dispatcher omitted exact harness/module entry {entry}"
        );
        assert_eq!(
            dispatcher.matches(entry).count(),
            1,
            "heavy entry {entry} must have one primary dispatcher assignment"
        );
        expected_by_class
            .entry(class)
            .or_default()
            .insert(expected_command(harness, entry));
    }
    let mut observed_by_class = std::collections::BTreeMap::<&str, BTreeSet<String>>::new();
    for class in ["byte-parity", "all-profile", "wsi", "stress"] {
        assert!(
            dispatcher.contains(&format!("{class})")),
            "heavy dispatcher omitted class {class}"
        );
        let output = Command::new("scripts/run-heavy-qualification.sh")
            .args(["--dry-run", class])
            .output()
            .unwrap_or_else(|error| panic!("cannot dry-run heavy class {class}: {error}"));
        assert!(output.status.success(), "heavy dry-run failed for {class}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let command_lines = stdout.lines().map(str::to_owned).collect::<Vec<_>>();
        let commands = command_lines.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(command_lines.len(), commands.len());
        assert_eq!(commands, expected_by_class[class]);
        observed_by_class.insert(class, commands);
    }
    let primary_union = observed_by_class
        .values()
        .flat_map(|commands| commands.iter().cloned())
        .collect::<BTreeSet<_>>();
    assert_eq!(primary_union.len(), EXPLICIT_HEAVY_TESTS.len());
    assert_eq!(observed_by_class["byte-parity"].len(), 1);
    assert_eq!(observed_by_class["all-profile"].len(), 1);
    assert_eq!(observed_by_class["wsi"].len(), 2);
    assert_eq!(observed_by_class["stress"].len(), 2);
    let all_output = Command::new("scripts/run-heavy-qualification.sh")
        .args(["--dry-run", "all"])
        .output()
        .expect("cannot dry-run all heavy entries");
    assert!(all_output.status.success());
    let all_stdout = String::from_utf8(all_output.stdout).unwrap();
    let all_lines = all_stdout.lines().map(str::to_owned).collect::<Vec<_>>();
    let all_commands = all_lines.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(all_lines.len(), EXPLICIT_HEAVY_TESTS.len());
    assert_eq!(all_lines.len(), all_commands.len());
    assert_eq!(all_commands, primary_union);
    for scope_note in [
        "ordinary, stress, and legacy scope",
        "explicit opt-in stress coverage",
        "Ordinary WSI byte parity plus the reduced stress pyramid",
    ] {
        assert!(
            dispatcher.contains(scope_note),
            "heavy dispatcher omitted secondary-scope note {scope_note}"
        );
    }

    assert!(codecs.contains("feature: [jpeg, charls, jpegxl, jpeg2000, deflate]"));
    assert!(codecs.contains("Compile feature-sensitive product surfaces"));
    assert!(!codecs.contains("--all-targets"));
    assert!(!codecs.contains("Prepare locked generation backend"));
    assert!(!codecs.contains("DTS_HIGHDICOM_PYTHON="));
    assert!(codecs.contains("Test feature-sensitive surfaces"));
    assert!(codecs.contains("--lib codecs::tests::"));
    for selector in [
        "--test codec__subsystem codec_backend_registry::",
        "--test codec__subsystem frame_codec_service::",
        "--test corpus_generation__subsystem curated_exceptional_execution::",
        "--test cli_sdk__nonfast curated_runtime_capabilities::",
        "--test engine__subsystem exceptional_sc_plan::",
        "--test schema_resources__subsystem project_artifacts::",
        "--test cli_sdk__nonfast runtime_capabilities::",
        "--test cli_sdk__nonfast \"validate_cli::$regression\" -- --exact",
    ] {
        assert!(
            codecs.contains(selector),
            "in-process codec qualification omitted bounded selector {selector}"
        );
    }
    assert!(!codecs.contains("composition_curated_migration"));
    assert!(
        codecs
            .contains("validate_command_reports_deflated_image_frame_decoded_frame_hash_mismatch")
    );
    assert!(!codecs.contains("--test report_cli"));
    assert!(!codecs.contains("--test validate_cli --test"));
    assert!(codecs.contains("Exercise feature corpus"));
    assert!(codecs.contains("--case-id"));
    assert!(codecs.contains("selected_cases=${#cases[@]}"));
    assert!(codecs.contains("[.files[].case_id] + [.skipped_cases[].case_id]"));
    for case_id in [
        "classic/sc/rgb_planar0_jpeg_baseline_8bit",
        "classic/sc/mono2_u8_jpeg_ls_lossless",
        "classic/sc/rgb_planar0_jpegxl_lossless",
        "classic/sc/rgb_jpegxl_lossy",
        "classic/sc/mono2_u16_jpeg2000_lossless",
        "classic/sc/mono2_u8_deflated_explicit_le",
        "derived/seg/binary_multiframe_deflated_image_frame",
    ] {
        assert!(codecs.contains(case_id), "codec matrix omitted {case_id}");
    }
    for forbidden in ["highdicom", "wsi/", "quantitative/"] {
        assert!(
            !codecs.contains(forbidden),
            "feature-independent work leaked into codec matrix: {forbidden}"
        );
    }
    assert!(codecs.contains("fragments_per_frame == [1,1]"));

    assert!(external.contains("feature: [htj2k_openjph, legacy_jpeg_dcmtk]"));
    assert!(!external.contains("--all-targets"));
    assert!(!external.contains("setup-uv"));
    assert!(external.contains("Exercise selected external codec cases"));
    assert!(external.contains("--case-id"));
    assert!(external.contains("[.files[].case_id] + [.skipped_cases[].case_id]"));
    for selector in [
        "--test codec__subsystem codec_backend_registry::",
        "--test codec__subsystem frame_codec_service::",
        "--test cli_sdk__nonfast curated_runtime_capabilities::",
        "--test engine__subsystem exceptional_sc_plan::",
        "--test cli_sdk__nonfast runtime_capabilities::",
    ] {
        assert!(
            external.contains(selector),
            "external codec qualification omitted bounded selector {selector}"
        );
    }
    for case_id in [
        "classic/sc/mono2_u16_htj2k_lossless",
        "classic/sc/mono2_u16_htj2k_lossy",
        "classic/sc/mono2_u16_jpeg_lossless_process_14",
        "classic/sc/mono2_u16_jpeg_lossless_sv1",
    ] {
        assert!(
            external.contains(case_id),
            "external codec matrix omitted {case_id}"
        );
    }
    for forbidden in ["highdicom", "wsi/", "quantitative/"] {
        assert!(
            !external.contains(forbidden),
            "feature-independent work leaked into external codec matrix: {forbidden}"
        );
    }

    assert!(release.contains("if: needs.selection.outputs.class == 'release-candidate'"));
    assert!(release.contains("needs: [selection, default]"));
    for (_, _, _, entry) in EXPLICIT_HEAVY_TESTS {
        assert!(
            !release.contains(entry),
            "release packaging job must inherit, not repeat, heavy entry {entry}"
        );
    }
    for required in [
        "cargo fmt --all -- --check",
        "git diff --check",
        "RUSTFLAGS='-D warnings' cargo check",
        "product_resource_lookup_audit",
        "non_rust_cli_consumer",
        "caller_content_consumer",
        "qualified_catalog_consumer",
        "structural_catalog_consumer",
        "upgrade_consumer",
        "release_process",
        "cargo package --locked",
        "sdk_external_consumer",
        "scripts/build-release-archive.sh",
        "scripts/verify-release-archive.sh",
        "release_archive",
        "actions/upload-artifact@v7",
        "needs.selection.outputs.revision",
    ] {
        assert!(
            release.contains(required),
            "release gate omitted {required}"
        );
    }
    assert!(release.contains("path: ${{ runner.temp }}/dist/*"));
    for selector in [
        "--test schema_resources__subsystem product_resource_lookup_audit::",
        "--test schema_resources__fast",
        "--test release_ci__fast",
        "--test release_ci__nonfast \"$module::\"",
        "--test release_ci__nonfast sdk_external_consumer::",
        "--test release_ci__nonfast release_archive::",
    ] {
        assert!(
            release.contains(selector),
            "release qualification omitted bounded selector {selector}"
        );
    }
    assert_eq!(heavy.matches("cargo package --locked").count(), 1);
    assert_eq!(heavy.matches("actions/upload-artifact@v7").count(), 1);
    assert_eq!(
        release.matches("cargo build --release --locked").count(),
        1,
        "the RC job must compile exactly one optimized candidate binary"
    );
    assert_eq!(
        release.matches("scripts/build-release-archive.sh").count(),
        1,
        "the RC job must construct exactly one candidate archive"
    );
    assert_eq!(
        release.matches("tar -xzf \"$ARCHIVE\"").count(),
        1,
        "installed consumers and the harness must reuse one extraction"
    );
    for binding in [
        "DTS_RELEASE_BINARY=$BINARY",
        "DTS_RELEASE_BINARY_SHA256=$BINARY_SHA256",
        "DTS_RELEASE_ARCHIVE=$ARCHIVE",
        "DTS_RELEASE_ARCHIVE_SHA256=$ARCHIVE_SHA256",
        "DTS_RELEASE_TARGET=$TARGET",
        "DTS_RELEASE_REVISION=$DTS_RELEASE_REVISION",
        "DTS_RELEASE_EXTRACTED_ROOT=$ROOT",
        "test \"$(sha256sum \"$INSTALLED_BINARY\"",
    ] {
        assert!(
            release.contains(binding),
            "RC reuse dataflow omitted {binding}"
        );
    }
    let harness_offset = release
        .find(
            "cargo test --locked --no-default-features --test release_ci__nonfast release_archive::",
        )
        .unwrap();
    let upload_offset = release.find("actions/upload-artifact@v7").unwrap();
    assert!(
        upload_offset > harness_offset,
        "candidate artifacts may be uploaded only after archive qualification passes"
    );
    assert!(!release.contains("archive: false"));

    let archive_harness = fs::read_to_string("tests/release_archive.rs").unwrap();
    assert!(archive_harness.contains("supplied_candidate"));
    assert!(archive_harness.contains("DTS_RELEASE_ARCHIVE_SHA256"));
    assert!(archive_harness.contains("DTS_RELEASE_BINARY_SHA256"));
    assert!(archive_harness.contains("DTS_RELEASE_EXTRACTED_ROOT"));
    assert!(archive_harness.contains("supplied archive does not match its immutable identity"));
    assert!(archive_harness.contains("installed archive binary differs"));
    assert!(archive_harness.contains("bad-checksum.tar.gz"));
    assert!(archive_harness.contains("tampered.tar.gz"));

    let regression_sources = [
        (
            "tests/product_resource_lookup_audit.rs",
            "findings.is_empty",
        ),
        ("tests/schema_artifacts.rs", "reject"),
        ("tests/black_box_cli_consumer.py", "expected=0"),
        ("tests/release_archive.rs", "parallelism"),
        ("tests/release_archive.rs", "checksum"),
        ("tests/upgrade_consumer.py", "9.0.0"),
    ];
    for (path, marker) in regression_sources {
        assert!(
            fs::read_to_string(path).unwrap().contains(marker),
            "{path} lacks deliberate regression marker {marker}"
        );
    }
}

#[test]
fn ignored_provider_inventory_is_owned_and_matches_serial_workflow() {
    let process = workflow("src/generation_backends/process.rs");
    let curated = workflow("tests/composition_curated_migration.rs");
    let quantitative = workflow("tests/composition_quantitative.rs");
    let sources = [&process, &curated, &quantitative];
    let provider_reason = "#[ignore = \"R1.4 native-provider-contract:";
    let fixture_reason =
        "#[ignore = \"subprocess fixture invoked by provider tests; not qualification evidence\"]";

    assert_eq!(
        sources
            .iter()
            .map(|source| source.matches(provider_reason).count())
            .sum::<usize>(),
        PROVIDER_IGNORED_TESTS.len(),
        "provider discovery count must stay synchronized with the serial workflow inventory"
    );
    assert_eq!(process.matches(fixture_reason).count(), 2);
    assert_eq!(
        sources
            .iter()
            .map(|source| source.matches("#[ignore").count())
            .sum::<usize>(),
        PROVIDER_IGNORED_TESTS.len() + 2,
        "every ignored entry in provider-owned sources needs provider or subprocess-fixture ownership"
    );
    assert!(!sources.iter().any(|source| source.contains("#[ignore]")));

    for (_, test) in PROVIDER_IGNORED_TESTS {
        let source = if test.starts_with("generation_backends::") {
            &process
        } else if test.contains("migrated_curated") {
            &curated
        } else {
            &quantitative
        };
        let function = format!("fn {test}()", test = test.rsplit("::").next().unwrap());
        let offset = source
            .find(&function)
            .unwrap_or_else(|| panic!("provider inventory names undiscoverable test {test}"));
        let prefix = &source[offset.saturating_sub(220)..offset];
        assert!(
            prefix.contains(provider_reason),
            "ignored provider test {test} lacks explicit R1.4 ownership"
        );
    }
}

#[test]
fn ci_build_storage_controls_cover_every_job_and_preserve_heavy_evidence() {
    let fast = workflow(".github/workflows/ci.yml");
    let heavy = workflow(".github/workflows/qualification.yml");
    let cargo = workflow("Cargo.toml");

    for profile in ["[profile.dev]", "[profile.test]"] {
        let body = cargo.split(profile).nth(1).unwrap();
        assert!(body.lines().take(3).any(|line| line == "debug = 0"));
        assert!(
            body.lines()
                .take(3)
                .any(|line| line == "incremental = false")
        );
    }

    for source in [&fast, &heavy] {
        let mut in_jobs = false;
        let mut in_steps = false;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                continue;
            }
            let indentation = line.len() - trimmed.len();
            if indentation == 0 {
                in_jobs = trimmed == "jobs:";
                in_steps = false;
            } else if in_jobs && indentation == 2 && trimmed.ends_with(':') {
                in_steps = false;
            } else if in_jobs && indentation == 4 && trimmed == "steps:" {
                in_steps = true;
            }
            if line.contains("${{ runner.") {
                assert!(
                    in_jobs && in_steps,
                    "runner context is unavailable outside workflow steps: {line}"
                );
            }
        }
    }

    let jobs = [
        (&fast, "  fast-pr:", None, "cargo-target-fast-pr"),
        (
            &heavy,
            "  selection:",
            Some("  native-provider-contract:"),
            "cargo-target-selection",
        ),
        (
            &heavy,
            "  native-provider-contract:",
            Some("  default:"),
            "cargo-target-native-provider",
        ),
        (
            &heavy,
            "  default:",
            Some("  standalone-release:"),
            "cargo-target-nightly-default",
        ),
        (
            &heavy,
            "  standalone-release:",
            Some("  in-process-codecs:"),
            "cargo-target-release-candidate",
        ),
        (
            &heavy,
            "  in-process-codecs:",
            Some("  external-codec-compile:"),
            "cargo-target-codec-${{ matrix.feature }}",
        ),
        (
            &heavy,
            "  external-codec-compile:",
            None,
            "cargo-target-external-codec-${{ matrix.feature }}",
        ),
    ];
    for (source, start, end, target_suffix) in jobs {
        let start_marker = format!("\n{start}");
        let mut job = source.split_once(&start_marker).unwrap().1;
        if let Some(end) = end {
            let end_marker = format!("\n{end}");
            job = job.split_once(&end_marker).unwrap().0;
        }
        for required in [
            "CARGO_INCREMENTAL: \"0\"",
            "CARGO_PROFILE_DEV_DEBUG: \"0\"",
            "CARGO_PROFILE_TEST_DEBUG: \"0\"",
            "CI_DISK_BUDGET_BYTES:",
            "Initialize isolated build root and cost clock",
            "if: always()",
            "scripts/report-ci-cost.sh",
        ] {
            assert!(job.contains(required), "{start} omitted {required}");
        }
        let export =
            format!("echo \"CARGO_TARGET_DIR=$RUNNER_TEMP/{target_suffix}\" >> \"$GITHUB_ENV\"");
        assert_eq!(
            job.matches(&export).count(),
            1,
            "{start} must export its unique target exactly once"
        );
        if let Some(cargo_work) = job.find("cargo ") {
            assert!(
                job.find(&export).unwrap() < cargo_work,
                "{start} exports its target after Cargo work begins"
            );
        }
        assert!(
            !job.contains("CARGO_TARGET_DIR: ${{ runner."),
            "{start} uses runner context while GitHub evaluates job env"
        );
    }

    assert_eq!(fast.matches("scripts/report-ci-cost.sh").count(), 1);
    assert_eq!(heavy.matches("scripts/report-ci-cost.sh").count(), 6);
    assert_eq!(fast.matches("if: always()").count(), 1);
    assert_eq!(heavy.matches("if: always()").count(), 6);
    for budget in ["4294967296", "6442450944", "12884901888", "17179869184"] {
        assert!(
            format!("{fast}\n{heavy}").contains(budget),
            "workflow budgets omitted {budget}"
        );
    }
    assert!(fast.contains("CI_COST_ENFORCE: ${{ job.status == 'success'"));
    assert!(heavy.contains("CI_COST_ENFORCE: ${{ job.status == 'success'"));
    assert!(heavy.contains("CI_COST_ENFORCE: \"0\""));
    assert!(!fast.contains("du -sk target"));
    assert!(!heavy.contains("CARGO_TARGET_DIR: target"));
}

#[test]
fn ci_cost_reporter_handles_missing_and_spaced_paths_and_fails_closed() {
    let fixture = std::env::temp_dir().join(format!("dts-ci-cost fixture-{}", std::process::id()));
    let target = fixture.join("target tree");
    let output = fixture.join("output tree");
    fs::create_dir_all(&target).unwrap();
    fs::create_dir_all(&output).unwrap();
    fs::write(target.join("artifact"), vec![0_u8; 2048]).unwrap();
    fs::write(output.join("one"), b"one").unwrap();
    fs::write(output.join("two"), b"two").unwrap();

    let run = |budget: &str, enforce: &str| {
        Command::new("sh")
            .arg("scripts/report-ci-cost.sh")
            .arg("fixture")
            .arg(budget)
            .arg(&target)
            .arg(&output)
            .arg(fixture.join("missing"))
            .env("CARGO_TARGET_DIR", &target)
            .env("CI_COST_ENFORCE", enforce)
            .output()
            .unwrap()
    };

    let report = run("4294967296", "1");
    assert!(report.status.success());
    let stdout = String::from_utf8(report.stdout).unwrap();
    for marker in [
        "ci_cost_elapsed_build_seconds=",
        "ci_cost_target_root=",
        "ci_cost_target_bytes=",
        "ci_cost_output_bytes=",
        "ci_cost_output_artifact_count=2",
        "ci_cost_disk_budget_bytes=4294967296",
    ] {
        assert!(stdout.contains(marker), "report omitted {marker}");
    }

    let over_budget = run("1", "1");
    assert_eq!(over_budget.status.code(), Some(1));
    let overflow = run("9007199254740992", "1");
    assert_eq!(overflow.status.code(), Some(2));
    let mismatch = Command::new("sh")
        .arg("scripts/report-ci-cost.sh")
        .arg("fixture")
        .arg("4294967296")
        .arg(&target)
        .env("CARGO_TARGET_DIR", fixture.join("different target"))
        .output()
        .unwrap();
    assert_eq!(mismatch.status.code(), Some(2));
    fs::remove_dir_all(&fixture).unwrap();
}
