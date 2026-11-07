"""Executable immutable-ID regression tests using mock Docker."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

MODULE = Path(__file__).resolve().parents[1] / "modules/pgvector.sh"

EXPECTED_IMAGE = (
    "sha256:366e1863bdeefd735562caf0ebb4c8b05b4cb37bd8"
    "eec60d516c9083209746a2"
)


class PgvectorIdentityRuntimeTests(unittest.TestCase):

    def test_rejects_container_substitution_before_registration(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-pgvector-identity-"
        ) as directory:
            root = Path(directory)
            docker = root / "docker"
            log = root / "calls.log"

            docker.write_text("""#!/bin/bash
set -eu
printf '%s\\n' "$*" >> "$DAIA_MOCK_LOG"
if [[ "$1 $2" == "container create" ]]; then
    printf '%064d\\n' 0 | tr 0 a
    exit 0
fi
if [[ "$1 $2" == "container inspect" ]]; then
    printf '%064d\\n' 0 | tr 0 b
    exit 0
fi
exit 99
""")
            docker.chmod(0o755)

            environment = os.environ.copy()
            environment["PATH"] = (
                str(root) + os.pathsep + environment.get("PATH", "")
            )
            environment["DAIA_MOCK_LOG"] = str(log)
            environment["DAIA_PGVECTOR_DOCKER_IMAGE_ID"] = EXPECTED_IMAGE

            # Override unrelated provisioning stages with inert shell
            # functions; only the container identity path is exercised.
            script = """
source "$1"
pgvector_load_verified_offline_image() { return 0; }
pgvector_ensure_volume() { return 0; }
pgvector_container_status() { return 1; }
pgvector_container_owned() { return 0; }
pgvector_container_image_id_matches() { return 0; }
pgvector_container_mount_matches() { return 0; }
pgvector_volume_owned() { return 0; }
pgvector_install
"""

            # The installed credentials helper is deliberately not mocked
            # or replaced. Without a root-owned isolated runtime, execution
            # must fail before reaching Docker creation.
            result = subprocess.run(
                ["bash", "-c", script, "daia-test", str(MODULE)],
                env=environment,
                capture_output=True,
                text=True,
                timeout=10,
            )

            self.assertNotEqual(result.returncode, 0)

            # This test must never invoke the actual Docker daemon.
            if log.exists():
                self.assertTrue(
                    all(
                        line.startswith("container ")
                        for line in log.read_text().splitlines()
                    )
                )


    def test_provisioning_rejects_substituted_id_without_registration(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-pgvector-substitution-"
        ) as directory:
            root = Path(directory)
            docker = root / "docker"
            calls = root / "calls.log"

            docker.write_text("""#!/bin/bash
set -eu
printf '%s\n' "$*" >> "$DAIA_MOCK_LOG"
if [[ "$1 $2" == "container create" ]]; then
    printf '%064d\n' 0 | tr 0 a
    exit 0
fi
if [[ "$1 $2" == "container inspect" ]]; then
    if [[ "$*" == *".HostConfig.SecurityOpt"* ]]; then
        printf '%s\n' 'no-new-privileges:true|false'
        exit 0
    fi
    printf '%064d\n' 0 | tr 0 b
    exit 0
fi
exit 99
""")
            docker.chmod(0o755)

            environment = os.environ.copy()
            environment["PATH"] = (
                str(root) + os.pathsep + environment.get("PATH", "")
            )
            environment["DAIA_MOCK_LOG"] = str(calls)
            environment["DAIA_PGVECTOR_DOCKER_IMAGE_ID"] = EXPECTED_IMAGE

            script = r"""
source "$1"

pgvector_container_status() { return 1; }
pgvector_container_owned() { return 0; }
pgvector_container_image_id_matches() { return 0; }
pgvector_container_mount_matches() { return 0; }
pgvector_volume_owned() { return 0; }

function /usr/libexec/daia-pgvector-verify-registration {
    printf 'VERIFY:%s\n' "$1" >> "$DAIA_MOCK_LOG"
    return 0
}

function /usr/bin/python3 {
    printf 'PYTHON:%s\n' "$*" >> "$DAIA_MOCK_LOG"
    return 0
}

pgvector_provision_container
"""

            result = subprocess.run(
                ["bash", "-c", script, "daia-test", str(MODULE)],
                env=environment,
                capture_output=True,
                text=True,
                timeout=10,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn(
                "identity changed during provisioning",
                result.stderr,
            )

            recorded = calls.read_text().splitlines()

            self.assertEqual(
                sum(line.startswith("container create ") for line in recorded),
                1,
            )
            self.assertEqual(
                sum(line.startswith("container inspect ") for line in recorded),
                2,
            )
            self.assertFalse(
                any(line.startswith(("VERIFY:", "PYTHON:")) for line in recorded),
                recorded,
            )


    def test_existing_container_requires_trusted_registration(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-pgvector-existing-"
        ) as directory:
            root = Path(directory)
            docker = root / "docker"
            calls = root / "calls.log"

            docker.write_text("""#!/bin/bash
set -eu
printf '%s\n' "$*" >> "$DAIA_MOCK_LOG"
if [[ "$1 $2" == "container inspect" ]]; then
    printf '%064d\n' 0 | tr 0 a
    exit 0
fi
exit 99
""")
            docker.chmod(0o755)

            environment = os.environ.copy()
            environment["PATH"] = (
                str(root) + os.pathsep + environment.get("PATH", "")
            )
            environment["DAIA_MOCK_LOG"] = str(calls)
            environment["DAIA_PGVECTOR_DOCKER_IMAGE_ID"] = EXPECTED_IMAGE

            script = r"""
source "$1"

pgvector_container_status() { return 0; }
pgvector_container_owned() { return 0; }
pgvector_container_image_id_matches() { return 0; }
pgvector_container_mount_matches() { return 0; }
pgvector_volume_owned() { return 0; }

function /usr/libexec/daia-pgvector-verify-registration {
    printf 'VERIFY:%s\n' "$1" >> "$DAIA_MOCK_LOG"
    return 1
}

function /usr/bin/python3 {
    printf 'PYTHON:%s\n' "$*" >> "$DAIA_MOCK_LOG"
    return 0
}

pgvector_provision_container
"""

            result = subprocess.run(
                ["bash", "-c", script, "daia-test", str(MODULE)],
                env=environment,
                capture_output=True,
                text=True,
                timeout=10,
            )

            self.assertNotEqual(result.returncode, 0)

            recorded = calls.read_text().splitlines()

            self.assertEqual(
                sum(line.startswith("VERIFY:") for line in recorded),
                1,
            )
            self.assertFalse(
                any(line.startswith("container create ") for line in recorded),
                recorded,
            )
            self.assertFalse(
                any(line.startswith("PYTHON:") for line in recorded),
                recorded,
            )


    def test_new_container_registration_outcomes(self):
        for registration_exit in (0, 1):
            with self.subTest(registration_exit=registration_exit):
                with tempfile.TemporaryDirectory(
                    prefix="daia-pgvector-registration-"
                ) as directory:
                    root = Path(directory)
                    docker = root / "docker"
                    calls = root / "calls.log"

                    docker.write_text("""#!/bin/bash
set -eu
printf '%s\n' "$*" >> "$DAIA_MOCK_LOG"
if [[ "$1 $2" == "container create" ]]; then
    printf '%064d\n' 0 | tr 0 a
    exit 0
fi
if [[ "$1 $2" == "container inspect" ]]; then
    if [[ "$*" == *".HostConfig.SecurityOpt"* ]]; then
        printf '%s\n' 'no-new-privileges:true|false'
        exit 0
    fi
    printf '%064d\n' 0 | tr 0 a
    exit 0
fi
exit 99
""")
                    docker.chmod(0o755)

                    environment = os.environ.copy()
                    environment["PATH"] = (
                        str(root) + os.pathsep
                        + environment.get("PATH", "")
                    )
                    environment["DAIA_MOCK_LOG"] = str(calls)
                    environment["DAIA_PGVECTOR_DOCKER_IMAGE_ID"] = EXPECTED_IMAGE
                    environment["DAIA_MOCK_REGISTRATION_EXIT"] = str(
                        registration_exit
                    )

                    script = r"""
source "$1"

pgvector_container_status() { return 1; }
pgvector_container_owned() {
    printf 'CHECK:ownership\n' >> "$DAIA_MOCK_LOG"
}
pgvector_container_image_id_matches() {
    printf 'CHECK:image\n' >> "$DAIA_MOCK_LOG"
}
pgvector_container_mount_matches() {
    printf 'CHECK:mounts\n' >> "$DAIA_MOCK_LOG"
}
pgvector_volume_owned() {
    printf 'CHECK:volume\n' >> "$DAIA_MOCK_LOG"
}

function /usr/libexec/daia-pgvector-verify-registration {
    printf 'VERIFY:%s\n' "$1" >> "$DAIA_MOCK_LOG"
    return 1
}

function /usr/bin/python3 {
    printf 'REGISTER:%s\n' "$*" >> "$DAIA_MOCK_LOG"
    return "$DAIA_MOCK_REGISTRATION_EXIT"
}

pgvector_provision_container
"""

                    result = subprocess.run(
                        ["bash", "-c", script, "daia-test", str(MODULE)],
                        env=environment,
                        capture_output=True,
                        text=True,
                        timeout=10,
                    )

                    self.assertEqual(
                        result.returncode,
                        registration_exit,
                        result.stderr,
                    )

                    recorded = calls.read_text().splitlines()

                    self.assertEqual(
                        sum(line.startswith("container create ") for line in recorded),
                        1,
                    )
                    self.assertEqual(
                        sum(line.startswith("container inspect ") for line in recorded),
                        2,
                    )

                    checks = [
                        "CHECK:ownership",
                        "CHECK:image",
                        "CHECK:mounts",
                        "CHECK:volume",
                    ]

                    for check in checks:
                        self.assertIn(check, recorded)

                    registration = [
                        line for line in recorded
                        if line.startswith("REGISTER:")
                    ]

                    self.assertEqual(len(registration), 1)
                    self.assertIn(
                        "/opt/daia/pgvector_register_container.py",
                        registration[0],
                    )
                    self.assertTrue(
                        registration[0].endswith("a" * 64)
                    )

                    register_index = recorded.index(registration[0])
                    self.assertTrue(
                        all(
                            recorded.index(check) < register_index
                            for check in checks
                        )
                    )
                    self.assertFalse(
                        any(line.startswith("VERIFY:") for line in recorded)
                    )


if __name__ == "__main__":
    unittest.main()
