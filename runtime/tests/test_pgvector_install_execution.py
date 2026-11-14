"""Execute the real pgvector_install function with inert dependencies."""

import os
from pathlib import Path
import subprocess
import unittest

MODULE = Path(__file__).resolve().parents[1] / "modules/pgvector.sh"
IMAGE = (
    "sha256:"
    "366e1863bdeefd735562caf0ebb4c8b05b4cb37bd8eec60d516c9083209746a2"
)

SCRIPT = r"""
set -euo pipefail
source "$DAIA_TEST_MODULE"

events=()

record() {
    events+=("$1")
}

pgvector_ensure_rag_network() {
    record network
    [[ "$DAIA_TEST_FAILURE" != network ]]
}

pgvector_load_verified_offline_image() {
    record image
    [[ "$DAIA_TEST_FAILURE" != image ]]
}

function /usr/bin/python3 {
    [[ "$*" == "/opt/daia/pgvector_credentials.py --state-directory /var/lib/daia/pgvector" ]] || return 98
    record credentials
    [[ "$DAIA_TEST_FAILURE" != credentials ]]
}

pgvector_ensure_volume() {
    record volume
    [[ "$DAIA_TEST_FAILURE" != volume ]]
}

pgvector_provision_container() {
    record provision
    [[ "$DAIA_TEST_FAILURE" != provision ]]
}

systemctl() {
    [[ "$*" == "start daia-pgvector.service" ]] || return 97
    record systemd
    [[ "$DAIA_TEST_FAILURE" != systemd ]]
}

status=0
pgvector_install || status=$?
printf 'STATUS:%s\n' "$status"
printf 'EVENTS:%s\n' "${events[*]}"
"""

class PgvectorInstallExecutionTests(unittest.TestCase):
    def execute(self, failure="none", image=IMAGE):
        env = os.environ.copy()
        env.update({
            "DAIA_TEST_MODULE": str(MODULE),
            "DAIA_TEST_FAILURE": failure,
            "DAIA_PGVECTOR_DOCKER_IMAGE_ID": image,
            "DAIA_PGVECTOR_CONTAINER": "daia-pgvector",
            "DAIA_PGVECTOR_VOLUME": "daia-pgvector-data",
        })
        return subprocess.run(
            [
                "unshare", "--user", "--map-root-user",
                "bash", "-c", SCRIPT,
            ],
            env=env, capture_output=True, text=True,
            timeout=15, check=False,
        )

    def assert_case(self, failure, expected):
        result = self.execute(failure)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("EVENTS:" + " ".join(expected), result.stdout)
        self.assertIn(
            "STATUS:" + ("0" if failure == "none" else "1"),
            result.stdout,
        )

    def test_success(self):
        self.assert_case(
            "none",
            ["network", "image", "credentials", "volume",
             "provision", "systemd"],
        )

    def test_network_failure(self):
        self.assert_case("network", ["network"])

    def test_image_failure(self):
        self.assert_case("image", ["network", "image"])

    def test_credential_failure(self):
        self.assert_case(
            "credentials", ["network", "image", "credentials"],
        )

    def test_volume_failure(self):
        self.assert_case(
            "volume", ["network", "image", "credentials", "volume"],
        )

    def test_provisioning_failure(self):
        self.assert_case(
            "provision",
            ["network", "image", "credentials", "volume", "provision"],
        )

    def test_systemd_failure(self):
        self.assert_case(
            "systemd",
            ["network", "image", "credentials", "volume",
             "provision", "systemd"],
        )

    def test_invalid_image_configuration_fails_closed(self):
        result = self.execute(image="sha256:untrusted")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("STATUS:1", result.stdout)
        self.assertIn("EVENTS:", result.stdout)
        self.assertEqual(
            result.stdout.split("EVENTS:", 1)[1].strip(), "",
        )


if __name__ == "__main__":
    unittest.main()
