
"""Packaging contracts for the typed SearXNG first-boot policy."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
FIRSTBOOT = ROOT / "runtime/firstboot.sh"
MODULES = ROOT / "runtime/modules"


class PackagingTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.lines = BUILDER.read_text().splitlines()
        cls.source = BUILDER.read_text()

    def test_policy_exists(self):
        self.assertTrue(
            (MODULES / "searxng_firstboot_policy.py").is_file()
        )

    def test_policy_compiles(self):
        path = MODULES / "searxng_firstboot_policy.py"
        compile(path.read_text(), str(path), "exec")

    def test_required_inventory(self):
        self.assertEqual(
            sum(
                line.strip().rstrip(chr(92)).strip()
                == "modules/searxng_firstboot_policy.py"
                for line in self.lines
            ),
            1,
        )

    def test_policy_source_staged_once(self):
        self.assertEqual(
            self.source.count(
                '"$DAIA_RUNTIME_SOURCE/modules/'
                'searxng_firstboot_policy.py"'
            ),
            1,
        )

    def test_policy_destination_staged_once(self):
        self.assertEqual(
            self.source.count(
                '"$DAIA_ROOT_TARGET/searxng_firstboot_policy.py"'
            ),
            1,
        )

    def test_policy_copy_mode(self):
        source = (
            '"$DAIA_RUNTIME_SOURCE/modules/'
            'searxng_firstboot_policy.py"'
        )
        matches = [
            i for i, line in enumerate(self.lines)
            if source in line
        ]
        self.assertEqual(len(matches), 1)
        i = matches[0]
        self.assertEqual(
            self.lines[i - 1].strip(),
            "copy_file " + chr(92),
        )
        self.assertEqual(
            self.lines[i + 2].strip(),
            "0644 || return 1",
        )

    def test_bootstrap_dependency_staged(self):
        self.assertIn(
            '"$DAIA_ROOT_TARGET/searxng_bootstrap.py"',
            self.source,
        )

    def test_retry_dependency_staged(self):
        self.assertIn(
            '"$DAIA_ROOT_TARGET/searxng_retry_policy.py"',
            self.source,
        )

    def test_dependencies_have_same_destination_directory(self):
        for name in (
            "searxng_firstboot_policy.py",
            "searxng_bootstrap.py",
            "searxng_retry_policy.py",
        ):
            self.assertIn(
                '"$DAIA_ROOT_TARGET/' + name + '"',
                self.source,
            )

    def test_firstboot_not_activated(self):
        self.assertNotIn(
            "searxng_firstboot_policy",
            FIRSTBOOT.read_text(),
        )

    def test_runtime_not_activated(self):
        self.assertNotIn(
            "searxng_firstboot_policy",
            (ROOT / "runtime/runtime.sh").read_text(),
        )


if __name__ == "__main__":
    unittest.main()
