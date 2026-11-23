"""Contract tests for the read-only SearXNG registration CLI."""

import ast
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-searxng-verify-registration"


class SearxngRegistrationCLITests(unittest.TestCase):
    def test_python_syntax(self):
        ast.parse(HELPER.read_text(encoding="utf-8"))

    def test_root_required(self):
        source = HELPER.read_text(encoding="utf-8")
        self.assertIn("os.geteuid() != 0", source)

    def test_fixed_command_interface(self):
        source = HELPER.read_text(encoding="utf-8")
        self.assertIn('sys.argv[1:] != ["--resolve-registered"]',
                      source)

    def test_trusted_registration_is_verified(self):
        source = HELPER.read_text(encoding="utf-8")
        self.assertIn("read_registration()", source)
        self.assertIn("verify_registration(expected_id)", source)
        self.assertIn("verified_id != expected_id", source)

    def test_installed_module_path(self):
        source = HELPER.read_text(encoding="utf-8")
        self.assertIn('MODULE_DIR = Path("/opt/daia")', source)
        self.assertNotIn(
            'Path("/opt/daia/runtime/modules")',
            source,
        )

    def test_packaging_contract(self):
        build = (ROOT / "build/build-payload.sh").read_text(
            encoding="utf-8"
        )
        expected = (
            "$DAIA_ROOT_TARGET/searxng_registration.py",
            "$DAIA_WORKSPACE/usr/libexec/daia-searxng-verify-container",
            "$DAIA_WORKSPACE/usr/libexec/daia-searxng-verify-registration",
        )
        for destination in expected:
            with self.subTest(destination=destination):
                self.assertIn(destination, build)

    def test_searxng_staging_is_conditional(self):
        build = (ROOT / "build/build-payload.sh").read_text(
            encoding="utf-8"
        )
        marker = "# Stage trusted SearXNG registration"
        self.assertEqual(build.count(marker), 1)

        section = build.split(marker, 1)[1].split(
            "record_staged_component", 1
        )[0]

        self.assertIn(
            "DAIA_ENABLE_SEARXNG:-false",
            section,
        )
        self.assertIn(
            '[[ -L "$DAIA_RUNTIME_SOURCE/$helper" ]]',
            section,
        )

    def test_no_docker_mutation(self):
        source = HELPER.read_text(encoding="utf-8")
        for forbidden in (
            "docker start", "docker create", "docker run",
            "docker network connect", "docker network create",
            "subprocess.run(", "subprocess.Popen(",
        ):
            self.assertNotIn(forbidden, source)


if __name__ == "__main__":
    unittest.main()
