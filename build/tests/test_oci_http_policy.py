import importlib.util
import sys
import unittest
from pathlib import Path
from urllib.parse import urlsplit


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_http", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class OCIHTTPPolicyTests(unittest.TestCase):
    DIGEST = "sha256:" + "a" * 64

    def plan(self):
        return MODULE.plan_pgvector_oci_acquisition(
            "pgvector/pgvector:pg17",
            self.DIGEST,
        )

    def test_manifest_request_is_digest_pinned(self):
        request = MODULE.build_oci_manifest_request(self.plan())
        self.assertEqual(
            request.url,
            "https://registry-1.docker.io/v2/"
            "pgvector/pgvector/manifests/" + self.DIGEST,
        )

    def test_manifest_request_has_no_url_credentials(self):
        request = MODULE.build_oci_manifest_request(self.plan())
        parsed = urlsplit(request.url)
        self.assertEqual(parsed.scheme, "https")
        self.assertIsNone(parsed.username)
        self.assertIsNone(parsed.password)
        self.assertFalse(parsed.query)
        self.assertFalse(parsed.fragment)

    def test_manifest_request_requires_bearer_token_separately(self):
        request = MODULE.build_oci_manifest_request(self.plan())
        self.assertNotIn("Authorization", dict(request.headers))
        self.assertEqual(request.auth_scheme, "Bearer")

    def test_manifest_request_rejects_untrusted_plan_type(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.build_oci_manifest_request(object())

    def test_blob_request_uses_approved_registry(self):
        request = MODULE.build_oci_blob_request(
            self.plan(), self.DIGEST
        )
        self.assertEqual(
            request.url,
            "https://registry-1.docker.io/v2/"
            "pgvector/pgvector/blobs/" + self.DIGEST,
        )

    def test_blob_request_rejects_invalid_digest(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.build_oci_blob_request(
                self.plan(), "sha256:invalid"
            )

    def test_blob_request_rejects_untrusted_plan_type(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.build_oci_blob_request(
                object(), self.DIGEST
            )

    def test_token_endpoint_is_fixed(self):
        request = MODULE.build_oci_token_http_request(self.plan())
        self.assertEqual(
            urlsplit(request.url).netloc,
            "auth.docker.io",
        )
        self.assertEqual(
            urlsplit(request.url).scheme,
            "https",
        )
        self.assertNotIn("Authorization", dict(request.headers))


if __name__ == "__main__":
    unittest.main()
