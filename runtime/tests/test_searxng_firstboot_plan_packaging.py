"""SearXNG first-boot planner packaging contracts."""

from pathlib import Path
import ast
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
PLAN = ROOT / "runtime/modules/searxng_firstboot_plan.py"

INVENTORY = (
    "            modules/searxng_firstboot_adapter.py \\\n"
)

STAGING = (
    '        copy_file \\\n'
    '            "$DAIA_RUNTIME_SOURCE/modules/searxng_firstboot_plan.py" \\\n'
    '            "$DAIA_ROOT_TARGET/searxng_firstboot_plan.py" \\\n'
    '            0644 || return 1\n'
)


class PlanPackagingTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.builder = BUILDER.read_text()

    def test_inventory(self):
        self.assertEqual(self.builder.count(INVENTORY), 1)

    def test_staging(self):
        self.assertEqual(self.builder.count(STAGING), 1)

    def test_source_once(self):
        self.assertEqual(
            self.builder.count(
                '"$DAIA_RUNTIME_SOURCE/modules/'
                'searxng_firstboot_plan.py"'
            ),
            1,
        )

    def test_destination_once(self):
        self.assertEqual(
            self.builder.count(
                '"$DAIA_ROOT_TARGET/searxng_firstboot_plan.py"'
            ),
            1,
        )

    def test_dependencies_staged(self):
        for name in (
            "searxng_firstboot_adapter.py",
            "searxng_firstboot_decision.py",
            "searxng_firstboot_policy.py",
            "searxng_config.py",
            "searxng_bootstrap.py",
            "searxng_observation.py",
        ):
            with self.subTest(name=name):
                self.assertIn(
                    '"$DAIA_ROOT_TARGET/' + name + '"',
                    self.builder,
                )

    def test_plan_compiles(self):
        ast.parse(PLAN.read_text())

    def test_not_activated(self):
        for name in ("firstboot.sh", "runtime.sh"):
            with self.subTest(name=name):
                self.assertNotIn(
                    "searxng_firstboot_plan",
                    (ROOT / "runtime" / name).read_text(),
                )

    def test_feature_gate(self):
        inventory = self.builder.index(INVENTORY)
        staging = self.builder.index(STAGING)
        self.assertLess(inventory, staging)
        self.assertIn(
            "DAIA_ENABLE_SEARXNG",
            self.builder[max(0, inventory - 4000):inventory],
        )


if __name__ == "__main__":
    unittest.main()
