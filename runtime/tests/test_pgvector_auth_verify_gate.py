"""Read-only source contract for pgvector authentication gating."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/pgvector.sh"
SERVICE = ROOT / "runtime/services/daia-pgvector.service"
PAYLOAD = ROOT / "build/build-payload.sh"


class PgvectorAuthVerificationGateTests(unittest.TestCase):

    def test_audit_precedes_extension_verification(self):
        text = MODULE.read_text()
        self.assertEqual(text.count("pgvector_verify() {"), 1)
        verify = text.split("pgvector_verify() {", 1)[1]

        readiness = verify.index("pg_isready")
        audit = verify.index(
            "/usr/libexec/daia-pgvector-audit-auth.py"
        )
        extension = verify.index("FROM pg_extension")

        self.assertLess(readiness, audit)
        self.assertLess(audit, extension)

    def test_audit_failure_blocks_verification(self):
        text = MODULE.read_text()
        verify = text.split("pgvector_verify() {", 1)[1]

        self.assertIn(
            '/usr/libexec/daia-pgvector-audit-auth.py '
            '"$container_id" >/dev/null || return 1',
            verify,
        )
        self.assertEqual(
            verify.count(
                "/usr/libexec/daia-pgvector-audit-auth.py"
            ),
            1,
        )

    def test_audit_is_packaged_but_not_service_activated(self):
        payload = PAYLOAD.read_text()
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/'
            'daia-pgvector-audit-auth.py"',
            payload,
        )

        service = SERVICE.read_text()
        self.assertNotIn(
            "daia-pgvector-audit-auth.py", service
        )
        self.assertNotIn(
            "daia-pgvector-init-extension.py", service
        )


if __name__ == "__main__":
    unittest.main()
