"""Fixed trusted Python import path for pgvector verification."""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VERIFIER = ROOT / "runtime/daia-pgvector-verify-registration"


class PgvectorImportPathTests(unittest.TestCase):
    def test_verifier_uses_fixed_module_directory(self):
        source = VERIFIER.read_text()

        self.assertIn(
            '"/opt/daia"',
            source,
        )
        self.assertIn(
            "sys.path.insert(0,",
            source,
        )

    def test_import_path_precedes_registration_import(self):
        source = VERIFIER.read_text()

        path_position = source.index("sys.path.insert(0,")
        import_position = source.index(
            "from pgvector_registration import"
        )

        self.assertLess(path_position, import_position)

    def test_no_environment_controlled_python_path(self):
        source = VERIFIER.read_text()

        self.assertNotIn("PYTHONPATH", source)
        self.assertNotIn("os.environ", source)


if __name__ == "__main__":
    unittest.main()
