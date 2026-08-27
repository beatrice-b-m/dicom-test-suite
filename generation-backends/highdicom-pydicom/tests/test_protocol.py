from __future__ import annotations

import unittest

from dts_highdicom_backend.protocol import RUNTIME_DISTRIBUTIONS, runtime_identity


class RuntimeIdentityTest(unittest.TestCase):
    def test_identity_is_complete_and_stable(self) -> None:
        first = runtime_identity()
        second = runtime_identity()
        self.assertEqual(first, second)
        self.assertEqual(first["python"]["implementation"], "cpython")
        self.assertEqual(first["python"]["version"], "3.12.12")
        self.assertEqual(
            [item["name"] for item in first["distributions"]],
            list(RUNTIME_DISTRIBUTIONS),
        )
        for distribution in first["distributions"]:
            self.assertRegex(distribution["files_sha256"], r"^[0-9a-f]{64}$")


if __name__ == "__main__":
    unittest.main()
