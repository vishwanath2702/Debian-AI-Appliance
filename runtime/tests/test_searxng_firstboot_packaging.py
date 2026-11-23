"""Inert SearXNG firstboot packaging contract tests."""

from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / "build/build-payload.sh"
ENTRY = ROOT / "runtime/daia-searxng-firstboot-provision"
RUNNER = ROOT / "runtime/modules/searxng_firstboot_runner.py"
LOCK = ROOT / "runtime/modules/searxng_firstboot_lock.py"
FIRSTBOOT = ROOT / "runtime/firstboot.sh"


class SearxngPackagingTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.source = BUILD.read_text()
        cls.entry = ENTRY.read_text()
        cls.runner = RUNNER.read_text()

    def test_required_sources_exist(self):
        for path in (ENTRY, RUNNER, LOCK):
            with self.subTest(path=path.name):
                self.assertTrue(path.is_file())
                self.assertFalse(path.is_symlink())

    def test_feature_gated_staging(self):
        start = self.source.index(
            'if [[ "${DAIA_ENABLE_SEARXNG:-false}" == "true" ]]; then'
        )
        end = self.source.index(
            '\n    fi',
            start,
        )
        block = self.source[start:end]
        for name in (
            "searxng_firstboot_lock.py",
            "searxng_firstboot_runner.py",
            "daia-searxng-firstboot-provision",
        ):
            self.assertIn(name, block)

    def test_exact_install_destinations_and_modes(self):
        contracts = (
            (
                "modules/searxng_firstboot_lock.py",
                "searxng_firstboot_lock.py",
                "0644",
            ),
            (
                "modules/searxng_firstboot_runner.py",
                "searxng_firstboot_runner.py",
                "0644",
            ),
            (
                "daia-searxng-firstboot-provision",
                "daia-searxng-firstboot-provision",
                "0755",
            ),
        )
        for source, destination, mode in contracts:
            with self.subTest(source=source):
                if source.startswith("modules/"):
                    destination_path = (
                        "$DAIA_ROOT_TARGET/" + destination
                    )
                else:
                    destination_path = (
                        "$DAIA_WORKSPACE/usr/libexec/" + destination
                    )
                expected = (
                    'copy_file \\\n'
                    f'            "$DAIA_RUNTIME_SOURCE/{source}" \\\n'
                    f'            "{destination_path}" \\\n'
                    f'            {mode} || return 1'
                )
                self.assertIn(expected, self.source)

    def test_missing_or_symlink_sources_refused(self):
        start = self.source.index(
            "# Stage the fixed firstboot execution boundary"
        )
        end = self.source.index(
            '"SearXNG firstboot lock, runner, and provisioning entrypoint staged without activation."',
            start,
        )
        block = self.source[start:end]
        self.assertIn(
            '[[ ! -f "$DAIA_RUNTIME_SOURCE/$helper" ]]',
            block,
        )
        self.assertIn(
            '[[ -L "$DAIA_RUNTIME_SOURCE/$helper" ]]',
            block,
        )
        self.assertIn("return 1", block)

    def test_runner_fixed_command_matches_entrypoint(self):
        self.assertIn(
            '"/usr/libexec/daia-searxng-firstboot-provision"',
            self.runner,
        )
        self.assertIn(
            "searxng_firstboot_provision_registered",
            self.entry,
        )
        self.assertIn(
            "daia-searxng-verify-registration",
            self.entry,
        )

    def test_firstboot_activation_still_deferred(self):
        main = FIRSTBOOT.read_text().split("\nmain()\n", 1)[1]
        self.assertIn(
            "\n    provision_searxng_firstboot\n",
            main,
        )
        self.assertNotIn(
            "\n    python3 /opt/daia/searxng_firstboot_runner.py\n",
            main,
        )

    def test_shell_syntax(self):
        for path in (BUILD, ENTRY):
            with self.subTest(path=path.name):
                result = subprocess.run(
                    ["bash", "-n", str(path)],
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(
                    result.returncode, 0, result.stderr
                )


if __name__ == "__main__":
    unittest.main()
