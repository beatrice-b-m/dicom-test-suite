import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import types
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("change_test_routing", ROOT / "scripts/route-changed-tests.py")
ROUTER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(ROUTER)


class ChangeTestRoutingFixtures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.config = ROUTER.load_json(ROOT / "product/change-test-routing.json")
        cls.ownership = ROUTER.load_json(ROOT / "product/test-ownership.json")

    def select(self, *paths, force_all=False):
        return ROUTER.select(list(paths), copy.deepcopy(self.config), self.ownership, force_all=force_all)

    @staticmethod
    def commands(result):
        return [" ".join(command["argv"]) for command in result["commands"]]

    def test_representative_surfaces_select_only_owning_bundles(self):
        fixtures = {
            "src/executor/engine.rs": (
                ["engine"],
                [
                    "cargo test --locked --no-default-features --lib executor::evidence::tests::",
                    "cargo test --locked --no-default-features --lib executor::materialization::tests::",
                    "cargo test --locked --no-default-features --lib executor::native_codec::tests::",
                    "cargo test --locked --no-default-features --lib executor::scheduler::tests::",
                    "cargo test --locked --no-default-features --lib executor::services::tests::",
                    "cargo test --locked --no-default-features --lib executor::transaction::tests::",
                    "cargo test --locked --no-default-features --test engine__subsystem",
                ],
                ["nightly", "release_candidate"],
            ),
            "src/codecs.rs": (
                ["codec"],
                [
                    "cargo test --locked --no-default-features --lib codecs::tests::",
                    "cargo test --locked --no-default-features --test codec__subsystem",
                ],
                ["codec_feature_matrix", "release_candidate"],
            ),
            "src/generation_backends/process.rs": (
                ["provider"],
                [
                    "cargo test --locked --no-default-features --lib generation_backends::discovery::tests::",
                    "cargo test --locked --no-default-features --lib generation_backends::parametric_map::tests::",
                    "cargo test --locked --no-default-features --lib generation_backends::process::tests::",
                    "cargo test --locked --no-default-features --lib generation_backends::scoord3d::tests::",
                    "cargo test --locked --no-default-features --lib generation_backends::staging::tests::",
                    "cargo test --locked --no-default-features --lib generation_backends::tests::",
                    "cargo test --locked --no-default-features --lib generation_backends::tid1500::caller_parameter_tests::",
                    "cargo test --locked --no-default-features --test provider__subsystem",
                ],
                ["native_provider_contract", "release_candidate"],
            ),
            "schemas/manifest.schema.json": (
                ["schema"],
                ["cargo test --locked --no-default-features --test schema_resources__subsystem"],
                ["release_candidate"],
            ),
            "src/sdk.rs": (
                ["sdk"],
                [
                    "cargo test --locked --no-default-features --test cli_sdk__nonfast sdk_facade::",
                    "cargo test --locked --no-default-features --test schema_resources__subsystem cli_contract_schema::",
                ],
                ["release_candidate"],
            ),
            "src/manifest_contract.rs": (
                ["assembly", "report", "schema", "sdk"],
                [
                    "cargo test --locked --no-default-features --lib report_contract::report_contract_tests::",
                    "cargo test --locked --no-default-features --test assembly__subsystem",
                    "cargo test --locked --no-default-features --test cli_sdk__nonfast report_cli::",
                    "cargo test --locked --no-default-features --test cli_sdk__nonfast sdk_facade::",
                    "cargo test --locked --no-default-features --test schema_resources__subsystem",
                ],
                ["release_candidate"],
            ),
            "cases/registry.json": (
                ["corpus"],
                [
                    "cargo test --locked --no-default-features --test corpus_generation__subsystem",
                    "cargo test --locked --no-default-features --test engine__subsystem corpus_plan::",
                ],
                ["explicit_heavy", "future_external_corpus", "release_candidate"],
            ),
        }
        for changed, (bundles, commands, deferred) in fixtures.items():
            with self.subTest(changed=changed):
                result = self.select(changed)
                self.assertEqual(result["bundle_ids"], bundles)
                self.assertEqual(self.commands(result), commands)
                self.assertEqual([item["id"] for item in result["deferred_evidence"]], deferred)

    def test_overlaps_and_multi_path_union_are_deterministic_and_deduplicated(self):
        codec_engine = self.select("src/executor/frame_codec.rs")
        self.assertEqual(codec_engine["bundle_ids"], ["codec", "engine"])
        provider_corpus = self.select("src/recipes/content_provider.rs")
        self.assertEqual(provider_corpus["bundle_ids"], ["corpus", "provider"])

        forward = self.select("src/sdk.rs", "src/codecs.rs", "src/sdk.rs")
        reverse = self.select("src/codecs.rs", "src/sdk.rs")
        self.assertEqual(forward["commands"], reverse["commands"])
        keys = [(item.get("kind", "test"), item.get("target"), item.get("module")) for item in forward["commands"]]
        self.assertEqual(len(keys), len(set(keys)))

        subsumed = self.select("src/planning.rs", "tests/corpus_plan.rs")
        engine_commands = [item for item in subsumed["commands"] if item.get("target") == "engine__subsystem"]
        self.assertEqual(engine_commands, [{
            "argv": ["cargo", "test", "--locked", "--no-default-features", "--test", "engine__subsystem"],
            "target": "engine__subsystem",
        }])

        covered = self.select("tests/ci_release_gates.rs", "tests/schema_artifacts.rs")
        self.assertEqual(covered["commands"], [])
        self.assertEqual(
            covered["covered_by_unconditional_fast"],
            ["release_ci__fast", "schema_resources__fast"],
        )

    def test_v2_discovery_schemas_route_live_identity_and_generic_schema_checks(self):
        expected_commands = [
            "cargo test --locked --no-default-features --lib identity::identity_domain_tests::",
            "cargo test --locked --no-default-features --test cli_sdk__nonfast capabilities_cli::",
            "cargo test --locked --no-default-features --test cli_sdk__nonfast version_cli::",
            "cargo test --locked --no-default-features --test schema_resources__subsystem",
        ]
        for schema in [
            "schemas/capabilities-result-v2.schema.json",
            "schemas/generation-result-v2.schema.json",
            "schemas/manifest-v1.schema.json",
            "schemas/version-result-v2.schema.json",
        ]:
            with self.subTest(schema=schema):
                result = self.select(schema)
                self.assertEqual(result["bundle_ids"], ["identity", "schema"])
                self.assertEqual(
                    result["matched_rules"][schema],
                    ["identity-discovery", "schema"],
                )
                self.assertEqual(self.commands(result), expected_commands)

    def test_composition_identity_schemas_route_live_producers_readers_and_schema_checks(self):
        for schema in [
            "schemas/composition-manifest-v1.schema.json",
            "schemas/composition-result-v2.schema.json",
        ]:
            with self.subTest(schema=schema):
                result = self.select(schema)
                self.assertEqual(
                    result["bundle_ids"],
                    ["composition", "identity", "schema", "sdk"],
                )
                self.assertEqual(
                    result["matched_rules"][schema],
                    ["composition-identity-contract", "schema"],
                )
                commands = self.commands(result)
                self.assertTrue(
                    any(
                        command.startswith(
                            "cargo test --locked --no-default-features --test composition__subsystem "
                        )
                        for command in commands
                    ),
                    commands,
                )
                self.assertIn(
                    "cargo test --locked --no-default-features --test cli_sdk__nonfast sdk_facade::",
                    commands,
                )
                self.assertIn(
                    "cargo test --locked --no-default-features --lib identity::identity_domain_tests::",
                    commands,
                )
                self.assertIn(
                    "cargo test --locked --no-default-features --test schema_resources__subsystem",
                    commands,
                )

    def test_assembly_identity_schemas_route_live_producers_readers_and_schema_checks(self):
        for schema in [
            "schemas/assembly-result-v2.schema.json",
            "schemas/structural-assembly-manifest-v2.schema.json",
        ]:
            with self.subTest(schema=schema):
                result = self.select(schema)
                self.assertEqual(
                    result["bundle_ids"],
                    ["assembly", "identity", "schema", "sdk"],
                )
                self.assertEqual(
                    result["matched_rules"][schema],
                    ["assembly-identity-contract", "schema"],
                )
                commands = self.commands(result)
                self.assertIn(
                    "cargo test --locked --no-default-features --test assembly__subsystem",
                    commands,
                )
                self.assertIn(
                    "cargo test --locked --no-default-features --test cli_sdk__nonfast sdk_facade::",
                    commands,
                )
                self.assertIn(
                    "cargo test --locked --no-default-features --lib identity::identity_domain_tests::",
                    commands,
                )
                self.assertIn(
                    "cargo test --locked --no-default-features --test schema_resources__subsystem",
                    commands,
                )

    def test_assembly_contract_fixtures_route_byte_plan_cli_and_reader_coverage(self):
        for fixture in [
            "tests/fixtures/cli/assembly-request-seed5.json",
            "tests/fixtures/cli/assembly-manifest-v1.json",
            "tests/fixtures/cli/assembly-result-v1.json",
        ]:
            with self.subTest(fixture=fixture):
                result = self.select(fixture)
                self.assertEqual(result["bundle_ids"], ["assembly", "schema", "sdk"])
                self.assertEqual(
                    result["matched_rules"][fixture],
                    ["assembly-contract-fixtures", "schema-fixtures"],
                )
                commands = self.commands(result)
                self.assertIn(
                    "cargo test --locked --no-default-features --test assembly__subsystem",
                    commands,
                )
                self.assertIn(
                    "cargo test --locked --no-default-features --test cli_sdk__nonfast sdk_facade::",
                    commands,
                )
                self.assertIn(
                    "cargo test --locked --no-default-features --test schema_resources__subsystem",
                    commands,
                )

    def test_shared_manifest_contract_routes_assembly_readers_and_semantics(self):
        result = self.select("src/manifest_contract.rs")
        self.assertEqual(result["bundle_ids"], ["assembly", "report", "schema", "sdk"])
        self.assertEqual(
            result["matched_rules"]["src/manifest_contract.rs"],
            ["manifest-contract"],
        )
        self.assertIn(
            "cargo test --locked --no-default-features --test assembly__subsystem",
            self.commands(result),
        )

    def test_structured_report_qualification_is_explicitly_deferred(self):
        result = self.select("tests/composition_structured_reports.rs")
        self.assertEqual(result["bundle_ids"], [])
        self.assertEqual(self.commands(result), [])
        self.assertIn(
            "native_provider_contract",
            [item["id"] for item in result["deferred_evidence"]],
        )

    def test_native_validation_routes_all_byte_stable_fixture_contracts(self):
        validation_commands = [
            "cargo test --locked --no-default-features --lib validation::advanced_blending_presentation_state_tests::",
            "cargo test --locked --no-default-features --lib validation::blending_presentation_state_tests::",
            "cargo test --locked --no-default-features --lib validation::color_softcopy_presentation_state_tests::",
            "cargo test --locked --no-default-features --lib validation::general_ecg_tests::",
            "cargo test --locked --no-default-features --lib validation::rt_image_tests::",
            "cargo test --locked --no-default-features --lib validation::rt_plan_tests::",
            "cargo test --locked --no-default-features --lib validation::rt_radiation_tests::",
            "cargo test --locked --no-default-features --lib validation::twelve_lead_ecg_tests::",
        ]
        for changed in [
            "src/validation.rs",
            "src/validation_advanced_blending_presentation_state_tests.rs",
            "src/validation_blending_presentation_state_tests.rs",
            "src/validation_color_softcopy_presentation_state_tests.rs",
            "src/validation_general_ecg_tests.rs",
            "src/validation_rt_image_tests.rs",
            "src/validation_rt_plan_tests.rs",
            "src/validation_rt_radiation_tests.rs",
            "src/validation_twelve_lead_ecg_tests.rs",
        ]:
            with self.subTest(changed=changed):
                result = self.select(changed)
                self.assertEqual(
                    result["bundle_ids"],
                    ["byte_stable_validation", "corpus"],
                )
                commands = self.commands(result)
                self.assertEqual(commands[:8], validation_commands)
                self.assertEqual(
                    sum(command["list_count"] for command in result["commands"] if command.get("kind") == "lib"),
                    36,
                )
                self.assertEqual(
                    [item["id"] for item in result["deferred_evidence"]],
                    ["explicit_heavy", "future_external_corpus", "release_candidate"],
                )
                routed = " ".join(commands)
                for forbidden in ["--ignored", "--features", "--release", "__nightly"]:
                    self.assertNotIn(forbidden, routed)

    def test_docs_only_selects_no_extra_work_and_unknown_source_fails_closed(self):
        result = self.select("docs/generation-guide.md")
        self.assertEqual(result["commands"], [])
        self.assertEqual(result["ignored_paths"], ["docs/generation-guide.md"])
        self.assertEqual([item["id"] for item in result["deferred_evidence"]], ["release_candidate"])
        with self.assertRaisesRegex(ROUTER.RoutingError, "unmapped executable/code/data path"):
            self.select("src/new_surface.rs")
        for invalid in ["../src/sdk.rs", "/src/sdk.rs", "src//sdk.rs", "src/sdk.rs\nother"]:
            with self.assertRaises(ROUTER.RoutingError):
                self.select(invalid)

    def test_cli_rejects_no_selection_mode_but_allows_a_real_empty_diff(self):
        self.assertEqual(ROUTER.main([]), 2)
        with mock.patch.object(ROUTER, "diff_paths", return_value=[]):
            self.assertEqual(
                ROUTER.main(["--dry-run", "--diff", "a" * 40, "b" * 40]),
                0,
            )

        with (
            mock.patch.object(ROUTER.subprocess, "run") as run,
            mock.patch("builtins.print") as output,
        ):
            self.assertEqual(
                ROUTER.main(["--dry-run", "--diff", "0" * 40, "b" * 40]),
                0,
            )
        run.assert_not_called()
        result = json.loads(output.call_args.args[0])
        self.assertEqual(result["bundle_ids"], ["all-ordinary"])
        self.assertEqual(
            [item["id"] for item in result["deferred_evidence"]],
            [
                "codec_feature_matrix", "explicit_heavy", "future_external_corpus",
                "native_provider_contract", "nightly", "release_candidate", "unrouted_lib_groups",
            ],
        )

    def test_top_level_rust_tests_derive_exact_target_and_module_from_ownership(self):
        result = self.select("tests/sdk_facade.rs")
        self.assertEqual(result["bundle_ids"], ["test:tests/sdk_facade.rs"])
        self.assertEqual(result["commands"][0]["target"], "cli_sdk__nonfast")
        self.assertEqual(result["commands"][0]["module"], "sdk_facade")
        self.assertTrue(result["commands"][0]["argv"][-1].endswith("sdk_facade::"))
        with self.assertRaisesRegex(ROUTER.RoutingError, "unowned Rust test source"):
            self.select("tests/new_unowned_test.rs")

    def test_name_status_diff_routes_both_rename_sides_and_deletions(self):
        output = (
            b"R100\0src/sdk.rs\0src/codecs.rs\0"
            b"D\0src/generation_backends/process.rs\0"
        )
        completed = types.SimpleNamespace(returncode=0, stdout=output, stderr=b"")
        with mock.patch.object(ROUTER.subprocess, "run", return_value=completed) as run:
            paths = ROUTER.diff_paths("a" * 40, "b" * 40)
        self.assertEqual(
            paths,
            ["src/sdk.rs", "src/codecs.rs", "src/generation_backends/process.rs"],
        )
        self.assertEqual(run.call_args.args[0][:4], ["git", "diff", "--name-status", "-z"])
        self.assertEqual(run.call_args.args[0][-1], "--")
        result = self.select(*paths)
        self.assertEqual(result["bundle_ids"], ["codec", "provider", "sdk"])

        for malformed in [b"Q\0src/sdk.rs\0", b"R101\0src/sdk.rs\0src/codecs.rs\0", b"R100\0src/sdk.rs\0"]:
            failed = types.SimpleNamespace(returncode=0, stdout=malformed, stderr=b"")
            with mock.patch.object(ROUTER.subprocess, "run", return_value=failed):
                with self.assertRaises(ROUTER.RoutingError):
                    ROUTER.diff_paths("a" * 40, "b" * 40)
        for revision in ["main", "A" * 40, "-" * 40]:
            with self.assertRaisesRegex(ROUTER.RoutingError, "immutable lowercase 40-hex"):
                ROUTER.diff_paths(revision, "b" * 40)
        self.assertEqual(
            ROUTER.main(["--dry-run", "--diff", "0" * 40, "main"]),
            2,
        )

    def test_all_ordinary_is_bounded_and_contains_no_forbidden_evidence(self):
        result = self.select(force_all=True)
        self.assertEqual(result["bundle_ids"], ["all-ordinary"])
        self.assertGreater(len(result["commands"]), 10)
        forbidden = {"--all-targets", "--all-features", "--features", "--ignored", "--release"}
        for command in result["commands"]:
            argv = command["argv"]
            self.assertEqual(argv[:4], ["cargo", "test", "--locked", "--no-default-features"])
            self.assertTrue(forbidden.isdisjoint(argv))
            if command.get("kind") != "lib":
                target = next(item for item in self.ownership["targets"] if item["name"] == command["target"])
                if not set(target["verification_classes"]) <= {"fast", "subsystem"}:
                    self.assertIn("module", command)
        text = json.dumps(result["commands"])
        for forbidden_target in ["__nightly", "__release_candidate", "--ignored", "--features"]:
            self.assertNotIn(forbidden_target, text)
        self.assertEqual(
            [item["id"] for item in result["deferred_evidence"]],
            [
                "codec_feature_matrix", "explicit_heavy", "future_external_corpus",
                "native_provider_contract", "nightly", "release_candidate", "unrouted_lib_groups",
            ],
        )

    def test_configuration_drift_and_mixed_target_broadening_fail_closed(self):
        drift = copy.deepcopy(self.config)
        drift["bundles"]["engine"]["commands"] = [{"target": "engine__nightly"}]
        with self.assertRaisesRegex(ROUTER.RoutingError, "module filter"):
            ROUTER.validate_config(drift, self.ownership)

        mixed = copy.deepcopy(self.config)
        mixed["bundles"]["sdk"]["commands"] = [{"target": "cli_sdk__nonfast"}]
        with self.assertRaisesRegex(ROUTER.RoutingError, "module filter"):
            ROUTER.validate_config(mixed, self.ownership)

        injected = copy.deepcopy(self.config)
        injected["bundles"]["engine"]["commands"] = [
            {"target": "engine__subsystem", "module": "executor_engine", "args": ["--ignored"]}
        ]
        with self.assertRaisesRegex(ROUTER.RoutingError, "structured target/module"):
            ROUTER.validate_config(injected, self.ownership)

        overlap = copy.deepcopy(self.config)
        overlap["rules"][0]["exact"].append("AGENTS.md")
        with self.assertRaisesRegex(ROUTER.RoutingError, "both routed and ignored"):
            ROUTER.validate_config(overlap, self.ownership)

        for bypass in [
            ("prefixes", "src/"),
            ("exact", "src/new_surface.rs"),
        ]:
            ignored = copy.deepcopy(self.config)
            ignored["ignored"][bypass[0]].append(bypass[1])
            with self.assertRaisesRegex(ROUTER.RoutingError, "governance allowlist"):
                ROUTER.validate_config(ignored, self.ownership)

        missing_fast = copy.deepcopy(self.config)
        missing_fast["unconditional_fast_targets"].pop()
        with self.assertRaisesRegex(ROUTER.RoutingError, "unconditional Fast target inventory drift"):
            ROUTER.validate_config(missing_fast, self.ownership)

    def test_heavy_cost_test_sources_defer_without_weakening_default_selection(self):
        provider = self.select("tests/composition_quantitative.rs")
        self.assertEqual(provider["commands"], [])
        self.assertEqual(
            [item["id"] for item in provider["deferred_evidence"]],
            ["native_provider_contract", "release_candidate"],
        )
        mixed = self.select("tests/generate_cli.rs")
        self.assertEqual(mixed["commands"], [])
        self.assertEqual(
            [item["id"] for item in mixed["deferred_evidence"]],
            ["explicit_heavy", "nightly", "release_candidate"],
        )
        release = self.select("tests/release_archive.rs")
        self.assertEqual(release["commands"], [])
        self.assertEqual(
            [item["id"] for item in release["deferred_evidence"]],
            ["release_candidate"],
        )

    def test_embedded_corpus_route_defers_and_cannot_run_r23_heavy_entry(self):
        result = self.select("cases/registry.json")
        deferred = {item["id"] for item in result["deferred_evidence"]}
        self.assertEqual(deferred, {"explicit_heavy", "future_external_corpus", "release_candidate"})
        command_text = "\n".join(self.commands(result))
        self.assertNotIn("--ignored", command_text)
        self.assertNotIn("case_recipe_catalog", command_text)
        source = (ROOT / "tests/case_recipe_catalog.rs").read_text(encoding="utf-8")
        self.assertIn(
            '#[ignore = "R2.3 explicit heavy qualification; run through scripts/run-heavy-qualification.sh"]',
            source,
        )

    def test_explicit_lib_filters_resolve_the_bound_ownership_counts(self):
        listing = subprocess.check_output(
            ["cargo", "test", "--locked", "--no-default-features", "--lib", "--", "--list", "--format", "terse"],
            cwd=ROOT,
            text=True,
        ).splitlines()
        groups = {item["source"]: item for item in self.ownership["entry_groups"]}
        filters = {}
        for bundle_id in ["engine", "codec", "provider"]:
            for command in self.config["bundles"][bundle_id]["commands"]:
                if command.get("kind") == "lib":
                    filters[command["module"]] = (command["source"], command["list_count"])
        self.assertEqual(len(filters), 14)
        for module, (source, expected_count) in filters.items():
            with self.subTest(module=module):
                observed = [line for line in listing if line.startswith(f"{module}::") and line.endswith(": test")]
                self.assertEqual(len(observed), expected_count)
                self.assertLessEqual(expected_count, groups[source]["entry_count"])
                observed_names = {line.removesuffix(": test").rsplit("::", 1)[-1] for line in observed}
                owned_names = set(groups[source]["entries"])
                self.assertTrue(observed_names <= owned_names)
                if expected_count == groups[source]["entry_count"]:
                    self.assertEqual(observed_names, owned_names)
        process_source = (ROOT / "src/generation_backends/process.rs").read_text(encoding="utf-8")
        self.assertEqual(process_source.count("#[ignore ="), 6)

    def test_current_tracked_executable_surfaces_are_routed_or_explicitly_ignored(self):
        tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).split(b"\0")
        checked = 0
        for raw in tracked:
            if not raw:
                continue
            path = raw.decode("utf-8")
            ROUTER.select([path], self.config, self.ownership)
            checked += 1
        self.assertGreater(checked, 800)

    def test_package_include_and_tracked_inventory_cover_router_artifacts(self):
        cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        self.assertIn('"product/**"', cargo)
        self.assertIn('"scripts/**"', cargo)
        self.assertIn('"tests/**"', cargo)
        for required in [
            "product/change-test-routing.json",
            "scripts/route-changed-tests.py",
            "tests/test_change_test_routing.py",
        ]:
            tracked = subprocess.run(
                ["git", "ls-files", "--error-unmatch", required], cwd=ROOT,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
            self.assertEqual(tracked.returncode, 0, f"router package artifact is not tracked: {required}")


if __name__ == "__main__":
    unittest.main()
