"""Static, inert tests for the fixed SearXNG provisioning entrypoint."""

from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
ENTRY = ROOT / "runtime/daia-searxng-firstboot-provision"
RUNNER = ROOT / "runtime/modules/searxng_firstboot_runner.py"
FIRSTBOOT = ROOT / "runtime/firstboot.sh"


class EntrypointTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.source = ENTRY.read_text()

    def test_shell_syntax(self):
        result = subprocess.run(
            ["bash", "-n", str(ENTRY)],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_root_required(self):
        self.assertIn("(( EUID == 0 )) || exit 1", self.source)

    def test_trusted_file_checks(self):
        self.assertIn('! -L "$trusted_file"', self.source)
        self.assertIn("stat -c '%u:%g:%a'", self.source)
        self.assertIn('"0:0:644"', self.source)

    def test_config_not_sourced(self):
        self.assertIn("searxng_config.enabled()", self.source)
        self.assertNotIn("source /opt/daia/config/pragna.conf", self.source)
        self.assertNotIn("source \"$config\"", self.source)

    def test_existing_initializer_reused(self):
        self.assertIn(
            "searxng_firstboot_initializer.initialize_from_firstboot()",
            self.source,
        )

    def test_existing_provisioner_reused(self):
        self.assertIn(
            "searxng_firstboot_provision_registered",
            self.source,
        )
        self.assertIn(
            'source "$DAIA_SEARXNG_PROVISION_MODULE"',
            self.source,
        )

    def test_registration_verified_last(self):
        provision = self.source.rindex(
            "\nsearxng_firstboot_provision_registered"
        )
        verify = self.source.rindex(
            "/usr/libexec/daia-searxng-verify-registration"
        )
        self.assertLess(provision, verify)

    def test_pinned_image_and_archive(self):
        self.assertIn(
            "sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d",
            self.source,
        )
        self.assertIn(
            "fbd19b12819ae924f5b4bc52cc51e1e05fed244be29f17dfc6cb81fae2139c1b",
            self.source,
        )

    def test_fixed_runner_command(self):
        self.assertIn(
            "/usr/libexec/daia-searxng-firstboot-provision",
            RUNNER.read_text(),
        )

    def test_firstboot_activation_deferred(self):
        main = FIRSTBOOT.read_text().split("\nmain()\n", 1)[1]
        self.assertIn(
            "\n    provision_searxng_firstboot\n",
            main,
        )

    def test_no_start_or_service_activation(self):
        self.assertNotIn("systemctl", self.source)
        self.assertNotIn("searxng_start_registered", self.source)
        self.assertNotIn("searxng_wait_ready", self.source)


if __name__ == "__main__":
    unittest.main()
