
"""SearXNG bootstrap payload packaging regression tests."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
BOOTSTRAP = ROOT / "runtime/modules/searxng_bootstrap.py"


class BootstrapPackagingTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.lines = BUILDER.read_text().splitlines()

    def test_bootstrap_source_exists(self):
        self.assertTrue(BOOTSTRAP.is_file())

    def test_bootstrap_compiles(self):
        compile(BOOTSTRAP.read_text(), str(BOOTSTRAP), "exec")

    def test_bootstrap_in_required_inventory(self):
        self.assertEqual(
            sum(
                line.strip().rstrip(chr(92)).strip()
                == "modules/searxng_bootstrap.py"
                for line in self.lines
            ),
            1,
        )

    def test_bootstrap_copy_source(self):
        self.assertEqual(
            sum(
                '"$DAIA_RUNTIME_SOURCE/modules/searxng_bootstrap.py"'
                in line
                for line in self.lines
            ),
            1,
        )

    def test_bootstrap_copy_destination(self):
        self.assertEqual(
            sum(
                '"$DAIA_ROOT_TARGET/searxng_bootstrap.py"'
                in line
                for line in self.lines
            ),
            1,
        )

    def test_bootstrap_copy_mode(self):
        index = next(
            i for i, line in enumerate(self.lines)
            if '"$DAIA_RUNTIME_SOURCE/modules/searxng_bootstrap.py"'
            in line
        )
        self.assertEqual(
            self.lines[index + 2].strip(),
            "0644 || return 1",
        )

    def test_no_firstboot_activation(self):
        source = (ROOT / "runtime/firstboot.sh").read_text()
        self.assertNotIn("searxng_bootstrap.py", source)

    def test_no_runtime_activation(self):
        source = (ROOT / "runtime/runtime.sh").read_text()
        self.assertNotIn("searxng_bootstrap.py", source)


if __name__ == "__main__":
    unittest.main()
