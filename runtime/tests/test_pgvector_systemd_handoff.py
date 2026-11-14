"""Regression contract for pgvector systemd handoff."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/pgvector.sh"


class PgvectorSystemdHandoffTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        source = MODULE.read_text()
        cls.install = source.split(
            "pgvector_install() {", 1
        )[1].split(
            "\n# Verify the immutable container", 1
        )[0]

    def test_provisioning_precedes_systemd(self):
        self.assertLess(
            self.install.index("pgvector_provision_container"),
            self.install.index(
                "systemctl start daia-pgvector.service"
            ),
        )

    def test_provisioning_failure_blocks_start(self):
        self.assertIn(
            "pgvector_provision_container || return 1",
            self.install,
        )

    def test_systemd_failure_is_propagated(self):
        self.assertIn(
            "if ! systemctl start daia-pgvector.service;",
            self.install,
        )
        self.assertIn(
            'echo "PostgreSQL systemd startup failed"',
            self.install,
        )

    def test_does_not_enable_service_prematurely(self):
        self.assertNotIn("systemctl enable", self.install)

    def test_preserves_registration_boundary(self):
        self.assertIn(
            "pgvector_provision_container || return 1",
            self.install,
        )


if __name__ == "__main__":
    unittest.main()
