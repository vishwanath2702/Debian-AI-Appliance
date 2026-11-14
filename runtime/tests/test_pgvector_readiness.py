import os
from pathlib import Path
import subprocess
import unittest

MODULE = Path(__file__).resolve().parents[1] / "modules" / "pgvector.sh"


class PgvectorReadinessTests(unittest.TestCase):
    def test_verify_rejects_non_root(self):
        if os.geteuid() == 0:
            self.skipTest("Requires non-root execution")

        result = subprocess.run(
            ["bash", "-c", 'source "$1"; pgvector_verify',
             "daia-test", str(MODULE)],
            capture_output=True,
            text=True,
            timeout=10,
        )
        self.assertNotEqual(result.returncode, 0)

    def test_verify_requires_registration(self):
        source = MODULE.read_text()
        function = source.split("pgvector_verify() {", 1)[1]
        self.assertIn("daia-pgvector-verify-registration", function)

    def test_verify_requires_security_checks(self):
        source = MODULE.read_text()
        function = source.split("pgvector_verify() {", 1)[1]
        for name in (
            "pgvector_container_owned",
            "pgvector_container_image_id_matches",
            "pgvector_container_mount_matches",
            "pgvector_volume_owned",
            "pgvector_container_security_matches",
        ):
            self.assertIn(name, function)

    def test_verify_checks_vector_extension(self):
        source = MODULE.read_text()
        function = source.split("pgvector_verify() {", 1)[1]
        self.assertIn("FROM pg_extension WHERE extname = 'vector'", function)
        self.assertIn('[[ "$extension" == "vector" ]] || return 1', function)
        self.assertIn("ON_ERROR_STOP=1", function)

    def test_verify_checks_postgres_readiness(self):
        source = MODULE.read_text()
        function = source.split("pgvector_verify() {", 1)[1]
        self.assertIn("pg_isready", function)
        self.assertIn('|| return 1', function)


if __name__ == "__main__":
    unittest.main()
