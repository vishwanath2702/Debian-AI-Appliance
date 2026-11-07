import hashlib
import importlib.util
import io
import sys
import unittest
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_bounds", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class OversizedReader:
    def read(self, size):
        return b"x" * (size + 1)


class OCIManifestBoundsTests(unittest.TestCase):
    DATA = b'{"schemaVersion":2}'

    @classmethod
    def digest(cls):
        return "sha256:" + hashlib.sha256(cls.DATA).hexdigest()

    def test_accepts_valid_bounded_manifest(self):
        result = MODULE.read_verified_oci_manifest(
            io.BytesIO(self.DATA),
            self.digest(),
            len(self.DATA),
        )
        self.assertEqual(result, self.DATA)

    def test_rejects_manifest_exceeding_limit(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.read_verified_oci_manifest(
                io.BytesIO(b""),
                self.digest(),
                16 * 1024 * 1024 + 1,
            )

    def test_rejects_truncated_manifest(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.read_verified_oci_manifest(
                io.BytesIO(self.DATA[:-1]),
                self.digest(),
                len(self.DATA),
            )

    def test_rejects_oversized_reader_chunk(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.read_verified_oci_manifest(
                OversizedReader(),
                self.digest(),
                len(self.DATA),
            )

    def test_rejects_boolean_expected_size(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.read_verified_oci_manifest(
                io.BytesIO(self.DATA),
                self.digest(),
                True,
            )

    def test_rejects_invalid_digest(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.read_verified_oci_manifest(
                io.BytesIO(self.DATA),
                "sha256:invalid",
                len(self.DATA),
            )

    def test_rejects_extra_response_bytes(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            MODULE.read_verified_oci_manifest(
                io.BytesIO(self.DATA + b" "),
                self.digest(),
                len(self.DATA),
            )


if __name__ == "__main__":
    unittest.main()
