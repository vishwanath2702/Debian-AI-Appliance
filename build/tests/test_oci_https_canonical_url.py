import importlib.util
import io
import sys
import unittest
from http.client import HTTPMessage
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location(
    "daia_oci_canonical_url", FETCHER
)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

TOKEN_URL = (
    "https://auth.docker.io/token?"
    "service=registry.docker.io&"
    "scope=repository%3Apgvector%2Fpgvector%3Apull"
)
MANIFEST_URL = (
    "https://registry-1.docker.io/v2/"
    "pgvector/pgvector/manifests/sha256:" + "a" * 64
)


class MockResponse:
    status = 200

    def __init__(self):
        self.headers = HTTPMessage()
        self.headers.add_header("Content-Length", "3")
        self.body = io.BytesIO(b"abc")

    def read(self, size=-1):
        return self.body.read(size)


class MockConnection:
    def __init__(self):
        self.requests = []
        self.closed = False

    def request(self, method, path, body=None, headers=None):
        self.requests.append((method, path, body, headers))

    def getresponse(self):
        return MockResponse()

    def close(self):
        self.closed = True


class CanonicalURLTests(unittest.TestCase):
    def fetch(self, url):
        connection = MockConnection()

        def factory(host, **kwargs):
            return connection

        result = MODULE.fetch_oci_https_response(
            url,
            max_bytes=1024,
            connection_factory=factory,
        )
        return result, connection

    def test_accepts_canonical_token_url(self):
        result, connection = self.fetch(TOKEN_URL)
        self.assertEqual(result, b"abc")
        self.assertTrue(connection.closed)

    def test_rejects_lowercase_percent_encoding(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(TOKEN_URL.replace("%3A", "%3a"))

    def test_rejects_unescaped_colons(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(TOKEN_URL.replace("%3A", ":"))

    def test_rejects_unescaped_slash(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(TOKEN_URL.replace("%2F", "/"))

    def test_rejects_empty_fragment_marker(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(TOKEN_URL + "#")

    def test_rejects_registry_empty_fragment_marker(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(MANIFEST_URL + "#")


if __name__ == "__main__":
    unittest.main()
