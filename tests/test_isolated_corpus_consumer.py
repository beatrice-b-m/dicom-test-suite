import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("isolated_proof", ROOT / "scripts/prove-isolated-corpus-consumer.py")
PROOF = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROOF)


class IsolatedConsumerContract(unittest.TestCase):
    def test_supported_imports_fail_closed(self):
        fixture = (ROOT / "tests/fixtures/isolated-corpus-consumer/main.rs").read_text()
        PROOF.assert_sdk_imports(fixture)
        for forbidden in ["use synth_dicom_gen::curated_plan;", "use synth_dicom_gen::{sdk, recipes};", "extern crate synth_dicom_gen;"]:
            with self.assertRaises(AssertionError):
                PROOF.assert_sdk_imports(fixture + forbidden)

    def test_subset_is_three_cases_and_unchanged_recipe_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve() / "bundle"
            PROOF.bundle(ROOT, root)
            definition = json.loads((root / "corpus-definition.json").read_bytes())
            registry = json.loads((root / "cases/registry.json").read_bytes())
            self.assertEqual(len(definition["profiles"]), 8)
            self.assertEqual(len(registry["cases"]), 3)
            self.assertEqual(len(definition["cases"]), 3)
            self.assertEqual(definition["definition_id"], "isolated-sdk.smoke")
            for case in definition["cases"]:
                path = case["recipe"]["path"]
                self.assertEqual((root / path).read_bytes(), (ROOT / path).read_bytes())
            self.assertTrue(all("metadata" in row["compatibility_axes"] for row in registry["cases"]))

    def test_cleanup_cannot_target_an_unowned_name(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(AssertionError):
                PROOF.remove_owned(Path(temp), "..")

    def test_lock_identity_includes_version_source_checksum(self):
        packages = PROOF.lock_packages('[[package]]\nname = "x"\nversion = "1"\nsource = "registry+x"\nchecksum = "abc"\n')
        self.assertEqual(packages, {("x", "1", "registry+x", "abc")})


if __name__ == "__main__":
    unittest.main()
