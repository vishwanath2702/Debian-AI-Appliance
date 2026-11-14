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
                authorization_exit=0, docker_exit=0, ready_exit=0,
                network_exit=0, registration_exit=0, stop_exit=0,
                audit_exit=0, init_exit=0):
        import shlex

        docker = self.bin / "docker"
        docker.write_text(
            "#!/bin/sh\n"
            'if [ "$1" = exec ] && [ "$3" = pg_isready ]; then\n'
            '    exit "$DAIA_TEST_READY_EXIT"\n'
            'fi\n'
            'printf "DOCKER:%s\n" "$*" >> "$DAIA_TEST_EVENTS"\n'
            'if [ "$1" = start ]; then\n'
            '    exit "$DAIA_TEST_DOCKER_EXIT"\n'
            'fi\n'
            'if [ "$1" = stop ]; then\n'
            '    exit "$DAIA_TEST_STOP_EXIT"\n'
            'fi\n'
            'exit 99\n'
        )
        docker.chmod(0o755)

        env = os.environ.copy()
        env["PATH"] = str(self.bin) + os.pathsep + env.get("PATH", "")
        env["DAIA_TEST_EVENTS"] = str(self.log)
        env["DAIA_TEST_DOCKER_EXIT"] = str(docker_exit)
        env["DAIA_TEST_READY_EXIT"] = str(ready_exit)
        env["DAIA_TEST_NETWORK_EXIT"] = str(network_exit)
        env["DAIA_TEST_REGISTRATION_EXIT"] = str(registration_exit)
        env["DAIA_TEST_STOP_EXIT"] = str(stop_exit)
        env["DAIA_TEST_AUDIT_EXIT"] = str(audit_exit)
        env["DAIA_TEST_INIT_EXIT"] = str(init_exit)

        script = (
            "function /usr/libexec/daia-pgvector-authorize-start {\n"
            '    printf "AUTHORIZE\n" >> "$DAIA_TEST_EVENTS"\n'
            "    printf '%s\\n' " + shlex.quote(authorized_id) + "\n"
            "    return " + str(authorization_exit) + "\n"
            "}\n"
            "function /usr/libexec/daia-rag-verify-network {\n"
            '    printf "NETWORK:%s:%s\\n" "$1" "$2" >> "$DAIA_TEST_EVENTS"\n'
            '    return "$DAIA_TEST_NETWORK_EXIT"\n'
            "}\n"
            "function /usr/libexec/daia-pgvector-verify-registration {\n"
            '    printf "VERIFY:%s\\n" "$1" >> "$DAIA_TEST_EVENTS"\n'
            '    return "$DAIA_TEST_REGISTRATION_EXIT"\n'
            "}\n"
            "function /usr/libexec/daia-pgvector-audit-auth.py {\n"
            '    printf "AUDIT:%s\n" "$1" >> "$DAIA_TEST_EVENTS"\n'
            '    return "$DAIA_TEST_AUDIT_EXIT"\n'
            "}\n"
            "function /usr/libexec/daia-pgvector-init-extension.py {\n"
            '    printf "INIT:%s\n" "$1" >> "$DAIA_TEST_EVENTS"\n'
            '    return "$DAIA_TEST_INIT_EXIT"\n'
            "}\n"
            "sleep() { :; }\n"
            'source "$DAIA_TEST_START_HELPER"\n'
        )
        env["DAIA_TEST_START_HELPER"] = str(START_HELPER)

        return subprocess.run(
            ["unshare", "--user", "--map-root-user",
             "bash", "-c", script],
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
            ["AUTHORIZE", "DOCKER:start " + CONTAINER_ID,
             "NETWORK:runtime:" + CONTAINER_ID,
             "AUDIT:" + CONTAINER_ID,
             "INIT:" + CONTAINER_ID],
        )


    def test_readiness_failure_blocks_audit_and_stops_container(self):
        result = self.execute(ready_exit=1)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("TCP readiness timed out", result.stderr)
        self.assertNotIn("AUDIT:", self.events())
        self.assertNotIn("INIT:", self.events())
        self.assertIn("VERIFY:" + CONTAINER_ID, self.events())
        self.assertIn(
            "DOCKER:stop --time 60 " + CONTAINER_ID,
            self.events(),
        )

    def test_auth_audit_failure_stops_registered_container(self):
        result = self.execute(audit_exit=1)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            self.events().splitlines(),
            [
                "AUTHORIZE",
                "DOCKER:start " + CONTAINER_ID,
                "NETWORK:runtime:" + CONTAINER_ID,
                "AUDIT:" + CONTAINER_ID,
                "VERIFY:" + CONTAINER_ID,
                "DOCKER:stop --time 60 " + CONTAINER_ID,
            ],
        )
        self.assertNotIn("INIT:", self.events())

    def test_extension_failure_stops_registered_container(self):
        result = self.execute(init_exit=1)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            self.events().splitlines(),
            [
                "AUTHORIZE",
                "DOCKER:start " + CONTAINER_ID,
                "NETWORK:runtime:" + CONTAINER_ID,
                "AUDIT:" + CONTAINER_ID,
                "INIT:" + CONTAINER_ID,
                "VERIFY:" + CONTAINER_ID,
                "DOCKER:stop --time 60 " + CONTAINER_ID,
            ],
        )

    def test_audit_failure_refuses_untrusted_cleanup(self):
        result = self.execute(
            audit_exit=1, registration_exit=1
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("DOCKER:stop", self.events())
        self.assertIn(
            "cleanup refused", result.stderr
        )

    def test_rejects_post_start_network_failure(self):
        result = self.execute(network_exit=1)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            self.events().splitlines(),
            ["AUTHORIZE", "DOCKER:start " + CONTAINER_ID,
             "NETWORK:runtime:" + CONTAINER_ID,
             "VERIFY:" + CONTAINER_ID,
             "DOCKER:stop --time 60 " + CONTAINER_ID],
        )

    def test_refuses_cleanup_without_registration(self):
        result = self.execute(network_exit=1, registration_exit=1)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            self.events().splitlines(),
            ["AUTHORIZE", "DOCKER:start " + CONTAINER_ID,
             "NETWORK:runtime:" + CONTAINER_ID,
             "VERIFY:" + CONTAINER_ID],
        )

    def test_reports_cleanup_failure(self):
        result = self.execute(network_exit=1, stop_exit=17)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cleanup failed", result.stderr)
        self.assertIn(
            "DOCKER:stop --time 60 " + CONTAINER_ID,
            self.events(),
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
