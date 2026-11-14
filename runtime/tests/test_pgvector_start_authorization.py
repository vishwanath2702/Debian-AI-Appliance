"""Executable mock-Docker tests for pgvector startup authorization."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-pgvector-authorize-start"
CONTAINER_ID = "a" * 64
IMAGE_ID = (
    "sha256:"
    "366e1863bdeefd735562caf0ebb4c8b05b4cb37bd8eec60d516c9083209746a2"
)

MOUNTS = (
    "volume|daia-pgvector-data|"
    "/var/lib/docker/volumes/daia-pgvector-data/_data|"
    "/var/lib/postgresql/data|true\n"
    "bind||/var/lib/daia/pgvector/postgres-password|"
    "/run/secrets/daia-pgvector-password|false\n"
    "bind||/var/lib/daia/pgvector/pgpass|"
    "/run/secrets/daia-pgvector-pgpass|false\n"
)


class StartupAuthorizationTests(unittest.TestCase):

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-start-auth-"
        )
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.log = self.root / "docker.log"

    def authorize(
        self,
        *,
        metadata=None,
        mounts=MOUNTS,
        volume_labels="pgvector\ttrue",
        verified_id=CONTAINER_ID,
        registration_success=True,
        security="no-new-privileges:true|false",
        network_mode="daia-rag-internal",
        auth_mode="prestart",
    ):
        if metadata is None:
            metadata = (
                CONTAINER_ID + "\tpgvector\ttrue\t" + IMAGE_ID
            )

        import shlex

        docker = self.bin / "docker"
        docker.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$*" >> "$DAIA_TEST_DOCKER_LOG"\n'
            'case "$*" in\n'
            '  info) exit 0 ;;\n'
            '  *volume\\ inspect*)\n'
            '    printf "%s\\n" '
            + shlex.quote(volume_labels) + '\n'
            '    ;;\n'
            '  *container\\ inspect*)\n'
            '    case "$*" in\n'
            '      *".Mounts"*)\n'
            '        printf "%s" ' + shlex.quote(mounts) + '\n'
            '        ;;\n'
            '      *".Config.Labels"*)\n'
            '        printf "%s\\n" '
            + shlex.quote(metadata) + '\n'
            '        ;;\n'
            '      *".HostConfig.NetworkMode"*)\n'
            '        printf "%s\\n" '
            + shlex.quote(network_mode) + '\n'
            '        ;;\n'
            '      *".HostConfig"*)\n'
            '        printf "%s\\n" '
            + shlex.quote(security) + '\n'
            '        ;;\n'
            '      *"{{.Id}}"*)\n'
            '        printf "%s\\n" '
            + shlex.quote(verified_id) + '\n'
            '        ;;\n'
            '      *) exit 98 ;;\n'
            '    esac\n'
            '    ;;\n'
            '  *) exit 99 ;;\n'
            'esac\n'
        )
        docker.chmod(0o755)

        env = os.environ.copy()
        env["PATH"] = str(self.bin) + os.pathsep + env.get("PATH", "")
        env["DAIA_TEST_DOCKER_LOG"] = str(self.log)

        verifier_exit = 0 if registration_success else 1

        script = r"""
function /usr/libexec/daia-rag-verify-network {
    printf 'NETWORK:%s:%s\n' "$1" "$2" >> "$DAIA_TEST_DOCKER_LOG"
    [[ "$1" == "$DAIA_TEST_AUTH_MODE" && "$2" == "$EXPECTED_CONTAINER_ID" ]]
}
function /usr/libexec/daia-pgvector-verify-registration {
    printf 'VERIFY:%s\n' "$1" >> "$DAIA_TEST_DOCKER_LOG"
    return "$VERIFIER_EXIT"
}
""" + "\nVERIFIER_EXIT=" + str(verifier_exit) + r"""
source "$DAIA_AUTH_HELPER" "$DAIA_TEST_AUTH_MODE"
"""

        env["DAIA_AUTH_HELPER"] = str(HELPER)
        env["EXPECTED_CONTAINER_ID"] = CONTAINER_ID
        env["DAIA_TEST_AUTH_MODE"] = auth_mode

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

    def assert_inspection_reached(self):
        events = self.events()
        self.assertIn("info\n", events)
        self.assertIn("container inspect --format", events)
        self.assertNotIn("container start", events)
        self.assertNotIn("container create", events)
        self.assertNotIn("volume create", events)

    def test_accepts_runtime_authorization(self):
        result = self.authorize(auth_mode="runtime")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), CONTAINER_ID)
        self.assertIn(
            "NETWORK:runtime:" + CONTAINER_ID,
            self.events(),
        )
        self.assertIn("VERIFY:" + CONTAINER_ID, self.events())

    def test_rejects_unknown_authorization_mode(self):
        result = self.authorize(auth_mode="invalid")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("VERIFY:", self.events())
        self.assertNotIn("container inspect", self.events())

    def test_accepts_verified_container(self):
        result = self.authorize()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), CONTAINER_ID)
        self.assert_inspection_reached()
        self.assertIn(
            "NETWORK:prestart:" + CONTAINER_ID,
            self.events(),
        )
        self.assertIn("volume inspect", self.events())
        self.assertIn("VERIFY:" + CONTAINER_ID, self.events())
        self.assertLess(
            self.events().index("volume inspect"),
            self.events().index("VERIFY:" + CONTAINER_ID),
        )

    def test_rejects_unsafe_security_configuration(self):
        for security in (
            "missing|false",
            "no-new-privileges:true|true",
            "no-new-privileges:false|false",
            "",
            "invalid",
            "no-new-privileges:true|false|extra",
        ):
            with self.subTest(security=security):
                self.log.unlink(missing_ok=True)
                result = self.authorize(security=security)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("VERIFY:", self.events())

    def test_rejects_unapproved_network_mode(self):
        for mode in ("host", "bridge", "", "daia-untrusted"):
            with self.subTest(network_mode=mode):
                self.log.unlink(missing_ok=True)
                result = self.authorize(network_mode=mode)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("VERIFY:", self.events())

    def test_security_inspection_is_required(self):
        result = self.authorize()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(".HostConfig", self.events())

    def test_rejects_invalid_metadata(self):
        cases = (
            "invalid",
            CONTAINER_ID + "\t\ttrue\t" + IMAGE_ID,
            CONTAINER_ID + "\tpgvector\t\t" + IMAGE_ID,
            CONTAINER_ID + "\tpgvector\ttrue\twrong",
            CONTAINER_ID + "\tpgvector\ttrue\t" + IMAGE_ID + "\textra",
        )
        for metadata in cases:
            with self.subTest(metadata=metadata):
                self.log.unlink(missing_ok=True)
                result = self.authorize(metadata=metadata)
                self.assertNotEqual(result.returncode, 0)
                self.assert_inspection_reached()
                self.assertNotIn(".Mounts", self.events())
                self.assertNotIn("volume inspect", self.events())
                self.assertNotIn("VERIFY:", self.events())

    def test_rejects_missing_pgpass_mount(self):
        missing = MOUNTS.replace(
            "bind||/var/lib/daia/pgvector/pgpass|"
            "/run/secrets/daia-pgvector-pgpass|false" + chr(10),
            "",
        )
        self.assertNotEqual(missing, MOUNTS)
        result = self.authorize(mounts=missing)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("VERIFY:", self.events())

    def test_rejects_invalid_mounts(self):
        cases = (
            "",
            MOUNTS.replace("/var/lib/docker/volumes/", ""),
            MOUNTS.replace("|true\n", "|false\n"),
            MOUNTS + "bind||/tmp/x|/tmp/y|true\n",
        )
        for mounts in cases:
            with self.subTest(mounts=mounts):
                self.log.unlink(missing_ok=True)
                result = self.authorize(mounts=mounts)
                self.assertNotEqual(result.returncode, 0)
                self.assert_inspection_reached()
                self.assertIn(".Mounts", self.events())
                self.assertNotIn("volume inspect", self.events())
                self.assertNotIn("VERIFY:", self.events())

    def test_rejects_untrusted_volume(self):
        result = self.authorize(volume_labels="other\tfalse")
        self.assertNotEqual(result.returncode, 0)
        self.assert_inspection_reached()
        self.assertIn("volume inspect", self.events())
        self.assertNotIn("VERIFY:", self.events())

    def test_rejects_identity_change(self):
        result = self.authorize(verified_id="b" * 64)
        self.assertNotEqual(result.returncode, 0)
        self.assert_inspection_reached()
        self.assertIn(".Mounts", self.events())
        self.assertIn("container inspect --format {{.Id}}", self.events())
        self.assertNotIn("volume inspect", self.events())
        self.assertNotIn("VERIFY:", self.events())

    def test_rejects_registration_failure(self):
        result = self.authorize(registration_success=False)
        self.assertNotEqual(result.returncode, 0)
        self.assert_inspection_reached()
        self.assertIn("VERIFY:" + CONTAINER_ID, self.events())
        self.assertIn("volume inspect", self.events())
        self.assertLess(
            self.events().index("volume inspect"),
            self.events().index("VERIFY:" + CONTAINER_ID),
        )


if __name__ == "__main__":
    unittest.main()
