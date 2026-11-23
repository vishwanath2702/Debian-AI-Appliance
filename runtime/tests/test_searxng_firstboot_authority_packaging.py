"""SearXNG firstboot authority packaging contract tests."""
import ast
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
MODULE = ROOT / "runtime/modules/searxng_firstboot_authority.py"
ENTRY = "modules/searxng_firstboot_authority.py"

class AuthorityPackagingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.builder = BUILDER.read_text()
        cls.firstboot = (ROOT / "runtime/firstboot.sh").read_text()
        cls.runtime = (ROOT / "runtime/runtime.sh").read_text()

    def test_source_exists_and_compiles(self):
        self.assertTrue(MODULE.is_file())
        ast.parse(MODULE.read_text())

    def test_inventory_once(self):
        self.assertEqual(
            self.builder.count("            " + ENTRY + " \\\n"), 1
        )

    def test_source_staged_once(self):
        self.assertEqual(self.builder.count(
            '"$DAIA_RUNTIME_SOURCE/' + ENTRY + '"'
        ), 1)

    def test_destination_staged_once(self):
        self.assertEqual(self.builder.count(
            '"$DAIA_ROOT_TARGET/searxng_firstboot_authority.py"'
        ), 1)

    def test_copy_mode(self):
        expected = (
            '        copy_file \\\n'
            '            "$DAIA_RUNTIME_SOURCE/' + ENTRY + '" \\\n'
            '            "$DAIA_ROOT_TARGET/searxng_firstboot_authority.py" \\\n'
            '            0644 || return 1'
        )
        self.assertEqual(self.builder.count(expected), 1)

    def test_feature_gate(self):
        gate = self.builder.index(
            'if [[ "${DAIA_ENABLE_SEARXNG:-false}" == "true" ]]'
        )
        inventory = self.builder.index(
            "            " + ENTRY + " \\\n"
        )
        self.assertLess(gate, inventory)

    def test_dependencies_staged(self):
        for name in (
            "searxng_firstboot_plan.py",
            "searxng_firstboot_authorization.py",
        ):
            with self.subTest(name=name):
                self.assertIn(
                    '"$DAIA_ROOT_TARGET/' + name + '"',
                    self.builder
                )

    def test_not_activated(self):
        for content in (self.firstboot, self.runtime):
            self.assertNotIn("searxng_firstboot_authority", content)

if __name__ == "__main__":
    unittest.main()
