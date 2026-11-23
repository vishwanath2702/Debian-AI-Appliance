"""Packaging contract for read-only SearXNG first-boot decisions."""

from pathlib import Path
import ast
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
MODULE = ROOT / "runtime/modules/searxng_firstboot_decision.py"

NAME = "searxng_firstboot_decision.py"

DEPENDENCIES = (
    "searxng_config.py",
    "searxng_bootstrap.py",
    "searxng_observation.py",
    "searxng_firstboot_policy.py",
    "searxng_retry_policy.py",
)


class PackagingTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.builder = BUILDER.read_text()

    def test_module_exists(self):
        self.assertTrue(MODULE.is_file())

    def test_module_compiles(self):
        ast.parse(MODULE.read_text())

    def test_inventory_entry(self):
        self.assertIn(
            "modules/" + NAME,
            self.builder,
        )

    def test_source_staged_once(self):
        self.assertEqual(
            self.builder.count(
                '$DAIA_RUNTIME_SOURCE/modules/' + NAME
            ),
            1,
        )

    def test_destination_staged_once(self):
        self.assertEqual(
            self.builder.count(
                '$DAIA_ROOT_TARGET/' + NAME
            ),
            1,
        )

    def test_copy_file_contract(self):
        expected = (
            'copy_file \\\n'
            '            "$DAIA_RUNTIME_SOURCE/modules/'
            + NAME
            + '" \\\n'
            '            "$DAIA_ROOT_TARGET/'
            + NAME
            + '" \\\n'
            '            0644 || return 1'
        )
        self.assertIn(expected, self.builder)

    def test_dependencies_staged(self):
        for name in DEPENDENCIES:
            with self.subTest(name=name):
                self.assertIn(
                    '$DAIA_RUNTIME_SOURCE/modules/' + name,
                    self.builder,
                )
                self.assertIn(
                    '$DAIA_ROOT_TARGET/' + name,
                    self.builder,
                )

    def test_not_activated_in_firstboot(self):
        self.assertNotIn(
            NAME.removesuffix(".py"),
            (ROOT / "runtime/firstboot.sh").read_text(),
        )

    def test_not_activated_in_runtime(self):
        self.assertNotIn(
            NAME.removesuffix(".py"),
            (ROOT / "runtime/runtime.sh").read_text(),
        )

    def test_no_duplicate_inventory(self):
        self.assertEqual(
            self.builder.count("modules/" + NAME),
            2,
        )

    def test_no_service_activation(self):
        self.assertNotIn(
            NAME.removesuffix(".py"),
            (ROOT / "runtime/modules/searxng.sh").read_text(),
        )


if __name__ == "__main__":
    unittest.main()
