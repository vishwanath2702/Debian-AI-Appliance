"""Packaging contract for the inactive SearXNG initializer."""

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
MODULE = ROOT / "runtime/modules/searxng_firstboot_initializer.py"


class InitializerPackagingTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.source = BUILDER.read_text()

    def test_module_exists(self):
        self.assertTrue(MODULE.is_file())
        self.assertFalse(MODULE.is_symlink())

    def test_source_staged_once(self):
        self.assertEqual(
            self.source.count(
                '"$DAIA_RUNTIME_SOURCE/modules/'
                'searxng_firstboot_initializer.py"'
            ), 1
        )

    def test_destination_staged_once(self):
        self.assertEqual(
            self.source.count(
                '"$DAIA_ROOT_TARGET/'
                'searxng_firstboot_initializer.py"'
            ), 1
        )

    def test_inventory_once(self):
        self.assertEqual(
            self.source.count(
                'modules/searxng_firstboot_initializer.py \\'
            ), 1
        )

    def test_copy_mode(self):
        lines = self.source.splitlines()
        matches = [
            i for i, line in enumerate(lines)
            if '"$DAIA_RUNTIME_SOURCE/modules/'
               'searxng_firstboot_initializer.py"' in line
        ]
        self.assertEqual(len(matches), 1)
        i = matches[0]
        self.assertGreaterEqual(i, 1)
        self.assertLess(i + 2, len(lines))
        self.assertEqual(lines[i - 1].strip(), "copy_file " + chr(92))
        self.assertEqual(
            lines[i + 1].strip(),
            '"$DAIA_ROOT_TARGET/'
            'searxng_firstboot_initializer.py" ' + chr(92),
        )
        self.assertEqual(lines[i + 2].strip(), "0644 || return 1")

    def test_feature_gate(self):
        start = self.source.index(
            'if [[ "${DAIA_ENABLE_SEARXNG:-false}" == "true" ]]'
        )
        stage = self.source.index(
            '"$DAIA_RUNTIME_SOURCE/modules/'
            'searxng_firstboot_initializer.py"'
        )
        self.assertLess(start, stage)

    def test_not_activated(self):
        firstboot = (ROOT / "runtime/firstboot.sh").read_text()
        main = firstboot[firstboot.index("\nmain()\n"):]
        self.assertIn(
            "\n    provision_searxng_firstboot\n", main
        )
        runtime = (ROOT / "runtime/runtime.sh").read_text()
        self.assertNotIn(
            "searxng_firstboot_initializer", runtime
        )


if __name__ == "__main__":
    unittest.main()
