"""Safety contract for DAIA PostgreSQL shutdown."""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
STOP_HELPER = ROOT / "runtime/daia-pgvector-stop"


class PgvectorShutdownContractTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        if not STOP_HELPER.is_file() or STOP_HELPER.is_symlink():
            raise AssertionError(
                "Pgvector shutdown helper is missing or unsafe"
            )

        cls.source = STOP_HELPER.read_text()

    def test_uses_docker_stop(self):
        self.assertIn("docker stop", self.source)

    def test_does_not_remove_container(self):
        self.assertNotIn("docker rm", self.source)

    def test_does_not_remove_volume(self):
        self.assertNotIn("docker volume rm", self.source)
        self.assertNotIn("docker volume prune", self.source)

    def test_does_not_use_force_kill(self):
        self.assertNotIn("docker kill", self.source)

    def test_has_strict_shell_mode(self):
        self.assertIn("set -euo pipefail", self.source)


class PgvectorShutdownBehaviorTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        if os.geteuid() != 0:
            self.skipTest(
                "Privileged shutdown behavior requires an isolated root test"
            )

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-shutdown-"
        )
        self.addCleanup(self.temp.cleanup)

        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()

        self.log = self.root / "docker.log"

        self.environment = os.environ.copy()
        self.environment["PATH"] = (
            str(self.bin) + os.pathsep
            + self.environment.get("PATH", "")
        )
        self.environment["DAIA_TEST_DOCKER_LOG"] = str(self.log)

    def mock_docker(
        self,
        component="pgvector",
        managed="true",
        container_id="a" * 64,
        inspect_failure=False,
    ):
        import shlex

        docker = self.bin / "docker"
        inspection = (
            container_id + "\t" + component + "\t" + managed
        )

        docker.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$*" >> "$DAIA_TEST_DOCKER_LOG"\n'
            'case "$*" in\n'
            '  info) exit 0 ;;\n'
            '  *inspect*)\n'
            + (
                '    exit 42\n'
                if inspect_failure
                else (
                    '    printf "%s\\n" '
                    + shlex.quote(inspection)
                    + '\n'
                )
            )
            + '    ;;\n'
            '  stop*) exit 0 ;;\n'
            '  *) exit 99 ;;\n'
            'esac\n'
        )

        docker.chmod(0o755)

    def shutdown(self):
        import subprocess

        return subprocess.run(
            ["bash", str(STOP_HELPER)],
            env=self.environment,
            capture_output=True,
            text=True,
        )

    def docker_log(self):
        if not self.log.exists():
            return ""
        return self.log.read_text()

    def test_stops_owned_container_by_id(self):
        self.mock_docker()
        result = self.shutdown()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("stop --time 60 " + "a" * 64,
                      self.docker_log())

    def test_rejects_wrong_component(self):
        self.mock_docker(component="unrelated")
        result = self.shutdown()

        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("stop --time", self.docker_log())

    def test_rejects_wrong_management_label(self):
        self.mock_docker(managed="false")
        result = self.shutdown()

        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("stop --time", self.docker_log())

    def test_rejects_invalid_container_id(self):
        self.mock_docker(container_id="invalid")
        result = self.shutdown()

        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("stop --time", self.docker_log())


    def test_rejects_container_name_reassignment(self):
        """An unowned replacement must not inherit A's authorization."""
        container_a = "a" * 64
        container_b = "b" * 64

        # Simulate the named container now resolving to B.
        # The single inspection returns B's ID and B's labels.
        self.mock_docker(
            container_id=container_b,
            component="unrelated",
            managed="false",
        )

        result = self.shutdown()
        log = self.docker_log()

        inspections = [
            line for line in log.splitlines()
            if line.startswith("container inspect ")
        ]

        stops = [
            line for line in log.splitlines()
            if line.startswith("stop --time ")
        ]

        self.assertNotEqual(
            result.returncode,
            0,
            "Unowned replacement container must be rejected",
        )
        self.assertEqual(
            len(inspections),
            1,
            "Identity and ownership must use one inspection",
        )
        self.assertEqual(
            stops,
            [],
            "Neither container A nor B may be stopped",
        )
        self.assertNotIn(container_a, log)
        self.assertNotIn(container_b, log)

    def test_rejects_malformed_inspection_responses(self):
        cases = (
            ("missing_component", "", "true", "a" * 64),
            ("missing_managed", "pgvector", "", "a" * 64),
            ("extra_field", "pgvector", "true\\textra", "a" * 64),
            ("embedded_tab", "pgvector\\textra", "true", "a" * 64),
            ("embedded_newline", "pgvector\\nextra", "true", "a" * 64),
            ("short_id", "pgvector", "true", "a" * 63),
            ("nonhex_id", "pgvector", "true", "g" * 64),
        )

        for name, component, managed, container_id in cases:
            with self.subTest(name=name):
                self.log.unlink(missing_ok=True)

                self.mock_docker(
                    component=component,
                    managed=managed,
                    container_id=container_id,
                )

                result = self.shutdown()
                log = self.docker_log()

                self.assertNotEqual(
                    result.returncode,
                    0,
                    f"{name}: malformed inspection was accepted",
                )
                self.assertNotIn(
                    "stop --time",
                    log,
                    f"{name}: malformed inspection triggered shutdown",
                )

    def test_rejects_inspection_failure(self):
        self.mock_docker(inspect_failure=True)
        result = self.shutdown()

        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("stop --time", self.docker_log())


    def test_rejects_missing_trusted_registration(self):
        """Docker labels alone must never authorize container shutdown."""
        self.mock_docker()

        result = self.shutdown()

        self.assertNotEqual(
            result.returncode,
            0,
            "Shutdown must reject a container without trusted registration",
        )
        self.assertNotIn(
            "stop --time",
            self.docker_log(),
            "Missing registration must prevent Docker stop",
        )


if __name__ == "__main__":
    unittest.main()
