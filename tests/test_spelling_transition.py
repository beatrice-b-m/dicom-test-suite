import copy
import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "spelling_transition_checker", ROOT / "scripts/check-spelling-transition.py"
)
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def inventory():
    classes = {
        name: {"owner": f"owner-{name}", "reason": f"reason-{name}"}
        for name in [
            "dicom_payload_identifier",
            "legacy_payload_schema_fixture_or_evidence",
            "locked_python_module_or_backend",
            "qualified_adapter_environment",
            "qualified_adapter_or_test_fixture",
        ]
    }
    return {
        "removed_environment": [],
        "removed_path_prefixes": [],
        "allowed_removed_occurrences": [],
        "retained_adapter_environment": [],
        "retained_classes": classes,
        "retained_occurrences": [],
        "retained_snapshot": CHECKER.snapshot([]),
    }


def approve(texts):
    approved = inventory()
    records = CHECKER.proposed_records(texts, approved)
    approved["retained_occurrences"] = records
    approved["retained_snapshot"] = CHECKER.snapshot(records)
    return approved


class SpellingTransitionCheckerTests(unittest.TestCase):
    def test_explicit_records_accept_the_reviewed_occurrence_and_count(self):
        texts = {"cases/fixture.json": '"manufacturer":"dicom-test-suite"'}
        approved = approve(texts)
        self.assertEqual(CHECKER.validate(texts, approved), [])

    def test_snapshot_regeneration_cannot_authorize_unknown_environment(self):
        baseline = {"cases/fixture.json": '"manufacturer":"dicom-test-suite"'}
        approved = approve(baseline)
        adversarial = dict(baseline)
        adversarial["src/new_stage.rs"] = (
            'let root = std::env::var_os("DTS_NEW_PRODUCT_ROOT");'
        )
        regenerated = CHECKER.proposed_records(adversarial, approved)
        approved["retained_snapshot"] = CHECKER.snapshot(regenerated)

        errors = CHECKER.validate(adversarial, approved)

        self.assertTrue(
            any("unapproved legacy environment access" in error for error in errors)
        )
        self.assertTrue(any("unlisted legacy occurrences" in error for error in errors))

    def test_snapshot_regeneration_cannot_authorize_unknown_production_path(self):
        baseline = {"cases/fixture.json": '"manufacturer":"dicom-test-suite"'}
        approved = approve(baseline)
        adversarial = dict(baseline)
        adversarial["src/new_stage.rs"] = (
            'let root = temp_dir().join("dts-new-product-staging-");'
        )
        regenerated = CHECKER.proposed_records(adversarial, approved)
        approved["retained_snapshot"] = CHECKER.snapshot(regenerated)

        errors = CHECKER.validate(adversarial, approved)

        self.assertTrue(
            any("legacy production path-building callsites" in error for error in errors)
        )
        self.assertTrue(any("unlisted legacy occurrences" in error for error in errors))

    def test_bootstrap_proposal_does_not_mutate_the_approved_inventory(self):
        texts = {"src/new_stage.rs": 'let root = temp_dir().join("dts-review-me-");'}
        approved = inventory()
        before = copy.deepcopy(approved)

        proposed = CHECKER.proposed_records(texts, approved)

        self.assertEqual(approved, before)
        self.assertEqual(proposed[0]["path"], "src/new_stage.rs")
        self.assertEqual(approved["retained_occurrences"], [])


if __name__ == "__main__":
    unittest.main()
