"""SearXNG first-boot initialization boundary."""

from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIRSTBOOT = ROOT / "runtime/firstboot.sh"


class FirstbootInitializerTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.source = FIRSTBOOT.read_text()

    def test_function_defined_once(self):
        self.assertEqual(
            self.source.count("initialize_searxng_registration()"),
            1,
        )

    def test_feature_gate(self):
        self.assertIn(
            '${DAIA_ENABLE_SEARXNG:-false}',
            self.source,
        )

    def test_root_guard(self):
        self.assertIn("(( EUID == 0 )) || return 1", self.source)

    def test_installed_path(self):
        self.assertIn(
            "/opt/daia/searxng_initialize.py",
            self.source,
        )

    def test_owner_mode_guard(self):
        self.assertIn("stat -c '%u:%g:%a'", self.source)
        self.assertIn('"0:0:644"', self.source)

    def test_no_adoption(self):
        self.assertNotIn("FileExistsError", self.source)

    def test_no_docker_in_initializer(self):
        section = self.source.partition(
            "initialize_searxng_registration()"
        )[2].partition("SEARXNG_INITIALIZE_PY")[2]
        self.assertNotIn("docker ", section.partition("mark_firstboot_complete")[0])

    def test_not_invoked_from_main(self):
        main_section = self.source.partition("main()")[2]
        self.assertTrue(main_section)
        self.assertNotIn(
            "initialize_searxng_registration",
            main_section,
        )

    def test_trusted_provisioner_defined_but_deferred(self):
        self.assertIn(
            "provision_searxng_firstboot()",
            self.source,
        )
        main_section = self.source.partition("main()")[2]
        self.assertIn(
            "\n    provision_searxng_firstboot\n",
            main_section,
        )

    def test_shell_syntax(self):
        result = subprocess.run(
            ["bash", "-n", str(FIRSTBOOT)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
