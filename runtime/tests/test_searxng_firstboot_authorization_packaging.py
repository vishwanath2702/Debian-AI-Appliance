"""SearXNG authorization policy packaging contracts."""

from pathlib import Path
import ast
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
AUTH = ROOT / "runtime/modules/searxng_firstboot_authorization.py"

ENTRY = "modules/searxng_firstboot_authorization.py"
SOURCE = (
    '"$DAIA_RUNTIME_SOURCE/modules/'
    'searxng_firstboot_authorization.py"'
)
DESTINATION = (
    '"$DAIA_ROOT_TARGET/'
    'searxng_firstboot_authorization.py"'
)


class AuthorizationPackagingTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.builder = BUILDER.read_text()

    def test_source_exists_and_compiles(self):
        self.assertTrue(AUTH.is_file())
        ast.parse(AUTH.read_text())

    def test_inventory_entry_once(self):
        self.assertEqual(
            self.builder.count("            " + ENTRY + "\n"),
            1,
        )

    def test_staging_source_once(self):
        self.assertEqual(self.builder.count(SOURCE), 1)

    def test_staging_destination_once(self):
        self.assertEqual(self.builder.count(DESTINATION), 1)

    def test_copy_mode(self):
        expected = (
            '        copy_file \\\n'
            '            ' + SOURCE + ' \\\n'
            '            ' + DESTINATION + ' \\\n'
            '            0644 || return 1\n'
        )
        self.assertEqual(self.builder.count(expected), 1)

    def test_dependencies_staged(self):
        for name in (
            "searxng_firstboot_plan.py",
            "searxng_firstboot_adapter.py",
            "searxng_firstboot_decision.py",
            "searxng_firstboot_policy.py",
        ):
            with self.subTest(name=name):
                self.assertIn(
                    '"$DAIA_ROOT_TARGET/' + name + '"',
                    self.builder,
                )

    def test_feature_gate(self):
        inventory = self.builder.index(
            "            " + ENTRY + "\n"
        )
        staging = self.builder.index(SOURCE)
        self.assertLess(inventory, staging)
        self.assertIn(
            "DAIA_ENABLE_SEARXNG",
            self.builder[max(0, inventory - 4000):inventory],
        )

    def test_not_activated(self):
        for name in ("firstboot.sh", "runtime.sh"):
            with self.subTest(name=name):
                self.assertNotIn(
                    "searxng_firstboot_authorization",
                    (ROOT / "runtime" / name).read_text(),
                )


if __name__ == "__main__":
    unittest.main()
