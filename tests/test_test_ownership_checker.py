import copy
import importlib.util
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "test_ownership_checker", ROOT / "scripts/check-test-ownership.py"
)
CHECKER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(CHECKER)


class TestOwnershipCheckerFixtures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads(
            (ROOT / "product/test-ownership.json").read_text(encoding="utf-8")
        )

    def assert_rejected(self, manifest, marker):
        with self.assertRaises(CHECKER.OwnershipError) as raised:
            CHECKER.verify(ROOT, manifest)
        self.assertIn(marker, str(raised.exception))

    def test_current_inventory_is_complete_and_singly_owned(self):
        report = CHECKER.verify(ROOT, copy.deepcopy(self.manifest))
        self.assertEqual(report["rust_test_targets"], 188)
        self.assertEqual(report["rust_test_entries"], 1375)
        self.assertEqual(
            sum(len(group.get("heavy_entries", [])) for group in self.manifest["entry_groups"]),
            6,
        )

    def test_fast_workflow_runs_checker_and_negative_fixtures(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertEqual(workflow.count("python3 scripts/check-test-ownership.py"), 1)
        self.assertEqual(
            workflow.count("python3 -m unittest tests/test_test_ownership_checker.py"), 1
        )

    def test_unowned_and_multiply_owned_targets_fail_closed(self):
        unowned = copy.deepcopy(self.manifest)
        unowned["targets"].pop()
        self.assert_rejected(unowned, "unowned Cargo test target")

        duplicate = copy.deepcopy(self.manifest)
        duplicate["targets"].append(copy.deepcopy(duplicate["targets"][0]))
        self.assert_rejected(duplicate, "multiply owned Cargo test target")

    def test_entry_drift_and_duplicate_source_ownership_fail_closed(self):
        drift = copy.deepcopy(self.manifest)
        drift["entry_groups"][0]["entry_count"] += 1
        self.assert_rejected(drift, "test-entry metadata drift")

        duplicate = copy.deepcopy(self.manifest)
        duplicate["entry_groups"].append(copy.deepcopy(duplicate["entry_groups"][0]))
        self.assert_rejected(duplicate, "multiply owned Rust test-entry source group")

    def test_heavy_or_unexplained_fast_metadata_fails_closed(self):
        heavy = copy.deepcopy(self.manifest)
        fast_group = next(
            group
            for group in heavy["entry_groups"]
            if group["source"] == "tests/compatibility_ownership.rs"
        )
        fast_group["cost_tier"] = "heavy"
        self.assert_rejected(heavy, "heavy or ignored test assigned to Fast")

        unexplained = copy.deepcopy(self.manifest)
        marked_group = next(
            group
            for group in unexplained["entry_groups"]
            if group["source"] == "tests/schema_artifacts.rs"
        )
        del marked_group["fast_cost_exemption"]
        self.assert_rejected(unexplained, "Fast heavy marker lacks cost exemption")


if __name__ == "__main__":
    unittest.main()
