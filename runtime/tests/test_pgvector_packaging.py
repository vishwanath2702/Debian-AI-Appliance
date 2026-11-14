"""Packaging contract for DAIA pgvector security helpers."""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"


class PgvectorPackagingContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = BUILDER.read_text()

    def test_stages_pgvector_systemd_service(self):
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/services/daia-pgvector.service"',
            self.source,
        )
        self.assertIn(
            '"$DAIA_SYSTEMD_TARGET/daia-pgvector.service"',
            self.source,
        )

    def test_pgvector_service_has_no_enablement_target(self):
        service = (ROOT / "runtime/services/daia-pgvector.service").read_text()
        self.assertNotIn("[Install]", service)
        self.assertNotIn("WantedBy=", service)

    def test_stages_registration_module(self):
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/pgvector_registration.py"',
            self.source,
        )
        self.assertIn(
            '"$DAIA_ROOT_TARGET/pgvector_registration.py"',
            self.source,
        )

    def test_stages_startup_helper(self):
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-pgvector-start"',
            self.source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-pgvector-start"',
            self.source,
        )

    def test_stages_start_authorization_helper(self):
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-pgvector-authorize-start"',
            self.source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-pgvector-authorize-start"',
            self.source,
        )

    def test_stages_shutdown_helper(self):
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-pgvector-stop"',
            self.source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-pgvector-stop"',
            self.source,
        )

    def test_stages_registration_verifier(self):
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-pgvector-verify-registration"',
            self.source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-pgvector-verify-registration"',
            self.source,
        )


if __name__ == "__main__":
    unittest.main()


class RAGNetworkPackagingTests(unittest.TestCase):
    def test_rag_network_verifier_packaged(self):
        source = (ROOT / "build/build-payload.sh").read_text()
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-rag-verify-network"',
            source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-rag-verify-network"',
            source,
        )
