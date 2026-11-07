import importlib.util
import sys
import unittest
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_redirect", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class OCIRedirectPolicyTests(unittest.TestCase):
    def check(self, source, destination, kind):
        return MODULE.validate_oci_redirect(
            source, destination, kind
        )

    def test_rejects_token_redirect(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.check(
                "https://auth.docker.io/token",
                "https://auth.docker.io/other",
                "token",
            )

    def test_rejects_manifest_redirect(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.check(
                "https://registry-1.docker.io/v2/"
                "pgvector/pgvector/manifests/sha256:" + "a" * 64,
                "https://registry-1.docker.io/other",
                "manifest",
            )

    def test_rejects_http_downgrade(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.check(
                "https://registry-1.docker.io/v2/blob",
                "http://storage.example.com/blob",
                "blob",
            )

    def test_rejects_embedded_credentials(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.check(
                "https://registry-1.docker.io/v2/blob",
                "https://user:password@storage.example.com/blob",
                "blob",
            )

    def test_rejects_fragment(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.check(
                "https://registry-1.docker.io/v2/blob",
                "https://storage.example.com/blob#fragment",
                "blob",
            )

    def test_rejects_unknown_redirect_kind(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.check(
                "https://registry-1.docker.io/v2/blob",
                "https://storage.example.com/blob",
                "unknown",
            )

    def test_rejects_unapproved_storage_host(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.check(
                "https://registry-1.docker.io/v2/blob",
                "https://untrusted.example.com/blob",
                "blob",
            )

    def test_rejects_redirect_chain(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.validate_oci_redirect(
                "https://registry-1.docker.io/v2/blob",
                "https://storage.example.com/blob",
                "blob",
                redirect_count=2,
            )


if __name__ == "__main__":
    unittest.main()
