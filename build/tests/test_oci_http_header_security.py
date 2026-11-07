import importlib.util
import sys
import unittest
from http.client import HTTPMessage
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location(
    "daia_oci_header_security", FETCHER
)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def make_headers(pairs):
    headers = HTTPMessage()
    for key, value in pairs:
        headers.add_header(key, value)
    return headers


class OCIHTTPHeaderSecurityTests(unittest.TestCase):
    def check(self, pairs):
        return MODULE.validate_oci_http_response(
            200, make_headers(pairs), 1024
        )

    def test_accepts_native_http_message(self):
        self.assertEqual(
            self.check([("Content-Length", "128")]),
            128,
        )

    def test_rejects_duplicate_content_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check([
                ("Content-Length", "128"),
                ("Content-Length", "256"),
            ])

    def test_rejects_mixed_case_duplicate_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check([
                ("Content-Length", "128"),
                ("content-length", "256"),
            ])

    def test_rejects_duplicate_content_encoding(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check([
                ("Content-Length", "128"),
                ("Content-Encoding", "identity"),
                ("content-encoding", "gzip"),
            ])

    def test_rejects_transfer_encoding(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check([
                ("Content-Length", "128"),
                ("Transfer-Encoding", "chunked"),
            ])

    def test_rejects_duplicate_transfer_encoding(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check([
                ("Content-Length", "128"),
                ("Transfer-Encoding", "identity"),
                ("transfer-encoding", "chunked"),
            ])

    def test_rejects_connection_upgrade(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check([
                ("Content-Length", "128"),
                ("Connection", "upgrade"),
                ("Upgrade", "websocket"),
            ])

    def test_rejects_comma_joined_content_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check([
                ("Content-Length", "128, 256"),
            ])


if __name__ == "__main__":
    unittest.main()
