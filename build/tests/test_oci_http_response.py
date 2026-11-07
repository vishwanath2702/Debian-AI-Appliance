import importlib.util
import sys
import unittest
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_response", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class OCIHTTPResponseTests(unittest.TestCase):
    def check(self, status=200, headers=None, limit=1024):
        return MODULE.validate_oci_http_response(
            status,
            {} if headers is None else headers,
            limit,
        )

    def test_accepts_bounded_success(self):
        self.assertEqual(
            self.check(headers={"Content-Length": "128"}),
            128,
        )

    def test_rejects_redirect(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check(status=302)

    def test_rejects_authentication_failure(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check(status=401)

    def test_rejects_server_error(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check(status=503)

    def test_rejects_missing_content_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check(headers={})

    def test_rejects_oversized_content(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check(headers={"Content-Length": "2048"})

    def test_rejects_negative_content_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check(headers={"Content-Length": "-1"})

    def test_rejects_invalid_content_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check(headers={"Content-Length": "invalid"})

    def test_rejects_boolean_limit(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check(headers={"Content-Length": "128"}, limit=True)

    def test_rejects_compressed_http_response(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check(headers={
                "Content-Length": "128",
                "Content-Encoding": "gzip",
            })


if __name__ == "__main__":
    unittest.main()
