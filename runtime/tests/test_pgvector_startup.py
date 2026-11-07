"""Fail-closed safety tests for DAIA pgvector startup."""

import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
START_HELPER = ROOT / "runtime/daia-pgvector-start"


class PgvectorStartupSafetyTests(unittest.TestCase):

    def test_helper_exists(self):
        self.assertTrue(START_HELPER.is_file())
        self.assertFalse(START_HELPER.is_symlink())

    def test_strict_shell_mode(self):
        source = START_HELPER.read_text()
        self.assertIn("set -euo pipefail", source)

    def test_requires_root(self):
        source = START_HELPER.read_text()
        self.assertIn('if [[ "$EUID" -ne 0 ]]', source)

    def test_authorization_precedes_start(self):
        source = START_HELPER.read_text()
        self.assertIn(
            "/usr/libexec/daia-pgvector-authorize-start",
            source,
        )
        self.assertIn(
            'if ! container_id="$(',
            source,
        )
        self.assertLess(
            source.index("daia-pgvector-authorize-start"),
            source.index('docker start "$container_id"'),
        )

    def test_requires_immutable_container_id(self):
        source = START_HELPER.read_text()
        self.assertIn(
            '[[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]',
            source,
        )
        self.assertIn(
            'docker start "$container_id" >/dev/null',
            source,
        )

    def test_no_unapproved_docker_mutations(self):
        source = START_HELPER.read_text()
        for operation in (
            "docker run",
            "docker create",
            "docker rm",
            "docker volume create",
            "docker volume rm",
            "docker load",
        ):
            with self.subTest(operation=operation):
                self.assertNotIn(operation, source)
        self.assertEqual(
            source.count('docker start "$container_id"'),
            1,
        )


if __name__ == "__main__":
    unittest.main()
