"""SearXNG initializer packaging contract tests."""

import re
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
INITIALIZER = ROOT / "runtime/modules/searxng_initialize.py"


class InitializerPackagingTests(unittest.TestCase):

    def test_initializer_source_exists(self):
        self.assertTrue(INITIALIZER.is_file())

    def test_initializer_is_in_module_inventory(self):
        source = BUILDER.read_text()
        self.assertEqual(
            len(re.findall(
                r"^\s*modules/searxng_initialize\.py\s*\\?\s*$",
                source,
                re.MULTILINE,
            )),
            1,
        )

    def test_initializer_install_source(self):
        source = BUILDER.read_text()
        self.assertEqual(
            source.count(
                '"$DAIA_RUNTIME_SOURCE/modules/'
                'searxng_initialize.py"'
            ),
            1,
        )

    def test_initializer_install_destination(self):
        source = BUILDER.read_text()
        self.assertEqual(
            source.count(
                '"$DAIA_ROOT_TARGET/'
                'searxng_initialize.py"'
            ),
            1,
        )

    def test_existing_modules_remain(self):
        source = BUILDER.read_text()
        for name in (
            "searxng_observation.py",
            "searxng_retry_policy.py",
        ):
            self.assertIn(name, source)

    def test_initializer_not_activated(self):
        firstboot = (ROOT / "runtime/firstboot.sh").read_text()
        runtime = (ROOT / "runtime/runtime.sh").read_text()

        main_section = firstboot.partition("main()")[2]
        self.assertTrue(main_section)
        self.assertEqual(main_section.count("initialize_searxng_registration"), 0)
        self.assertNotIn("searxng_initialize", runtime)

    def test_production_entry_remains_root_only(self):
        source = INITIALIZER.read_text()
        entry = source.split(
            "def initialize(directory=DEFAULT_DIRECTORY):",
            1,
        )[1]

        self.assertIn("os.geteuid() == 0", entry)
        self.assertIn(
            "directory == DEFAULT_DIRECTORY", entry
        )
        self.assertIn("_initialize(directory)", entry)
        self.assertNotIn("transition=", entry)


    def test_initializer_has_independent_copy_file_command(self):
        import re

        builder = (
            Path(__file__).resolve().parents[2]
            / "build/build-payload.sh"
        ).read_text()

        pattern = re.compile(
            r"copy_file\s*\\\n"
            r'\s*"\$DAIA_RUNTIME_SOURCE/modules/searxng_initialize\.py"\s*\\\n'
            r'\s*"\$DAIA_ROOT_TARGET/searxng_initialize\.py"\s*\\\n'
            r"\s*0644\s*\|\|\s*return\s+1"
        )
        self.assertEqual(len(pattern.findall(builder)), 1)

        self.assertNotRegex(
            builder,
            r"searxng_retry_policy\.py.*\\\n"
            r'\s*"\$DAIA_RUNTIME_SOURCE/modules/searxng_initialize\.py"',
        )


if __name__ == "__main__":
    unittest.main()
