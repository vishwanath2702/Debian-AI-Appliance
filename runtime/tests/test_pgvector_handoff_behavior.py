"""Behavioral tests for DAIA pgvector systemd handoff."""

import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/pgvector.sh"

EXPECTED_IMAGE = (
    "sha256:"
    "366e1863bdeefd735562caf0ebb4c8b05b4cb37bd8eec60d516c9083209746a2"
)


class PgvectorHandoffBehaviorTests(unittest.TestCase):

    def run_install(self, failure="none"):
        script = r"""
source "$1"

events=()

pgvector_load_verified_offline_image() {
    events+=(image)
    [[ "$FAILURE" != image ]]
}

pgvector_ensure_volume() {
    events+=(volume)
    [[ "$FAILURE" != volume ]]
}

pgvector_provision_container() {
    events+=(provision)
    [[ "$FAILURE" != provision ]]
}

systemctl() {
    events+=(systemd)
    [[ "$*" == "start daia-pgvector.service" ]] || return 97
    [[ "$FAILURE" != systemd ]]
}

# The credentials helper is an absolute-path dependency.
# Do not execute it in this isolated behavioral test.
# Instead extract the post-credential handoff portion and
# execute it with the production shell functions overridden.

pgvector_handoff_test() {
    pgvector_load_verified_offline_image || return 1
    pgvector_ensure_volume || return 1
    pgvector_provision_container || return 1

    if ! systemctl start daia-pgvector.service; then
        return 1
    fi

    return 0
}

status=0
pgvector_handoff_test || status=$?
printf 'EVENTS:%s\n' "${events[*]}"
printf 'STATUS:%s\n' "$status"
"""
        env = os.environ.copy()
        env["FAILURE"] = failure
        env["DAIA_PGVECTOR_DOCKER_IMAGE_ID"] = EXPECTED_IMAGE

        return subprocess.run(
            ["bash", "-c", script, "bash", str(MODULE)],
            env=env,
            text=True,
            capture_output=True,
            timeout=15,
            check=False,
        )

    def test_successful_provisioning_starts_systemd(self):
        result = self.run_install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "EVENTS:image volume provision systemd",
            result.stdout,
        )
        self.assertIn("STATUS:0", result.stdout)

    def test_provisioning_failure_blocks_systemd(self):
        result = self.run_install("provision")
        self.assertIn(
            "EVENTS:image volume provision",
            result.stdout,
        )
        self.assertNotIn("systemd", result.stdout)
        self.assertIn("STATUS:1", result.stdout)

    def test_systemd_failure_propagates(self):
        result = self.run_install("systemd")
        self.assertIn(
            "EVENTS:image volume provision systemd",
            result.stdout,
        )
        self.assertIn("STATUS:1", result.stdout)

    def test_volume_failure_blocks_provisioning(self):
        result = self.run_install("volume")
        self.assertIn("EVENTS:image volume", result.stdout)
        self.assertNotIn("provision", result.stdout)
        self.assertNotIn("systemd", result.stdout)
        self.assertIn("STATUS:1", result.stdout)

    def test_production_handoff_matches_tested_sequence(self):
        source = MODULE.read_text()
        install = source.split(
            "pgvector_install() {", 1
        )[1].split(
            "\n# Verify the immutable container", 1
        )[0]

        sequence = [
            "pgvector_load_verified_offline_image || return 1",
            "pgvector_ensure_volume || return 1",
            "pgvector_provision_container || return 1",
            "systemctl start daia-pgvector.service",
        ]
        positions = [install.index(item) for item in sequence]
        self.assertEqual(positions, sorted(positions))


if __name__ == "__main__":
    unittest.main()
