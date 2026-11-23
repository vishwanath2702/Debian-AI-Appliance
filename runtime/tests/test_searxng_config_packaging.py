
"""SearXNG configuration reader packaging contracts."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
CONFIG = ROOT / "runtime/modules/searxng_config.py"
FIRSTBOOT = ROOT / "runtime/firstboot.sh"


class ConfigPackagingTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.lines = BUILDER.read_text().splitlines()

    def test_source_exists(self):
        self.assertTrue(CONFIG.is_file())

    def test_source_compiles(self):
        compile(CONFIG.read_text(), str(CONFIG), "exec")

    def test_required_inventory(self):
        self.assertEqual(
            sum(
                line.strip().rstrip(chr(92)).strip()
                == "modules/searxng_config.py"
                for line in self.lines
            ),
            1,
        )

    def test_copy_source(self):
        self.assertEqual(
            sum(
                '"$DAIA_RUNTIME_SOURCE/modules/searxng_config.py"'
                in line
                for line in self.lines
            ),
            1,
        )

    def test_copy_destination(self):
        self.assertEqual(
            sum(
                '"$DAIA_ROOT_TARGET/searxng_config.py"'
                in line
                for line in self.lines
            ),
            1,
        )

    def test_copy_command_and_mode(self):
        matches = [
            i for i, line in enumerate(self.lines)
            if '"$DAIA_RUNTIME_SOURCE/modules/searxng_config.py"'
            in line
        ]
        self.assertEqual(len(matches), 1)
        index = matches[0]
        self.assertEqual(
            self.lines[index - 1].strip(),
            "copy_file " + chr(92),
        )
        self.assertEqual(
            self.lines[index + 2].strip(),
            "0644 || return 1",
        )

    def test_bootstrap_still_packaged(self):
        self.assertIn(
            '"$DAIA_ROOT_TARGET/searxng_bootstrap.py"',
            BUILDER.read_text(),
        )

    def test_no_firstboot_activation(self):
        self.assertNotIn(
            "searxng_config.py",
            FIRSTBOOT.read_text(),
        )


if __name__ == "__main__":
    unittest.main()
