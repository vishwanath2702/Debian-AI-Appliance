import importlib.util
import sys
import unittest
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_trust", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class OCITrustedDigestTests(unittest.TestCase):
    VALID = "sha256:" + "a" * 64

    def test_accepts_valid_sha256_digest(self):
        self.assertEqual(
            MODULE.validate_trusted_oci_digest(self.VALID),
            self.VALID,
        )

    def test_rejects_empty_digest(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.validate_trusted_oci_digest("")

    def test_rejects_missing_digest(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.validate_trusted_oci_digest(None)

    def test_rejects_uppercase_digest(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.validate_trusted_oci_digest(
                "sha256:" + "A" * 64
            )

    def test_rejects_short_digest(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.validate_trusted_oci_digest(
                "sha256:" + "a" * 63
            )

    def test_rejects_mutable_tag(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.validate_trusted_oci_digest(
                "pgvector/pgvector:pg17"
            )

    def test_rejects_surrounding_whitespace(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.validate_trusted_oci_digest(
                " " + self.VALID
            )

    def test_rejects_non_string_value(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.validate_trusted_oci_digest(123)


if __name__ == "__main__":
    unittest.main()
