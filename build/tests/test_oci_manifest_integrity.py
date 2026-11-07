import hashlib
import importlib.util
import sys
import unittest
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_manifest_fetcher", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class OCIManifestIntegrityTests(unittest.TestCase):
    MANIFEST = b'{"schemaVersion":2,"mediaType":"test"}'

    @classmethod
    def digest(cls):
        return "sha256:" + hashlib.sha256(cls.MANIFEST).hexdigest()

    def test_accepts_exact_manifest_bytes(self):
        result = MODULE.verify_oci_manifest_bytes(
            self.MANIFEST, self.digest(), len(self.MANIFEST)
        )
        self.assertEqual(result, len(self.MANIFEST))

    def test_rejects_tampered_manifest(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.verify_oci_manifest_bytes(
                self.MANIFEST + b" ", self.digest(), len(self.MANIFEST) + 1
            )

    def test_rejects_wrong_size(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.verify_oci_manifest_bytes(
                self.MANIFEST, self.digest(), len(self.MANIFEST) + 1
            )

    def test_rejects_invalid_expected_digest(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.verify_oci_manifest_bytes(
                self.MANIFEST, "sha256:invalid", len(self.MANIFEST)
            )

    def test_rejects_conflicting_registry_digest_header(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.verify_oci_manifest_bytes(
                self.MANIFEST,
                self.digest(),
                len(self.MANIFEST),
                response_digest="sha256:" + "0" * 64,
            )

    def test_rejects_nonbytes_manifest(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.verify_oci_manifest_bytes(
                '{"schemaVersion":2}',
                self.digest(),
                len(self.MANIFEST),
            )


if __name__ == "__main__":
    unittest.main()
