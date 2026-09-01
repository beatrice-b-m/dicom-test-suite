"""Entrypoint loading must not charge every generator to one invocation."""

from __future__ import annotations

import subprocess
import sys
import unittest



class EntrypointImportTest(unittest.TestCase):
    def test_generator_modules_remain_lazy(self) -> None:
        modules = [
            "parametric_map",
            "scoord3d",
            "tid1500",
            "wsi_tile_segmentation",
        ]
        assertion = ";".join(
            [
                "import sys",
                "import dts_highdicom_backend.__main__",
                *(
                    f"assert 'dts_highdicom_backend.{module}' not in sys.modules"
                    for module in modules
                ),
            ]
        )
        completed = subprocess.run(
            [sys.executable, "-I", "-X", "utf8", "-c", assertion],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
