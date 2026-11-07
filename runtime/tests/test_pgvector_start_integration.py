"""Executable mock-Docker tests for DAIA pgvector startup."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
START_HELPER = ROOT / "runtime/daia-pgvector-start"
CONTAINER_ID = "a" * 64


class PgvectorStartupIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="daia-start-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.log = self.root / "events.log"

    def execute(self, *, authorized_id=CONTAINER_ID,
                authorization_exit=0, docker_exit=0):
        import shlex

        docker = self.bin / "docker"
        docker.write_text(
            "#!/bin/sh\n"
            'printf "DOCKER:%s\n" "$*" >> "$DAIA_TEST_EVENTS"\n'
            'if [ "$1" = start ]; then\n'
            '    exit "$DAIA_TEST_DOCKER_EXIT"\n'
            'fi\n'
            'exit 99\n'
        )
        docker.chmod(0o755)

        env = os.environ.copy()
        env["PATH"] = str(self.bin) + os.pathsep + env.get("PATH", "")
        env["DAIA_TEST_EVENTS"] = str(self.log)
        env["DAIA_TEST_DOCKER_EXIT"] = str(docker_exit)

        script = (
            "function /usr/libexec/daia-pgvector-authorize-start {\n"
            '    printf "AUTHORIZE\n" >> "$DAIA_TEST_EVENTS"\n'
            "    printf '%s\\n' " + shlex.quote(authorized_id) + "\n"
            "    return " + str(authorization_exit) + "\n"
            "}\n"
            'source "$DAIA_TEST_START_HELPER"\n'
        )
        env["DAIA_TEST_START_HELPER"] = str(START_HELPER)

        return subprocess.run(
            ["bash", "-c", script],
            env=env,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def events(self):
        return self.log.read_text() if self.log.exists() else ""

    def test_starts_authorized_immutable_id(self):
        result = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.events().splitlines(),
            ["AUTHORIZE", "DOCKER:start " + CONTAINER_ID],
        )

    def test_rejects_authorization_failure(self):
        result = self.execute(authorization_exit=1)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.events().splitlines(), ["AUTHORIZE"])

    def test_rejects_malformed_container_ids(self):
        for value in ("", "invalid", "b" * 63, "B" * 64,
                      "a" * 64 + "\nextra"):
            with self.subTest(value=value):
                self.log.unlink(missing_ok=True)
                result = self.execute(authorized_id=value)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(
                    self.events().splitlines(),
                    ["AUTHORIZE"],
                )

    def test_propagates_docker_start_failure(self):
        result = self.execute(docker_exit=17)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            self.events().splitlines(),
            ["AUTHORIZE", "DOCKER:start " + CONTAINER_ID],
        )


if __name__ == "__main__":
    unittest.main()
