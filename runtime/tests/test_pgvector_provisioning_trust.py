"""Static trust-boundary regression checks for pgvector provisioning."""

from pathlib import Path
import unittest

MODULE = Path(__file__).resolve().parents[1] / "modules/pgvector.sh"


class PgvectorProvisioningTrustTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = MODULE.read_text()

    def test_existing_container_requires_registration(self):
        self.assertIn(
            'if [[ "$status" -eq 0 ]]; then',
            self.source,
        )
        self.assertIn(
            '/usr/libexec/daia-pgvector-verify-registration',
            self.source,
        )

    def test_container_prevents_privilege_escalation(self):
        self.assertIn(
            "--security-opt no-new-privileges:true",
            self.source,
        )

    def test_security_checked_before_registration(self):
        verification = (
            'pgvector_container_security_matches "$container_id" || return 1'
        )
        registration = (
            "/usr/bin/python3 /opt/daia/"
            "pgvector_register_container.py"
        )

        self.assertIn(verification, self.source)
        self.assertLess(
            self.source.index(verification),
            self.source.index(registration),
        )

    def test_create_captures_immutable_identity(self):
        self.assertIn(
            'container_id="$(docker container create',
            self.source,
        )
        self.assertIn(
            'if [[ "$verified_id" != "$container_id" ]]; then',
            self.source,
        )

    def test_registration_only_after_identity_verification(self):
        marker = 'if [[ "$verified_id" != "$container_id" ]]; then'
        publication = (
            '/usr/bin/python3 /opt/daia/'
            'pgvector_register_container.py'
        )
        self.assertLess(
            self.source.index(marker),
            self.source.index(publication),
        )


if __name__ == "__main__":
    unittest.main()
