"""Packaging contract for DAIA benchmark HTML reports."""

import ast
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RENDERER = ROOT / "runtime/daia-benchmark-report.py"
MANAGER = ROOT / "runtime/daia-model-manager.py"
BUILDER = ROOT / "build/build-payload.sh"

INSTALLED_RENDERER = (
    "/usr/libexec/daia/daia-benchmark-report.py"
)


class BenchmarkReportPackagingTests(unittest.TestCase):
    def test_renderer_source_exists_and_parses(self):
        self.assertTrue(RENDERER.is_file())
        self.assertFalse(RENDERER.is_symlink())
        ast.parse(RENDERER.read_text(encoding="utf-8"))

    def test_builder_requires_renderer(self):
        source = BUILDER.read_text(encoding="utf-8")
        self.assertIn(
            "        daia-benchmark-report.py \\\n",
            source,
        )

    def test_builder_stages_renderer_at_installed_path(self):
        source = BUILDER.read_text(encoding="utf-8")
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-benchmark-report.py"',
            source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE' + INSTALLED_RENDERER + '"',
            source,
        )

    def test_manager_uses_installed_renderer_path(self):
        source = MANAGER.read_text(encoding="utf-8")
        self.assertIn(
            '"/usr/libexec/daia/daia-benchmark-report.py"',
            source,
        )
        self.assertIn(
            '"daia-benchmark-report.py"',
            source,
        )


if __name__ == "__main__":
    unittest.main()
